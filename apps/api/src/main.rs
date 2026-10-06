mod db;
mod routes;
mod services;

use crate::db::repository::TransactionRepo;
use crate::services::blockchain::BlockchainService;
use crate::services::price::PriceService;
use axum::{
    Router,
    routing::get,
    http::Method,
    response::Json,
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let alchemy_key = std::env::var("ALCHEMY_API_KEY").expect("ALCHEMY_API_KEY must be set");
    let threshold: f64 = std::env::var("THRESHOLD_USD").unwrap_or_else(|_| "500000".to_string())
        .parse()
        .unwrap_or(500_000.0);

    let port = std::env::var("PORT").unwrap_or_else(|_| "4000".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;

    println!("Connected to DB");

    let repo = Arc::new(TransactionRepo::new(pool));
    let price_svc = PriceService::new();

    let blockchain = Arc::new(BlockchainService::new(
        Arc::clone(&repo),
        price_svc,
        threshold,
        &alchemy_key,
    ));
    tokio::spawn(async move {blockchain.start().await});

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_origin(Any);

    let app = Router::new()
        .route("/health", get(|| async {Json(json!({"status": "ok"}))}))
        .route("/", get(|| async {Json(json!({"message": "Whale Alert API"}))}))
        .route("/transactions/{hash}", get(routes::transactions::list_transactions))
        .layer(cors)
        .with_state(repo);

    let addr = format!("0.0.0.0:{}", port);
    println!("API running on http://localhost:{}", port);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}