use anyhow::{bail, Result};
use chrono::{DateTime, Duration as ChronoDuration, SecondsFormat, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
struct CacheEntry {
    price: f64,
    expires_at: Instant,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryPoint {
    pub t: i64,
    pub price: f64,
}

type HistoryCache = Arc<Mutex<HashMap<String, (Instant, Vec<HistoryPoint>)>>>;

#[derive(Clone)]
pub struct PriceService {
    client: Client,
    api_key: String,
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
    history_cache: HistoryCache,
}

#[derive(Deserialize)]
struct PricesResponse {
    data: Vec<TokenPrices>,
}

#[derive(Deserialize)]
struct TokenPrices {
    symbol: String,
    #[serde(default)]
    prices: Vec<PricePoint>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct PricePoint {
    currency: String,
    value: String,
}

#[derive(Deserialize)]
struct HistoryResponse {
    data: Vec<HistoryRow>,
}

#[derive(Deserialize)]
struct HistoryRow {
    value: String,
    timestamp: String,
}

impl PriceService {
    pub fn new(alchemy_key: &str) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("failed to build HTTP client"),
            api_key: alchemy_key.to_string(),
            cache: Arc::new(Mutex::new(HashMap::new())),
            history_cache: Arc::new(Mutex::new(HashMap::new())),
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

        let url = format!(
            "https://api.g.alchemy.com/prices/v1/{}/tokens/by-symbol",
            self.api_key
        );

        let res: PricesResponse = async {
            self.client
                .get(&url)
                .query(&[("symbols", upper.as_str())])
                .send()
                .await?
                .error_for_status()?
                .json()
                .await
        }
        .await
        .map_err(|e: reqwest::Error| e.without_url())?;

        let token = res
            .data
            .into_iter()
            .find(|t| t.symbol.eq_ignore_ascii_case(&upper))
            .ok_or_else(|| anyhow::anyhow!("Alchemy returned no entry for {}", upper))?;

        if let Some(err) = token.error {
            bail!("Alchemy price error for {}: {}", upper, err);
        }

        let price = token
            .prices
            .iter()
            .find(|p| p.currency.eq_ignore_ascii_case("usd"))
            .and_then(|p| p.value.parse::<f64>().ok())
            .filter(|p| *p > 0.0)
            .ok_or_else(|| anyhow::anyhow!("No USD price for {}", upper))?;

        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(
                upper,
                CacheEntry {
                    price,
                    expires_at: Instant::now() + Duration::from_secs(60),
                },
            );
        }
        Ok(price)
    }

    pub async fn get_history(&self, symbol: &str, hours: i64, interval: &str) -> Result<Vec<HistoryPoint>> {
        let symbol = symbol.to_uppercase();
        let key = format!("{}:{}:{}", symbol, hours, interval);

        {
            let cache = self.history_cache.lock().unwrap();
            if let Some((expires_at, points)) = cache.get(&key) {
                if *expires_at > Instant::now() {
                    return Ok(points.clone());
                }
            }
        }

        let end = Utc::now();
        let start = end - ChronoDuration::hours(hours);
        let body = json!({
            "symbol": symbol,
            "startTime": start.to_rfc3339_opts(SecondsFormat::Secs, true),
            "endTime": end.to_rfc3339_opts(SecondsFormat::Secs, true),
            "interval": interval,
        });

        let url = format!(
            "https://api.g.alchemy.com/prices/v1/{}/tokens/historical",
            self.api_key
        );

        let res: HistoryResponse = async {
            self.client
                .post(&url)
                .json(&body)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await
        }
        .await
        .map_err(|e: reqwest::Error| e.without_url())?;

        let points: Vec<HistoryPoint> = res
            .data
            .iter()
            .filter_map(|row| {
                let t = DateTime::parse_from_rfc3339(&row.timestamp).ok()?.timestamp_millis();
                let price = row.value.parse::<f64>().ok()?;
                Some(HistoryPoint { t, price })
            })
            .collect();

        if points.is_empty() {
            bail!("Alchemy returned no price history for {}", symbol);
        }

        self.history_cache
            .lock()
            .unwrap()
            .insert(key, (Instant::now() + Duration::from_secs(300), points.clone()));
        Ok(points)
    }
}