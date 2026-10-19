use chrono::Datelike;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

pub struct Budget {
    limit: u64,
    used: AtomicU64,
    day: AtomicI64,
}

impl Budget {
    pub fn new(limit: u64) -> Self {
        Self { limit, used: AtomicU64::new(0), day: AtomicI64::new(0) }
    }

    fn today() -> i64 {
        chrono::Utc::now().date_naive().num_days_from_ce() as i64
    }

    fn roll(&self) {
        let t = Self::today();
        if self.day.swap(t, Ordering::Relaxed) != t {
            self.used.store(0, Ordering::Relaxed);
        }
    }

    pub fn has_room(&self) -> bool {
        self.roll();
        self.used.load(Ordering::Relaxed) < self.limit
    }

    pub fn add(&self, tokens: u64) {
        self.roll();
        self.used.fetch_add(tokens, Ordering::Relaxed);
    }

    pub fn seed(&self, tokens: u64) {
        self.roll();
        self.used.store(tokens, Ordering::Relaxed);
    }

    pub fn used(&self) -> u64 {
        self.roll();
        self.used.load(Ordering::Relaxed)
    }

    pub fn limit(&self) -> u64 {
        self.limit
    }
}