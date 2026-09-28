//! The usage-anomaly row written by the `usage_anomaly` job and read by the
//! overview queue (`repositories::overview::queues`).
//!
//! Instance-wide across *users* by design: the detector compares whole-gateway
//! traffic against its own baseline, so narrowing these rows to a project would
//! claim a precision the data does not have.

#[derive(Debug, Clone)]
pub struct UsageAnomalyRow {
    pub metric: String,
    pub window_start: chrono::DateTime<chrono::Utc>,
    pub observed: i64,
    pub baseline: i64,
    pub detected_at: chrono::DateTime<chrono::Utc>,
}
