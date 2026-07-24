//! Shared display formatting for the Evals page views.

// Why: 1 is the floor of the 1-5 scale, not zero, so the bar fill reflects
// the band rather than the raw number.
pub(super) fn score_pct(score: f64) -> i64 {
    if score <= 0.0 {
        return 0;
    }
    (((score - 1.0) / 4.0) * 100.0).round().clamp(0.0, 100.0) as i64
}

pub(super) fn share_pct(value: i64, total: i64) -> i64 {
    if total <= 0 || value <= 0 {
        return 0;
    }
    ((value as f64 / total as f64) * 100.0).round() as i64
}

pub(super) fn format_cost(microdollars: i64) -> String {
    let dollars = microdollars as f64 / 1_000_000.0;
    if dollars == 0.0 {
        "$0".to_owned()
    } else if dollars < 0.01 {
        format!("${dollars:.6}")
    } else {
        format!("${dollars:.4}")
    }
}

pub(super) fn local_time(ts: chrono::DateTime<chrono::Utc>) -> String {
    ts.with_timezone(&chrono::Local)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

pub(super) fn short_id(id: &str) -> String {
    id.chars().take(14).collect()
}

pub(super) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let head: String = s.chars().take(max).collect();
        format!("{head}…")
    }
}
