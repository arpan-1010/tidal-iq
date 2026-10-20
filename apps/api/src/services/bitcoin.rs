use crate::db::models::NewTransaction;
use crate::db::repository::TransactionRepo;
use crate::services::price::PriceService;
use anyhow::{bail, Result};
use chrono::DateTime;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

const SATS_PER_BTC: f64 = 100_000_000.0;
const POLL_SECS: u64 = 30;
const MAX_CATCH_UP: u64 = 3;

struct BtcTransfer {
    txid: String,
    from: String,
    to: String,
    sats: u64,
}

pub struct BitcoinService {
    repo: Arc<TransactionRepo>,
    price_svc: PriceService,
    threshold_usd: f64,
    rpc_url: String,
    client: Client,
}

impl BitcoinService {
    pub fn new(
        repo: Arc<TransactionRepo>,
        price_svc: PriceService,
        threshold_usd: f64,
        alchemy_key: &str,
    ) -> Self {
        Self {
            repo,
            price_svc,
            threshold_usd,
            rpc_url: format!("https://bitcoin-mainnet.g.alchemy.com/v2/{}", alchemy_key),
            client: Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .expect("failed to build HTTP client"),
        }
    }

    pub async fn start(self: Arc<Self>) {
        println!("Bitcoin service starting (threshold : ${})", self.threshold_usd);
        let mut last_height: u64 = 0;
        loop {
            if let Err(e) = self.poll(&mut last_height).await {
                eprintln!("Bitcoin error : {}", e);
            }
            sleep(Duration::from_secs(POLL_SECS)).await;
        }
    }

    async fn rpc(&self, method: &str, params: Value) -> Result<Value> {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });

        let mut res: Value = async {
            self.client.post(&self.rpc_url).json(&body)
                .send().await?
                .error_for_status()?
                .json().await
        }
        .await
        .map_err(|e: reqwest::Error| e.without_url())?;

        if let Some(err) = res.get("error").filter(|e| !e.is_null()) {
            bail!("{} failed: {}", method, err);
        }
        Ok(res.get_mut("result").map(Value::take).unwrap_or(Value::Null))
    }

    async fn poll(&self, last_height: &mut u64) -> Result<()> {
        let tip = parse_height(&self.rpc("getblockcount", json!([])).await?)
            .ok_or_else(|| anyhow::anyhow!("could not read block height"))?;

        if tip <= *last_height {
            return Ok(());
        }
        let first = if *last_height == 0 {
            tip
        } else {
            (*last_height + 1).max(tip.saturating_sub(MAX_CATCH_UP - 1))
        };

        for height in first..=tip {
            self.process_block(height).await?;
            *last_height = height;
        }
        Ok(())
    }

    async fn process_block(&self, height: u64) -> Result<()> {
        let hash = self.rpc("getblockhash", json!([height])).await?;
        let block = self.rpc("getblock", json!([hash, 3])).await?;

        let transfers = extract_transfers(&block);
        // println!(
        //     "BTC block {} - {} txs",
        //     height,
        //     block["tx"].as_array().map(|t| t.len()).unwrap_or(0)
        // );
        if transfers.is_empty() {
            return Ok(());
        }

        let btc_price = self.price_svc.get_usd_price("BTC").await?;
        let dt = DateTime::from_timestamp(block["time"].as_i64().unwrap_or(0), 0)
            .unwrap_or_default()
            .naive_utc();

        for t in transfers {
            let value_usd = (t.sats as f64 / SATS_PER_BTC) * btc_price;
            if value_usd < self.threshold_usd {
                continue;
            }
            //println!("BTC transfer: ${:.2}", value_usd);

            let new_tx = NewTransaction {
                hash: t.txid,
                chain: "bitcoin".to_string(),
                from_address: t.from,
                to_address: t.to,
                value: t.sats.to_string(),
                value_usd,
                token: "native".to_string(),
                token_symbol: "BTC".to_string(),
                block_number: height as i32,
                tx_timestamp: dt,
            };
            self.save(&new_tx).await;
        }
        Ok(())
    }

    async fn save(&self, tx: &NewTransaction) {
        match self.repo.get_by_hash(&tx.hash).await {
            Ok(Some(_)) => return,
            Ok(None) => {}
            Err(e) => { eprintln!("DB check error: {}", e); return; }
        }
        match self.repo.create(tx).await {
            Ok(_) => {}
            //Ok(_) => eprintln!("Saved: {} ${:.2} | {}...", tx.token_symbol, tx.value_usd, &tx.hash[..10]),
            Err(e) => eprintln!("DB insert error: {}", e),
        }
    }
}

fn parse_height(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| {
        let s = v.as_str()?;
        match s.strip_prefix("0x") {
            Some(hex) => u64::from_str_radix(hex, 16).ok(),
            None => s.parse().ok(),
        }
    })
}

fn to_sats(btc: f64) -> u64 {
    (btc * SATS_PER_BTC).round() as u64
}

fn extract_transfers(block: &Value) -> Vec<BtcTransfer> {
    let mut out = Vec::new();
    let Some(txs) = block["tx"].as_array() else { return out };

    for tx in txs {
        let (Some(txid), Some(vin), Some(vout)) = (
            tx["txid"].as_str(),
            tx["vin"].as_array(),
            tx["vout"].as_array(),
        ) else { continue };

        if vin.iter().any(|i| i.get("coinbase").is_some()) {
            continue;
        }

        let input_addrs: Vec<&str> = vin
            .iter()
            .filter_map(|i| i["prevout"]["scriptPubKey"]["address"].as_str())
            .collect();
        let input_set: HashSet<&str> = input_addrs.iter().copied().collect();

        let mut total: u64 = 0;
        let mut biggest: Option<(u64, &str)> = None;
        for o in vout {
            let Some(addr) = o["scriptPubKey"]["address"].as_str() else { continue };
            if input_set.contains(addr) { continue; }
            let sats = to_sats(o["value"].as_f64().unwrap_or(0.0));
            total += sats;
            if biggest.map_or(true, |(b, _)| sats > b) {
                biggest = Some((sats, addr));
            }
        }

        if let (Some((_, to)), true) = (biggest, total > 0) {
            let from = match input_set.len() {
                0 => "unknown".to_string(),
                1 => input_addrs[0].to_string(),
                n => format!("{} (+{} more)", input_addrs[0], n - 1),
            };
            out.push(BtcTransfer { txid: txid.to_string(), from, to: to.to_string(), sats: total });
        }
    }
    out
}