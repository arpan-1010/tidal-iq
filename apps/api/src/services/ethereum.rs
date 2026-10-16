use crate::db::models::NewTransaction;
use crate::db::repository::TransactionRepo;
use crate::services::price::PriceService;
use alloy::consensus::Transaction;
use alloy::eips::BlockNumberOrTag;
use alloy::providers::{Provider, ProviderBuilder};
use anyhow::Result;
use chrono::DateTime;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

const MAX_CATCH_UP: u64 = 5;

struct TokenInfo {
    symbol: &'static str,
    decimals: u8,
}

fn watched_tokens() -> HashMap<&'static str, TokenInfo> {
    let mut m = HashMap::new();
    m.insert("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48", TokenInfo {symbol: "USDC", decimals: 6});
    m.insert("0xdac17f958d2ee523a2206206994597c13d831ec7", TokenInfo {symbol: "USDT", decimals: 6});
    m.insert("0x6b175474e89094c44da98b954eedeac495271d0f", TokenInfo {symbol: "DAI", decimals: 18});
    m.insert("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599", TokenInfo {symbol: "WBTC", decimals: 8});
    m
}

pub struct EthereumService {
    repo: Arc<TransactionRepo>,
    price_svc: PriceService,
    threshold_usd: f64,
    alchemy_url: String,
}

impl EthereumService {
    pub fn new(repo: Arc<TransactionRepo>, price_svc: PriceService, threshold_usd: f64, alchemy_key: &str) -> Self {
        Self {
            repo,
            price_svc,
            threshold_usd,
            alchemy_url: format!("https://eth-mainnet.g.alchemy.com/v2/{}", alchemy_key)
        }
    }

    pub async fn start(self: Arc<Self>) {
        println!("Ethereum service starting (threshold : ${})", self.threshold_usd);

        let url = match self.alchemy_url.parse() {
            Ok(u) => u,
            Err(e) => {
                eprintln!("Invalid Alchemy URL: {}", e);
                return;
            }
        };

        let provider = ProviderBuilder::new().connect_http(url);
        let mut last_block: u64 = 0;

        loop {
            if let Err(e) = self.poll(&provider, &mut last_block).await {
                eprintln!("Ethereum error : {}", e);
            }

            sleep(Duration::from_secs(4)).await;
        }
    }

    async fn poll(&self, provider: &impl Provider, last_block: &mut u64) -> Result<()> {
        let latest = provider.get_block_number().await?;
        if latest <= *last_block {
            return Ok(());
        }

        let first = if *last_block == 0 {
            latest
        } else {
            (*last_block + 1).max(latest.saturating_sub(MAX_CATCH_UP - 1))
        };

        for number in first..=latest {
            self.process_block(provider, number).await?;
            *last_block = number;
        }
        Ok(())
    }

    async fn process_block(&self, provider: &impl Provider, block_number: u64) -> Result<()> {
        let block = provider
            .get_block_by_number(BlockNumberOrTag::Number(block_number))
            .full()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Block {} not found", block_number))?;

        println!("Block {} - {} txs", block_number, block.transactions.len());

        let dt = DateTime::from_timestamp(block.header.timestamp as i64, 0)
            .unwrap_or_default()
            .naive_utc();

        if let alloy::rpc::types::BlockTransactions::Full(txs) = &block.transactions {
            let eth_price = self.price_svc.get_usd_price("ETH").await?;

            for tx in txs {
                let value = tx.inner.value().saturating_to::<u128>();
                if value == 0 { continue; }

                let value_usd = (value as f64 / 1e18) * eth_price;
                if value_usd < self.threshold_usd { continue; }

                println!("ETH transfer: ${:.2}", value_usd);

                let new_tx = NewTransaction {
                    hash: tx.inner.tx_hash().to_string(),
                    chain: "ethereum".to_string(),
                    from_address: tx.inner.signer().to_string(),
                    to_address: tx.inner.to().map(|a| a.to_string()).unwrap_or_default(),
                    value: value.to_string(),
                    value_usd,
                    token: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
                    token_symbol: "ETH".to_string(),
                    block_number: block_number as i32,
                    tx_timestamp: dt,
                };
                self.save(&new_tx).await;
            }
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
            Ok(_id) => eprintln!(
                "Saved: {} ${:.2} | {}...",
                tx.token_symbol, tx.value_usd, &tx.hash[..10] 
            ),
            Err(e) => eprintln!("DB insert error: {}", e),
        }
    }
}