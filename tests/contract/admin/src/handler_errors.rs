//! The failure half of the admin HTTP contract.
//!
//! Requests that are wrong on purpose: unknown ids, malformed payloads,
//! missing credentials, nonsense query strings. Where the exact code is a
//! judgement call the case asserts the weaker property (`ClientError`,
//! `NotServerError`) rather than pinning a code nobody chose.

use axum::http::StatusCode;

use crate::app::{ADMIN_API_PREFIX, App, Call};
use crate::principal::Principal;
use crate::tempdb::TempDb;
use crate::{globals, principal};

#[derive(Clone, Copy)]
enum Expect {
    Status(StatusCode),
    // The API answers 401/403; the SSR plane redirects to sign-in.
    Refused,
    // axum's extractor, not the handler, picks between 400 and 422.
    ClientError,
    NotServerError,
}

impl Expect {
    fn accepts(self, status: StatusCode) -> bool {
        match self {
            Self::Status(want) => status == want,
            Self::Refused => {
                matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
                    || status.is_redirection()
            },
            Self::ClientError => status.is_client_error(),
            Self::NotServerError => !status.is_server_error(),
        }
    }

    fn describe(self) -> String {
        match self {
            Self::Status(want) => want.as_u16().to_string(),
            Self::Refused => "401, 403, or a redirect".to_owned(),
            Self::ClientError => "a 4xx".to_owned(),
            Self::NotServerError => "anything but a 5xx".to_owned(),
        }
    }
}

struct Case {
    method: &'static str,
    path: &'static str,
    principal: Principal,
    content_type: Option<&'static str>,
    body: Option<&'static str>,
    expect: Expect,
    // Stops an unrelated 400 from passing as the intended validation 400.
    marker: Option<&'static str>,
}

const fn get(path: &'static str, expect: Expect, marker: Option<&'static str>) -> Case {
    Case {
        method: "get",
        path,
        principal: Principal::Admin,
        content_type: None,
        body: None,
        expect,
        marker,
    }
}

const fn json(
    method: &'static str,
    path: &'static str,
    body: &'static str,
    expect: Expect,
    marker: Option<&'static str>,
) -> Case {
    Case {
        method,
        path,
        principal: Principal::Admin,
        content_type: Some("application/json"),
        body: Some(body),
        expect,
        marker,
    }
}

const fn anon(method: &'static str, path: &'static str, body: &'static str) -> Case {
    Case {
        method,
        path,
        principal: Principal::Anonymous,
        content_type: Some("application/json"),
        body: Some(body),
        expect: Expect::Refused,
        marker: None,
    }
}

const fn non_admin(method: &'static str, path: &'static str, body: &'static str) -> Case {
    Case {
        method,
        path,
        principal: Principal::NonAdmin,
        content_type: Some("application/json"),
        body: Some(body),
        expect: Expect::Refused,
        marker: None,
    }
}

const OK: StatusCode = StatusCode::OK;
const BAD_REQUEST: StatusCode = StatusCode::BAD_REQUEST;
const NOT_FOUND: StatusCode = StatusCode::NOT_FOUND;
const UNAUTHORIZED: StatusCode = StatusCode::UNAUTHORIZED;
const UNSUPPORTED_MEDIA: StatusCode = StatusCode::UNSUPPORTED_MEDIA_TYPE;
const UNPROCESSABLE: StatusCode = StatusCode::UNPROCESSABLE_ENTITY;

const UNKNOWN_ID: [Case; 16] = [
    get(
        "/admin/contexts/no-such-context",
        Expect::Status(NOT_FOUND),
        Some("No conversation, AI request, or message rows match that context id."),
    ),
    get(
        "/admin/requests/no-such-request",
        Expect::Status(NOT_FOUND),
        Some("No audit chain found for that id."),
    ),
    get(
        "/admin/sessions/no-such-session",
        Expect::Status(NOT_FOUND),
        Some("No AI requests, contexts, or transcript rows match that session id."),
    ),
    get(
        "/admin/traces/no-such-trace",
        Expect::Status(NOT_FOUND),
        Some("No spans found for that session or trace id."),
    ),
    get(
        "/admin/plugins/no-such-plugin",
        Expect::Status(NOT_FOUND),
        Some("No such plugin."),
    ),
    get(
        "/admin/skills/no-such-skill",
        Expect::Status(NOT_FOUND),
        Some("No such skill."),
    ),
    get(
        "/admin/mcp/no-such-server",
        Expect::Status(NOT_FOUND),
        Some("No such MCP server."),
    ),
    get(
        "/admin/groups/no-such-group",
        Expect::Status(NOT_FOUND),
        Some("No such group."),
    ),
    get(
        "/admin/projects/no-such-project",
        Expect::Status(NOT_FOUND),
        Some("No such project."),
    ),
    get(
        "/admin/marketplaces/no-such-marketplace",
        Expect::Status(NOT_FOUND),
        None,
    ),
    get("/admin/users/no-such-user", Expect::Status(NOT_FOUND), None),
    get(
        "/admin/api/chain/no-such-chain",
        Expect::Status(NOT_FOUND),
        None,
    ),
    get(
        "/admin/governance/decisions/no-such-decision",
        Expect::Status(NOT_FOUND),
        None,
    ),
    get(
        "/api/public/admin/users/no-such-user/detail",
        Expect::Status(NOT_FOUND),
        Some("User not found"),
    ),
    get(
        "/api/public/admin/agents/no-such-agent",
        Expect::Status(NOT_FOUND),
        Some("Agent not found"),
    ),
    get(
        "/api/public/admin/users/no-such-user/usage",
        Expect::Status(OK),
        Some("\"events\""),
    ),
];

// axum fixes the extractor codes: unparseable JSON 400, wrong shape 422, no
// `content-type` 415. The rest is handler validation, asserted by marker.
const MALFORMED: [Case; 13] = [
    json(
        "post",
        "/api/public/admin/users",
        "{ this is not json",
        Expect::Status(BAD_REQUEST),
        None,
    ),
    json(
        "post",
        "/api/public/admin/users",
        r#"{"name": 5}"#,
        Expect::Status(UNPROCESSABLE),
        None,
    ),
    json(
        "put",
        "/api/public/admin/users/no-such-user",
        "]]]",
        Expect::Status(BAD_REQUEST),
        None,
    ),
    json(
        "post",
        "/api/public/admin/gateway/routes",
        "{{{",
        Expect::Status(BAD_REQUEST),
        None,
    ),
    json(
        "post",
        "/api/public/admin/access-control/entity/not-an-entity-kind/x/rules",
        r#"{"rule_type": "user", "rule_value": "someone", "access": "allow"}"#,
        Expect::Status(BAD_REQUEST),
        Some("invalid entity_type"),
    ),
    json(
        "put",
        "/api/public/admin/access-control/entity/not-an-entity-kind/x",
        "{}",
        Expect::ClientError,
        None,
    ),
    json(
        "patch",
        "/api/public/admin/access-control/entity/not-an-entity-kind/x/default",
        r#"{"default_included": true}"#,
        Expect::Status(BAD_REQUEST),
        Some("invalid entity_type"),
    ),
    json(
        "post",
        "/api/public/admin/access-control/entity/skill/some-skill/rules",
        r#"{"rule_type": "organization", "rule_value": "acme", "access": "allow"}"#,
        Expect::Status(BAD_REQUEST),
        Some("invalid rule_type"),
    ),
    json(
        "post",
        "/api/public/admin/access-control/entity/skill/some-skill/rules",
        r#"{"rule_type": "user", "rule_value": "someone", "access": "maybe"}"#,
        Expect::Status(BAD_REQUEST),
        Some("invalid access"),
    ),
    json(
        "post",
        "/api/public/admin/access-control/entity/skill/some-skill/rules",
        r#"{"rule_type": "user", "rule_value": "", "access": "allow"}"#,
        Expect::Status(BAD_REQUEST),
        Some("rule_value required"),
    ),
    json(
        "post",
        "/api/public/admin/access-control/bulk-template",
        r#"{"entity_type": "not-an-entity-kind", "subject_type": "user",
            "subject_value": "someone", "action": "allow"}"#,
        Expect::Status(BAD_REQUEST),
        Some("invalid entity_type"),
    ),
    json(
        "post",
        "/api/public/admin/access-control/bulk-template",
        r#"{"entity_type": "skill", "subject_type": "organization",
            "subject_value": "acme", "action": "allow"}"#,
        Expect::Status(BAD_REQUEST),
        Some("invalid subject_type"),
    ),
    json(
        "post",
        "/api/public/admin/users/no-such-user/share-token",
        "{}",
        Expect::Status(NOT_FOUND),
        Some("User not found"),
    ),
];

// Claude Code reads a hook error status as "hook unavailable" and lets the call
// through, so `/hooks/govern` answers 200 with a decision.
const HOOKS: [Case; 6] = [
    json(
        "post",
        "/hooks/track",
        "{ not json",
        Expect::Status(BAD_REQUEST),
        None,
    ),
    json(
        "post",
        "/hooks/track",
        r#"{"hook_event_name": "Stop"}"#,
        Expect::Status(UNAUTHORIZED),
        None,
    ),
    json(
        "post",
        "/govern/authz",
        "{ not json",
        Expect::Status(BAD_REQUEST),
        None,
    ),
    json(
        "post",
        "/govern/authz",
        "{}",
        Expect::Status(UNPROCESSABLE),
        None,
    ),
    json(
        "post",
        "/hooks/govern",
        "{ not json",
        Expect::Status(BAD_REQUEST),
        None,
    ),
    json("post", "/hooks/govern", "{}", Expect::Status(OK), None),
];

const UNAUTHENTICATED: [Case; 10] = [
    anon("post", "/api/public/admin/users", "{}"),
    anon("put", "/api/public/admin/users/someone", "{}"),
    anon("delete", "/api/public/admin/users/someone", "{}"),
    anon("patch", "/api/public/admin/gateway", "{}"),
    anon("post", "/api/public/admin/gateway/routes", "{}"),
    anon("put", "/api/public/admin/access-control/bulk", "{}"),
    anon("post", "/api/public/admin/management/devices", "{}"),
    anon("post", "/admin/devices/pats", "{}"),
    non_admin("post", "/api/public/admin/users", "{}"),
    non_admin("post", "/admin/devices/pats", "{}"),
];

const NO_CONTENT_TYPE: [Case; 2] = [
    Case {
        method: "post",
        path: "/api/public/admin/users",
        principal: Principal::Admin,
        content_type: None,
        body: Some("{}"),
        expect: Expect::Status(UNSUPPORTED_MEDIA),
        marker: None,
    },
    Case {
        method: "post",
        path: "/hooks/govern",
        principal: Principal::Admin,
        content_type: None,
        body: Some("{}"),
        expect: Expect::Status(UNSUPPORTED_MEDIA),
        marker: None,
    },
];

const BAD_QUERY: [Case; 13] = [
    get(
        "/admin/requests?page=not-a-number",
        Expect::Status(BAD_REQUEST),
        None,
    ),
    get(
        "/admin/requests?page=99999999999999999999",
        Expect::Status(BAD_REQUEST),
        None,
    ),
    get(
        "/admin/traces?page=not-a-number",
        Expect::Status(BAD_REQUEST),
        None,
    ),
    get(
        "/admin/contexts?page=not-a-number",
        Expect::Status(BAD_REQUEST),
        None,
    ),
    get(
        "/api/public/admin/events?limit=not-a-number",
        Expect::Status(BAD_REQUEST),
        None,
    ),
    get(
        "/api/public/admin/users/search?limit=not-a-number",
        Expect::Status(BAD_REQUEST),
        None,
    ),
    // Postgres rejects a negative LIMIT; `list_events` clamps to `1..=500`.
    // The echoed values catch an endpoint that stopped clamping altogether.
    get(
        "/api/public/admin/events?limit=-1",
        Expect::Status(OK),
        Some(r#""limit":1,"offset":0"#),
    ),
    get(
        "/api/public/admin/events?limit=-1&offset=-1",
        Expect::Status(OK),
        Some(r#""limit":1,"offset":0"#),
    ),
    get(
        "/api/public/admin/events?limit=999999999",
        Expect::Status(OK),
        Some(r#""limit":500,"offset":0"#),
    ),
    get(
        "/admin/requests?from=not-a-date&to=also-not-a-date",
        Expect::NotServerError,
        None,
    ),
    get(
        "/admin/reports/customer?month=99999-99",
        Expect::NotServerError,
        None,
    ),
    get(LONG_SEARCH_REQUESTS, Expect::NotServerError, None),
    get(LONG_SEARCH_CONTEXTS, Expect::NotServerError, None),
];

const LONG_SEARCH_REQUESTS: &str = concat!(
    "/admin/requests?tab=log&q=",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
);

const LONG_SEARCH_CONTEXTS: &str = concat!(
    "/admin/contexts?q=",
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
);

#[tokio::test(flavor = "multi_thread")]
async fn admin_routes_refuse_malformed_requests_without_faulting() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        eprintln!("no DATABASE_URL — skipping admin error-path suite");
        return;
    };

    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);

    // A moved prefix would turn every API case into a vacuous 404.
    assert_eq!(
        ADMIN_API_PREFIX, "/api/public/admin",
        "the API cases below spell the mount prefix out; update them together"
    );

    let mut failures = Vec::new();
    for case in UNKNOWN_ID
        .iter()
        .chain(MALFORMED.iter())
        .chain(HOOKS.iter())
        .chain(UNAUTHENTICATED.iter())
        .chain(NO_CONTENT_TYPE.iter())
        .chain(BAD_QUERY.iter())
    {
        let (status, body) = app
            .call(Call {
                method: case.method,
                path: case.path,
                principal: case.principal,
                content_type: case.content_type,
                body: case.body,
            })
            .await;

        if !case.expect.accepts(status) {
            failures.push(report(case, status, &body, &case.expect.describe()));
            continue;
        }
        if let Some(marker) = case.marker
            && !body.contains(marker)
        {
            failures.push(report(
                case,
                status,
                &body,
                "a body naming the reason it was refused",
            ));
        }
    }

    db.cleanup().await;

    assert!(
        failures.is_empty(),
        "{} error-path case(s) answered wrongly:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn report(case: &Case, status: StatusCode, body: &str, wanted: &str) -> String {
    let head: String = body.chars().take(300).collect();
    format!(
        "  {} {} [{}] -> {} : expected {wanted}\n      body: {head}",
        case.method.to_uppercase(),
        case.path,
        case.principal.label(),
        status.as_u16()
    )
}
