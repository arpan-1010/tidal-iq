mod db;
mod routes;
mod services;
mod ai;

use crate::db::repository::TransactionRepo;
use crate::services::bitcoin::BitcoinService;
use crate::services::ethereum::EthereumService;
use crate::services::price::PriceService;
use crate::services::solana::SolanaService;
use axum::{http::Method, response::Json, routing::{get, post}, Router};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

fn env_f64(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let alchemy_key = std::env::var("ALCHEMY_API_KEY").expect("ALCHEMY_API_KEY must be set");
    let default_threshold = env_f64("THRESHOLD_USD", 500_000.0);
    let eth_threshold = env_f64("ETH_THRESHOLD_USD", default_threshold);
    let sol_threshold = env_f64("SOL_THRESHOLD_USD", default_threshold);
    let btc_threshold = env_f64("BTC_THRESHOLD_USD", default_threshold);
    let port = std::env::var("PORT").unwrap_or_else(|_| "4000".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;
    println!("Connected to DB");

    let repo = Arc::new(TransactionRepo::new(pool));

    {
        let repo = Arc::clone(&repo);
        tokio::spawn(async move {
            loop {
                if let Err(e) = repo.score_unscored().await { eprintln!("score error: {}", e); }
                if let Err(e) = repo.flag_anomalies().await { eprintln!("anomaly error: {}", e); }
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
        });
    }

    let ai_client = match ai::AiConfig::from_env() {
        Some(cfg) => {
            let client = ai::nvidia::NvidiaClient::new(cfg, repo.pool().clone());
            let bg = Arc::clone(&client);
            tokio::spawn(async move { bg.start().await });
            Some(client)
        }
        None => {
            println!("AI layer disabled (set NVIDIA_API_KEY to enable)");
            None
        }
    };

    let ai_state = Arc::new(ai::AiState::new(Arc::clone(&repo), ai_client));
    if ai_state.client.is_some() {
        let s = Arc::clone(&ai_state);
        tokio::spawn(async move { s.run_worker().await });
    }

    if ai_state.client.is_some() {
        let s = Arc::clone(&ai_state);
        tokio::spawn(async move { s.run_embedder().await });
    }

    if ai_state.client.is_some() {
        let s = Arc::clone(&ai_state);
        tokio::spawn(ai_state.clone().run_digest_worker());
    }

    let price_svc = PriceService::new(&alchemy_key);

    let ethereum = Arc::new(EthereumService::new(
        Arc::clone(&repo),
        price_svc.clone(),
        eth_threshold,
        &alchemy_key,
    ));
    tokio::spawn(async move { ethereum.start().await });

    let solana = Arc::new(SolanaService::new(
        Arc::clone(&repo),
        price_svc.clone(),
        sol_threshold,
        &alchemy_key,
    ));
    tokio::spawn(async move { solana.start().await });

    let bitcoin = Arc::new(BitcoinService::new(
        Arc::clone(&repo),
        price_svc.clone(),
        btc_threshold,
        &alchemy_key,
    ));
    tokio::spawn(async move { bitcoin.start().await });

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_origin(Any);

    let tx_routes = Router::new()
        .route("/transactions", get(routes::transactions::list_transactions))
        .route("/transactions/{hash}", get(routes::transactions::get_transaction))
        .route("/stats", get(routes::transactions::get_stats))
        .route("/volume", get(routes::transactions::get_volume))
        .route("/insights", get(routes::transactions::get_insights)) 
        .with_state(Arc::clone(&repo));

    let price_routes = Router::new()
        .route("/price-history", get(routes::prices::price_history))
        .with_state(price_svc);

    let ai_routes = Router::new()
        .route("/ai/summarize-transaction", post(routes::ai::summarize_transaction))
        .route("/ai/transactions/{hash}", get(routes::ai::get_analysis))
        .route("/ai/models", get(routes::ai::models))
        .route("/ai/status", get(routes::ai::status))
        .route("/ai/probe", get(routes::ai::probe))
        .route("/ai/probe-embed", get(routes::ai::probe_embed))
        .route("/ai/search", get(routes::ai::search_get).post(routes::ai::search_post))
        .route("/ai/chat", post(routes::ai::chat_post))
        .route("/ai/chat/stream", post(routes::ai::chat_stream))
        .route("/ai/wallets/{chain}/{address}", get(routes::ai::wallet_profile))
        .route("/ai/feedback", post(routes::ai::feedback))
        .route("/ai/narrative", get(routes::ai::narrative_get))
        .route("/ai/digest", get(routes::ai::digest_get))
        .with_state(Arc::clone(&ai_state));

    let app = Router::new()
        .route("/health", get(|| async { Json(json!({"status": "ok"})) }))
        .route("/", get(|| async { Json(json!({"message": "TidalIq API"})) }))
        .merge(tx_routes)
        .merge(price_routes)
        .merge(ai_routes)
        .layer(cors);

    let addr = format!("0.0.0.0:{}", port);
    println!("API running on http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}