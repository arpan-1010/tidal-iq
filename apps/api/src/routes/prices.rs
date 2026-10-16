use crate::services::price::PriceService;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub symbol: Option<String>,
    pub hours: Option<i64>,
    pub bucket: Option<String>,
}

pub async fn price_history(
    State(price): State<PriceService>,
    Query(params): Query<HistoryQuery>,
) -> Result<Json<Value>, StatusCode> {
    let symbol = params.symbol.unwrap_or_else(|| "BTC".to_string()).to_uppercase();
    if !matches!(symbol.as_str(), "BTC" | "ETH" | "SOL") {
        return Err(StatusCode::BAD_REQUEST);
    }

    let interval = match params.bucket.as_deref() {
        Some("day") => "1d",
        _ => "1h",
    };
    let hours = params.hours.unwrap_or(168).clamp(1, 24 * 365);

    price
        .get_history(&symbol, hours, interval)
        .await
        .map(|points| Json(json!({ "data": points })))
        .map_err(|e| {
            eprintln!("Price history error: {}", e);
            StatusCode::BAD_GATEWAY
        })
}