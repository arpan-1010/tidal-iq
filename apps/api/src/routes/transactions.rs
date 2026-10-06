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
    let limit = params.limit.unwrap_or(50).min(200);
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