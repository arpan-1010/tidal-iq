use super::context::fmt_usd;
use super::embed::vec_literal;
use super::nvidia::{GenOpts, Tier};
use super::safety::{citations_in, numbers_in, validate_chat_text};
use super::{prompts, AiState, DISCLAIMER};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

const INTRO: &str = "I'm TidalIQ's on-chain analyst. I answer questions about the large Bitcoin, \
Ethereum and Solana transfers TidalIQ tracks, such as the biggest moves today, unusual transfers, \
or repeated senders. I don't give trading advice.";

pub const REFUSAL: &str = "Ask me about Whale Transaction related questions only.";

const TOPIC_WORDS: &[&str] = &[
    "btc", "bitcoin", "eth", "ethereum", "ether", "sol", "solana", "crypto", "onchain", "chain",
    "token", "tokens", "coin", "coins", "usdt", "usdc", "stablecoin", "transfer", "transfers",
    "transaction", "transactions", "tx", "txs", "wallet", "wallets", "address", "addresses",
    "whale", "whales", "sender", "senders", "receiver", "recipient", "exchange", "volume",
    "usd", "dollar", "dollars", "million", "billion", "unusual", "anomaly", "anomalies",
    "flagged", "flag", "flags", "repeated", "repeat", "largest", "biggest", "big", "large",
    "moves", "move", "moved", "sent", "send", "flow", "flows", "activity", "score", "impact",
    "today", "yesterday", "week", "month", "hour", "hours", "days", "narrative", "digest",
];

const FOLLOW_UP_WORDS: &[&str] = &[
    "more", "why", "how", "explain", "details", "detail", "and", "also", "those", "that",
    "them", "these", "same", "other", "another", "else", "previous", "earlier", "compare",
];

fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(String::from)
        .collect()
}

fn is_amount(w: &str) -> bool {
    match w.char_indices().last() {
        Some((i, c)) if matches!(c, 'k' | 'm' | 'b') && i > 0 => w[..i].chars().all(|d| d.is_ascii_digit()),
        _ => false,
    }
}

pub fn off_topic_reply(message: &str, first_turn: bool) -> Option<&'static str> {
    let ws = words(message);
    let has = |list: &[&str]| ws.iter().any(|w| list.contains(&w.as_str()));

    let on_topic = message.contains('$') || ws.iter().any(|w| is_amount(w)) || has(TOPIC_WORDS);
    if on_topic {
        return None;
    }

    let smalltalk = has(&["hello", "hi", "hey", "thanks", "thank", "yourself", "bye"])
        || (has(&["who", "what"]) && has(&["you", "your"]));
    if smalltalk {
        return Some(INTRO);
    }

    let follow_up = !first_turn && ws.len() <= 6 && has(FOLLOW_UP_WORDS);
    if follow_up {
        return None;
    }

    Some(REFUSAL)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Citation {
    pub n: usize,
    pub hash: String,
    pub chain: String,
    pub token: String,
    pub value_usd: Option<f64>,
    pub time: String,
    pub summary: Option<String>,
    pub explorer_url: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct ChatAnswer {
    pub session_id: String,
    pub message_id: i64,
    pub text: String,
    pub citations: Vec<Citation>,
    pub source: String,
    pub model: Option<String>,
    pub disclaimer: &'static str,
}

pub struct ChatPlan {
    pub session_id: Uuid,
    pub citations: Vec<Citation>,
    pub user_prompt: String,
    pub allowed: Vec<f64>,
    pub cached: Option<Value>,
    pub question: String,
    pub qvec: Option<String>,
    pub first_turn: bool,
    pub canned: Option<String>,
}

fn fallback_text(plan: &ChatPlan) -> String {
    if plan.citations.is_empty() {
        return "I couldn't find matching large transactions in TidalIQ's tracked data. \
                Try asking about ETH, SOL or BTC transfers, a time window like \"today\" or \
                \"yesterday\", or an amount such as \"over $5M\"."
            .to_string();
    }
    let mut s = String::from("Here are the closest matches I found:\n");
    for c in plan.citations.iter().take(5) {
        let usd = c.value_usd.map(fmt_usd).unwrap_or_else(|| "n/a".to_string());
        s.push_str(&format!("[{}] {} {} {} at {}", c.n, c.chain, c.token, usd, c.time));
        if let Some(sum) = &c.summary {
            s.push_str(&format!(": {sum}"));
        }
        s.push('\n');
    }
    s.trim_end().to_string()
}

fn cited(plan: &ChatPlan, text: &str) -> Vec<Citation> {
    let mut ns = citations_in(text);
    ns.sort();
    ns.dedup();
    ns.into_iter()
        .filter(|n| *n >= 1)
        .filter_map(|n| plan.citations.get(n - 1).cloned())
        .collect()
}

impl AiState {
    async fn new_session(&self) -> Result<Uuid> {
        let (id,): (Uuid,) =
            sqlx::query_as("INSERT INTO chat_sessions DEFAULT VALUES RETURNING id")
                .fetch_one(self.pool())
                .await?;
        Ok(id)
    }

    async fn save_assistant(&self, session_id: Uuid, text: &str, cites: &[Citation]) -> Result<i64> {
        let (id,): (i64,) = sqlx::query_as(
            "INSERT INTO chat_messages (session_id, role, content, citations)
             VALUES ($1, 'assistant', $2, $3) RETURNING id",
        )
        .bind(session_id)
        .bind(text)
        .bind(json!(cites))
        .fetch_one(self.pool())
        .await?;
        Ok(id)
    }

    pub async fn prepare_chat(&self, session: Option<&str>, message: &str) -> Result<ChatPlan> {
        let pool = self.pool();

        let session_id = match session.and_then(|s| Uuid::parse_str(s).ok()) {
            Some(id) => {
                let ok: Option<(i32,)> = sqlx::query_as("SELECT 1 FROM chat_sessions WHERE id = $1")
                    .bind(id)
                    .fetch_optional(pool)
                    .await?;
                if ok.is_some() { id } else { self.new_session().await? }
            }
            None => self.new_session().await?,
        };

        let history: Vec<(String, String)> = sqlx::query_as(
            "SELECT role, content FROM (
                 SELECT id, role, content FROM chat_messages
                 WHERE session_id = $1 ORDER BY id DESC LIMIT 6
             ) t ORDER BY id",
        )
        .bind(session_id)
        .fetch_all(pool)
        .await?;
        let first_turn = history.is_empty();

        sqlx::query("INSERT INTO chat_messages (session_id, role, content) VALUES ($1, 'user', $2)")
            .bind(session_id)
            .bind(message)
            .execute(pool)
            .await?;

                if let Some(reply) = off_topic_reply(message, first_turn) {
            return Ok(ChatPlan {
                session_id,
                citations: vec![],
                user_prompt: String::new(),
                allowed: vec![],
                cached: None,
                question: message.to_string(),
                qvec: None,
                first_turn,
                canned: Some(reply.to_string()),
            });
        }

        let cache_embed = async {
            if !first_turn {
                return None::<String>;
            }
            let c = self.client.as_ref()?;
            let mut v = c.embed(&[message.to_string()], "query").await.ok()?;
            if v.is_empty() {
                return None;
            }
            Some(vec_literal(&v.remove(0)))
        };
        let (qvec, search_res) = tokio::join!(cache_embed, self.search(message, 8));
        let search_val = search_res?;

        let mut cached: Option<Value> = None;
        if let Some(v) = &qvec {
            let hit: Option<(Value, f64)> = sqlx::query_as(
                "SELECT answer, (embedding <=> $1::halfvec) AS distance
                 FROM ai_semantic_cache
                 WHERE kind = 'chat' AND created_at > now() - interval '10 minutes'
                 ORDER BY embedding <=> $1::halfvec LIMIT 1",
            )
            .bind(v)
            .fetch_optional(pool)
            .await?;
            if let Some((ans, d)) = hit {
                if d < 0.05 {
                    cached = Some(ans);
                }
            }
        }

        let results = search_val["results"].as_array().cloned().unwrap_or_default();
        let citations: Vec<Citation> = results
            .iter()
            .enumerate()
            .map(|(i, r)| Citation {
                n: i + 1,
                hash: r["hash"].as_str().unwrap_or("").to_string(),
                chain: r["chain"].as_str().unwrap_or("").to_string(),
                token: r["token"].as_str().unwrap_or("").to_string(),
                value_usd: r["value_usd"].as_f64(),
                time: r["time"].as_str().unwrap_or("").to_string(),
                summary: r["summary"].as_str().map(String::from),
                explorer_url: r["explorer_url"].as_str().unwrap_or("").to_string(),
            })
            .collect();

        let stats: Vec<(String, i64, f64, f64)> = sqlx::query_as(
            "SELECT chain, COUNT(*), COALESCE(SUM(value_usd), 0)::float8,
                    COALESCE(MAX(value_usd), 0)::float8
             FROM transactions
             WHERE tx_timestamp > (now() at time zone 'utc') - interval '24 hours'
             GROUP BY chain",
        )
        .fetch_all(pool)
        .await?;

        let tx_json: Vec<Value> = results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                json!({
                    "n": i + 1,
                    "chain": r["chain"],
                    "token": r["token"],
                    "value_usd": r["value_usd"].as_f64().map(|v| v.round()),
                    "from": r["from"],
                    "to": r["to"],
                    "time": r["time"],
                    "importance": r["importance_tier"],
                    "summary": r["summary"],
                })
            })
            .collect();
        let stats_json: Vec<Value> = stats
            .iter()
            .map(|(chain, n, total, biggest)| {
                json!({ "chain": chain, "count": n, "total_usd": total.round(), "largest_usd": biggest.round() })
            })
            .collect();

        let data = json!({
            "now_utc": chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string(),
            "how_the_question_was_interpreted": search_val["filters"],
            "transactions": tx_json,
            "last_24h_by_chain": stats_json,
        });

        let convo: String = history
            .iter()
            .map(|(role, content)| format!("{role}: {}", content.chars().take(400).collect::<String>()))
            .collect::<Vec<_>>()
            .join("\n");
        let user_prompt = format!(
            "<conversation_so_far>\n{convo}\n</conversation_so_far>\n<data>{}</data>\n<question>{message}</question>\nAnswer now.",
            serde_json::to_string(&data)?
        );

        let mut allowed: Vec<f64> = vec![results.len() as f64];
        let mut sum = 0.0;
        for c in &citations {
            if let Some(v) = c.value_usd {
                allowed.push(v);
                sum += v;
            }
            allowed.extend(numbers_in(&c.time));
            if let Some(s) = &c.summary {
                allowed.extend(numbers_in(s));
            }
        }
        allowed.push(sum);
        let (mut total_n, mut total_usd) = (0i64, 0.0f64);
        for (_, n, total, biggest) in &stats {
            allowed.push(*n as f64);
            allowed.push(*total);
            allowed.push(*biggest);
            total_n += n;
            total_usd += total;
        }
        allowed.push(total_n as f64);
        allowed.push(total_usd);
        allowed.extend(numbers_in(message));
        for (_, content) in &history {
            allowed.extend(numbers_in(content));
        }

        Ok(ChatPlan {
            session_id,
            citations,
            user_prompt,
            allowed,
            cached,
            question: message.to_string(),
            qvec,
            first_turn,
            canned: None
        })
    }

    pub async fn answer_from_cache(&self, plan: &ChatPlan, ans: &Value) -> Result<ChatAnswer> {
        let text = ans["text"].as_str().unwrap_or("").to_string();
        let cites: Vec<Citation> = serde_json::from_value(ans["citations"].clone()).unwrap_or_default();
        let id = self.save_assistant(plan.session_id, &text, &cites).await?;
        Ok(ChatAnswer {
            session_id: plan.session_id.to_string(),
            message_id: id,
            text,
            citations: cites,
            source: "cache".into(),
            model: ans["model"].as_str().map(String::from),
            disclaimer: DISCLAIMER,
        })
    }

        pub async fn answer_canned(&self, plan: &ChatPlan, text: &str) -> Result<ChatAnswer> {
        let id = self.save_assistant(plan.session_id, text, &[]).await?;
        Ok(ChatAnswer {
            session_id: plan.session_id.to_string(),
            message_id: id,
            text: text.to_string(),
            citations: vec![],
            source: "template".into(),
            model: None,
            disclaimer: DISCLAIMER,
        })
    }

        pub async fn finish_chat(&self, plan: &ChatPlan, raw: Option<(String, String)>) -> Result<ChatAnswer> {
        let (text, source, model) = match raw {
            Some((text, _)) if text.trim().trim_matches('"') == REFUSAL => {
                (REFUSAL.to_string(), "template", None)
            }
            Some((text, model)) => {
                match validate_chat_text(&text, &plan.allowed, plan.citations.len(), !plan.citations.is_empty(), 900) {
                    Ok(()) => (text.trim().to_string(), "llm", Some(model)),
                    Err(why) => {
                        eprintln!("chat answer rejected: {why}");
                        (fallback_text(plan), "template", None)
                    }
                }
            }
            None => (fallback_text(plan), "template", None),
        };

        let cites = cited(plan, &text);
        let id = self.save_assistant(plan.session_id, &text, &cites).await?;

        if source == "llm" && plan.first_turn {
            if let Some(v) = &plan.qvec {
                let _ = sqlx::query(
                    "INSERT INTO ai_semantic_cache (kind, question, embedding, answer)
                     VALUES ('chat', $1, $2::halfvec, $3)",
                )
                .bind(&plan.question)
                .bind(v)
                .bind(json!({ "text": &text, "citations": &cites, "model": &model }))
                .execute(self.pool())
                .await;
                let _ = sqlx::query("DELETE FROM ai_semantic_cache WHERE created_at < now() - interval '1 day'")
                    .execute(self.pool())
                    .await;
            }
        }

        Ok(ChatAnswer {
            session_id: plan.session_id.to_string(),
            message_id: id,
            text,
            citations: cites,
            source: source.to_string(),
            model,
            disclaimer: DISCLAIMER,
        })
    }

    pub async fn chat(&self, session: Option<&str>, message: &str) -> Result<ChatAnswer> {
        let plan = self.prepare_chat(session, message).await?;
                if let Some(text) = &plan.canned {
            return self.answer_canned(&plan, text).await;
        }
        if let Some(ans) = &plan.cached {
            return self.answer_from_cache(&plan, ans).await;
        }

        let raw = match &self.client {
            Some(c) => {
                let opts = GenOpts::new(Tier::Strong, "chat", prompts::CHAT_V)
                    .max_tokens(600)
                    .temperature(0.2);
                match c.generate_text(prompts::CHAT_SYSTEM, &plan.user_prompt, &opts).await {
                    Ok(comp) => Some((comp.text, comp.model)),
                    Err(e) => {
                        eprintln!("chat generation failed: {e}");
                        None
                    }
                }
            }
            None => None,
        };
        self.finish_chat(&plan, raw).await
    }
}