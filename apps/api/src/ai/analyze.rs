use super::context::{fmt_amount, fmt_usd, short_addr, TxContext};
use super::nvidia::{AiError, GenOpts, Tier};
use super::scoring::{detect_flags, rule_score, tier_for, Flag, RuleScore};
use super::{prompts, safety, AiState, DISCLAIMER};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Deserialize)]
struct LlmAnalysis {
    summary: String,
    #[serde(default)]
    importance_adjustment: i32,
    #[serde(default)]
    importance_reason: Option<String>,
    #[serde(default)]
    anomaly_note: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Analysis {
    pub tx_hash: String,
    pub summary: String,
    pub importance_score: f32,
    pub importance_tier: String,
    pub importance_reason: Option<String>,
    pub anomaly_flags: Value,
    pub anomaly_note: Option<String>,
    pub source: String,
    pub model: Option<String>,
    pub prompt_version: Option<String>,
    pub explorer_url: String,
    pub disclaimer: &'static str,
}

pub enum Outcome {
    NotFound,
    Stored(Analysis),
    Fallback(Analysis),
}

#[derive(sqlx::FromRow)]
struct AnalysisRow {
    tx_hash: String,
    summary: String,
    importance_score: f32,
    importance_tier: String,
    importance_reason: Option<String>,
    anomaly_flags: Value,
    anomaly_note: Option<String>,
    source: String,
    model: Option<String>,
    prompt_version: Option<String>,
    explorer_url: Option<String>,
}

fn clean_opt(s: Option<String>) -> Option<String> {
    s.map(|x| x.trim().to_string()).filter(|x| {
        !x.is_empty() && !x.eq_ignore_ascii_case("null") && !x.eq_ignore_ascii_case("none")
    })
}

fn template_summary(c: &TxContext, flags: &[Flag]) -> String {
    let from = c.from_label.clone().unwrap_or_else(|| short_addr(&c.from_address));
    let to = c.to_label.clone().unwrap_or_else(|| short_addr(&c.to_address));
    let mut s = format!(
        "{} {} ({}) moved from {} to {} on {}.",
        fmt_amount(c.amount),
        c.symbol,
        fmt_usd(c.value_usd),
        from,
        to,
        c.chain
    );
    if let Some(f) = flags.first() {
        s.push(' ');
        s.push_str(&f.detail);
        s.push('.');
    }
    s
}

fn short_hash(h: &str) -> &str {
    &h[..10.min(h.len())]
}

impl AiState {
    fn template_analysis(&self, c: &TxContext, flags: &[Flag], rule: &RuleScore) -> Analysis {
        Analysis {
            tx_hash: c.hash.clone(),
            summary: template_summary(c, flags),
            importance_score: rule.score,
            importance_tier: rule.tier.to_string(),
            importance_reason: None,
            anomaly_flags: json!(flags),
            anomaly_note: None,
            source: "template".into(),
            model: None,
            prompt_version: None,
            explorer_url: c.explorer_url.clone(),
            disclaimer: DISCLAIMER,
        }
    }

    pub async fn get_analysis(&self, hash: &str) -> Result<Option<Analysis>> {
        let row: Option<AnalysisRow> = sqlx::query_as(
            "SELECT tx_hash, summary, importance_score, importance_tier, importance_reason,
                    anomaly_flags, anomaly_note, source, model, prompt_version,
                    context->>'explorer_url' AS explorer_url
             FROM ai_analyses WHERE tx_hash = $1",
        )
        .bind(hash)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|r| Analysis {
            tx_hash: r.tx_hash,
            summary: r.summary,
            importance_score: r.importance_score,
            importance_tier: r.importance_tier,
            importance_reason: r.importance_reason,
            anomaly_flags: r.anomaly_flags,
            anomaly_note: r.anomaly_note,
            source: r.source,
            model: r.model,
            prompt_version: r.prompt_version,
            explorer_url: r.explorer_url.unwrap_or_default(),
            disclaimer: DISCLAIMER,
        }))
    }

    async fn store_analysis(&self, a: &Analysis, context: &Value) -> Result<()> {
        sqlx::query(
            "INSERT INTO ai_analyses
               (tx_hash, summary, importance_score, importance_tier, importance_reason,
                anomaly_flags, anomaly_note, context, source, model, prompt_version)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
             ON CONFLICT (tx_hash) DO UPDATE SET
               summary = EXCLUDED.summary,
               importance_score = EXCLUDED.importance_score,
               importance_tier = EXCLUDED.importance_tier,
               importance_reason = EXCLUDED.importance_reason,
               anomaly_flags = EXCLUDED.anomaly_flags,
               anomaly_note = EXCLUDED.anomaly_note,
               context = EXCLUDED.context,
               source = EXCLUDED.source,
               model = EXCLUDED.model,
               prompt_version = EXCLUDED.prompt_version",
        )
        .bind(&a.tx_hash)
        .bind(&a.summary)
        .bind(a.importance_score)
        .bind(&a.importance_tier)
        .bind(&a.importance_reason)
        .bind(&a.anomaly_flags)
        .bind(&a.anomaly_note)
        .bind(context)
        .bind(&a.source)
        .bind(&a.model)
        .bind(&a.prompt_version)
        .execute(self.pool())
        .await?;

        let reasons: Option<String> = a
            .anomaly_flags
            .as_array()
            .filter(|v| !v.is_empty())
            .map(|v| {
                v.iter()
                    .filter_map(|f| f["detail"].as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            });
        let has_flags = reasons.is_some();

        sqlx::query(
            "UPDATE transactions SET
               ai_summary = $1,
               impact_score = $2,
               is_anomaly = CASE WHEN $3 THEN true ELSE is_anomaly END,
               anomaly_reason = CASE
                   WHEN $4::text IS NULL THEN anomaly_reason
                   WHEN anomaly_reason IS NULL THEN $4::text
                   WHEN position($4::text in anomaly_reason) > 0 THEN anomaly_reason
                   ELSE anomaly_reason || '; ' || $4::text END
             WHERE hash = $5",
        )
        .bind(&a.summary)
        .bind(a.importance_score)
        .bind(has_flags)
        .bind(reasons)
        .bind(&a.tx_hash)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn analyze_tx(&self, hash: &str, force: bool) -> Result<Outcome> {
        if !force {
            if let Some(a) = self.get_analysis(hash).await? {
                return Ok(Outcome::Stored(a));
            }
        }
        let Some(ctx) = self.build_context(hash).await? else {
            return Ok(Outcome::NotFound);
        };

        let flags = detect_flags(&ctx);
        let rule = rule_score(&ctx, &flags);
        let view = ctx.llm_view(&flags, &rule);

        let mut audit = view.clone();
        audit["explorer_url"] = json!(ctx.explorer_url);

        let Some(client) = &self.client else {
            return Ok(Outcome::Fallback(self.template_analysis(&ctx, &flags, &rule)));
        };

        let user = format!(
            "<transaction_data>{}</transaction_data>\nReturn the JSON object now.",
            serde_json::to_string(&view)?
        );
        let opts = GenOpts::new(Tier::Fast, "analysis", prompts::ANALYSIS_V).max_tokens(450);

        match client
            .generate_object::<LlmAnalysis>(prompts::ANALYSIS_SYSTEM, &user, &opts)
            .await
        {
            Ok((llm, comp)) => {
                let reason = clean_opt(llm.importance_reason.clone());
                let note = clean_opt(llm.anomaly_note.clone());

                let checked = safety::validate_text(&llm.summary, &ctx, 300)
                    .and_then(|_| match &reason {
                        Some(r) => safety::validate_text(r, &ctx, 160),
                        None => Ok(()),
                    })
                    .and_then(|_| match &note {
                        Some(n) => safety::validate_text(n, &ctx, 160),
                        None => Ok(()),
                    });

                match checked {
                    Ok(()) => {
                        let adj = llm.importance_adjustment.clamp(-10, 10) as f32;
                        let final_score = (rule.score + adj).clamp(0.0, 100.0);
                        let a = Analysis {
                            tx_hash: ctx.hash.clone(),
                            summary: llm.summary.trim().to_string(),
                            importance_score: final_score,
                            importance_tier: tier_for(final_score).to_string(),
                            importance_reason: reason,
                            anomaly_flags: json!(flags),
                            anomaly_note: note,
                            source: "llm".into(),
                            model: Some(comp.model),
                            prompt_version: Some(prompts::ANALYSIS_V.into()),
                            explorer_url: ctx.explorer_url.clone(),
                            disclaimer: DISCLAIMER,
                        };
                        self.store_analysis(&a, &audit).await?;
                        Ok(Outcome::Stored(a))
                    }
                    Err(why) => {
                        eprintln!("AI output rejected for {}: {why}", short_hash(hash));
                        let a = self.template_analysis(&ctx, &flags, &rule);
                        self.store_analysis(&a, &audit).await?;
                        Ok(Outcome::Stored(a))
                    }
                }
            }
            Err(AiError::Invalid(m)) => {
                eprintln!("AI gave unusable JSON for {}: {m}", short_hash(hash));
                let a = self.template_analysis(&ctx, &flags, &rule);
                self.store_analysis(&a, &audit).await?;
                Ok(Outcome::Stored(a))
            }
            Err(e) => {
                eprintln!("AI unavailable: {e}");
                Ok(Outcome::Fallback(self.template_analysis(&ctx, &flags, &rule)))
            }
        }
    }

    pub async fn run_worker(self: Arc<Self>) {
        println!("AI analysis worker started (min impact {})", self.min_impact());
        loop {
            let wait = match self.run_batch().await {
                Ok(true) => 15,
                Ok(false) => 300,
                Err(e) => {
                    eprintln!("AI worker error: {e}");
                    30
                }
            };
            sleep(Duration::from_secs(wait)).await;
        }
    }

    async fn run_batch(&self) -> Result<bool> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT t.hash FROM transactions t
             LEFT JOIN ai_analyses a ON a.tx_hash = t.hash
             WHERE a.tx_hash IS NULL AND t.impact_score >= $1
             ORDER BY t.tx_timestamp DESC LIMIT 5",
        )
        .bind(self.min_impact())
        .fetch_all(self.pool())
        .await?;

        for (hash,) in rows {
            match self.analyze_tx(&hash, false).await? {
                Outcome::Fallback(_) => return Ok(false),
                //_ => println!("AI analysis saved for {}...", short_hash(&hash)),
                _ => {}
            }
        }
        Ok(true)
    }
}