use crate::db::models::NewTransaction;
use crate::db::repository::TransactionRepo;
use crate::services::price::PriceService;
use anyhow::{bail, Result};
use chrono::DateTime;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

const LAMPORTS_PER_SOL: f64 = 1_000_000_000.0;
const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
const POLL_SECS: u64 = 2;

const MAX_SLOTS_PER_POLL: u64 = 1;

struct Transfer {
    signature: String,
    from: String,
    to: String,
    lamports: u64,
}

pub struct SolanaService {
    repo: Arc<TransactionRepo>,
    price_svc: PriceService,
    threshold_usd: f64,
    rpc_url: String,
    client: Client,
}

impl SolanaService {
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
            rpc_url: format!("https://solana-mainnet.g.alchemy.com/v2/{}", alchemy_key),
            client: Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                .expect("failed to build HTTP client"),
        }
    }

    pub async fn start(self: Arc<Self>) {
        println!("Solana service starting (threshold : ${})", self.threshold_usd);
        let mut last_slot: u64 = 0;
        loop {
            if let Err(e) = self.poll(&mut last_slot).await {
                eprintln!("Solana error : {}", e);
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

        if let Some(err) = res.get("error") {
            bail!("{} failed: {}", method, err);
        }
        Ok(res.get_mut("result").map(Value::take).unwrap_or(Value::Null))
    }

    async fn poll(&self, last_slot: &mut u64) -> Result<()> {
        let latest = self
            .rpc("getSlot", json!([{ "commitment": "finalized" }]))
            .await?
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("getSlot returned no slot"))?;

        if latest <= *last_slot {
            return Ok(());
        }
        let first = if *last_slot == 0 {
            latest
        } else {
            (*last_slot + 1).max(latest.saturating_sub(MAX_SLOTS_PER_POLL - 1))
        };
        *last_slot = latest;

        for slot in first..=latest {
            if let Err(e) = self.process_slot(slot).await {
                let msg = e.to_string();

                if !msg.contains("-32007") && !msg.contains("-32009") {
                    eprintln!("Solana slot {} : {}", slot, msg);
                }
            }
        }
        Ok(())
    }

    async fn process_slot(&self, slot: u64) -> Result<()> {
        let block = self
            .rpc("getBlock", json!([slot, {
                "encoding": "jsonParsed",
                "transactionDetails": "full",
                "rewards": false,
                "maxSupportedTransactionVersion": 1,
                "commitment": "finalized"
            }]))
            .await?;

        let transfers = extract_transfers(&block);
        //println!("Slot {} - {} SOL transfers", slot, transfers.len());
        if transfers.is_empty() {
            return Ok(());
        }

        let sol_price = self.price_svc.get_usd_price("SOL").await?;
        let dt = DateTime::from_timestamp(block["blockTime"].as_i64().unwrap_or(0), 0)
            .unwrap_or_default()
            .naive_utc();

        for t in transfers {
            let value_usd = (t.lamports as f64 / LAMPORTS_PER_SOL) * sol_price;
            if value_usd < self.threshold_usd {
                continue;
            }
            //println!("SOL transfer: ${:.2}", value_usd);

            let new_tx = NewTransaction {
                hash: t.signature,
                chain: "solana".to_string(),
                from_address: t.from,
                to_address: t.to,
                value: t.lamports.to_string(),
                value_usd,
                token: SOL_MINT.to_string(),
                token_symbol: "SOL".to_string(),
                block_number: slot as i32,
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

fn extract_transfers(block: &Value) -> Vec<Transfer> {
    let mut out = Vec::new();
    let Some(txs) = block["transactions"].as_array() else { return out };

    for tx in txs {
        if !tx["meta"]["err"].is_null() { continue; }
        let Some(signature) = tx["transaction"]["signatures"][0].as_str() else { continue };
        let Some(instructions) = tx["transaction"]["message"]["instructions"].as_array() else { continue };

        let best = instructions
            .iter()
            .filter(|ix| ix["program"] == "system" && ix["parsed"]["type"] == "transfer")
            .filter_map(|ix| {
                let info = &ix["parsed"]["info"];
                Some(Transfer {
                    signature: signature.to_string(),
                    from: info["source"].as_str()?.to_string(),
                    to: info["destination"].as_str()?.to_string(),
                    lamports: info["lamports"].as_u64()?,
                })
            })
            .max_by_key(|t| t.lamports);

        if let Some(t) = best { out.push(t); }
    }
    out
}