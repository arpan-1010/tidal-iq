use super::chat::Citation;
use super::context::{explorer_tx_url, fmt_usd, short_addr};
use super::nvidia::{GenOpts, Tier};
use super::safety::{citations_in, numbers_in, validate_chat_text};
use super::{prompts, AiState, DISCLAIMER};
use anyhow::Result;
use chrono::{Timelike, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

#[derive(sqlx::FromRow)]
struct TxRow {
    hash: String,
    chain: String,
    token: String,
    value_usd: Option<f64>,
    from_address: Option<String>,
    to_address: Option<String>,
    time: String,
    summary: Option<String>,
    tier: Option<String>,
    flags: Option<Value>,
}

macro_rules! tx_select {
    ($tail:literal) => {
        concat!(
            "SELECT t.hash, t.chain, COALESCE(t.token_symbol, '?') AS token,
                    t.value_usd::float8 AS value_usd, t.from_address, t.to_address,
                    t.tx_timestamp::text AS time, a.summary,
                    a.importance_tier AS tier, a.anomaly_flags AS flags
             FROM transactions t LEFT JOIN ai_analyses a ON a.tx_hash = t.hash ",
            $tail
        )
    };
}

#[derive(Serialize, Clone, Debug)]
pub struct NarrativeOut {
    pub id: Option<i64>,
    pub window_hours: i32,
    pub text: String,
    pub citations: Vec<Citation>, // only the ones the text cites
    pub stats: Vec<Value>,        // per-chain totals for the window
    pub source: String,           // "llm" | "template" | "cache"
    pub model: Option<String>,
    pub created_at: Option<String>,
    pub disclaimer: &'static str,
}

fn cite(n: usize, r: &TxRow) -> Citation {
    Citation {
        n,
        hash: r.hash.clone(),
        chain: r.chain.clone(),
        token: r.token.clone(),
        value_usd: r.value_usd,
        time: r.time.clone(),
        summary: r.summary.clone(),
        explorer_url: explorer_tx_url(&r.chain, &r.hash),
    }
}

fn cited_from(all: &[Option<Citation>], text: &str) -> Vec<Citation> {
    let mut ns = citations_in(text);
    ns.sort();
    ns.dedup();
    ns.into_iter()
        .filter(|n| *n >= 1)
        .filter_map(|n| all.get(n - 1).cloned().flatten())
        .collect()
}

impl AiState {
    /// Multi-transaction narrative for the last `hours` hours (roadmap 9.2).
    pub async fn narrative(&self, hours: i32) -> Result<Option<NarrativeOut>> {
        let hours = hours.clamp(1, 48);
        let pool = self.pool();

        // 1. Per-chain totals for the window
        let chain_rows: Vec<(String, i64, f64, f64)> = sqlx::query_as(
            "SELECT chain, COUNT(*), COALESCE(SUM(value_usd), 0)::float8, COALESCE(MAX(value_usd), 0)::float8
             FROM transactions
             WHERE tx_timestamp > (now() at time zone 'utc') - make_interval(hours => $1)
             GROUP BY chain",
        )
        .bind(hours)
        .fetch_all(pool)
        .await?;
        if chain_rows.is_empty() {
            return Ok(None);
        }
        let stats: Vec<Value> = chain_rows
            .iter()
            .map(|(c, n, total, big)| json!({ "chain": c, "count": n, "total_usd": total.round(), "largest_usd": big.round() }))
            .collect();

        // 2. Fresh cached narrative? (30 minutes)
        let cached: Option<(i64, String, Vec<String>, Option<String>, String)> = sqlx::query_as(
            "SELECT id, narrative, tx_hashes, model, created_at::text FROM narratives
             WHERE window_hours = $1 AND created_at > now() - interval '30 minutes'
             ORDER BY id DESC LIMIT 1",
        )
        .bind(hours)
        .fetch_optional(pool)
        .await?;

        if let Some((id, text, hashes, model, created)) = cached {
            let rows: Vec<TxRow> = sqlx::query_as(tx_select!("WHERE t.hash = ANY($1)"))
                .bind(&hashes)
                .fetch_all(pool)
                .await?;
            let all: Vec<Option<Citation>> = hashes
                .iter()
                .enumerate()
                .map(|(i, h)| rows.iter().find(|r| &r.hash == h).map(|r| cite(i + 1, r)))
                .collect();
            return Ok(Some(NarrativeOut {
                id: Some(id),
                window_hours: hours,
                citations: cited_from(&all, &text),
                text,
                stats,
                source: "cache".into(),
                model,
                created_at: Some(created),
                disclaimer: DISCLAIMER,
            }));
        }

        // 3. Facts: the 12 largest transactions + repeat senders
        let rows: Vec<TxRow> = sqlx::query_as(tx_select!(
            "WHERE t.tx_timestamp > (now() at time zone 'utc') - make_interval(hours => $1)
             ORDER BY t.value_usd DESC NULLS LAST LIMIT 12"
        ))
        .bind(hours)
        .fetch_all(pool)
        .await?;

        let repeats: Vec<(String, Option<String>, Option<String>, i64, f64)> = sqlx::query_as(
            "SELECT t.chain, MAX(l.label), t.from_address, COUNT(*), COALESCE(SUM(t.value_usd), 0)::float8
             FROM transactions t
             LEFT JOIN wallet_labels l ON l.chain = t.chain AND l.address = t.from_address
             WHERE t.tx_timestamp > (now() at time zone 'utc') - make_interval(hours => $1)
               AND t.from_address IS NOT NULL
             GROUP BY t.chain, t.from_address
             HAVING COUNT(*) >= 2
             ORDER BY 5 DESC LIMIT 5",
        )
        .bind(hours)
        .fetch_all(pool)
        .await?;

        let tx_json: Vec<Value> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let flags: Vec<String> = r
                    .flags
                    .as_ref()
                    .and_then(|f| f.as_array())
                    .map(|a| a.iter().filter_map(|x| x["code"].as_str().map(String::from)).collect())
                    .unwrap_or_default();
                json!({
                    "n": i + 1,
                    "chain": r.chain,
                    "token": r.token,
                    "value_usd": r.value_usd.map(|v| v.round()),
                    "from": r.from_address.as_deref().map(short_addr),
                    "to": r.to_address.as_deref().map(short_addr),
                    "time": r.time,
                    "importance": r.tier,
                    "flags": flags,
                    "summary": r.summary,
                })
            })
            .collect();
        let repeat_json: Vec<Value> = repeats
            .iter()
            .map(|(chain, label, addr, n, total)| {
                json!({
                    "chain": chain,
                    "wallet": label.clone().or_else(|| addr.as_deref().map(short_addr)),
                    "transfers": n,
                    "total_usd": total.round(),
                })
            })
            .collect();

        let data = json!({
            "window_hours": hours,
            "totals_by_chain": stats,
            "transactions": tx_json,
            "repeat_senders": repeat_json,
        });
        let data_str = serde_json::to_string(&data)?;
        let mut allowed = numbers_in(&data_str);
        allowed.push(rows.len() as f64);

        // 4. Model writes it, validation decides if it ships
        let all: Vec<Option<Citation>> = rows.iter().enumerate().map(|(i, r)| Some(cite(i + 1, r))).collect();
        let mut text = String::new();
        let mut source = "template";
        let mut model: Option<String> = None;

        if let Some(c) = &self.client {
            let opts = GenOpts::new(Tier::Strong, "narrative", prompts::NARRATIVE_V)
                .max_tokens(500)
                .temperature(0.3);
            let user = format!("<data>{data_str}</data>\nWrite the narrative now.");
            match c.generate_text(prompts::NARRATIVE_SYSTEM, &user, &opts).await {
                Ok(comp) => match validate_chat_text(&comp.text, &allowed, rows.len(), true, 900) {
                    Ok(()) => {
                        text = comp.text.trim().to_string();
                        source = "llm";
                        model = Some(comp.model);
                    }
                    Err(why) => eprintln!("narrative rejected: {why}"),
                },
                Err(e) => eprintln!("narrative generation failed: {e}"),
            }
        }

        if source == "template" {
            let (n, total) = chain_rows.iter().fold((0i64, 0.0f64), |a, r| (a.0 + r.1, a.1 + r.2));
            text = format!(
                "In the last {hours} hours TidalIQ tracked {n} large transfers worth {} in total.",
                fmt_usd(total)
            );
            if let Some(r) = rows.first() {
                text.push_str(&format!(
                    " The largest was {} {} worth {} [1].",
                    r.chain,
                    r.token,
                    fmt_usd(r.value_usd.unwrap_or(0.0))
                ));
            }
        }

        // 5. Store (model-written only)
        let (mut id, mut created) = (None, None);
        if source == "llm" {
            let hashes: Vec<String> = rows.iter().map(|r| r.hash.clone()).collect();
            let row: Option<(i64, String)> = sqlx::query_as(
                "INSERT INTO narratives (window_hours, narrative, tx_hashes, model)
                 VALUES ($1, $2, $3, $4) RETURNING id, created_at::text",
            )
            .bind(hours)
            .bind(&text)
            .bind(&hashes)
            .bind(&model)
            .fetch_optional(pool)
            .await?;
            if let Some((i, c)) = row {
                id = Some(i);
                created = Some(c);
            }
        }

        Ok(Some(NarrativeOut {
            id,
            window_hours: hours,
            citations: cited_from(&all, &text),
            text,
            stats,
            source: source.into(),
            model,
            created_at: created,
            disclaimer: DISCLAIMER,
        }))
    }

    /// Daily digest to Telegram (roadmap 9.4). Does nothing unless
    /// TELEGRAM_BOT_TOKEN and TELEGRAM_CHAT_ID are set.
    pub async fn run_digest_worker(self: Arc<Self>) {
        let (Ok(token), Ok(chat_id)) = (std::env::var("TELEGRAM_BOT_TOKEN"), std::env::var("TELEGRAM_CHAT_ID")) else {
            return;
        };
        let hour: u32 = std::env::var("AI_DIGEST_HOUR_UTC").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
        let http = reqwest::Client::new();
        let mut last_sent: Option<chrono::NaiveDate> = None;
        eprintln!("digest worker: sending daily at {hour:02}:00 UTC");

        loop {
            tokio::time::sleep(Duration::from_secs(600)).await;
            let now = Utc::now();
            if now.hour() != hour || last_sent == Some(now.date_naive()) {
                continue;
            }
            let out = match self.narrative(24).await {
                Ok(Some(n)) if n.source != "template" => n,
                Ok(_) => continue, // nothing worth sending; retry in 10 minutes
                Err(e) => {
                    eprintln!("digest failed: {e}");
                    continue;
                }
            };
            let mut msg = format!("TidalIQ daily digest\n\n{}\n", out.text);
            if !out.citations.is_empty() {
                msg.push_str("\nSources:\n");
                for c in out.citations.iter().take(6) {
                    msg.push_str(&format!("[{}] {}\n", c.n, c.explorer_url));
                }
            }
            msg.push_str(&format!("\n{DISCLAIMER}"));

            let res = http
                .post(format!("https://api.telegram.org/bot{token}/sendMessage"))
                .json(&json!({ "chat_id": chat_id, "text": msg, "disable_web_page_preview": true }))
                .send()
                .await;
            match res {
                Ok(r) if r.status().is_success() => last_sent = Some(now.date_naive()),
                Ok(r) => eprintln!("digest: telegram returned {}", r.status()),
                Err(e) => eprintln!("digest: telegram error: {}", e.without_url()),
            }
        }
    }
}