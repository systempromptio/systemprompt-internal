//! Tier B: seeds the demo dashboards with real, differing telemetry for two
//! roles (`just e2e-live-demo-seed`).
//!
//! Everything here goes through the same public wire a Claude Code session
//! uses — `/api/public/hooks/track` and `/api/public/hooks/govern` with a
//! hook-audience JWT, and `/v1/messages` through the gateway — so the rows the
//! `/admin/demo` pages read are produced by the ingestion path they claim to
//! visualise, not by INSERTs. The hook token is minted the way a non-admin can
//! actually mint one: PKCE sign-in, `POST /v1/auth/bridge/oauth-client` to
//! provision the `bridge:<user_id>` client, then a `client_credentials`
//! exchange with `audience=hook`. `admin keys issue-plugin-token` refuses a
//! non-admin and is deliberately not used.
//!
//! Environment: `E2E_BASE_URL`, `E2E_ODOO_URL` (as `live_smoke`),
//! `DEMO_SEED_DATABASE_URL` / `SYSTEMPROMPT_TEST_DATABASE_URL` for the
//! read-back assertions, `DEMO_SEED_EXPECT_PLUGIN_ID=0` to tolerate a server
//! built before the hook handler stored the JWT's `plugin_id` claim.

use base64::Engine;
use serde_json::{Value, json};

use crate::live_smoke::{
    ADMIN_LOGIN, SALES_LOGIN, env_or, require_local_stack, seed_odoo_users, sign_in, step,
};

const HOOK_PLUGIN: &str = "systemprompt-business";

struct Seat {
    login: &'static str,
    bearer: String,
    hook_token: String,
    user_id: String,
    gateway_session: String,
    sessions: Vec<String>,
}

fn claim(token: &str, key: &str) -> String {
    let payload = token.split('.').nth(1).expect("jwt has a payload segment");
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .expect("jwt payload is base64url");
    let claims: Value = serde_json::from_slice(&decoded).expect("jwt payload is json");
    claims[key]
        .as_str()
        .unwrap_or_else(|| panic!("token carries no {key} claim: {claims:#}"))
        .to_owned()
}

async fn mint_hook_token(http: &reqwest::Client, base: &str, bearer: &str) -> String {
    let client: Value = http
        .post(format!("{base}/v1/auth/bridge/oauth-client"))
        .bearer_auth(bearer)
        .json(&json!({}))
        .send()
        .await
        .expect("bridge oauth-client answers")
        .json()
        .await
        .expect("bridge oauth-client is json");
    let client_id = client["client_id"]
        .as_str()
        .unwrap_or_else(|| panic!("no bridge client provisioned: {client:#}"));
    let client_secret = client["client_secret"]
        .as_str()
        .unwrap_or_else(|| panic!("no bridge client secret: {client:#}"));

    let token: Value = http
        .post(format!("{base}/api/v1/core/oauth/token"))
        .form(&[
            ("grant_type", "client_credentials"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("scope", "hook:track hook:govern"),
            ("audience", "hook"),
            ("plugin_id", HOOK_PLUGIN),
        ])
        .send()
        .await
        .expect("token endpoint answers")
        .json()
        .await
        .expect("token response is json");
    token["access_token"]
        .as_str()
        .unwrap_or_else(|| panic!("hook token exchange failed: {token:#}"))
        .to_owned()
}

async fn track(http: &reqwest::Client, base: &str, seat: &Seat, event: Value) {
    let resp = http
        .post(format!(
            "{base}/api/public/hooks/track?plugin_id={HOOK_PLUGIN}"
        ))
        .bearer_auth(&seat.hook_token)
        .json(&event)
        .send()
        .await
        .expect("hooks/track answers");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert!(
        status.is_success(),
        "hooks/track rejected {} for {} ({status}): {body}",
        event["hook_event_name"],
        seat.login
    );
}

async fn govern(
    http: &reqwest::Client,
    base: &str,
    seat: &Seat,
    session: &str,
    tool: &str,
) -> String {
    let body: Value = http
        .post(format!(
            "{base}/api/public/hooks/govern?plugin_id={HOOK_PLUGIN}"
        ))
        .bearer_auth(&seat.hook_token)
        .json(&json!({
            "hook_event_name": "PreToolUse",
            "session_id": session,
            "cwd": "/var/www/html/systemprompt-internal",
            "transcript_path": "/tmp/demo-seed-transcript",
            "permission_mode": "default",
            "tool_name": tool,
            "tool_input": { "demo": true },
            "tool_use_id": format!("toolu_gov_{}", uuid::Uuid::new_v4().simple()),
        }))
        .send()
        .await
        .expect("hooks/govern answers")
        .json()
        .await
        .expect("hooks/govern is json");
    body["hookSpecificOutput"]["permissionDecision"]
        .as_str()
        .unwrap_or_else(|| panic!("no permissionDecision for {tool}: {body:#}"))
        .to_owned()
}

// The attribution rule the demo pages implement is same-user, same-window: an
// `ai_requests` row only lands inside a hook session's window if it is made
// between that session's events. So the gateway call goes here, between the
// Skill event and the Stop, and not as a separate pass afterwards.
async fn gateway_ping(http: &reqwest::Client, base: &str, seat: &Seat, prompt: &str) {
    let resp = http
        .post(format!("{base}/v1/messages"))
        .bearer_auth(&seat.bearer)
        .header("x-session-id", &seat.gateway_session)
        .json(&json!({
            "model": "claude-haiku-4-5-20251001",
            "max_tokens": 16,
            "messages": [{ "role": "user", "content": prompt }],
        }))
        .send()
        .await
        .expect("gateway answers");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert!(
        status.is_success(),
        "gateway /v1/messages rejected {} ({status}): {body}",
        seat.login
    );
}

fn hook_event(kind: &str, session: &str, extra: Value) -> Value {
    let mut event = json!({
        "hook_event_name": kind,
        "session_id": session,
        "cwd": "/var/www/html/systemprompt-internal",
        "transcript_path": "/tmp/demo-seed-transcript",
        "permission_mode": "default",
        "tool_use_id": format!("toolu_{}", uuid::Uuid::new_v4().simple()),
    });
    if let (Some(target), Some(fields)) = (event.as_object_mut(), extra.as_object()) {
        for (k, v) in fields {
            target.insert(k.clone(), v.clone());
        }
    }
    event
}

async fn run_session(
    http: &reqwest::Client,
    base: &str,
    seat: &mut Seat,
    skill: &str,
    mcp_tools: &[&str],
) {
    let session = uuid::Uuid::new_v4().to_string();
    seat.sessions.push(session.clone());
    track(
        http,
        base,
        seat,
        hook_event("SessionStart", &session, json!({})),
    )
    .await;
    track(
        http,
        base,
        seat,
        hook_event(
            "UserPromptSubmit",
            &session,
            json!({ "prompt": format!("run {skill}") }),
        ),
    )
    .await;
    track(
        http,
        base,
        seat,
        hook_event(
            "PostToolUse",
            &session,
            json!({
                "tool_name": "Skill",
                "tool_input": { "skill": skill },
                "tool_response": { "ok": true },
            }),
        ),
    )
    .await;
    gateway_ping(http, base, seat, "Reply with the single word OK.").await;
    for tool in mcp_tools {
        track(
            http,
            base,
            seat,
            hook_event(
                "PostToolUse",
                &session,
                json!({
                    "tool_name": tool,
                    "tool_input": { "demo": true },
                    "tool_response": { "ok": true },
                }),
            ),
        )
        .await;
    }
    track(http, base, seat, hook_event("Stop", &session, json!({}))).await;
    track(
        http,
        base,
        seat,
        hook_event("SessionEnd", &session, json!({})),
    )
    .await;
}

async fn seat_for(http: &reqwest::Client, base: &str, login: &'static str) -> Seat {
    let bearer = sign_in(http, base, login, None).await;
    let hook_token = mint_hook_token(http, base, &bearer).await;
    Seat {
        login,
        user_id: claim(&bearer, "sub"),
        gateway_session: claim(&bearer, "session_id"),
        bearer,
        hook_token,
        sessions: Vec::new(),
    }
}

fn database_url() -> String {
    std::env::var("DEMO_SEED_DATABASE_URL")
        .or_else(|_| std::env::var("SYSTEMPROMPT_TEST_DATABASE_URL"))
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect(
            "set DEMO_SEED_DATABASE_URL to the RUNNING server's database — the seed asserts on \
             the rows it just produced (`just e2e-live-demo-seed` reads it from the profile)",
        )
}

async fn assert_rows(sales: &Seat) {
    let pool = sqlx::PgPool::connect(&database_url())
        .await
        .expect("the seed connects to the server's database");

    let skills: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM plugin_usage_events \
         WHERE user_id = $1 AND tool_name = 'Skill' AND session_id = ANY($2)",
    )
    .bind(&sales.user_id)
    .bind(&sales.sessions)
    .fetch_one(&pool)
    .await
    .expect("skill events read back");
    assert!(skills > 0, "the salesperson's Skill events landed");

    let scoped: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM plugin_usage_events \
         WHERE user_id = $1 AND session_id = ANY($2) AND plugin_id = $3",
    )
    .bind(&sales.user_id)
    .bind(&sales.sessions)
    .bind(HOOK_PLUGIN)
    .fetch_one(&pool)
    .await
    .expect("plugin-scoped events read back");
    if env_or("DEMO_SEED_EXPECT_PLUGIN_ID", "1") == "0" {
        eprintln!("⚠ DEMO_SEED_EXPECT_PLUGIN_ID=0 — plugin_id rows seen: {scoped}");
    } else {
        assert!(
            scoped > 0,
            "hooks/track must store the JWT's plugin_id claim on plugin_usage_events — none of \
             the salesperson's {} sessions carry plugin_id={HOOK_PLUGIN}. Rebuild and restart the \
             server with the hook fix, or run with DEMO_SEED_EXPECT_PLUGIN_ID=0.",
            sales.sessions.len()
        );
    }

    let held: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM governance_decisions \
         WHERE user_id = $1 AND policy = 'require_approval' AND decision = 'pending'",
    )
    .bind(&sales.user_id)
    .fetch_one(&pool)
    .await
    .expect("held decisions read back");
    assert!(
        held > 0,
        "the salesperson's note_add must be HELD (require_approval/pending) in \
         governance_decisions"
    );
}

#[tokio::test]
async fn live_demo_seed_fills_the_demo_dashboards() {
    let base = env_or("E2E_BASE_URL", "http://localhost:8081");
    let odoo_url = env_or("E2E_ODOO_URL", "http://localhost:8070");
    require_local_stack(&base, &odoo_url);
    let http = reqwest::Client::new();

    step("server health");
    let health = http.get(format!("{base}/health")).send().await;
    assert!(
        health.is_ok_and(|r| r.status().is_success()),
        "no server at {base} — start one with `just start` (or set E2E_BASE_URL)"
    );

    seed_odoo_users(&http, &odoo_url).await;

    step("mint a hook token for each role (bridge client → client_credentials)");
    let mut admin = seat_for(&http, &base, ADMIN_LOGIN).await;
    let mut sales = seat_for(&http, &base, SALES_LOGIN).await;

    step("admin sessions: user report ×3, activity report ×2");
    for _ in 0..3 {
        run_session(
            &http,
            &base,
            &mut admin,
            "systemprompt-admin:admin-user-report",
            &["mcp__odoo__crm_lead_search"],
        )
        .await;
    }
    for _ in 0..2 {
        run_session(
            &http,
            &base,
            &mut admin,
            "systemprompt-admin:admin-activity-report",
            &["mcp__odoo__crm_lead_create"],
        )
        .await;
    }

    step("salesperson sessions: manage_leads ×2");
    for _ in 0..2 {
        run_session(
            &http,
            &base,
            &mut sales,
            "systemprompt-business:manage_leads",
            &["mcp__odoo__crm_lead_search"],
        )
        .await;
    }

    step("governed PreToolUse: blocked delete, held write, allowed read");
    let sales_session = sales
        .sessions
        .last()
        .cloned()
        .expect("the salesperson ran a session");
    let denied = govern(
        &http,
        &base,
        &sales,
        &sales_session,
        "mcp__odoo__crm_lead_delete",
    )
    .await;
    assert_eq!(denied, "deny", "tool_blocklist refuses a delete tool");
    let held = govern(&http, &base, &sales, &sales_session, "mcp__odoo__note_add").await;
    assert_eq!(held, "ask", "require_approval holds note_add for a human");

    let admin_session = admin
        .sessions
        .last()
        .cloned()
        .expect("the admin ran a session");
    let allowed = govern(
        &http,
        &base,
        &admin,
        &admin_session,
        "mcp__odoo__crm_lead_search",
    )
    .await;
    assert_eq!(allowed, "allow", "a read tool runs unattended");

    step("read the rows back");
    assert_rows(&sales).await;

    step("done — demo dashboards seeded for both roles");
}
