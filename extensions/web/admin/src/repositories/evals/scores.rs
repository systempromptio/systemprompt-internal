//! Judged-score read models for the Evals page.
//!
//! The counterpart to [`super::distribution`]: that module describes what went
//! through the gateway, this one describes what the judge thought of it.

use serde::Serialize;
use sqlx::PgPool;

use crate::util::time_range::TimeRange;

/// Aggregate score picture for the window, over whatever has been judged so
/// far.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct EvalScoreSummary {
    pub scored_count: i64,
    pub mean_score: f64,
    pub pass_count: i64,
    pub partial_count: i64,
    pub fail_count: i64,
    pub flagged_count: i64,
    pub judge_cost_microdollars: i64,
}

pub async fn get_eval_score_summary(
    pool: &PgPool,
    range: TimeRange,
) -> Result<EvalScoreSummary, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT
            COUNT(*)::bigint AS "scored_count!",
            COALESCE(AVG(overall_score), 0)::float8 AS "mean_score!",
            COUNT(*) FILTER (WHERE verdict = 'pass')::bigint AS "pass_count!",
            COUNT(*) FILTER (WHERE verdict = 'partial')::bigint AS "partial_count!",
            COUNT(*) FILTER (WHERE verdict = 'fail')::bigint AS "fail_count!",
            COUNT(*) FILTER (WHERE cardinality(flags) > 0)::bigint AS "flagged_count!",
            COALESCE(SUM(judge_cost_microdollars), 0)::bigint AS "judge_cost!"
          FROM eval_results
          WHERE created_at >= $1 AND created_at < $2"#,
        range.from,
        range.to,
    )
    .fetch_one(pool)
    .await?;

    Ok(EvalScoreSummary {
        scored_count: row.scored_count,
        mean_score: row.mean_score,
        pass_count: row.pass_count,
        partial_count: row.partial_count,
        fail_count: row.fail_count,
        flagged_count: row.flagged_count,
        judge_cost_microdollars: row.judge_cost,
    })
}

/// Mean judged score per model, so the distribution table can show quality
/// beside cost and latency.
#[derive(Debug, Clone, Serialize)]
pub struct ModelScoreRow {
    pub model: String,
    pub scored_count: i64,
    pub mean_score: f64,
    pub fail_count: i64,
}

pub async fn list_model_scores(
    pool: &PgPool,
    range: TimeRange,
) -> Result<Vec<ModelScoreRow>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT
            model AS "model!",
            COUNT(*)::bigint AS "scored_count!",
            COALESCE(AVG(overall_score), 0)::float8 AS "mean_score!",
            COUNT(*) FILTER (WHERE verdict = 'fail')::bigint AS "fail_count!"
          FROM eval_results
          WHERE created_at >= $1 AND created_at < $2
          GROUP BY model"#,
        range.from,
        range.to,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ModelScoreRow {
            model: r.model,
            scored_count: r.scored_count,
            mean_score: r.mean_score,
            fail_count: r.fail_count,
        })
        .collect())
}

/// Per-model win rate across pairwise comparisons in the window.
#[derive(Debug, Clone, Serialize)]
pub struct ModelWinRateRow {
    pub model: String,
    pub comparisons: i64,
    pub wins: i64,
    pub losses: i64,
    pub ties: i64,
}

pub async fn list_model_win_rates(
    pool: &PgPool,
    range: TimeRange,
) -> Result<Vec<ModelWinRateRow>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"WITH sides AS (
            SELECT model_a AS model,
                   CASE winner WHEN 'a' THEN 'win' WHEN 'b' THEN 'loss' ELSE 'tie' END AS outcome
            FROM eval_pairs
            WHERE created_at >= $1 AND created_at < $2
            UNION ALL
            SELECT model_b AS model,
                   CASE winner WHEN 'b' THEN 'win' WHEN 'a' THEN 'loss' ELSE 'tie' END AS outcome
            FROM eval_pairs
            WHERE created_at >= $1 AND created_at < $2
        )
        SELECT
            model AS "model!",
            COUNT(*)::bigint AS "comparisons!",
            COUNT(*) FILTER (WHERE outcome = 'win')::bigint AS "wins!",
            COUNT(*) FILTER (WHERE outcome = 'loss')::bigint AS "losses!",
            COUNT(*) FILTER (WHERE outcome = 'tie')::bigint AS "ties!"
        FROM sides
        GROUP BY model
        ORDER BY COUNT(*) FILTER (WHERE outcome = 'win') DESC"#,
        range.from,
        range.to,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ModelWinRateRow {
            model: r.model,
            comparisons: r.comparisons,
            wins: r.wins,
            losses: r.losses,
            ties: r.ties,
        })
        .collect())
}
