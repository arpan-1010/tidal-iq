use super::context::{fmt_usd, short_addr};
use super::nvidia::AiError;
use super::AiState;
use anyhow::Result;
use chrono::NaiveDateTime;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[derive(sqlx::FromRow)]
struct DocRow {
    tx_hash: String,
    summary: String,
    importance_tier: String,
    anomaly_flags: Value,
    chain: String,
    token_symbol: String,
    value_usd: Option<f64>,
    from_address: String,
    to_address: String,
    tx_timestamp: NaiveDateTime,
}

pub fn vec_literal(v: &[f32]) -> String {
    let mut s = String::with_capacity(v.len() * 10 + 2);
    s.push('[');
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&x.to_string());
    }
    s.push(']');
    s
}

fn doc_text(r: &DocRow) -> String {
    let flags: Vec<&str> = r
        .anomaly_flags
        .as_array()
        .map(|a| a.iter().filter_map(|f| f["code"].as_str()).collect())
        .unwrap_or_default();
    let flag_text = if flags.is_empty() { "none".to_string() } else { flags.join(", ") };
    format!(
        "{} {} transfer of {} from {} to {} at {}. Importance: {}. Flags: {}. {}",
        r.chain,
        r.token_symbol,
        fmt_usd(r.value_usd.unwrap_or(0.0)),
        short_addr(&r.from_address),
        short_addr(&r.to_address),
        r.tx_timestamp.format("%Y-%m-%d %H:%M UTC"),
        r.importance_tier,
        flag_text,
        r.summary
    )
}

impl AiState {
    async fn embed_batch(&self) -> Result<Option<usize>> {
        let Some(client) = &self.client else { return Ok(None) };

        let rows: Vec<DocRow> = sqlx::query_as(
            "SELECT a.tx_hash, a.summary, a.importance_tier, a.anomaly_flags,
                    t.chain, t.token_symbol, t.value_usd::float8 AS value_usd,
                    t.from_address, t.to_address, t.tx_timestamp
             FROM ai_analyses a
             JOIN transactions t ON t.hash = a.tx_hash
             LEFT JOIN tx_embeddings e ON e.tx_hash = a.tx_hash
             WHERE e.tx_hash IS NULL
             ORDER BY t.tx_timestamp DESC
             LIMIT 16",
        )
        .fetch_all(self.pool())
        .await?;
        if rows.is_empty() {
            return Ok(Some(0));
        }

        let texts: Vec<String> = rows.iter().map(doc_text).collect();
        let vectors = match client.embed(&texts, "passage").await {
            Ok(v) => v,
            Err(AiError::Invalid(m)) => anyhow::bail!(m),
            Err(e) => {
                eprintln!("embedder paused: {e}");
                return Ok(None);
            }
        };
        if vectors.len() != rows.len() {
            anyhow::bail!("embedding count mismatch: got {} for {} inputs", vectors.len(), rows.len());
        }

        let model = client.registry().await.embed;
        for (row, v) in rows.iter().zip(vectors.iter()) {
            sqlx::query(
                "INSERT INTO tx_embeddings (tx_hash, embedding, model)
                 VALUES ($1, $2::halfvec, $3)
                 ON CONFLICT (tx_hash) DO NOTHING",
            )
            .bind(&row.tx_hash)
            .bind(vec_literal(v))
            .bind(&model)
            .execute(self.pool())
            .await?;
        }
        Ok(Some(rows.len()))
    }

    pub async fn run_embedder(self: Arc<Self>) {
        println!("AI embedder started");
        loop {
            let wait = match self.embed_batch().await {
                Ok(Some(0)) => 60,
                Ok(Some(n)) => {
                    //println!("Embedded {n} transactions");
                    5
                }
                Ok(None) => 300,
                Err(e) => {
                    eprintln!("embedder error: {e}");
                    120
                }
            };
            sleep(Duration::from_secs(wait)).await;
        }
    }
}