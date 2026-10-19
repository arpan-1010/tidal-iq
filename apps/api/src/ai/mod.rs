pub mod analyze;
pub mod budget;
pub mod context;
pub mod nvidia;
pub mod prompts;
pub mod safety;
pub mod scoring;
pub mod embed;
pub mod ratelimit;
pub mod search;
pub mod chat;
pub mod profile;
pub mod narrative;

use crate::db::repository::TransactionRepo;
use nvidia::NvidiaClient;
use sqlx::PgPool;
use std::sync::Arc;

pub const DISCLAIMER: &str = "AI-generated insights, not financial advice.";

pub const EMBED_DIM: usize = 2048;

use std::env;

fn env_bool(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(default)
}

fn env_list(name: &str) -> Vec<String> {
    env::var(name)
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn env_num<T: std::str::FromStr>(name: &str, default: T) -> T {
    env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

#[derive(Clone, Debug)]
pub struct AiConfig {
    pub enabled: bool,
    pub api_key: String,
    pub base_url: String,
    pub fast_models: Vec<String>,
    pub strong_models: Vec<String>,
    pub embed_model: String,
    pub rpm: u32,
    pub daily_token_budget: u64,
    pub cost_per_1k_tokens: f64,
    pub min_impact: f32,
    pub feature_search: bool,
    pub feature_chat: bool,
    pub feature_profiles: bool,
    pub feature_narratives: bool,
    pub no_think_models: Vec<String>,
    pub embed_dimensions: Option<u32>,
    pub embed_truncate: bool,
}

impl AiConfig {
    pub fn from_env() -> Option<Self> {
        if !env_bool("AI_ENABLED", true) {
            return None;
        }
        let api_key = env::var("NVIDIA_API_KEY").ok().filter(|k| !k.trim().is_empty())?;
        Some(Self {
            enabled: true,
            api_key,
            base_url: env::var("NVIDIA_BASE_URL")
                .unwrap_or_else(|_| "https://integrate.api.nvidia.com/v1".into())
                .trim_end_matches('/')
                .to_string(),
            fast_models: env_list("AI_FAST_MODELS"),
            strong_models: env_list("AI_STRONG_MODELS"),
            embed_model: env::var("AI_EMBED_MODEL")
                .unwrap_or_else(|_| "nvidia/nv-embedqa-e5-v5".into()),
            rpm: env_num("AI_RPM", 30u32).clamp(1, 40),
            daily_token_budget: env_num("AI_DAILY_TOKEN_BUDGET", 400_000u64),
            cost_per_1k_tokens: env_num("AI_COST_PER_1K_TOKENS", 0.0f64),
            min_impact: env_num("AI_MIN_IMPACT", 40.0f32),
            feature_search: env_bool("AI_FEATURE_SEARCH", true),
            feature_chat: env_bool("AI_FEATURE_CHAT", true),
            feature_profiles: env_bool("AI_FEATURE_PROFILES", true),
            feature_narratives: env_bool("AI_FEATURE_NARRATIVES", true),
            no_think_models: env_list("AI_NO_THINK_MODELS"),
            embed_dimensions: env::var("AI_EMBED_DIMENSIONS").ok().and_then(|v| v.parse().ok()),
            embed_truncate: env_bool("AI_EMBED_TRUNCATE", false),
        })
    }
}

pub struct AiState {
    pub repo: Arc<TransactionRepo>,
    pub client: Option<Arc<NvidiaClient>>,
    pub limiter: ratelimit::ClientLimiter,
}

impl AiState {
    pub fn new(repo: Arc<TransactionRepo>, client: Option<Arc<NvidiaClient>>) -> Self {
        Self { repo, client, limiter: ratelimit::ClientLimiter::new(20) } // 20 AI requests/min per client
    }

    pub fn pool(&self) -> &PgPool {
        self.repo.pool()
    }

    pub fn min_impact(&self) -> f32 {
        self.client.as_ref().map(|c| c.cfg.min_impact).unwrap_or(40.0)
    }
}