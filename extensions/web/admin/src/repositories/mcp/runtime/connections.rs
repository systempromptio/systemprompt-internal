//! Who is attached to an MCP server right now.
//!
//! Two tables hold live attachments, one per kind of server. A proxied
//! (external) server keeps a row in `mcp_external_sessions` for every upstream
//! session the gateway is relaying, touched on every call and expiring an
//! hour after the last one. An in-process server keeps its own transport
//! session in `mcp_sessions`, stamped with the server id at creation and the
//! user once the proxy has verified the caller. Both are read here as one
//! list, so the pages can say "N connected" without caring which kind a server
//! is — the kind is carried on each row for the reader who does.
//!
//! "Connected" means unexpired. Nothing here decides whether a server is
//! alive; that is `repositories::overview::liveness::liveness_state`, which
//! the pages feed with the `last_seen` reported here.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt::identifiers::{SessionId, UserId};

#[derive(Debug, Clone)]
pub struct McpConnectionCount {
    pub server_name: String,
    pub connections: i64,
    pub distinct_users: i64,
    pub last_seen: Option<DateTime<Utc>>,
}

// Why: Unexpired attachments per server, from both session tables at once.
// The external table has no activity column of its own, but every touch
// resets its expiry to exactly one hour out, so expiry minus that hour is the
// last touch.
pub async fn list_mcp_connections(pool: &PgPool) -> Result<Vec<McpConnectionCount>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"WITH live AS (
             SELECT server_name, user_id,
                    expires_at - INTERVAL '1 hour' AS last_seen
               FROM mcp_external_sessions
              WHERE expires_at > NOW()
             UNION ALL
             SELECT mcp_server_id AS server_name, user_id, last_activity_at AS last_seen
               FROM mcp_sessions
              WHERE mcp_server_id IS NOT NULL
                AND status = 'active'
                AND expires_at > NOW()
           )
           SELECT
             server_name AS "server_name!",
             COUNT(*)::BIGINT AS "connections!",
             COUNT(DISTINCT user_id)::BIGINT AS "distinct_users!",
             MAX(last_seen) AS "last_seen?"
           FROM live
          GROUP BY server_name
          ORDER BY server_name"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| McpConnectionCount {
            server_name: r.server_name,
            connections: r.connections,
            distinct_users: r.distinct_users,
            last_seen: r.last_seen,
        })
        .collect())
}

#[derive(Debug, Clone)]
pub struct McpConnectionRow {
    pub session_id: SessionId,
    pub user_id: Option<UserId>,
    pub user_label: Option<String>,
    // Why: `external` for a relayed upstream session, `in_process` for a
    // server's own transport session — the two tables the row came from.
    pub kind: String,
    pub client_name: Option<String>,
    pub last_seen: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub connected: bool,
}

// Why: The attachments one server has held, live ones first and then the most
// recently active, so an operator sees who is on it now and who just left.
pub async fn list_mcp_connections_for_server(
    pool: &PgPool,
    server_name: &str,
    limit: i64,
) -> Result<Vec<McpConnectionRow>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"WITH attached AS (
             SELECT session_id, user_id::TEXT AS user_id,
                    'external'::TEXT AS kind,
                    NULL::TEXT AS client_name,
                    expires_at - INTERVAL '1 hour' AS last_seen,
                    expires_at
               FROM mcp_external_sessions
              WHERE server_name = $1
             UNION ALL
             SELECT session_id, user_id::TEXT,
                    'in_process'::TEXT,
                    initialize_params -> 'clientInfo' ->> 'name',
                    last_activity_at,
                    expires_at
               FROM mcp_sessions
              WHERE mcp_server_id = $1
           )
           SELECT
             a.session_id AS "session_id!: SessionId",
             a.user_id AS "user_id?: UserId",
             COALESCE(u.display_name, u.full_name, u.name, u.email) AS "user_label?",
             a.kind AS "kind!",
             a.client_name AS "client_name?",
             a.last_seen AS "last_seen?",
             a.expires_at AS "expires_at!",
             (a.expires_at > NOW()) AS "connected!"
           FROM attached a
           LEFT JOIN users u ON u.id = a.user_id
          ORDER BY (a.expires_at > NOW()) DESC, a.last_seen DESC NULLS LAST
          LIMIT $2"#,
        server_name,
        limit
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| McpConnectionRow {
            session_id: r.session_id,
            user_id: r.user_id,
            user_label: r.user_label,
            kind: r.kind,
            client_name: r.client_name,
            last_seen: r.last_seen,
            expires_at: r.expires_at,
            connected: r.connected,
        })
        .collect())
}
