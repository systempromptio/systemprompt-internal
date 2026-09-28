//! Query-parameter coverage for the server-rendered admin pages.
//!
//! Each case asserts the status and a marker string from
//! `storage/files/admin/templates/` that only the intended branch emits. The
//! database holds almost nothing, so most markers are empty-state messages:
//! they prove the query ran rather than erroring into `unwrap_or_default()`.

use axum::http::StatusCode;

use crate::app::{App, Call};
use crate::principal::Principal;
use crate::tempdb::TempDb;
use crate::{globals, principal, seed};

struct Variant {
    path: &'static str,
    marker: &'static str,
}

const fn v(path: &'static str, marker: &'static str) -> Variant {
    Variant { path, marker }
}

const REQUESTS_TITLE: &str = "Inference Requests";
const REQUESTS_OVERVIEW: &str = "Latency distribution";
const REQUESTS_EMPTY: &str = "No inference requests match.";
// Match the data attribute the page's JS relies on, not a stylesheet class.
const REQUESTS_LOG_ROWS: &str = "data-request-row";
const TRACES_TITLE: &str = "Trace Explorer";
const TRACES_EMPTY: &str = "No traces in this window";
const CONTEXTS_TITLE: &str = "Conversation KPIs";
const CONTEXTS_EMPTY: &str = "No conversation matches the selected scope and filters.";
const CONTEXTS_USERS_EMPTY: &str =
    "Nobody under the selected group, project and time range has a conversation.";

const REQUESTS: [Variant; 28] = [
    v("/admin/requests", REQUESTS_LOG_ROWS),
    v("/admin/requests?tab=log", REQUESTS_LOG_ROWS),
    v("/admin/requests?tab=overview", REQUESTS_OVERVIEW),
    v("/admin/requests?tab=overview", "pre-flight denies"),
    v(
        "/admin/requests?tab=models",
        "attributed to the model that produced them.",
    ),
    v(
        "/admin/requests?tab=providers",
        "rolled up to the upstream provider.",
    ),
    v("/admin/requests?tab=status", "Outcome mix for the window."),
    v("/admin/requests?tab=nonsense", REQUESTS_LOG_ROWS),
    v("/admin/requests?tab=log&page=2", REQUESTS_EMPTY),
    v("/admin/requests?tab=log&page=100000", REQUESTS_EMPTY),
    // Postgres rejects a negative OFFSET; the page must clamp.
    v("/admin/requests?tab=log&page=-5", REQUESTS_LOG_ROWS),
    v(
        "/admin/requests?tab=log&model=claude-opus-5",
        r#"filter-ribbon__chip-value">claude-opus-5</span>"#,
    ),
    v(
        "/admin/requests?tab=log&provider=anthropic",
        r#"filter-ribbon__chip-value">anthropic</span>"#,
    ),
    v(
        "/admin/requests?tab=log&status=error",
        r#"filter-ribbon__chip-value">error</span>"#,
    ),
    v(
        "/admin/requests?tab=log&q=needle-xyz",
        r#"filter-ribbon__chip-value">needle-xyz</span>"#,
    ),
    v("/admin/requests?preset=15m", r#"data-window="15m""#),
    v("/admin/requests?preset=7d", r#"data-window="7d""#),
    v("/admin/requests?preset=30d", r#"data-window="30d""#),
    v("/admin/requests?preset=nonsense", REQUESTS_TITLE),
    v(
        "/admin/requests?from=2026-01-01T00:00:00Z&to=2026-02-01T00:00:00Z",
        REQUESTS_TITLE,
    ),
    v(
        "/admin/requests?tab=log&sort=cost&dir=asc",
        REQUESTS_LOG_ROWS,
    ),
    v(
        "/admin/requests?tab=log&sort=nonsense&dir=nonsense",
        REQUESTS_LOG_ROWS,
    ),
    v("/admin/requests?tab=log&tool=no-such-tool", REQUESTS_EMPTY),
    // `unrouted` is the drill-down sentinel for "no model", not a model name.
    v("/admin/requests?model=unrouted", REQUESTS_EMPTY),
    v("/admin/requests?provider=unrouted", REQUESTS_EMPTY),
    v("/admin/requests?range=7d", r#"data-window="7d""#),
    v(
        "/admin/requests?scope=project:no-such-project",
        REQUESTS_EMPTY,
    ),
    v("/admin/requests?scope=nonsense", REQUESTS_LOG_ROWS),
];

const TRACES: [Variant; 11] = [
    v("/admin/traces", TRACES_EMPTY),
    v("/admin/traces?preset=7d", r#"name="preset" value="7d""#),
    v("/admin/traces?page=3", TRACES_EMPTY),
    v("/admin/traces?page=99999", TRACES_EMPTY),
    v("/admin/traces?page=-2", TRACES_EMPTY),
    v("/admin/traces?deny_only=true", "is-active"),
    v("/admin/traces?error_only=true", "is-active"),
    v("/admin/traces?sort=cost&dir=asc", TRACES_EMPTY),
    v("/admin/traces?policy=blocklist&decision=deny", TRACES_EMPTY),
    v(
        "/admin/traces?agent_scope=global&agent_id=no-such-agent",
        TRACES_EMPTY,
    ),
    v("/admin/traces?from=not-a-date&to=not-a-date", TRACES_TITLE),
];

const CONTEXTS: [Variant; 9] = [
    v("/admin/contexts", CONTEXTS_USERS_EMPTY),
    v("/admin/contexts?view=all", CONTEXTS_EMPTY),
    v("/admin/contexts?view=users", CONTEXTS_USERS_EMPTY),
    v("/admin/contexts?q=needle-xyz", r#"value="needle-xyz""#),
    v("/admin/contexts?since=7d", r#"name="since" value="7d""#),
    v("/admin/contexts?since=30d", r#"name="since" value="30d""#),
    v("/admin/contexts?since=nonsense", CONTEXTS_TITLE),
    v("/admin/contexts?limit=1", CONTEXTS_USERS_EMPTY),
    v("/admin/contexts?limit=100000", CONTEXTS_USERS_EMPTY),
];

const OVERVIEW: [Variant; 5] = [
    v("/admin", "p50 latency"),
    v("/admin", "Most used models"),
    v("/admin", "Waiting on a person"),
    // Handlebars escapes `=` inside attribute values, so match aria-current,
    // not the pill's href.
    v("/admin?preset=7d", r#"aria-current="page">7d</a>"#),
    v("/admin?preset=30d", "vs previous 30d"),
];

const PAGES: [Variant; 6] = [
    v("/admin/users", "contract-admin@contract.test"),
    v("/admin/users?unknown_param=1&page=99", "No users match"),
    v(
        "/admin/users?filter=unassigned&sort=name&dir=asc",
        "Unassigned",
    ),
    v(
        "/admin/plugins",
        "A plugin bundles skills, MCP servers, agents and hooks",
    ),
    v("/admin/skills", "The instruction sets people invoke"),
    v("/admin/mcp", "what it is serving right now"),
];

#[tokio::test(flavor = "multi_thread")]
async fn admin_pages_render_the_branch_their_query_selects() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        eprintln!("no DATABASE_URL — skipping admin handler-variant suite");
        return;
    };

    let credentials = principal::provision(&db.pool).await;

    // Exactly one request: page 1 of the log has rows, page 2 is empty.
    let request_user = seed::insert_user(
        &db.pool,
        &seed::unique("variant-user"),
        "variant-user@contract.test",
    )
    .await;
    seed::insert_request(
        &db.pool,
        &seed::RequestSpec {
            id: seed::unique("variant-request"),
            user_id: &request_user,
            session_id: None,
            trace_id: None,
            context_id: None,
            status: "completed",
        },
    )
    .await;

    let app = App::new(&db.pool, credentials);

    let mut failures = Vec::new();
    for variant in OVERVIEW
        .iter()
        .chain(REQUESTS.iter())
        .chain(TRACES.iter())
        .chain(CONTEXTS.iter())
        .chain(PAGES.iter())
    {
        let (status, body) = app.call(Call::get(variant.path, Principal::Admin)).await;

        if status != StatusCode::OK {
            failures.push(format!(
                "  {} -> {} (expected 200){}",
                variant.path,
                status.as_u16(),
                snippet(&body)
            ));
            continue;
        }
        if !body.contains(variant.marker) {
            failures.push(format!(
                "  {} -> 200 but the body never contained {:?} — the query selected a \
                 different branch than the one it names{}",
                variant.path,
                variant.marker,
                snippet(&body)
            ));
        }
    }

    db.cleanup().await;

    assert!(
        failures.is_empty(),
        "{} page variant(s) rendered the wrong thing:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn snippet(body: &str) -> String {
    let head: String = body.chars().take(300).collect();
    format!("\n      body: {head}")
}
