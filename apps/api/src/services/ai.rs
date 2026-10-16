use crate::db::repository::TransactionRepo;
use anyhow::{bail, Result};
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

pub struct AiService {
    repo: Arc<TransactionRepo>,
    client: Client,
    base_url: String,
    model: String,
    api_key: String,
}

impl AiService {
    pub fn new(repo: Arc<TransactionRepo>, base_url: String, model: String, api_key: String) -> Self {
        Self {
            repo,
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client"),
            base_url,
            model,
            api_key,
        }
    }

    pub async fn start(self: Arc<Self>) {
        println!("AI service starting (model: {})", self.model);
        loop {
            if let Err(e) = self.run_batch().await {
                eprintln!("AI error : {}", e);
                sleep(Duration::from_secs(30)).await;
            }
            sleep(Duration::from_secs(15)).await;
        }
    }

    async fn run_batch(&self) -> Result<()> {
        let rows = self.repo.get_unsummarized(5).await?;
        for (hash, chain, from, to, symbol, usd, anomaly, reason) in rows {
            let prompt = format!(
                "Transaction on {chain}: {symbol} worth ${usd:.0} from {from} to {to}.{}",
                match (anomaly, reason) {
                    (Some(true), Some(r)) => format!(" Flagged unusual: {r}."),
                    _ => String::new(),
                }
            );
            let summary = self.complete(&prompt).await?;
            self.repo.set_summary(&hash, &summary).await?;
            println!("AI summary saved for {}...", &hash[..10.min(hash.len())]);
            sleep(Duration::from_secs(8)).await;
        }
        Ok(())
    }

    async fn complete(&self, prompt: &str) -> Result<String> {
        let body = json!({
            "model": self.model,
            "temperature": 0.2,
            "max_tokens": 1500,
            "messages": [
                { "role": "system", "content":
                  "You write one-sentence summaries of large crypto transactions for a dashboard. \
                   Use only the facts given. Never guess who owns an address. Max 25 words." },
                { "role": "user", "content": prompt }
            ]
        });

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
                let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.without_url())?;

        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.without_url())?;
        if !status.is_success() {
            bail!(
                "AI HTTP {} from {} : {}",
                status,
                url,
                text.chars().take(300).collect::<String>()
            );
        }
        let res: Value = serde_json::from_str(&text)?;

        match res["choices"][0]["message"]["content"].as_str() {
            Some(s) if !s.trim().is_empty() => Ok(s.trim().to_string()),
            _ => bail!(
                "AI response had no content. finish_reason={} raw={}",
                res["choices"][0]["finish_reason"],
                text.chars().take(400).collect::<String>()
            ),
        }
    }
}