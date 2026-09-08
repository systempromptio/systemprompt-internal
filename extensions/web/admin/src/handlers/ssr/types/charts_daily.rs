//! Daily demo charts retained for the internal console.

use crate::repositories::demo::series::DailyBucket;

use super::{ChartBarView, ChartView, bar_pct};

// Why: the demo series is already gap-filled day by day, so the axis labels are
// the first and last bucket rather than a requested range — an empty series has
// no days to label and falls through to the empty message.
pub(crate) fn daily_count_chart(
    buckets: &[DailyBucket],
    title: &'static str,
    tone: &'static str,
    empty_message: &'static str,
) -> ChartView {
    let max = buckets.iter().map(|b| b.count).max().unwrap_or(0);
    let total: i64 = buckets.iter().map(|b| b.count).sum();
    let failures: i64 = buckets.iter().map(|b| b.failures).sum();
    ChartView {
        title,
        subtitle: format!("{total} in the window · {failures} failed · peak {max} per day"),
        tone,
        series: buckets
            .iter()
            .map(|b| ChartBarView {
                pct: bar_pct(b.count, max),
                tooltip: format!("{}: {} · {} failed", b.day, b.count, b.failures),
            })
            .collect(),
        has_data: max > 0,
        y_max_display: max.to_string(),
        y_mid_display: ((max + 1) / 2).to_string(),
        x_start_display: format_day(buckets.first()),
        x_mid_display: format_day(buckets.get(buckets.len() / 2)),
        x_end_display: format_day(buckets.last()),
        empty_message,
    }
}

fn format_day(bucket: Option<&DailyBucket>) -> String {
    bucket.map_or_else(String::new, |b| b.day.format("%b %d").to_string())
}
