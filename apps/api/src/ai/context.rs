use super::{scoring::{Flag, RuleScore}, AiState};
use anyhow::Result;
use chrono::{NaiveDateTime, Timelike};
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::FromRow;

#[derive(FromRow)]
struct TxRow {
    hash: String,
    chain: String,
    token_symbol: String,
    value_usd: Option<f64>,
    from_address: String,
    to_address: String,
    value: String,
    tx_timestamp: NaiveDateTime,
}

#[derive(Serialize, Clone, Debug)]
pub struct RelatedTx {
    #[serde(skip)]
    pub hash: String,
    pub value_usd: f64,
    pub to_short: String,
    pub time: String,
}

#[derive(Clone, Debug)]
pub struct TxContext {
    pub hash: String,
    pub chain: String,
    pub symbol: String,
    pub amount: f64,
    pub value_usd: f64,
    pub from_address: String,
    pub to_address: String,
    pub time: String,
    pub hour_utc: u32,
    pub explorer_url: String,
    pub from_label: Option<String>,
    pub to_label: Option<String>,
    pub from_avg_usd_30d: Option<f64>,
    pub from_tx_count_30d: i64,
    pub multiple_of_from_avg: Option<f64>,
    pub token_avg_usd_30d: Option<f64>,
    pub multiple_of_token_avg: Option<f64>,
    pub prior_tx_count_all: i64,
    pub count_last_hour: i64,
    pub related_recent: Vec<RelatedTx>,
    pub related_total_usd: f64,
}

pub fn short_addr(a: &str) -> String {
    let (addr, rest) = match a.split_once(' ') {
        Some((first, rest)) => (first, Some(rest)),
        None => (a, None),
    };
    let short = if addr.len() > 12 && addr.is_ascii() {
        format!("{}…{}", &addr[..6], &addr[addr.len() - 4..])
    } else {
        addr.to_string()
    };
    match rest {
        Some(r) => format!("{short} {r}"),
        None => short,
    }
}

pub fn explorer_tx_url(chain: &str, hash: &str) -> String {
    match chain {
        "ethereum" => format!("https://etherscan.io/tx/{hash}"),
        "solana" => format!("https://solscan.io/tx/{hash}"),
        "bitcoin" => format!("https://mempool.space/tx/{hash}"),
        _ => String::new(),
    }
}

pub fn explorer_addr_url(chain: &str, addr: &str) -> String {
    match chain {
        "ethereum" => format!("https://etherscan.io/address/{addr}"),
        "solana" => format!("https://solscan.io/account/{addr}"),
        "bitcoin" => format!("https://mempool.space/address/{addr}"),
        _ => String::new(),
    }
}

fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn fmt_usd(v: f64) -> String {
    if v >= 1e9 { format!("${:.1}B", v / 1e9) }
    else if v >= 1e6 { format!("${:.1}M", v / 1e6) }
    else if v >= 1e3 { format!("${:.0}K", v / 1e3) }
    else { format!("${:.0}", v) }
}

pub fn fmt_amount(v: f64) -> String {
    if v >= 1000.0 { thousands(v.round() as u64) }
    else if v >= 1.0 { format!("{:.2}", v) }
    else { format!("{:.4}", v) }
}

fn r1(x: f64) -> f64 { (x * 10.0).round() / 10.0 }

fn fmt_time(t: &NaiveDateTime) -> String {
    t.format("%Y-%m-%d %H:%M UTC").to_string()
}

impl TxContext {
    pub fn llm_view(&self, flags: &[Flag], rule: &RuleScore) -> Value {
        json!({
            "chain": self.chain,
            "token": self.symbol,
            "amount": if self.amount >= 100.0 { self.amount.round() } else { (self.amount * 100.0).round() / 100.0 },
            "value_usd": self.value_usd.round(),
            "time_utc": self.time,
            "from": {
                "address": short_addr(&self.from_address),
                "label": self.from_label,
                "avg_tx_usd_30d": self.from_avg_usd_30d.map(|v| v.round()),
                "tx_count_30d": self.from_tx_count_30d,
                "this_tx_multiple_of_avg": self.multiple_of_from_avg.map(r1),
            },
            "to": {
                "address": short_addr(&self.to_address),
                "label": self.to_label,
            },
            "token_avg_tx_usd_30d": self.token_avg_usd_30d.map(|v| v.round()),
            "this_tx_multiple_of_token_avg": self.multiple_of_token_avg.map(r1),
            "transfers_from_sender_in_last_hour": self.count_last_hour,
            "related_recent_from_sender_24h": self.related_recent,
            "related_total_usd_incl_this": self.related_total_usd.round(),
            "rule_flags": flags,
            "rule_importance": { "score": r1(rule.score as f64), "tier": rule.tier, "factors": rule.factors },
        })
    }
}

impl AiState {
    pub async fn build_context(&self, hash: &str) -> Result<Option<TxContext>> {
        let pool = self.pool();

        let row: Option<TxRow> = sqlx::query_as(
            "SELECT hash, chain, token_symbol, value_usd::float8 AS value_usd,
                    from_address, to_address, value, tx_timestamp
             FROM transactions WHERE hash = $1 LIMIT 1",
        )
        .bind(hash)
        .fetch_optional(pool)
        .await?;
        let Some(r) = row else { return Ok(None) };

        let value_usd = r.value_usd.unwrap_or(0.0);
        let decimals = match r.chain.as_str() { "ethereum" => 18, "solana" => 9, "bitcoin" => 8, _ => 0 };
        let amount = r.value.parse::<f64>().unwrap_or(0.0) / 10f64.powi(decimals);

        let (from_avg, from_n): (Option<f64>, i64) = sqlx::query_as(
            "SELECT AVG(value_usd::float8), COUNT(*) FROM transactions
             WHERE chain = $1 AND from_address = $2 AND hash <> $4
               AND tx_timestamp < $3::timestamp
               AND tx_timestamp > $3::timestamp - interval '30 days'",
        )
        .bind(&r.chain).bind(&r.from_address).bind(r.tx_timestamp).bind(&r.hash)
        .fetch_one(pool).await?;

        let (prior_all,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM transactions
             WHERE chain = $1 AND from_address = $2 AND hash <> $4
               AND tx_timestamp < $3::timestamp",
        )
        .bind(&r.chain).bind(&r.from_address).bind(r.tx_timestamp).bind(&r.hash)
        .fetch_one(pool).await?;

        let (tok_avg, _tok_n): (Option<f64>, i64) = sqlx::query_as(
            "SELECT AVG(value_usd::float8), COUNT(*) FROM transactions
             WHERE chain = $1 AND token_symbol = $2 AND hash <> $4
               AND tx_timestamp < $3::timestamp
               AND tx_timestamp > $3::timestamp - interval '30 days'",
        )
        .bind(&r.chain).bind(&r.token_symbol).bind(r.tx_timestamp).bind(&r.hash)
        .fetch_one(pool).await?;

        let (last_hour,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM transactions
             WHERE chain = $1 AND from_address = $2
               AND tx_timestamp <= $3::timestamp
               AND tx_timestamp > $3::timestamp - interval '1 hour'",
        )
        .bind(&r.chain).bind(&r.from_address).bind(r.tx_timestamp)
        .fetch_one(pool).await?;

        let rel_rows: Vec<(String, f64, String, NaiveDateTime)> = sqlx::query_as(
            "SELECT hash, COALESCE(value_usd, 0)::float8, to_address, tx_timestamp
             FROM transactions
             WHERE chain = $1 AND from_address = $2 AND hash <> $4
               AND tx_timestamp <= $3::timestamp
               AND tx_timestamp > $3::timestamp - interval '24 hours'
             ORDER BY tx_timestamp DESC LIMIT 5",
        )
        .bind(&r.chain).bind(&r.from_address).bind(r.tx_timestamp).bind(&r.hash)
        .fetch_all(pool).await?;

        let related_recent: Vec<RelatedTx> = rel_rows
            .into_iter()
            .map(|(h, v, to, t)| RelatedTx {
                hash: h,
                value_usd: v,
                to_short: short_addr(&to),
                time: fmt_time(&t),
            })
            .collect();
        let related_total_usd = value_usd + related_recent.iter().map(|x| x.value_usd).sum::<f64>();

        let from_label = self.label_for(&r.chain, &r.from_address).await;
        let to_label = self.label_for(&r.chain, &r.to_address).await;

        let multiple = |avg: Option<f64>| avg.filter(|a| *a > 0.0).map(|a| value_usd / a);

        Ok(Some(TxContext {
            explorer_url: explorer_tx_url(&r.chain, &r.hash),
            hash: r.hash,
            symbol: r.token_symbol,
            amount,
            value_usd,
            time: fmt_time(&r.tx_timestamp),
            hour_utc: r.tx_timestamp.hour(),
            multiple_of_from_avg: if from_n > 0 { multiple(from_avg) } else { None },
            from_avg_usd_30d: if from_n > 0 { from_avg } else { None },
            from_tx_count_30d: from_n,
            multiple_of_token_avg: multiple(tok_avg),
            token_avg_usd_30d: tok_avg,
            prior_tx_count_all: prior_all,
            count_last_hour: last_hour,
            related_recent,
            related_total_usd,
            from_label,
            to_label,
            from_address: r.from_address,
            to_address: r.to_address,
            chain: r.chain,
        }))
    }

    async fn label_for(&self, chain: &str, addr: &str) -> Option<String> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT label FROM wallet_labels WHERE chain = $1 AND address = $2",
        )
        .bind(chain)
        .bind(addr)
        .fetch_optional(self.pool())
        .await
        .ok()
        .flatten();
        row.map(|(l,)| super::safety::sanitize_label(&l))
    }
}