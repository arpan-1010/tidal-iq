use super::context::TxContext;

const FORBIDDEN: &[&str] = &[
    "guarantee",
    "will pump",
    "will dump",
    "buy now",
    "sell now",
    "financial advice",
    "price target",
    "to the moon",
    "you should buy",
    "you should sell",
    "bullish",
    "bearish",
];

pub fn sanitize_label(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '&' | '(' | ')'))
        .take(40)
        .collect::<String>()
        .trim()
        .to_string()
}

fn has_long_token(text: &str) -> bool {
    text.split_whitespace().any(|tok| {
        let t = tok.trim_matches(|c: char| !c.is_alphanumeric());
        t.len() >= 26 && t.chars().all(|c| c.is_ascii_alphanumeric())
    })
}

fn strip_addr_tokens(text: &str) -> String {
    text.split_whitespace()
        .filter(|t| !(t.contains('…') || t.contains("...") || t.starts_with("0x")))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn numbers_in(text: &str) -> Vec<f64> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == ',' || chars[i] == '.') {
            i += 1;
        }
        let mut raw: String = chars[start..i].iter().collect();
        while raw.ends_with(',') || raw.ends_with('.') {
            raw.pop();
        }
        let Ok(mut n) = raw.replace(',', "").parse::<f64>() else {
            continue;
        };

        let rest: String = chars[i..]
            .iter()
            .take(10)
            .collect::<String>()
            .to_lowercase();
        let trimmed = rest.trim_start();
        let mult = if trimmed.starts_with("billion") {
            1e9
        } else if trimmed.starts_with("million") {
            1e6
        } else if trimmed.starts_with("thousand") {
            1e3
        } else {
            let mut it = rest.chars();
            let c1 = it.next();
            let boundary = !it.next().map_or(false, |c| c.is_alphabetic());
            match c1 {
                Some('b') if boundary => 1e9,
                Some('m') if boundary => 1e6,
                Some('k') if boundary => 1e3,
                _ => 1.0,
            }
        };
        n *= mult;
        out.push(n);
    }
    out
}

fn allowed_numbers(c: &TxContext) -> Vec<f64> {
    let mut v = vec![
        c.value_usd,
        c.amount,
        c.related_total_usd,
        c.from_tx_count_30d as f64,
        c.count_last_hour as f64,
        c.prior_tx_count_all as f64,
    ];
    for o in [
        c.from_avg_usd_30d,
        c.multiple_of_from_avg,
        c.token_avg_usd_30d,
        c.multiple_of_token_avg,
    ] {
        if let Some(x) = o {
            v.push(x);
        }
    }
    for r in &c.related_recent {
        v.push(r.value_usd);
    }
    v.extend(numbers_in(&c.time));
    v
}

fn number_ok(n: f64, allowed: &[f64]) -> bool {
    if n < 100.0 && n.fract() == 0.0 {
        return true;
    }
    allowed.iter().any(|a| {
        let tol = if a.abs() < 100.0 {
            0.51
        } else {
            a.abs() * 0.03
        };
        (n - a).abs() <= tol
    })
}

pub fn validate_text(text: &str, ctx: &TxContext, max_chars: usize) -> Result<(), String> {
    let t = text.trim();
    if t.is_empty() {
        return Err("empty".into());
    }
    if t.chars().count() > max_chars {
        return Err("too long".into());
    }
    if has_long_token(t) {
        return Err("contains an address/hash-like token".into());
    }

    let lower = t.to_lowercase();
    for p in FORBIDDEN {
        if lower.contains(p) {
            return Err(format!("forbidden phrase: {p}"));
        }
    }

    let allowed = allowed_numbers(ctx);
    for n in numbers_in(&strip_addr_tokens(t)) {
        if !number_ok(n, &allowed) {
            return Err(format!("ungrounded number: {n}"));
        }
    }
    Ok(())
}

pub fn citations_in(text: &str) -> Vec<usize> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            let mut j = i + 1;
            let mut num = String::new();
            while j < chars.len() && chars[j].is_ascii_digit() {
                num.push(chars[j]);
                j += 1;
            }
            if !num.is_empty() && j < chars.len() && chars[j] == ']' {
                if let Ok(n) = num.parse::<usize>() {
                    out.push(n);
                }
                i = j;
            }
        }
        i += 1;
    }
    out
}

pub fn validate_chat_text(
    text: &str,
    allowed: &[f64],
    max_cite: usize,
    require_cite: bool,
    max_chars: usize,
) -> Result<(), String> {
    let t = text.trim();
    if t.is_empty() {
        return Err("empty".into());
    }
    if t.chars().count() > max_chars {
        return Err("too long".into());
    }
    if has_long_token(t) {
        return Err("contains an address/hash-like token".into());
    }

    let lower = t.to_lowercase();
    for p in FORBIDDEN {
        if lower.contains(p) {
            return Err(format!("forbidden phrase: {p}"));
        }
    }

    let cites = citations_in(t);
    if cites.iter().any(|&n| n == 0 || n > max_cite) {
        return Err("cites a transaction number that does not exist".into());
    }
    if require_cite && cites.is_empty() {
        return Err("no citations".into());
    }

    for n in numbers_in(&strip_addr_tokens(t)) {
        if !number_ok(n, allowed) {
            return Err(format!("ungrounded number: {n}"));
        }
    }
    Ok(())
}