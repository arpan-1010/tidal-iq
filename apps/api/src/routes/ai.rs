use crate::ai::{analyze::Outcome, AiState, DISCLAIMER};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::ai::ratelimit::client_key;
use axum::http::HeaderMap;

use axum::response::sse::{Event, KeepAlive, KeepAliveStream, Sse};
use std::convert::Infallible;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use crate::ai::nvidia::{GenOpts, Tier};
use crate::ai::prompts;

#[derive(Deserialize)]
pub struct SummarizeReq {
    pub hash: String,
}

pub async fn summarize_transaction(
    State(ai): State<Arc<AiState>>,
    Json(req): Json<SummarizeReq>,
) -> Result<Json<Value>, StatusCode> {
    let hash = req.hash.trim();
    if hash.is_empty() || hash.len() > 128 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let outcome = ai.analyze_tx(hash, false).await.map_err(|e| {
        eprintln!("summarize error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match outcome {
        Outcome::NotFound => Err(StatusCode::NOT_FOUND),
        Outcome::Stored(a) => {
            let ai_generated = a.source == "llm";
            Ok(Json(json!({ "data": a, "ai_generated": ai_generated, "disclaimer": DISCLAIMER })))
        }
        Outcome::Fallback(a) => Ok(Json(json!({
            "data": a, "ai_generated": false, "degraded": true, "disclaimer": DISCLAIMER
        }))),
    }
}

pub async fn get_analysis(
    State(ai): State<Arc<AiState>>,
    Path(hash): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    match ai.get_analysis(&hash).await {
        Ok(Some(a)) => Ok(Json(json!({ "data": a }))),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            eprintln!("get_analysis error: {e}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn models(State(ai): State<Arc<AiState>>) -> Json<Value> {
    match &ai.client {
        None => Json(json!({ "enabled": false })),
        Some(c) => {
            let reg = c.registry().await;
            Json(json!({
                "enabled": true,
                "fast": reg.fast,
                "strong": reg.strong,
                "embed": reg.embed,
                "available_count": reg.all.len(),
                "available": reg.all,
                "fetched_at": reg.fetched_at,
            }))
        }
    }
}

pub async fn status(State(ai): State<Arc<AiState>>) -> Result<Json<Value>, StatusCode> {
    let Some(c) = &ai.client else {
        return Ok(Json(json!({ "enabled": false, "disclaimer": DISCLAIMER })));
    };
    let (ok, failed, avg_ms, cost): (i64, i64, f64, f64) = sqlx::query_as(
        "SELECT COUNT(*) FILTER (WHERE ok), COUNT(*) FILTER (WHERE NOT ok),
                COALESCE(AVG(latency_ms), 0)::float8,
                COALESCE(SUM(cost_usd_est), 0)::float8
         FROM ai_calls WHERE created_at >= date_trunc('day', now())",
    )
    .fetch_one(ai.pool())
    .await
    .map_err(|e| {
        eprintln!("status error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({
        "enabled": true,
        "tokens_used_today": c.budget.used(),
        "token_budget": c.budget.limit(),
        "calls_ok_today": ok,
        "calls_failed_today": failed,
        "avg_latency_ms": avg_ms.round(),
        "cost_usd_est_today": cost,
        "features": {
            "search": c.cfg.feature_search,
            "chat": c.cfg.feature_chat,
            "profiles": c.cfg.feature_profiles,
            "narratives": c.cfg.feature_narratives,
        },
        "disclaimer": DISCLAIMER,
    })))
}

#[derive(Deserialize)]
pub struct ProbeQuery {
    pub models: Option<String>,
    pub nothink: Option<bool>,
    pub max_tokens: Option<u32>,
    pub timeout: Option<u64>,
}

pub async fn probe(
    State(ai): State<Arc<AiState>>,
    Query(q): Query<ProbeQuery>,
) -> Json<Value> {
    let Some(c) = &ai.client else {
        return Json(json!({ "enabled": false }));
    };
    let list: Vec<String> = match q.models {
        Some(s) => s
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .take(12)
            .collect(),
        None => PROBE_SHORTLIST.iter().map(|s| s.to_string()).collect(),
    };
    let results = c
        .probe(
            &list,
            q.nothink.unwrap_or(false),
            q.max_tokens.unwrap_or(40).clamp(10, 2000),
            q.timeout.unwrap_or(20).clamp(5, 90),
        )
        .await;
    Json(json!({ "results": results }))
}

const PROBE_SHORTLIST: &[&str] = &[
    "google/gemma-3-12b-it",
    "google/gemma-4-31b-it",
    "mistralai/mistral-large-2-instruct",
    "nvidia/nemotron-3-super-120b-a12b",
    "deepseek-ai/deepseek-v4.1-flash",
    "moonshotai/kimi-k2.6",
    "z-ai/glm-5.3-flash",
    "nvidia/nemotron-nano-3-30b-a3b",
    "nvidia/nemotron-3.5-lightning-30b-a3b",
];

#[derive(Deserialize)]
pub struct ProbeEmbedQuery {
    pub models: Option<String>,
    pub timeout: Option<u64>,
    pub dimensions: Option<u32>,
}

const EMBED_SHORTLIST: &[&str] = &[
    "nvidia/embed-qa-4",
    "nvidia/llama-3.2-nv-embedqa-1b-v1",
    "nvidia/nemotron-3-embed-1b",
    "snowflake/arctic-embed-l",
    "nvidia/nv-embedqa-mistral-7b-v2",
];

pub async fn probe_embed(
    State(ai): State<Arc<AiState>>,
    Query(q): Query<ProbeEmbedQuery>,
) -> Json<Value> {
    let Some(c) = &ai.client else {
        return Json(json!({ "enabled": false }));
    };
    let list: Vec<String> = match q.models {
        Some(s) => s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).take(8).collect(),
        None => EMBED_SHORTLIST.iter().map(|s| s.to_string()).collect(),
    };
        let results = c
        .probe_embeds(&list, q.timeout.unwrap_or(30).clamp(5, 90), q.dimensions)
        .await;
    Json(json!({ "results": results }))
}

#[derive(Deserialize)]
pub struct SearchReq {
    pub query: String,
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct SearchGet {
    pub q: String,
    pub limit: Option<i64>,
}

async fn do_search(
    ai: &Arc<AiState>,
    headers: &HeaderMap,
    q: &str,
    limit: Option<i64>,
) -> Result<Json<Value>, StatusCode> {
    let q = q.trim();
    if q.is_empty() || q.chars().count() > 300 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !ai.limiter.allow(&client_key(headers)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
        if !ai.limiter.allow(&client_key(headers)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    if crate::ai::chat::off_topic_reply(q, true).is_some() {
        return Ok(Json(json!({
            "query": q,
            "count": 0,
            "ai_used": false,
            "semantic": false,
            "filters": {
                "anomaly_only": false,
                "chains": null,
                "flags": null,
                "max_usd": null,
                "min_usd": null,
                "period": null,
                "semantic_text": null,
                "symbols": null
            },
            "results": [],
            "available_tokens": null,
            "note": "Not found",
            "disclaimer": DISCLAIMER,
        })));
    }

    ai.search(q, limit.unwrap_or(10).clamp(1, 50))
        .await
        .map(Json)
        .map_err(|e| {
            eprintln!("search error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn search_post(
    State(ai): State<Arc<AiState>>,
    headers: HeaderMap,
    Json(req): Json<SearchReq>,
) -> Result<Json<Value>, StatusCode> {
    do_search(&ai, &headers, &req.query, req.limit).await
}

pub async fn search_get(
    State(ai): State<Arc<AiState>>,
    headers: HeaderMap,
    Query(q): Query<SearchGet>,
) -> Result<Json<Value>, StatusCode> {
    do_search(&ai, &headers, &q.q, q.limit).await
}

#[derive(Deserialize)]
pub struct ChatReq {
    pub session_id: Option<String>,
    pub message: String,
}

fn chat_gate(ai: &Arc<AiState>, headers: &HeaderMap, message: &str) -> Result<(), StatusCode> {
    let m = message.trim();
    if m.is_empty() || m.chars().count() > 500 {
        return Err(StatusCode::BAD_REQUEST);
    }
    match &ai.client {
        Some(c) if c.cfg.feature_chat => {}
        _ => return Err(StatusCode::SERVICE_UNAVAILABLE),
    }
    if !ai.limiter.allow(&client_key(headers)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    Ok(())
}

pub async fn chat_post(
    State(ai): State<Arc<AiState>>,
    headers: HeaderMap,
    Json(req): Json<ChatReq>,
) -> Result<Json<Value>, StatusCode> {
    chat_gate(&ai, &headers, &req.message)?;
    let answer = ai
        .chat(req.session_id.as_deref(), req.message.trim())
        .await
        .map_err(|e| {
            eprintln!("chat error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "data": answer, "disclaimer": DISCLAIMER })))
}

async fn emit(tx: &mpsc::Sender<Result<Event, Infallible>>, name: &str, data: Value) {
    let _ = tx.send(Ok(Event::default().event(name).data(data.to_string()))).await;
}

pub async fn chat_stream(
    State(ai): State<Arc<AiState>>,
    headers: HeaderMap,
    Json(req): Json<ChatReq>,
) -> Result<Sse<KeepAliveStream<ReceiverStream<Result<Event, Infallible>>>>, StatusCode> {
    chat_gate(&ai, &headers, &req.message)?;

    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(64);
    let message = req.message.trim().to_string();
    let session = req.session_id.clone();

    tokio::spawn(async move {
        let plan = match ai.prepare_chat(session.as_deref(), &message).await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("chat prepare failed: {e}");
                emit(&tx, "error", json!({ "message": "Something went wrong. Please try again." })).await;
                return;
            }
        };

        let sources: Vec<Value> = plan.citations.iter().map(|c| json!({
            "n": c.n, "chain": c.chain, "token": c.token, "value_usd": c.value_usd,
            "time": c.time, "explorer_url": c.explorer_url
        })).collect();
        emit(&tx, "meta", json!({ "session_id": plan.session_id.to_string(), "sources": sources })).await;

                if let Some(text) = &plan.canned {
            emit(&tx, "token", json!({ "t": text })).await;
            match ai.answer_canned(&plan, text).await {
                Ok(ans) => emit(&tx, "done", json!(ans)).await,
                Err(e) => {
                    eprintln!("chat canned answer failed: {e}");
                    emit(&tx, "error", json!({ "message": "Something went wrong. Please try again." })).await;
                }
            }
            return;
        }

        if let Some(cached) = &plan.cached {
            if let Some(t) = cached["text"].as_str() {
                emit(&tx, "token", json!({ "t": t })).await;
            }
            match ai.answer_from_cache(&plan, cached).await {
                Ok(ans) => emit(&tx, "done", json!(ans)).await,
                Err(e) => {
                    eprintln!("chat cache answer failed: {e}");
                    emit(&tx, "error", json!({ "message": "Something went wrong. Please try again." })).await;
                }
            }
            return;
        }

        let raw = match ai.client.clone() {
            Some(client) => {
                let (tok_tx, mut tok_rx) = mpsc::channel::<String>(64);
                let opts = GenOpts::new(Tier::Strong, "chat", prompts::CHAT_V)
                    .max_tokens(600)
                    .temperature(0.2);
                let system = prompts::CHAT_SYSTEM;
                let prompt = plan.user_prompt.clone();

                let gen = async {
                    client.stream_text(system, &prompt, &opts, tok_tx).await
                };
                let forward = async {
                    while let Some(piece) = tok_rx.recv().await {
                        emit(&tx, "token", json!({ "t": piece })).await;
                    }
                };
                let (res, _) = tokio::join!(gen, forward);
                match res {
                    Ok(c) => Some((c.text, c.model)),
                    Err(e) => {
                        eprintln!("chat stream failed: {e}");
                        None
                    }
                }
            }
            None => None,
        };

        match ai.finish_chat(&plan, raw).await {
            Ok(ans) => emit(&tx, "done", json!(ans)).await,
            Err(e) => {
                eprintln!("chat finish failed: {e}");
                emit(&tx, "error", json!({ "message": "Something went wrong. Please try again." })).await;
            }
        }
    });

    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default()))
}

pub async fn wallet_profile(
    State(ai): State<Arc<AiState>>,
    headers: HeaderMap,
    Path((chain, address)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    match &ai.client {
        Some(c) if c.cfg.feature_profiles => {}
        _ => return Err(StatusCode::SERVICE_UNAVAILABLE),
    }
    if address.len() > 120 || chain.len() > 20 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !ai.limiter.allow(&client_key(&headers)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    match ai.wallet_profile(&chain, &address).await {
        Ok(Some(p)) => Ok(Json(json!({ "data": p, "disclaimer": DISCLAIMER }))),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            eprintln!("wallet profile error: {e}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Deserialize)]
pub struct FeedbackReq {
    pub kind: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub helpful: bool,
    pub comment: Option<String>,
}

pub async fn feedback(
    State(ai): State<Arc<AiState>>,
    headers: HeaderMap,
    Json(req): Json<FeedbackReq>,
) -> Result<StatusCode, StatusCode> {
    if !["summary", "chat", "search", "narrative", "profile"].contains(&req.kind.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.reference.is_empty() || req.reference.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.comment.as_ref().map_or(false, |c| c.chars().count() > 500) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !ai.limiter.allow(&client_key(&headers)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    ai.save_feedback(&req.kind, &req.reference, req.helpful, req.comment.as_deref())
        .await
        .map_err(|e| {
            eprintln!("feedback error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct NarrativeQ {
    pub hours: Option<i32>,
}

async fn narrative_inner(ai: Arc<AiState>, headers: HeaderMap, hours: i32) -> Result<Json<Value>, StatusCode> {
    match &ai.client {
        Some(c) if c.cfg.feature_narratives => {}
        _ => return Err(StatusCode::SERVICE_UNAVAILABLE),
    }
    if !ai.limiter.allow(&client_key(&headers)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    match ai.narrative(hours).await {
        Ok(Some(n)) => Ok(Json(json!({ "data": n, "disclaimer": DISCLAIMER }))),
        Ok(None) => Ok(Json(json!({ "data": null, "message": "No tracked transactions in this window.", "disclaimer": DISCLAIMER }))),
        Err(e) => {
            eprintln!("narrative error: {e}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn narrative_get(
    State(ai): State<Arc<AiState>>,
    headers: HeaderMap,
    Query(q): Query<NarrativeQ>,
) -> Result<Json<Value>, StatusCode> {
    narrative_inner(ai, headers, q.hours.unwrap_or(6)).await
}

pub async fn digest_get(
    State(ai): State<Arc<AiState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    narrative_inner(ai, headers, 24).await
}