//! The server-rendered pages, driven against a database that has rows in it.
//!
//! Detail pages must render a seeded id and 404 an unknown one; list pages
//! are re-driven with rows so the row markup, not the empty state, is
//! asserted.

use axum::http::StatusCode;
use sqlx::PgPool;
use systemprompt::identifiers::UserId;

use crate::app::{App, Call};
use crate::principal::Principal;
use crate::tempdb::TempDb;
use crate::{globals, principal, seed};

// The pages cross-link, so one session carries every seeded row.
#[expect(
    clippy::struct_field_names,
    reason = "each field is named for the id column it seeds"
)]
struct Trail {
    user_id: UserId,
    session_id: String,
    context_id: String,
    trace_id: String,
    request_id: String,
}

async fn seed_trail(pool: &PgPool) -> Trail {
    let user_id_str = seed::unique("trail-user");
    let user_id =
        seed::insert_user(pool, &user_id_str, &format!("{user_id_str}@contract.test")).await;

    let session_id = seed::unique("trail-session");
    seed::insert_session(pool, &session_id, &user_id).await;

    // `ContextId` must parse as a UUID, so no `unique` prefix here.
    let context_id = uuid::Uuid::new_v4().to_string();
    seed::insert_context(
        pool,
        &context_id,
        &user_id,
        Some(&session_id),
        "Contract conversation",
    )
    .await;

    let trace_id = seed::unique("trail-trace");
    let request_id = seed::unique("trail-request");
    seed::insert_request(
        pool,
        &seed::RequestSpec {
            id: request_id.clone(),
            user_id: &user_id,
            session_id: Some(&session_id),
            trace_id: Some(&trace_id),
            context_id: Some(&context_id),
            status: "completed",
        },
    )
    .await;
    seed::insert_request(
        pool,
        &seed::RequestSpec {
            id: seed::unique("trail-request-failed"),
            user_id: &user_id,
            session_id: Some(&session_id),
            trace_id: Some(&trace_id),
            context_id: Some(&context_id),
            status: "error",
        },
    )
    .await;

    seed::insert_decision(
        pool,
        &seed::DecisionSpec {
            id: seed::unique("trail-allow"),
            user_id: &user_id,
            session_id: &session_id,
            decision: "allow",
            policy: "scope_check",
            tool_name: "Read",
        },
    )
    .await;
    seed::insert_decision(
        pool,
        &seed::DecisionSpec {
            id: seed::unique("trail-deny"),
            user_id: &user_id,
            session_id: &session_id,
            decision: "deny",
            policy: "blocklist",
            tool_name: "Bash",
        },
    )
    .await;

    seed::insert_summary(pool, &session_id, &user_id, "Contract session").await;
    seed::insert_event(pool, &user_id, &session_id, "Read").await;
    seed::insert_event(pool, &user_id, &session_id, "Bash").await;

    Trail {
        user_id,
        session_id,
        context_id,
        trace_id,
        request_id,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn seeded_detail_pages_render_the_record_and_miss_cleanly() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        eprintln!("no DATABASE_URL — skipping seeded SSR suite");
        return;
    };

    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);
    let trail = seed_trail(&db.pool).await;

    let found: [(&str, String, String); 6] = [
        (
            "the session detail page",
            format!("/admin/sessions/{}", trail.session_id),
            trail.session_id.clone(),
        ),
        (
            "the context detail page",
            format!("/admin/contexts/{}", trail.context_id),
            trail.context_id.clone(),
        ),
        (
            "the trace detail page, addressed by trace id",
            format!("/admin/traces/{}", trail.trace_id),
            "Waterfall".to_owned(),
        ),
        (
            "the trace detail page, addressed by session id",
            format!("/admin/traces/{}", trail.session_id),
            "Waterfall".to_owned(),
        ),
        (
            "the governance audit chain for a request",
            format!("/admin/requests/{}", trail.request_id),
            "Policy chain".to_owned(),
        ),
        (
            "the per-user page",
            format!("/admin/users/{}", trail.user_id.as_str()),
            trail.user_id.as_str().to_owned(),
        ),
    ];

    let mut failures = Vec::new();
    for (label, path, marker) in found {
        let (status, body) = app.call(Call::get(&path, Principal::Admin)).await;
        if status != StatusCode::OK {
            failures.push(format!(
                "  {label} -> {} (expected 200): {}",
                status.as_u16(),
                body.chars().take(240).collect::<String>()
            ));
            continue;
        }
        if !body.contains(&marker) {
            failures.push(format!(
                "  {label} -> 200 but the body never carried {marker:?} — the page rendered \
                 without the record it was asked for"
            ));
        }
    }

    let missing: [(&str, String); 5] = [
        (
            "a session id in no table",
            "/admin/sessions/no-such-session".to_owned(),
        ),
        (
            "a context id that is a well-formed UUID but matches nothing",
            format!("/admin/contexts/{}", uuid::Uuid::new_v4()),
        ),
        (
            "a context id that is not a UUID at all",
            "/admin/contexts/not-a-uuid".to_owned(),
        ),
        (
            "a trace id in no table",
            "/admin/traces/no-such-trace".to_owned(),
        ),
        (
            "a request id in no table",
            "/admin/requests/no-such-request".to_owned(),
        ),
    ];
    for (label, path) in missing {
        let (status, body) = app.call(Call::get(&path, Principal::Admin)).await;
        if status != StatusCode::NOT_FOUND {
            failures.push(format!(
                "  {label} -> {} (expected 404): {}",
                status.as_u16(),
                body.chars().take(200).collect::<String>()
            ));
        }
    }

    for path in [
        format!("/admin/sessions/{}", trail.session_id),
        format!("/admin/contexts/{}", trail.context_id),
        format!("/admin/traces/{}", trail.trace_id),
        format!("/admin/requests/{}", trail.request_id),
    ] {
        let (status, _) = app.call(Call::get(&path, Principal::NonAdmin)).await;
        if !(status == StatusCode::FORBIDDEN || status.is_redirection()) {
            failures.push(format!(
                "  {path} as a non-admin -> {} (expected a refusal)",
                status.as_u16()
            ));
        }
    }

    db.cleanup().await;
    assert!(
        failures.is_empty(),
        "{} seeded detail-page case(s) failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn seeded_list_pages_render_rows_rather_than_the_empty_state() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };

    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);
    let trail = seed_trail(&db.pool).await;

    // (label, path, marker the populated branch emits, marker it must NOT emit)
    let cases: [(&str, String, String, Option<&str>); 10] = [
        (
            "the trace explorer",
            "/admin/traces".to_owned(),
            trail.session_id.clone(),
            Some("No traces in the selected window."),
        ),
        (
            "the trace explorer filtered to denials",
            "/admin/traces?deny_only=true".to_owned(),
            trail.session_id.clone(),
            None,
        ),
        (
            "the trace explorer filtered to errors",
            "/admin/traces?error_only=true".to_owned(),
            trail.session_id.clone(),
            None,
        ),
        (
            "the trace explorer filtered by policy and decision",
            "/admin/traces?policy=blocklist&decision=deny".to_owned(),
            trail.session_id.clone(),
            None,
        ),
        (
            "the trace explorer sorted by cost",
            "/admin/traces?sort=cost&dir=asc".to_owned(),
            trail.session_id.clone(),
            None,
        ),
        (
            "the contexts list",
            "/admin/contexts".to_owned(),
            "Contract conversation".to_owned(),
            Some("No conversation context matches the selected scope and filters."),
        ),
        (
            "the contexts list grouped by user",
            "/admin/contexts?view=users".to_owned(),
            trail.user_id.as_str().to_owned(),
            Some(
                "Nobody under the selected group, project and time range has a conversation context.",
            ),
        ),
        (
            "the contexts list searched for the seeded name",
            "/admin/contexts?q=Contract".to_owned(),
            "Contract conversation".to_owned(),
            None,
        ),
        (
            "the sessions list",
            "/admin/sessions".to_owned(),
            trail.context_id.clone(),
            None,
        ),
        (
            "the roster",
            "/admin/users".to_owned(),
            trail.user_id.as_str().to_owned(),
            None,
        ),
    ];

    let mut failures = Vec::new();
    for (label, path, marker, forbidden) in cases {
        let (status, body) = app.call(Call::get(&path, Principal::Admin)).await;
        if status != StatusCode::OK {
            failures.push(format!(
                "  {label} -> {} (expected 200): {}",
                status.as_u16(),
                body.chars().take(240).collect::<String>()
            ));
            continue;
        }
        if !body.contains(&marker) {
            failures.push(format!(
                "  {label} -> 200 but never rendered {marker:?}, so the seeded row did not \
                 reach the template"
            ));
        }
        if let Some(empty_message) = forbidden
            && body.contains(empty_message)
        {
            failures.push(format!(
                "  {label} -> rendered the empty state {empty_message:?} despite having rows"
            ));
        }
    }

    db.cleanup().await;
    assert!(
        failures.is_empty(),
        "{} seeded list-page case(s) failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn analytics_pages_aggregate_the_seeded_trail() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };

    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);
    let trail = seed_trail(&db.pool).await;

    let mut failures = Vec::new();
    let paths: [(&str, String); 6] = [
        (
            "the requests log filtered to the seeded model",
            "/admin/requests?tab=log&model=claude-contract-model".to_owned(),
        ),
        (
            "the requests log filtered to failures",
            "/admin/requests?tab=log&status=error".to_owned(),
        ),
        (
            "the requests log searched for the seeded session",
            format!("/admin/requests?tab=log&q={}", trail.session_id),
        ),
        (
            "the model breakdown",
            "/admin/requests?tab=models".to_owned(),
        ),
        (
            "the provider breakdown",
            "/admin/requests?tab=providers".to_owned(),
        ),
        ("the outcome mix", "/admin/requests?tab=status".to_owned()),
    ];
    for (label, path) in paths {
        let (status, body) = app.call(Call::get(&path, Principal::Admin)).await;
        if status != StatusCode::OK {
            failures.push(format!(
                "  {label} -> {} (expected 200): {}",
                status.as_u16(),
                body.chars().take(240).collect::<String>()
            ));
        }
    }

    let (_, body) = app
        .call(Call::get("/admin/requests?tab=models", Principal::Admin))
        .await;
    if !body.contains("claude-contract-model") {
        failures.push(
            "  the model breakdown never named the seeded model — the rollup query \
             returned nothing"
                .to_owned(),
        );
    }

    db.cleanup().await;
    assert!(
        failures.is_empty(),
        "{} analytics page case(s) failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_gateway_conversation_is_readable_by_its_owner_and_invisible_to_everyone_else() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        eprintln!("no DATABASE_URL — skipping seeded SSR suite");
        return;
    };

    let credentials = principal::provision(&db.pool).await;
    let owner = credentials.non_admin_user_id.clone();

    let context_id = uuid::Uuid::new_v4().to_string();
    seed::insert_context(&db.pool, &context_id, &owner, None, "Owned conversation").await;
    let request_id = seed::unique("owned-request");
    seed::insert_request(
        &db.pool,
        &seed::RequestSpec {
            id: request_id.clone(),
            user_id: &owner,
            session_id: None,
            trace_id: None,
            context_id: Some(&context_id),
            status: "completed",
        },
    )
    .await;
    seed::insert_offered_tools(&db.pool, &request_id).await;
    sqlx::query(
        "INSERT INTO ai_request_messages (id, request_id, role, content, sequence_number)
         VALUES ($1, $2, 'user', $3, 0)",
    )
    .bind(seed::unique("owned-message"))
    .bind(&request_id)
    .bind("=== USER PROMPT ===\nrotate the deploy key")
    .execute(&*db.pool)
    .await
    .expect("seed the owner's prompt");

    let stranger_id = seed::unique("stranger");
    let stranger = seed::insert_user(
        &db.pool,
        &stranger_id,
        &format!("{stranger_id}@contract.test"),
    )
    .await;
    let stranger_context = uuid::Uuid::new_v4().to_string();
    seed::insert_context(
        &db.pool,
        &stranger_context,
        &stranger,
        None,
        "Someone else's conversation",
    )
    .await;
    seed::insert_request(
        &db.pool,
        &seed::RequestSpec {
            id: seed::unique("stranger-request"),
            user_id: &stranger,
            session_id: None,
            trace_id: None,
            context_id: Some(&stranger_context),
            status: "completed",
        },
    )
    .await;

    let app = App::new(&db.pool, credentials);
    let mut failures = Vec::new();

    let own_url = format!("/admin/history/conversations/{context_id}");
    let (status, body) = app.call(Call::get(&own_url, Principal::NonAdmin)).await;
    if status == StatusCode::OK {
        if !body.contains("rotate the deploy key") {
            failures
                .push("  the owner's page rendered without the prompt it was asked for".to_owned());
        }
        if body.contains("=== USER PROMPT ===") {
            failures.push(
                "  the gateway marker framing reached the owner's page unstripped".to_owned(),
            );
        }
    } else {
        failures.push(format!(
            "  the owner reading their own conversation -> {} (expected 200): {}",
            status.as_u16(),
            body.chars().take(240).collect::<String>()
        ));
    }

    // Answers exactly as a non-existent id, so ids cannot be enumerated.
    let cases = [
        (
            "a conversation owned by another user",
            format!("/admin/history/conversations/{stranger_context}"),
        ),
        (
            "a well-formed UUID matching no conversation",
            format!("/admin/history/conversations/{}", uuid::Uuid::new_v4()),
        ),
        (
            "an id that is not a UUID at all",
            "/admin/history/conversations/not-a-uuid".to_owned(),
        ),
    ];
    for (label, path) in cases {
        let (status, body) = app.call(Call::get(&path, Principal::NonAdmin)).await;
        if status != StatusCode::NOT_FOUND {
            failures.push(format!(
                "  {label} -> {} (expected 404): {}",
                status.as_u16(),
                body.chars().take(200).collect::<String>()
            ));
        }
    }

    let (status, body) = app.call(Call::get(&own_url, Principal::Admin)).await;
    if status != StatusCode::OK {
        failures.push(format!(
            "  an admin reading another user's conversation -> {} (expected 200)",
            status.as_u16()
        ));
    } else if !body.contains(&format!("/admin/contexts/{context_id}")) {
        failures.push("  the admin view offered no link on to the context page".to_owned());
    }

    db.cleanup().await;
    assert!(
        failures.is_empty(),
        "{} conversation-page case(s) failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
