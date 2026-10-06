use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
struct CacheEntry {
    price: f64,
    expires_at: Instant,
}

#[derive(Clone)]
pub struct PriceService {
    client: Client,
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
}

#[derive(Deserialize)]
struct CoinGeckoResponse {
    #[serde(flatten)]
    prices: HashMap<String, HashMap<String, f64>>,
}

impl PriceService {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn coingecko_id(symbol: &str) -> Option<&'static str> {
        match symbol.to_uppercase().as_str() {
            "ETH" => Some("ethereum"),
            "BTC" => Some("bitcoin"),
            "WBTC" => Some("wrapped-bitcoin"),
            "USDC" => Some("usd-coin"),
            "USDT" => Some("tether"),
            "DAI" => Some("dai"),
            _ => None,
        }
    }

    pub async fn get_usd_price(&self, symbol: &str) -> Result<f64> {
        let upper = symbol.to_uppercase();

        if matches!(upper.as_str(), "USDC" | "USDT" | "DAI" | "BUSD") {
            return Ok(1.0);
        }

        {
            let cache = self.cache.lock().unwrap();
            if let Some(entry) = cache.get(&upper) {
                if entry.expires_at > Instant::now() {
                    return Ok(entry.price);
                }
            }
        }

        let coin_id = Self::coingecko_id(&upper)
            .ok_or_else(|| anyhow::anyhow!("No CoinGecko ID for {}", upper))?;

        let url = format!(
            "https://api.coingecko.com/api/v3/simple/price?ids{}&vs_currencies=usd",
            coin_id
        );
        let res: CoinGeckoResponse = self.client.get(&url).send().await?.json().await?;

        let price = res.prices
            .get(coin_id)
            .and_then(|m| m.get("usd"))
            .copied()
            .unwrap_or(0.0);

        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(upper, CacheEntry {
                price,
                expires_at: Instant::now() + Duration::from_secs(60),
            });
        }
        Ok(price)
    }

    pub async fn wei_to_usd(&self, wei: u128, symbol: &str) -> Result<f64> {
        let price = self.get_usd_price(symbol).await?;
        let eth_value = wei as f64 / 1e18;
        Ok(eth_value * price)
    }
}