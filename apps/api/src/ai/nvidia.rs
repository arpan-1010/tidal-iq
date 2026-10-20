use super::{budget::Budget, AiConfig};
use anyhow::Result;
use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{sleep, Instant};
use std::collections::HashMap;
use std::sync::Mutex as StdMutex;
use std::time::Instant as StdInstant;
use futures_util::StreamExt;

#[derive(Debug)]
pub enum AiError {
    BudgetExceeded,
    Unavailable(String),
    Invalid(String),
}

impl fmt::Display for AiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiError::BudgetExceeded => write!(f, "daily AI token budget exceeded"),
            AiError::Unavailable(m) => write!(f, "AI unavailable: {m}"),
            AiError::Invalid(m) => write!(f, "invalid AI output: {m}"),
        }
    }
}
impl std::error::Error for AiError {}

#[derive(Clone, Copy, Debug)]
pub enum Tier {
    Fast,
    Strong,
}

#[derive(Clone, Debug)]
pub struct GenOpts {
    pub tier: Tier,
    pub feature: &'static str,
    pub prompt_version: &'static str,
    pub temperature: f32,
    pub max_tokens: u32,
}

impl GenOpts {
    pub fn new(tier: Tier, feature: &'static str, prompt_version: &'static str) -> Self {
        Self { tier, feature, prompt_version, temperature: 0.2, max_tokens: 700 }
    }
    pub fn max_tokens(mut self, n: u32) -> Self { self.max_tokens = n; self }
    pub fn temperature(mut self, t: f32) -> Self { self.temperature = t; self }
}

#[derive(Clone, Debug)]
pub struct Completion {
    pub text: String,
    pub model: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

struct Limiter {
    next: Mutex<Instant>,
    interval: Duration,
}

impl Limiter {
    fn new(rpm: u32) -> Self {
        Self {
            next: Mutex::new(Instant::now()),
            interval: Duration::from_millis(60_000 / rpm.max(1) as u64),
        }
    }

    async fn acquire(&self) {
        let wait = {
            let mut next = self.next.lock().await;
            let now = Instant::now();
            let slot = if *next > now { *next } else { now };
            *next = slot + self.interval;
            slot.saturating_duration_since(now)
        };
        if !wait.is_zero() {
            sleep(wait).await;
        }
    }

    async fn penalize(&self, d: Duration) {
        let mut next = self.next.lock().await;
        let t = Instant::now() + d;
        if t > *next {
            *next = t;
        }
    }
}

#[derive(Default, Clone, Debug, serde::Serialize)]
pub struct ModelRegistry {
    pub all: Vec<String>,
    pub fast: Vec<String>,
    pub strong: Vec<String>,
    pub embed: String,
    pub fetched_at: Option<String>,
}

const NOT_CHAT: &[&str] = &[
    "embed", "rerank", "guard", "safety", "reward", "clip", "vila", "-vl", "vision",
    "parse", "retriev", "ocr", "whisper", "tts", "asr", "diffusion", "flux",
    "stable", "translate", "nemoguard", "paligemma", "neva", "kosmos", "deplot",
    "fuyu", "riva", "audio", "image", "bge", "arctic-embed", "nv-embed",
];

fn param_billions(id: &str) -> Option<f32> {
    id.split(|c: char| c == '/' || c == '-' || c == '_')
        .filter_map(|tok| {
            let t = tok.to_lowercase();
            let rest = t.strip_suffix('b')?;
            if let Some((a, b)) = rest.split_once('x') {
                Some(a.parse::<f32>().ok()? * b.parse::<f32>().ok()?)
            } else {
                rest.parse::<f32>().ok()
            }
        })
        .next()
}

fn looks_like_chat_model(id: &str) -> bool {
    let l = id.to_lowercase();
    if NOT_CHAT.iter().any(|k| l.contains(k)) {
        return false;
    }
    l.contains("instruct") || l.contains("chat") || l.contains("nemotron") || l.contains("gpt-oss")
}

fn family_rank(id: &str) -> u8 {
    const GOOD: &[&str] = &[
        "llama", "nemotron", "mistral", "mixtral", "qwen", "gemma", "gpt-oss", "deepseek", "phi",
    ];
    let l = id.to_lowercase();
    if GOOD.iter().any(|g| l.contains(g)) { 0 } else { 1 }
}

impl ModelRegistry {
    fn build(all: Vec<String>, cfg: &AiConfig) -> Self {
        let chat: Vec<String> = all.iter().filter(|m| looks_like_chat_model(m)).cloned().collect();

                let size = |m: &String| param_billions(m).unwrap_or(0.0);
        let cmp_f = |a: f32, b: f32| a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal);

        let mut small_first = chat.clone();
        small_first.sort_by(|a, b| {
            family_rank(a).cmp(&family_rank(b)).then(cmp_f(size(a), size(b)))
        });
        let mut big_first = chat.clone();
        big_first.sort_by(|a, b| {
            family_rank(a).cmp(&family_rank(b)).then(cmp_f(size(b), size(a)))
        });

        let auto_fast: Vec<String> = small_first
            .iter()
            .filter(|m| (7.0..=40.0).contains(&size(m)))
            .take(6)
            .cloned()
            .collect();
        let auto_strong: Vec<String> = big_first
            .iter()
            .filter(|m| (30.0..=130.0).contains(&size(m)))
            .take(6)
            .cloned()
            .collect();

        let pick = |preferred: &[String], auto: Vec<String>| -> Vec<String> {
            let mut out: Vec<String> =
                preferred.iter().filter(|m| all.contains(m)).cloned().collect();
            for m in auto {
                if !out.contains(&m) {
                    out.push(m);
                }
            }
            out.truncate(8);
            out
        };

        let embed = cfg.embed_model.clone();
        if !all.is_empty() && !all.contains(&embed) {
            eprintln!("warning: embed model {embed} is not in NVIDIA's model list");
        }

        Self {
            fast: pick(&cfg.fast_models, auto_fast),
            strong: pick(&cfg.strong_models, auto_strong),
            all,
            embed,
            fetched_at: Some(chrono::Utc::now().to_rfc3339()),
        }
    }
}

pub struct NvidiaClient {
    http: Client,
    pub cfg: AiConfig,
    limiter: Limiter,
    registry: RwLock<ModelRegistry>,
    pub budget: Arc<Budget>,
    pool: PgPool,
    dead: StdMutex<HashMap<String, StdInstant>>,
}

fn strip_think(s: &str) -> String {
    let mut out = s.to_string();
    while let (Some(a), Some(b)) = (out.find("<think>"), out.find("</think>")) {
        if b > a {
            out.replace_range(a..b + "</think>".len(), "");
        } else {
            break;
        }
    }
    out.trim().to_string()
}

pub fn extract_json_object(s: &str) -> Option<&str> {
    let start = s.find('{')?;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (i, c) in s[start..].char_indices() {
        if in_str {
            if esc { esc = false; }
            else if c == '\\' { esc = true; }
            else if c == '"' { in_str = false; }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[start..start + i + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

impl NvidiaClient {
    pub fn new(cfg: AiConfig, pool: PgPool) -> Arc<Self> {
        let budget = Arc::new(Budget::new(cfg.daily_token_budget));
        Arc::new(Self {
            http: Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .expect("failed to build HTTP client"),
            limiter: Limiter::new(cfg.rpm),
            registry: RwLock::new(ModelRegistry::default()),
            budget,
            pool,
            dead: StdMutex::new(HashMap::new()),
            cfg,
        })
    }

    pub async fn start(self: Arc<Self>) {
        if let Ok((tokens,)) = sqlx::query_as::<_, (i64,)>(
            "SELECT COALESCE(SUM(prompt_tokens + completion_tokens), 0)::bigint
             FROM ai_calls WHERE created_at >= date_trunc('day', now())",
        )
        .fetch_one(&self.pool)
        .await
        {
            self.budget.seed(tokens.max(0) as u64);
        }

        loop {
            if let Err(e) = self.refresh_models().await {
                eprintln!("NVIDIA model refresh failed: {e}");
            }
            sleep(Duration::from_secs(6 * 3600)).await;
        }
    }

    pub async fn refresh_models(&self) -> Result<()> {
        let url = format!("{}/models", self.cfg.base_url);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&self.cfg.api_key)
            .send()
            .await
            .map_err(|e| e.without_url())?;
        if !resp.status().is_success() {
            anyhow::bail!("GET /models returned {}", resp.status());
        }
        let v: Value = resp.json().await.map_err(|e| e.without_url())?;
        let all: Vec<String> = v["data"]
            .as_array()
            .map(|a| a.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect())
            .unwrap_or_default();

        let reg = ModelRegistry::build(all, &self.cfg);
        // println!(
        //     "NVIDIA models: {} available | fast={:?} | strong={:?} | embed={}",
        //     reg.all.len(), reg.fast, reg.strong, reg.embed
        // );
        *self.registry.write().await = reg;
        Ok(())
    }

    pub async fn registry(&self) -> ModelRegistry {
        self.registry.read().await.clone()
    }

    async fn candidates(&self, tier: Tier) -> Vec<String> {
        let reg = self.registry.read().await;
        let mut list = match tier {
            Tier::Fast => reg.fast.clone(),
            Tier::Strong => reg.strong.clone(),
        };
        
        if list.is_empty() {
            list = match tier {
                Tier::Fast => self.cfg.fast_models.clone(),
                Tier::Strong => self.cfg.strong_models.clone(),
            };
        }
        let now = StdInstant::now();
        let mut dead = self.dead.lock().unwrap();
        dead.retain(|_, until| *until > now);
        list.retain(|m| !dead.contains_key(m));
        list
    }

    fn mark_dead(&self, model: &str, secs: u64) {
        eprintln!("AI: parking model {model} for {secs}s");
        self.dead
            .lock()
            .unwrap()
            .insert(model.to_string(), StdInstant::now() + Duration::from_secs(secs));
    }

    async fn log_call(&self, feature: &str, model: &str, pv: &str, pt: i64, ct: i64,
                      ms: i64, ok: bool, err: Option<&str>) {
        let cost = (pt + ct) as f64 / 1000.0 * self.cfg.cost_per_1k_tokens;
        let _ = sqlx::query(
            "INSERT INTO ai_calls
             (feature, model, prompt_version, prompt_tokens, completion_tokens,
              latency_ms, cost_usd_est, ok, error)
             VALUES ($1,$2,$3,$4,$5,$6,$7::float8::numeric,$8,$9)",
        )
        .bind(feature).bind(model).bind(pv)
        .bind(pt as i32).bind(ct as i32).bind(ms as i32)
        .bind(cost).bind(ok).bind(err)
        .execute(&self.pool)
        .await;
    }

    pub async fn generate_text(
        &self,
        system: &str,
        user: &str,
        opts: &GenOpts,
    ) -> Result<Completion, AiError> {
        if !self.budget.has_room() {
            return Err(AiError::BudgetExceeded);
        }
        let models = self.candidates(opts.tier).await;
        if models.is_empty() {
            return Err(AiError::Unavailable("no chat models resolved".into()));
        }

        let url = format!("{}/chat/completions", self.cfg.base_url);
        let mut last_err = String::from("no attempt made");

        for model in models {
            self.limiter.acquire().await;
            let started = Instant::now();
            let messages = if model.to_lowercase().contains("gemma") {
                json!([{ "role": "user", "content": format!("{system}\n\n{user}") }])
            } else {
                json!([
                    { "role": "system", "content": system },
                    { "role": "user", "content": user }
                ])
            };
            let mut body = json!({
                "model": model,
                "temperature": opts.temperature,
                "max_tokens": opts.max_tokens,
                "messages": messages
            });
            if self.cfg.no_think_models.contains(&model) {
                body["chat_template_kwargs"] = json!({ "enable_thinking": false });
            }

            let resp = match self.http.post(&url).bearer_auth(&self.cfg.api_key)
                .json(&body).send().await
            {
                Ok(r) => r,
                Err(e) => {
                    last_err = e.without_url().to_string();
                    self.log_call(opts.feature, &model, opts.prompt_version, 0, 0,
                        started.elapsed().as_millis() as i64, false, Some(&last_err)).await;
                    continue;
                }
            };

            let status = resp.status();
            if status == StatusCode::TOO_MANY_REQUESTS {
                let secs = resp.headers().get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(10)
                    .min(60);
                self.limiter.penalize(Duration::from_secs(secs)).await;
                last_err = format!("429 from {model}, backing off {secs}s");
                self.log_call(opts.feature, &model, opts.prompt_version, 0, 0,
                    started.elapsed().as_millis() as i64, false, Some(&last_err)).await;
                continue;
            }

            let text = match resp.text().await {
                Ok(t) => t,
                Err(e) => { last_err = e.without_url().to_string(); continue; }
            };
            let ms = started.elapsed().as_millis() as i64;

            if !status.is_success() {

                if matches!(status.as_u16(), 403 | 404 | 410) {
                    self.mark_dead(&model, 6 * 3600);
                } else if matches!(status.as_u16(), 502 | 503 | 504) {
                    self.mark_dead(&model, 120);
                }

                last_err = format!("HTTP {status} from {model}: {}",
                    text.chars().take(200).collect::<String>());
                self.log_call(opts.feature, &model, opts.prompt_version, 0, 0, ms,
                    false, Some(&last_err)).await;
                continue;
            }

            let v: Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => { last_err = e.to_string(); continue; }
            };
            let content = v["choices"][0]["message"]["content"].as_str().unwrap_or("");
            let clean = strip_think(content);
            let pt = v["usage"]["prompt_tokens"].as_i64().unwrap_or(0);
            let ct = v["usage"]["completion_tokens"].as_i64().unwrap_or(0);
            self.budget.add((pt + ct).max(0) as u64);

            if clean.is_empty() {
                self.mark_dead(&model, 60);
                last_err = format!("empty content from {model} (finish_reason={})",
                    v["choices"][0]["finish_reason"]);
                self.log_call(opts.feature, &model, opts.prompt_version, pt, ct, ms,
                    false, Some(&last_err)).await;
                continue;
            }

            self.log_call(opts.feature, &model, opts.prompt_version, pt, ct, ms, true, None).await;
            return Ok(Completion { text: clean, model, prompt_tokens: pt, completion_tokens: ct });
        }

        Err(AiError::Unavailable(last_err))
    }

    pub async fn stream_text(
        &self,
        system: &str,
        user: &str,
        opts: &GenOpts,
        out: tokio::sync::mpsc::Sender<String>,
    ) -> Result<Completion, AiError> {
        if !self.budget.has_room() {
            return Err(AiError::BudgetExceeded);
        }
        let models = self.candidates(opts.tier).await;
        if models.is_empty() {
            return Err(AiError::Unavailable("no chat models resolved".into()));
        }

        let url = format!("{}/chat/completions", self.cfg.base_url);
        let mut last_err = String::from("no attempt made");

        for model in models {
            self.limiter.acquire().await;
            let started = Instant::now();

            let messages = if model.contains("gemma") {
                json!([{ "role": "user", "content": format!("{system}\n\n{user}") }])
            } else {
                json!([
                    { "role": "system", "content": system },
                    { "role": "user", "content": user }
                ])
            };
            let mut body = json!({
                "model": model,
                "temperature": opts.temperature,
                "max_tokens": opts.max_tokens,
                "stream": true,
                "stream_options": { "include_usage": true },
                "messages": messages
            });
            if self.cfg.no_think_models.contains(&model) {
                body["chat_template_kwargs"] = json!({ "enable_thinking": false });
            }

            let resp = match self
                .http
                .post(&url)
                .bearer_auth(&self.cfg.api_key)
                .timeout(Duration::from_secs(90))
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    last_err = e.without_url().to_string();
                    self.log_call(opts.feature, &model, opts.prompt_version, 0, 0,
                        started.elapsed().as_millis() as i64, false, Some(&last_err)).await;
                    continue;
                }
            };

            let status = resp.status();
            if status == StatusCode::TOO_MANY_REQUESTS {
                let secs = resp.headers().get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(10)
                    .min(60);
                self.limiter.penalize(Duration::from_secs(secs)).await;
                last_err = format!("429 from {model}, backing off {secs}s");
                self.log_call(opts.feature, &model, opts.prompt_version, 0, 0,
                    started.elapsed().as_millis() as i64, false, Some(&last_err)).await;
                continue;
            }
            if !status.is_success() {
                let text = resp.text().await.unwrap_or_default();
                last_err = format!("HTTP {status} from {model}: {}",
                    text.chars().take(200).collect::<String>());
                self.log_call(opts.feature, &model, opts.prompt_version, 0, 0,
                    started.elapsed().as_millis() as i64, false, Some(&last_err)).await;
                continue;
            }

            let mut stream = resp.bytes_stream();
            let mut buf: Vec<u8> = Vec::new();
            let mut full = String::new();
            let (mut pt, mut ct) = (0i64, 0i64);
            let mut stream_err: Option<String> = None;

            'read: loop {
                let chunk = match tokio::time::timeout(Duration::from_secs(30), stream.next()).await {
                    Ok(Some(Ok(c))) => c,
                    Ok(Some(Err(e))) => { stream_err = Some(e.without_url().to_string()); break; }
                    Ok(None) => break,
                    Err(_) => { stream_err = Some("stream stalled for 30s".into()); break; }
                };
                buf.extend_from_slice(&chunk);

                while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
                    let line_bytes: Vec<u8> = buf.drain(..=pos).collect();
                    let line = String::from_utf8_lossy(&line_bytes);
                    let line = line.trim();
                    let Some(data) = line.strip_prefix("data:") else { continue };
                    let data = data.trim();
                    if data == "[DONE]" { break 'read; }
                    let Ok(v) = serde_json::from_str::<Value>(data) else { continue };

                    if v["usage"].is_object() {
                        pt = v["usage"]["prompt_tokens"].as_i64().unwrap_or(pt);
                        ct = v["usage"]["completion_tokens"].as_i64().unwrap_or(ct);
                    }
                    if let Some(piece) = v["choices"][0]["delta"]["content"].as_str() {
                        if !piece.is_empty() {
                            full.push_str(piece);
                            if out.send(piece.to_string()).await.is_err() {
                                break 'read;
                            }
                        }
                    }
                }
            }

            let ms = started.elapsed().as_millis() as i64;
            if ct == 0 {
                ct = (full.len() / 4) as i64;
                pt = ((system.len() + user.len()) / 4) as i64;
            }
            self.budget.add((pt + ct).max(0) as u64);

            let clean = strip_think(&full);
            if clean.is_empty() {
                last_err = stream_err.unwrap_or_else(|| format!("empty stream from {model}"));
                self.log_call(opts.feature, &model, opts.prompt_version, pt, ct, ms,
                    false, Some(&last_err)).await;
                continue;
            }

            self.log_call(opts.feature, &model, opts.prompt_version, pt, ct, ms,
                stream_err.is_none(), stream_err.as_deref()).await;
            return Ok(Completion { text: clean, model, prompt_tokens: pt, completion_tokens: ct });
        }

        Err(AiError::Unavailable(last_err))
    }

    pub async fn generate_object<T: DeserializeOwned>(
        &self,
        system: &str,
        user: &str,
        opts: &GenOpts,
    ) -> Result<(T, Completion), AiError> {
        let mut sys = format!(
            "{system}\n\nReturn ONLY one valid JSON object. No markdown, no commentary."
        );
        for attempt in 0..2 {
            let c = self.generate_text(&sys, user, opts).await?;
            if let Some(raw) = extract_json_object(&c.text) {
                if let Ok(obj) = serde_json::from_str::<T>(raw) {
                    return Ok((obj, c));
                }
            }
            if attempt == 0 {
                sys.push_str("\nYour previous reply was not valid JSON matching the schema. \
                              Reply again with ONLY the JSON object.");
            }
        }
        Err(AiError::Invalid("model did not return schema-valid JSON".into()))
    }

    pub async fn embed(&self, texts: &[String], input_type: &str) -> Result<Vec<Vec<f32>>, AiError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        if !self.budget.has_room() {
            return Err(AiError::BudgetExceeded);
        }
        let model = {
            let r = self.registry.read().await;
            if r.embed.is_empty() { self.cfg.embed_model.clone() } else { r.embed.clone() }
        };

        self.limiter.acquire().await;
        let started = Instant::now();
        let mut body = json!({ "model": model, "input": texts, "input_type": input_type });
        if let Some(d) = self.cfg.embed_dimensions {
            body["dimensions"] = json!(d);
        }
        let resp = self.http
            .post(format!("{}/embeddings", self.cfg.base_url))
            .bearer_auth(&self.cfg.api_key)
            .json(&json!({ "model": model, "input": texts, "input_type": input_type }))
            .send().await
            .map_err(|e| AiError::Unavailable(e.without_url().to_string()))?;

        let status = resp.status();
        let ms = started.elapsed().as_millis() as i64;
        if status == StatusCode::TOO_MANY_REQUESTS {
            self.limiter.penalize(Duration::from_secs(10)).await;
        }
        let text = resp.text().await.map_err(|e| AiError::Unavailable(e.without_url().to_string()))?;
        if !status.is_success() {
            let msg = format!("embeddings HTTP {status}: {}", text.chars().take(200).collect::<String>());
            self.log_call("embed", &model, "-", 0, 0, ms, false, Some(&msg)).await;
            return Err(AiError::Unavailable(msg));
        }

        let v: Value = serde_json::from_str(&text).map_err(|e| AiError::Invalid(e.to_string()))?;
        let mut out = Vec::with_capacity(texts.len());
                for item in v["data"].as_array().ok_or_else(|| AiError::Invalid("no data[]".into()))? {
                        let mut vec: Vec<f32> = item["embedding"].as_array()
                .ok_or_else(|| AiError::Invalid("no embedding".into()))?
                .iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect();

            if vec.len() > super::EMBED_DIM && self.cfg.embed_truncate {
                vec.truncate(super::EMBED_DIM);
                let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for x in vec.iter_mut() {
                        *x /= norm;
                    }
                }
            }

            if vec.len() != super::EMBED_DIM {
                return Err(AiError::Invalid(format!(
                    "embedding model returned {} dimensions but the table expects {}",
                    vec.len(),
                    super::EMBED_DIM
                )));
            }
            out.push(vec);
        }
        let tokens = v["usage"]["total_tokens"].as_i64().unwrap_or(0);
        self.budget.add(tokens.max(0) as u64);
        self.log_call("embed", &model, "-", tokens, 0, ms, true, None).await;
        Ok(out)
    }

        async fn probe_one(&self, m: &str, nothink: bool, max_tokens: u32, timeout_secs: u64) -> Value {
        let url = format!("{}/chat/completions", self.cfg.base_url);
        self.limiter.acquire().await;
        let started = Instant::now();
        let mut body = json!({
            "model": m,
            "max_tokens": max_tokens,
            "temperature": 0,
            "messages": [{ "role": "user", "content": "Reply with exactly: ok" }]
        });
        if nothink {
            body["chat_template_kwargs"] = json!({ "enable_thinking": false });
        }

        let mut finish = Value::Null;
        let mut reasoning_chars = 0usize;
        let (status, detail) = match self
            .http
            .post(&url)
            .bearer_auth(&self.cfg.api_key)
            .timeout(Duration::from_secs(timeout_secs))
            .json(&body)
            .send()
            .await
        {
            Ok(r) => {
                let s = r.status();
                let t = r.text().await.unwrap_or_default();
                if s.is_success() {
                    let v: Value = serde_json::from_str(&t).unwrap_or(Value::Null);
                    let c = v["choices"][0]["message"]["content"].as_str().unwrap_or("");
                    finish = v["choices"][0]["finish_reason"].clone();
                    reasoning_chars = v["choices"][0]["message"]["reasoning_content"]
                        .as_str()
                        .map(|x| x.len())
                        .unwrap_or(0);
                    let msg = if strip_think(c).is_empty() {
                        "empty content (reasoning model?)".to_string()
                    } else {
                        format!("ok: {}", strip_think(c).chars().take(40).collect::<String>())
                    };
                    (s.as_u16(), msg)
                } else {
                    (s.as_u16(), t.chars().take(120).collect::<String>())
                }
            }
            Err(e) if e.is_timeout() => (0, format!("timed out after {timeout_secs}s")),
            Err(e) => (0, e.without_url().to_string()),
        };

        json!({
            "model": m,
            "status": status,
            "result": detail,
            "finish_reason": finish,
            "reasoning_chars": reasoning_chars,
            "ms": started.elapsed().as_millis() as u64
        })
    }

    pub async fn probe(
        &self,
        models: &[String],
        nothink: bool,
        max_tokens: u32,
        timeout_secs: u64,
    ) -> Vec<Value> {
        futures_util::future::join_all(
            models.iter().map(|m| self.probe_one(m, nothink, max_tokens, timeout_secs)),
        )
        .await
    }

            async fn probe_embed_one(&self, model: &str, timeout_secs: u64, dims: Option<u32>) -> Value {
        self.limiter.acquire().await;
        let started = Instant::now();
        let mut body = json!({
            "model": model,
            "input": ["Binance moved 12,450 ETH to an unlabelled wallet"],
            "input_type": "passage"
        });
        if let Some(d) = dims {
            body["dimensions"] = json!(d);
        }
        let res = self
            .http
            .post(format!("{}/embeddings", self.cfg.base_url))
            .bearer_auth(&self.cfg.api_key)
            .timeout(Duration::from_secs(timeout_secs))
            .json(&body)
            .send()
            .await;

        let (status, detail, dim) = match res {
            Ok(r) => {
                let s = r.status();
                let t = r.text().await.unwrap_or_default();
                if s.is_success() {
                    let v: Value = serde_json::from_str(&t).unwrap_or(Value::Null);
                    let d = v["data"][0]["embedding"].as_array().map(|a| a.len()).unwrap_or(0);
                    let msg = if d > 0 { "ok".to_string() } else { "no embedding in response".to_string() };
                    (s.as_u16(), msg, d)
                } else {
                    (s.as_u16(), t.chars().take(200).collect::<String>(), 0)
                }
            }
            Err(e) if e.is_timeout() => (0, format!("timed out after {timeout_secs}s"), 0),
            Err(e) => (0, e.without_url().to_string(), 0),
        };

        json!({
            "model": model,
            "requested_dimensions": dims,
            "status": status,
            "result": detail,
            "dimension": dim,
            "fits_table": dim == super::EMBED_DIM,
            "ms": started.elapsed().as_millis() as u64
        })
    }

    pub async fn probe_embeds(&self, models: &[String], timeout_secs: u64, dims: Option<u32>) -> Vec<Value> {
        futures_util::future::join_all(
            models.iter().map(|m| self.probe_embed_one(m, timeout_secs, dims)),
        )
        .await
    }
}