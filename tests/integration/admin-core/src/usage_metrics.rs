//! The `admin_usage_daily_rollups` recompute — idempotent by design, so
//! running it twice must not double anything.

use systemprompt_web_admin::repositories::dashboard::usage_rollups;

use crate::fixtures::{RequestSpec, insert_request, insert_user, unclaimed_email, unique};
use crate::tempdb::TempDb;

#[tokio::test]
async fn daily_rollups_recompute_idempotently() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("rollup")).await;

    insert_request(&db.pool, &RequestSpec::completed(&unique("req"), &user)).await;
    sqlx::query(
        "INSERT INTO plugin_usage_daily
            (id, date, user_id, event_type, tool_name, event_count)
         VALUES ($1, (NOW() AT TIME ZONE 'UTC')::DATE, $2, 'PostToolUse', 'Edit', 4)",
    )
    .bind(unique("pud"))
    .bind(user.as_str())
    .execute(&*db.pool)
    .await
    .expect("seed plugin_usage_daily");

    usage_rollups::upsert_daily_rollups_for_window(&db.pool, 1)
        .await
        .expect("first rollup");
    usage_rollups::upsert_daily_rollups_for_window(&db.pool, 1)
        .await
        .expect("second rollup");

    let row: (i64, i64) = sqlx::query_as(
        "SELECT tool_uses, ai_requests_count
         FROM admin_usage_daily_rollups WHERE user_id = $1",
    )
    .bind(user.as_str())
    .fetch_one(&*db.pool)
    .await
    .expect("read rollup row");

    assert_eq!(row.0, 4, "tool uses recomputed, not doubled");
    assert_eq!(row.1, 1, "one gateway request");
    db.cleanup().await;
}
