use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Transaction {
    pub id: String,
    pub hash: String,
    pub chain: String,
    pub from_address: String,
    pub to_address: String,
    pub value: String,
    pub value_usd: Option<sqlx::types::Decimal>,
    pub token: String,
    pub token_symbol: String,
    pub block_number: i32,
    pub tx_timestamp: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub ai_summary: Option<String>,
    pub impact_score: Option<f32>,
    pub is_anomaly: Option<bool>,
    pub anomaly_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewTransaction {
    pub hash: String,
    pub chain: String,
    pub from_address: String,
    pub to_address: String,
    pub value: String,
    pub value_usd: f64,
    pub token: String,
    pub token_symbol: String,
    pub block_number: i32,
    pub tx_timestamp: NaiveDateTime,
}