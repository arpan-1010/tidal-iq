use super::context::{fmt_usd, TxContext};
use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct Flag {
    pub code: &'static str,
    pub detail: String,
}

#[derive(Clone, Debug)]
pub struct RuleScore {
    pub score: f32,
    pub tier: &'static str,
    pub factors: Vec<String>,
}

pub fn tier_for(score: f32) -> &'static str {
    if score < 25.0 { "Low" }
    else if score < 50.0 { "Medium" }
    else if score < 75.0 { "High" }
    else { "Critical" }
}

pub fn detect_flags(c: &TxContext) -> Vec<Flag> {
    let mut flags = Vec::new();

    if let (Some(m), Some(avg)) = (c.multiple_of_from_avg, c.from_avg_usd_30d) {
        if c.from_tx_count_30d >= 3 && m >= 5.0 {
            flags.push(Flag {
                code: "UNUSUAL_SIZE",
                detail: format!(
                    "{:.1}x this wallet's 30-day average transfer ({})", m, fmt_usd(avg)
                ),
            });
        }
    }

    if c.prior_tx_count_all == 0 && c.count_last_hour <= 1 {
        flags.push(Flag {
            code: "FIRST_LARGE_MOVE",
            detail: "First large transfer recorded from this address".to_string(),
        });
    }

    if c.count_last_hour >= 3 {
        flags.push(Flag {
            code: "RAPID_SUCCESSION",
            detail: format!(
                "{} large transfers from this address within one hour", c.count_last_hour
            ),
        });
    }

    if let Some(m) = c.multiple_of_token_avg {
        if m >= 10.0 {
            flags.push(Flag {
                code: "TOKEN_OUTLIER",
                detail: format!("{:.1}x the 30-day average {} transfer", m, c.symbol),
            });
        }
    }

    flags
}

pub fn rule_score(c: &TxContext, flags: &[Flag]) -> RuleScore {
    let mut factors = Vec::new();

    let size_norm = if c.value_usd > 0.0 {
        ((c.value_usd.ln() - 1e5f64.ln()) / (1e8f64.ln() - 1e5f64.ln())).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut score = (size_norm * 60.0) as f32;
    factors.push(format!("size {} -> {:.0} pts", fmt_usd(c.value_usd), score));

    if let Some(m) = c.multiple_of_from_avg {
        let pts = if m >= 5.0 { 15.0 } else if m >= 2.0 { 7.0 } else { 0.0 };
        if pts > 0.0 {
            score += pts;
            factors.push(format!("{:.1}x sender average -> +{:.0}", m, pts));
        }
    }
    if let Some(m) = c.multiple_of_token_avg {
        let pts = if m >= 5.0 { 10.0 } else if m >= 2.0 { 4.0 } else { 0.0 };
        if pts > 0.0 {
            score += pts;
            factors.push(format!("{:.1}x token average -> +{:.0}", m, pts));
        }
    }
    if c.from_label.is_some() || c.to_label.is_some() {
        score += 8.0;
        factors.push("known labelled wallet involved -> +8".to_string());
    }
    if c.hour_utc < 6 {
        score += 3.0;
        factors.push("off-hours (00-06 UTC) -> +3".to_string());
    }
    if !flags.is_empty() {
        let pts = (flags.len() as f32 * 5.0).min(15.0);
        score += pts;
        factors.push(format!("{} anomaly flag(s) -> +{:.0}", flags.len(), pts));
    }

    let score = score.clamp(0.0, 100.0);
    RuleScore { score, tier: tier_for(score), factors }
}