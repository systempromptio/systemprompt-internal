//! The session registry: what a live agent session is and where it is working.
//!
//! `plugin_session_summaries` counts what a session did; these writes record
//! where it is doing it (`cwd`, `workspace`, `git_branch`), whether it is still
//! alive (`last_event_at`), what it is doing right now (`current_activity`),
//! and what it has spent so far (`live_cost_microdollars`, `context_pct`).
//!
//! `handle` is the address other people and agents use to reach a session. It
//! is derived from the workspace rather than the branch: a handle that changed
//! on `git switch` would break every reference to it mid-conversation.

use sqlx::PgPool;
use systemprompt::identifiers::SessionId;

const MAX_HANDLE_ATTEMPTS: u8 = 20;

#[must_use]
pub fn derive_workspace(cwd: &str) -> Option<String> {
    let trimmed = cwd.trim().trim_end_matches('/');
    let segment = trimmed.rsplit('/').find(|s| !s.is_empty())?;
    let sanitized = sanitize_segment(segment);
    (!sanitized.is_empty()).then_some(sanitized)
}

fn sanitize_segment(segment: &str) -> String {
    segment
        .chars()
        .filter_map(|c| match c {
            'a'..='z' | '0'..='9' | '-' | '_' | '.' => Some(c),
            'A'..='Z' => Some(c.to_ascii_lowercase()),
            ' ' => Some('-'),
            _ => None,
        })
        .collect::<String>()
        .trim_matches(['-', '.', '_'])
        .to_owned()
}

fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.is_unique_violation())
}

// Why: uniqueness is enforced by a partial index over live sessions, so this
// races against other sessions claiming the same base. Retrying on conflict is
// correct where read-then-write would not be.
pub async fn assign_session_handle(pool: &PgPool, session_id: &SessionId, base: &str) {
    if base.is_empty() {
        return;
    }

    for suffix in 1..=MAX_HANDLE_ATTEMPTS {
        let candidate = if suffix == 1 {
            base.to_owned()
        } else {
            format!("{base}#{suffix}")
        };

        let result = sqlx::query!(
            r"UPDATE plugin_session_summaries
              SET handle = $2, updated_at = NOW()
              WHERE session_id = $1 AND handle IS NULL",
            session_id.as_str(),
            candidate,
        )
        .execute(pool)
        .await;

        match result {
            Ok(_) => return,
            Err(ref e) if is_unique_violation(e) => (),
            Err(e) => {
                tracing::warn!(error = %e, session_id = %session_id.as_str(), "Failed to assign session handle");
                return;
            },
        }
    }

    tracing::warn!(
        base = %base,
        session_id = %session_id.as_str(),
        "Exhausted session handle suffixes"
    );
}

pub async fn update_session_activity(pool: &PgPool, session_id: &SessionId, activity: &str) {
    if activity.is_empty() {
        return;
    }

    let result = sqlx::query!(
        r"UPDATE plugin_session_summaries
          SET current_activity = $2, updated_at = NOW()
          WHERE session_id = $1",
        session_id.as_str(),
        activity,
    )
    .execute(pool)
    .await;

    if let Err(e) = result {
        tracing::warn!(error = %e, "Failed to update session activity");
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StatuslineParams<'a> {
    pub pool: &'a PgPool,
    pub session_id: &'a SessionId,
    pub model: Option<&'a str>,
    pub live_cost_microdollars: Option<i64>,
    pub context_pct: Option<i16>,
}

pub async fn update_session_statusline(params: &StatuslineParams<'_>) {
    let result = sqlx::query!(
        r"UPDATE plugin_session_summaries
          SET model = COALESCE($2, model),
              live_cost_microdollars = COALESCE($3, live_cost_microdollars),
              context_pct = COALESCE($4, context_pct),
              last_event_at = NOW(),
              updated_at = NOW()
          WHERE session_id = $1",
        params.session_id.as_str(),
        params.model,
        params.live_cost_microdollars,
        params.context_pct,
    )
    .execute(params.pool)
    .await;

    if let Err(e) = result {
        tracing::warn!(error = %e, "Failed to update session statusline");
    }
}
