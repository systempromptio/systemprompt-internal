//! Spend-tab view builder: the fast/slow latency split at the caller's SLO.
//!
//! Split from `view.rs` at the 300-line ceiling; the shared label helpers
//! stay there and are imported here.

use crate::handlers::ssr::format::format_duration_ms;
use crate::repositories::analytics::site::latency::LatencySplit;

use super::context::FastSlowView;

// Why: this platform has no fast/slow request pools, so the split is stated
// as what it actually is — a latency bucket at the caller's SLO threshold —
// with the percentiles and breach share beside it and untimed requests shown
// rather than folded away. The displays derive from the threshold the query
// actually bound, so the caption can never contradict the split.
pub(super) fn fast_slow(split: &LatencySplit) -> FastSlowView {
    let timed = split.fast + split.slow;
    let threshold = format_duration_ms(i64::from(split.threshold_ms));
    FastSlowView {
        within_label: format!("Within SLO (<{threshold})"),
        breach_label: format!("Breaching SLO (>={threshold})"),
        fast: split.fast,
        slow: split.slow,
        untimed: split.untimed,
        threshold_display: threshold,
        breach_pct_display: if timed > 0 {
            let permille = split.slow.saturating_mul(1000) / timed;
            format!("{}.{}%", permille / 10, permille % 10)
        } else {
            "–".to_owned()
        },
        p50_display: format_duration_ms(split.p50_ms.round() as i64),
        p95_display: format_duration_ms(split.p95_ms.round() as i64),
        has_data: timed + split.untimed > 0,
    }
}
