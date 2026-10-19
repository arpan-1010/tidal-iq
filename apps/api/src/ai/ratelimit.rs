use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct ClientLimiter {
    inner: Mutex<HashMap<String, (Instant, u32)>>,
    max_per_min: u32,
}

impl ClientLimiter {
    pub fn new(max_per_min: u32) -> Self {
        Self { inner: Mutex::new(HashMap::new()), max_per_min }
    }

    /// true = allowed. Fixed one-minute window per client key.
    pub fn allow(&self, key: &str) -> bool {
        let mut m = self.inner.lock().unwrap();
        let now = Instant::now();
        if m.len() > 10_000 {
            m.retain(|_, v| now.duration_since(v.0) < Duration::from_secs(60));
        }
        let e = m.entry(key.to_string()).or_insert((now, 0));
        if now.duration_since(e.0) >= Duration::from_secs(60) {
            *e = (now, 0);
        }
        e.1 += 1;
        e.1 <= self.max_per_min
    }
}

/// Identify the caller. X-Forwarded-For is only trustworthy behind your own proxy
/// (Railway/Render/Fly set it). Locally everyone is "local".
pub fn client_key(headers: &axum::http::HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "local".to_string())
}