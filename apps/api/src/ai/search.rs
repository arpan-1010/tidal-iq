use super::context::{explorer_tx_url, short_addr};
use super::embed::vec_literal;
use super::nvidia::{GenOpts, Tier};
use super::{prompts, AiState, DISCLAIMER};
use anyhow::Result;
use chrono::{Duration, NaiveDateTime, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
struct ParsedQuery {
    chains: Option<Vec<String>>,
    symbols: Option<Vec<String>>,
    min_usd: Option<f64>,
    max_usd: Option<f64>,
    period: Option<String>,
    anomaly_only: Option<bool>,
    semantic_text: Option<String>,
    flags: Option<Vec<String>>,
}

#[derive(Serialize, Default, Clone, Debug)]
pub struct Filters {
    pub chains: Option<Vec<String>>,
    pub symbols: Option<Vec<String>>,
    pub min_usd: Option<f64>,
    pub max_usd: Option<f64>,
    pub period: Option<String>,
    pub anomaly_only: bool,
    pub semantic_text: Option<String>,
    pub flags: Option<Vec<String>>,
}

fn validate(p: ParsedQuery) -> Filters {
    let norm_chain = |s: &str| -> Option<&'static str> {
        match s.trim().to_lowercase().as_str() {
            "ethereum" | "eth" => Some("ethereum"),
            "solana" | "sol" => Some("solana"),
            "bitcoin" | "btc" => Some("bitcoin"),
            _ => None,
        }
    };
    let mut chains: Vec<String> = p
        .chains
        .unwrap_or_default()
        .iter()
        .filter_map(|c| norm_chain(c))
        .map(String::from)
        .collect();
    chains.sort();
    chains.dedup();

    let mut symbols: Vec<String> = p
        .symbols
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty() && s.len() <= 10 && s.chars().all(|c| c.is_ascii_alphanumeric()))
        .take(5)
        .collect();
    symbols.sort();
    symbols.dedup();

    let mut flags: Vec<String> = p
        .flags
        .unwrap_or_default()
        .into_iter()
        .map(|f| f.trim().to_uppercase())
        .filter(|f| matches!(f.as_str(), "UNUSUAL_SIZE" | "FIRST_LARGE_MOVE" | "RAPID_SUCCESSION" | "TOKEN_OUTLIER"))
        .collect();
    flags.sort();
    flags.dedup();

    Filters {
        chains: if chains.is_empty() { None } else { Some(chains) },
        symbols: if symbols.is_empty() { None } else { Some(symbols) },
        min_usd: p.min_usd.filter(|v| v.is_finite() && *v >= 0.0),
        max_usd: p.max_usd.filter(|v| v.is_finite() && *v >= 0.0),
        period: p.period.filter(|x| {
            matches!(x.as_str(), "last_hour" | "last_24h" | "today" | "yesterday" | "last_7d" | "last_30d")
        }),
        anomaly_only: p.anomaly_only.unwrap_or(false),
        semantic_text: p
            .semantic_text
            .map(|s| s.trim().chars().take(200).collect::<String>())
            .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("null")),
        flags: if flags.is_empty() { None } else { Some(flags) },
    }
}

fn heuristic_filters(q: &str) -> Filters {
    let l = q.to_lowercase();
    let words: Vec<&str> = l.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let has = |w: &str| words.contains(&w);

    let mut chains = Vec::new();
    if has("bitcoin") || has("btc") { chains.push("bitcoin".to_string()); }
    if has("ethereum") || has("eth") { chains.push("ethereum".to_string()); }
    if has("solana") || has("sol") { chains.push("solana".to_string()); }

    let period = if l.contains("last hour") { Some("last_hour") }
        else if has("yesterday") { Some("yesterday") }
        else if has("today") { Some("today") }
        else if has("week") { Some("last_7d") }
        else if has("month") { Some("last_30d") }
        else { None };

    Filters {
        chains: if chains.is_empty() { None } else { Some(chains) },
        period: period.map(String::from),
        anomaly_only: words.iter().any(|w| w.starts_with("anomal") || *w == "unusual" || *w == "suspicious"),
        ..Default::default()
    }
}

fn period_bounds(p: &str, now: NaiveDateTime) -> (Option<NaiveDateTime>, Option<NaiveDateTime>) {
    let today = now.date().and_time(NaiveTime::MIN);
    match p {
        "last_hour" => (Some(now - Duration::hours(1)), None),
        "last_24h" => (Some(now - Duration::hours(24)), None),
        "today" => (Some(today), None),
        "yesterday" => (Some(today - Duration::days(1)), Some(today)),
        "last_7d" => (Some(now - Duration::days(7)), None),
        "last_30d" => (Some(now - Duration::days(30)), None),
        _ => (None, None),
    }
}

#[derive(sqlx::FromRow)]
struct SearchRow {
    hash: String,
    chain: String,
    token_symbol: String,
    value_usd: Option<f64>,
    from_address: String,
    to_address: String,
    tx_timestamp: NaiveDateTime,
    summary: Option<String>,
    importance_tier: Option<String>,
    importance_score: Option<f32>,
    distance: Option<f64>,
}

impl AiState {
    pub async fn search(&self, query: &str, limit: i64) -> Result<Value> {
        let now = Utc::now().naive_utc();
        let mut note: Option<String> = None;
        let mut ai_used = false;
        let mut qvec: Option<String> = None;

        let use_ai = self.client.as_ref().map(|c| c.cfg.feature_search).unwrap_or(false);
        let mut filters = Filters::default();

        if let (true, Some(client)) = (use_ai, &self.client) {
            let opts = GenOpts::new(Tier::Fast, "search_parse", prompts::SEARCH_PARSE_V).max_tokens(300);
            let user = format!(
                "Current time (UTC): {}\n<user_query>{}</user_query>\nReturn the JSON object now.",
                now.format("%Y-%m-%d %H:%M"),
                query
            );
            match client
                .generate_object::<ParsedQuery>(prompts::SEARCH_PARSE_SYSTEM, &user, &opts)
                .await
            {
                Ok((p, _)) => {
                    filters = validate(p);
                    ai_used = true;
                }
                Err(e) => {
                    eprintln!("search parse failed: {e}");
                    note = Some("AI query understanding is unavailable; used basic keyword filters.".into());
                }
            }

            if let Some(text) = filters.semantic_text.clone() {
                match client.embed(&[text], "query").await {
                    Ok(mut v) if !v.is_empty() => qvec = Some(vec_literal(&v.remove(0))),
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("search embed failed: {e}");
                        note = Some("Semantic matching is unavailable; showing filter matches only.".into());
                    }
                }
            }
        }
        if !ai_used {
            filters = heuristic_filters(query);
            if note.is_none() {
                note = Some("AI search is off; used basic keyword filters.".into());
            }
        }

        let (since, until) = filters
            .period
            .as_deref()
            .map(|p| period_bounds(p, now))
            .unwrap_or((None, None));

        let rows: Vec<SearchRow> = sqlx::query_as(
            "SELECT t.hash, t.chain, t.token_symbol, t.value_usd::float8 AS value_usd,
                    t.from_address, t.to_address, t.tx_timestamp,
                    a.summary, a.importance_tier, a.importance_score,
                    (e.embedding <=> $1::halfvec) AS distance
             FROM transactions t
             LEFT JOIN ai_analyses a ON a.tx_hash = t.hash
             LEFT JOIN tx_embeddings e ON e.tx_hash = t.hash
             WHERE ($2::text[] IS NULL OR t.chain = ANY($2))
               AND ($3::text[] IS NULL OR t.token_symbol = ANY($3))
               AND ($4::float8 IS NULL OR t.value_usd::float8 >= $4)
               AND ($5::float8 IS NULL OR t.value_usd::float8 <= $5)
               AND ($6::timestamp IS NULL OR t.tx_timestamp >= $6)
               AND ($7::timestamp IS NULL OR t.tx_timestamp < $7)
               AND ($8::bool = false OR t.is_anomaly = true)
               AND ($9::text[] IS NULL OR EXISTS (
               SELECT 1 FROM jsonb_array_elements(a.anomaly_flags) f
               WHERE f->>'code' = ANY($9)))
               ORDER BY distance ASC NULLS LAST, a.importance_score DESC NULLS LAST, t.tx_timestamp DESC
               LIMIT $10",
        )
        .bind(&qvec)
        .bind(&filters.chains)
        .bind(&filters.symbols)
        .bind(filters.min_usd)
        .bind(filters.max_usd)
        .bind(since)
        .bind(until)
        .bind(filters.anomaly_only)
        .bind(&filters.flags)
        .bind(limit)
        .fetch_all(self.pool())
        .await?;

        let results: Vec<Value> = rows
            .into_iter()
            .map(|r| {
                json!({
                    "hash": r.hash,
                    "chain": r.chain,
                    "token": r.token_symbol,
                    "value_usd": r.value_usd,
                    "from": short_addr(&r.from_address),
                    "to": short_addr(&r.to_address),
                    "time": r.tx_timestamp.format("%Y-%m-%d %H:%M UTC").to_string(),
                    "summary": r.summary,
                    "summary_pending": r.summary.is_none(),
                    "importance_tier": r.importance_tier,
                    "importance_score": r.importance_score,
                    "similarity": r.distance.map(|d| ((1.0 - d) * 1000.0).round() / 1000.0),
                    "explorer_url": explorer_tx_url(&r.chain, &r.hash),
                })
            })
            .collect();

        let mut available_tokens: Option<Vec<String>> = None;
        if results.is_empty() {
            let t: Vec<(String,)> =
                sqlx::query_as("SELECT DISTINCT token_symbol FROM transactions ORDER BY 1")
                    .fetch_all(self.pool())
                    .await?;
            available_tokens = Some(t.into_iter().map(|(s,)| s).collect());
        }

        Ok(json!({
            "query": query,
            "filters": filters,
            "ai_used": ai_used,
            "semantic": qvec.is_some(),
            "count": results.len(),
            "results": results,
            "available_tokens": available_tokens,
            "note": note,
            "disclaimer": DISCLAIMER,
        }))
    }
}