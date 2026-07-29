//! Pure value formatting shared by the web crates' display layers.
//!
//! One implementation per concern so a cost or duration renders identically
//! on every page that shows it.

// Why: Ids are long and the leading segment is the distinguishing part, so a
// table cell shows the head and puts the full value in a `title`.
pub fn short_id(id: &str) -> String {
    const KEEP: usize = 12;
    if id.chars().count() > KEEP {
        let head: String = id.chars().take(KEEP).collect();
        format!("{head}…")
    } else {
        id.to_owned()
    }
}
// Why: `—` rather than `$0`: a session with no billed traffic has no cost to
// show, which is different from one that cost nothing.
pub fn format_cost(microdollars: i64) -> String {
    if microdollars <= 0 {
        return "—".to_owned();
    }
    let dollars = microdollars as f64 / 1_000_000.0;
    if dollars >= 1.0 {
        format!("${dollars:.2}")
    } else if dollars >= 0.01 {
        format!("${dollars:.4}")
    } else {
        format!("${dollars:.6}")
    }
}
pub fn short_num(n: i64) -> String {
    let abs = n.unsigned_abs();
    if abs >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if abs >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
pub fn format_duration_ms(ms: i64) -> String {
    if ms < 1000 {
        format!("{ms} ms")
    } else if ms < 60_000 {
        format!("{:.2} s", ms as f64 / 1000.0)
    } else if ms < 3_600_000 {
        format!("{:.1} min", ms as f64 / 60_000.0)
    } else {
        format!("{:.1} h", ms as f64 / 3_600_000.0)
    }
}
