use crate::db::repository::TransactionRepo;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct ListQuery {
    pub chain: Option<String>,
    pub limit: Option<i64>,
}

pub async fn list_transactions(
    State(repo): State<Arc<TransactionRepo>>,
    Query(params): Query<ListQuery>,
) -> Result<Json<Value>, StatusCode> {
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let chain = params.chain.as_deref();

    repo.get_recent(limit, chain)
        .await
        .map(|txs| {
            let count = txs.len();
            Json(json!({"data": txs, "count": count}))
        })
        .map_err(|e| {
            eprintln!("DB error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn get_transaction(
    State(repo): State<Arc<TransactionRepo>>,
    Path(hash): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    repo.get_by_hash(&hash)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(|tx| Json(json!({"data": tx})))
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn get_stats(
    State(repo): State<Arc<TransactionRepo>>,
) -> Result<Json<Value>, StatusCode> {
    repo.get_chain_stats()
        .await
        .map(|stats| Json(json!({ "data": stats })))
        .map_err(|e| {
            eprintln!("DB error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

#[derive(Deserialize)]
pub struct VolumeQuery {
    pub bucket: Option<String>,
    pub hours: Option<i32>,
}

pub async fn get_volume(
    State(repo): State<Arc<TransactionRepo>>,
    Query(params): Query<VolumeQuery>,
) -> Result<Json<Value>, StatusCode> {
    let bucket = match params.bucket.as_deref() {
        Some("day") => "day",
        _ => "hour",
    };
    let hours = params.hours.unwrap_or(168).clamp(1, 2160);

    repo.get_volume_series(bucket, hours)
        .await
        .map(|rows| Json(json!({ "data": rows })))
        .map_err(|e| {
            eprintln!("Volume query error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

#[derive(serde::Serialize)]
pub struct InsightsResponse {
    top: Vec<crate::db::repository::InsightRow>,
    anomalies: Vec<crate::db::repository::InsightRow>,
}

pub async fn get_insights(
    State(repo): State<Arc<TransactionRepo>>,
) -> Result<Json<InsightsResponse>, axum::http::StatusCode> {
    let top = repo.get_insights(false, 5).await.map_err(|e| {
        eprintln!("insights error: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let anomalies = repo.get_insights(true, 5).await.map_err(|e| {
        eprintln!("insights error: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(InsightsResponse { top, anomalies }))
}