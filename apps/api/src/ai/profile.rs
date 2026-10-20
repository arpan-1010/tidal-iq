use super::context::{fmt_usd, short_addr};
use super::nvidia::{GenOpts, Tier};
use super::safety::{numbers_in, validate_chat_text};
use super::{prompts, AiState, DISCLAIMER};
use anyhow::Result;
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Serialize, Clone, Debug)]
pub struct WalletProfile {
    pub chain: String,
    pub address: String,
    pub label: Option<String>,
    pub profile: String,
    pub stats: Value,
    pub source: String,
    pub model: Option<String>,
    pub updated_at: Option<String>,
    pub disclaimer: &'static str,
}

fn template_profile(label: &Option<String>, addr: &str, stats: &Value) -> String {
    let who = label.clone().unwrap_or_else(|| short_addr(addr));
    let sent = stats["sent_count"].as_i64().unwrap_or(0);
    let recv = stats["received_count"].as_i64().unwrap_or(0);
    let mut s = format!(
        "In TidalIQ's tracked data, {who} sent {sent} large transfer{} ",
        if sent == 1 { "" } else { "s" }
    );
    s.push_str(&format!(
        "totalling {}, and received {recv}.",
        fmt_usd(stats["sent_total_usd"].as_f64().unwrap_or(0.0))
    ));
    s
}

impl AiState {
    pub async fn wallet_profile(&self, chain: &str, address: &str) -> Result<Option<WalletProfile>> {
        let pool = self.pool();

        let cached: Option<(String, Value, Option<String>, String)> = sqlx::query_as(
            "SELECT profile, stats, model, updated_at::text FROM wallet_profiles
             WHERE chain = $1 AND address = $2 AND updated_at > now() - interval '6 hours'",
        )
        .bind(chain)
        .bind(address)
        .fetch_optional(pool)
        .await?;

        let label: Option<(String, Option<String>)> =
            sqlx::query_as("SELECT label, category FROM wallet_labels WHERE chain = $1 AND address = $2")
                .bind(chain)
                .bind(address)
                .fetch_optional(pool)
                .await?;
        let label_text = label.as_ref().map(|(l, _)| l.clone());

        if let Some((profile, stats, model, updated)) = cached {
            return Ok(Some(WalletProfile {
                chain: chain.into(),
                address: address.into(),
                label: label_text,
                profile,
                stats,
                source: "cache".into(),
                model,
                updated_at: Some(updated),
                disclaimer: DISCLAIMER,
            }));
        }
        
        let sent: (i64, f64, f64, f64, i64, Option<String>, Option<String>) = sqlx::query_as(
            "SELECT COUNT(*),
                    COALESCE(SUM(value_usd), 0)::float8,
                    COALESCE(AVG(value_usd), 0)::float8,
                    COALESCE(MAX(value_usd), 0)::float8,
                    COUNT(*) FILTER (WHERE tx_timestamp > (now() at time zone 'utc') - interval '30 days'),
                    MIN(tx_timestamp)::text,
                    MAX(tx_timestamp)::text
             FROM transactions WHERE chain = $1 AND from_address = $2",
        )
        .bind(chain)
        .bind(address)
        .fetch_one(pool)
        .await?;

        let recv: (i64, f64) = sqlx::query_as(
            "SELECT COUNT(*), COALESCE(SUM(value_usd), 0)::float8
             FROM transactions WHERE chain = $1 AND to_address = $2",
        )
        .bind(chain)
        .bind(address)
        .fetch_one(pool)
        .await?;

        if sent.0 == 0 && recv.0 == 0 {
            return Ok(None);
        }

        let tokens: Vec<(String, i64, f64)> = sqlx::query_as(
            "SELECT COALESCE(token_symbol, '?'), COUNT(*), COALESCE(SUM(value_usd), 0)::float8
             FROM transactions WHERE chain = $1 AND from_address = $2
             GROUP BY 1 ORDER BY 3 DESC LIMIT 3",
        )
        .bind(chain)
        .bind(address)
        .fetch_all(pool)
        .await?;

        let stats = json!({
            "sent_count": sent.0,
            "sent_total_usd": sent.1.round(),
            "sent_avg_usd": sent.2.round(),
            "sent_largest_usd": sent.3.round(),
            "sent_last_30d": sent.4,
            "first_seen": sent.5.as_deref().map(|s| s.chars().take(10).collect::<String>()),
            "last_seen": sent.6.as_deref().map(|s| s.chars().take(10).collect::<String>()),
            "received_count": recv.0,
            "received_total_usd": recv.1.round(),
            "top_tokens": tokens.iter().map(|(t, n, v)| json!({ "token": t, "count": n, "total_usd": v.round() })).collect::<Vec<_>>(),
        });

        let facts = json!({
            "chain": chain,
            "label": label_text,
            "label_category": label.as_ref().and_then(|(_, c)| c.clone()),
            "stats": stats,
        });
        let facts_str = serde_json::to_string(&facts)?;
        let allowed = numbers_in(&facts_str);

        let mut source = "template";
        let mut model: Option<String> = None;
        let mut text = template_profile(&label_text, address, &stats);

        if let Some(c) = &self.client {
            let opts = GenOpts::new(Tier::Fast, "profile", prompts::PROFILE_V)
                .max_tokens(220)
                .temperature(0.2);
            let user = format!("<facts>{facts_str}</facts>\nWrite the profile now.");
            match c.generate_text(prompts::PROFILE_SYSTEM, &user, &opts).await {
                Ok(comp) => match validate_chat_text(&comp.text, &allowed, 0, false, 600) {
                    Ok(()) => {
                        text = comp.text.trim().to_string();
                        source = "llm";
                        model = Some(comp.model);
                    }
                    Err(why) => eprintln!("wallet profile rejected: {why}"),
                },
                Err(e) => eprintln!("wallet profile generation failed: {e}"),
            }
        }

        let mut updated: Option<String> = None;
        if source == "llm" {
            let row: Option<(String,)> = sqlx::query_as(
                "INSERT INTO wallet_profiles (chain, address, profile, stats, model, prompt_version, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, now())
                 ON CONFLICT (chain, address) DO UPDATE
                 SET profile = EXCLUDED.profile, stats = EXCLUDED.stats, model = EXCLUDED.model,
                     prompt_version = EXCLUDED.prompt_version, updated_at = now()
                 RETURNING updated_at::text",
            )
            .bind(chain)
            .bind(address)
            .bind(&text)
            .bind(&stats)
            .bind(&model)
            .bind(prompts::PROFILE_V)
            .fetch_optional(pool)
            .await?;
            updated = row.map(|r| r.0);
        }

        Ok(Some(WalletProfile {
            chain: chain.into(),
            address: address.into(),
            label: label_text,
            profile: text,
            stats,
            source: source.into(),
            model,
            updated_at: updated,
            disclaimer: DISCLAIMER,
        }))
    }

    pub async fn save_feedback(&self, kind: &str, reference: &str, helpful: bool, comment: Option<&str>) -> Result<()> {
        sqlx::query("INSERT INTO ai_feedback (kind, ref, helpful, comment) VALUES ($1, $2, $3, $4)")
            .bind(kind)
            .bind(reference)
            .bind(helpful)
            .bind(comment)
            .execute(self.pool())
            .await?;
        Ok(())
    }
}