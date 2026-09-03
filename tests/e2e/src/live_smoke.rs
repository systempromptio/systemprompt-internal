//! Tier B: the live-stack smoke (`just e2e-live`).
//!
//! Drives a RUNNING local stack over real HTTP — nothing in-process, nothing
//! mocked. It reuses whatever is already up (it never starts or restarts a
//! server), idempotently seeds two Odoo users, and walks Ed's checklist:
//! sign in as each role, diff the manifests, run the salesperson's chatter
//! tools through the gateway's MCP proxy.
//!
//! Environment: `E2E_BASE_URL` (default http://localhost:8081),
//! `E2E_ODOO_URL` (default http://localhost:8070), `E2E_ODOO_DB`
//! (default odoo_local), `E2E_ODOO_ADMIN` / `E2E_ODOO_ADMIN_PW`
//! (default admin/admin).

use std::collections::BTreeSet;

use base64::Engine;
use sha2::Digest;

// The real people, not invented ones. The local stack is a clone of
// production, so the accounts the tests sign in as are the accounts production
// has — a test that passes for `e2e-sales@systemprompt.local` proves the role
// mapping works for a user who exists nowhere but the test.
//
// `ed+notadmin@` is the same person deliberately holding no admin group: the
// manifest diff is only evidence of anything if one side genuinely lacks what
// the other has, and a plus-address is a real deliverable mailbox rather than a
// second identity to keep in step.
//
// Safe only because `require_local_stack` below refuses to let this file point
// at anything but a local host. It writes: it resets both passwords to
// `PASSWORD` and creates leads.
const ADMIN_LOGIN: &str = "ed@systemprompt.io";
const SALES_LOGIN: &str = "ed+notadmin@systemprompt.io";
const PASSWORD: &str = "e2e-live-password-2026";

// Refuse to run against anything that is not a local stack.
//
// This suite seeds users, rewrites their passwords, and creates CRM leads. It
// does that against the real logins now, which is only sound while the target
// is a clone. The guarantee cannot be a convention in a comment: one exported
// `E2E_ODOO_URL` pointing at the Fly app would run all of it against
// production, as the people it names. So both endpoints are checked against
// the loopback host and the test aborts before its first write otherwise.
fn require_local_stack(base: &str, odoo: &str) {
    for (label, url) in [("E2E_BASE_URL", base), ("E2E_ODOO_URL", odoo)] {
        let host = url
            .split("://")
            .nth(1)
            .unwrap_or(url)
            .split('/')
            .next()
            .unwrap_or("")
            .rsplit('@')
            .next()
            .unwrap_or("")
            .split(':')
            .next()
            .unwrap_or("");
        assert!(
            matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "::1"),
            "{label} points at {host:?}, which is not a local host. This suite \
             signs in as real accounts and writes to them; it runs against a \
             local clone only. Refusing to continue."
        );
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn step(name: &str) {
    eprintln!("▶ {name}");
}

struct Odoo {
    http: reqwest::Client,
    url: String,
    db: String,
    admin_uid: i64,
    admin_pw: String,
}

impl Odoo {
    async fn rpc(&self, service: &str, method: &str, args: serde_json::Value) -> serde_json::Value {
        let body = serde_json::json!({
            "jsonrpc": "2.0", "method": "call", "id": 1,
            "params": { "service": service, "method": method, "args": args },
        });
        let resp: serde_json::Value = self
            .http
            .post(format!("{}/jsonrpc", self.url))
            .json(&body)
            .send()
            .await
            .expect("odoo answers")
            .json()
            .await
            .expect("odoo speaks json");
        assert!(
            resp.get("error").is_none(),
            "odoo rpc {service}.{method} failed: {resp:#}"
        );
        resp["result"].clone()
    }

    async fn execute(
        &self,
        model: &str,
        method: &str,
        args: serde_json::Value,
    ) -> serde_json::Value {
        self.rpc(
            "object",
            "execute_kw",
            serde_json::json!([
                self.db,
                self.admin_uid,
                self.admin_pw,
                model,
                method,
                args,
                {}
            ]),
        )
        .await
    }

    async fn group_id(&self, module: &str, name: &str) -> i64 {
        let rows = self
            .execute(
                "ir.model.data",
                "search_read",
                serde_json::json!([[["module", "=", module], ["name", "=", name]]]),
            )
            .await;
        rows[0]["res_id"].as_i64().expect("group xml id resolves")
    }

    // Create-or-update: the seed is safe to run on every invocation.
    async fn ensure_user(&self, login: &str, group_ids: &[i64]) -> i64 {
        let existing = self
            .execute(
                "res.users",
                "search",
                serde_json::json!([[["login", "=", login]]]),
            )
            .await;
        let uid = match existing
            .as_array()
            .and_then(|a| a.first())
            .and_then(serde_json::Value::as_i64)
        {
            Some(uid) => uid,
            None => self
                .execute(
                    "res.users",
                    "create",
                    serde_json::json!([{ "name": login, "login": login, "email": login }]),
                )
                .await
                .as_i64()
                .expect("user created"),
        };
        self.execute(
            "res.users",
            "write",
            serde_json::json!([[uid], {
                "password": PASSWORD,
                "groups_id": [[6, 0, group_ids]],
            }]),
        )
        .await;
        uid
    }

    // A CRM lead owned by the salesperson — the record their chatter tools
    // are allowed to write on. Idempotent by name.
    async fn ensure_lead(&self, name: &str, owner_uid: i64) -> i64 {
        let existing = self
            .execute(
                "crm.lead",
                "search",
                serde_json::json!([[["name", "=", name]]]),
            )
            .await;
        if let Some(id) = existing
            .as_array()
            .and_then(|a| a.first())
            .and_then(serde_json::Value::as_i64)
        {
            self.execute(
                "crm.lead",
                "write",
                serde_json::json!([[id], { "user_id": owner_uid }]),
            )
            .await;
            return id;
        }
        self.execute(
            "crm.lead",
            "create",
            serde_json::json!([{ "name": name, "user_id": owner_uid }]),
        )
        .await
        .as_i64()
        .expect("lead created")
    }
}

fn pkce_pair() -> (String, String) {
    let verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(uuid::Uuid::new_v4().as_bytes())
        + &base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(uuid::Uuid::new_v4().as_bytes());
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(sha2::Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

// `resource` (RFC 8707) binds the minted token to one protected resource —
// the MCP proxy only accepts tokens minted for its own URL.
async fn sign_in(
    http: &reqwest::Client,
    base: &str,
    login: &str,
    resource: Option<&str>,
) -> String {
    let (verifier, challenge) = pkce_pair();
    let resp = http
        .post(format!("{base}/admin/auth/odoo/login"))
        .json(&serde_json::json!({
            "login": login,
            "credential": PASSWORD,
            "client_id": "marketplace-admin",
            "redirect_uri": "/admin/login",
            "code_challenge": challenge,
            "code_challenge_method": "S256",
            "resource": resource,
        }))
        .send()
        .await
        .expect("login endpoint answers");
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    assert!(
        status.is_success(),
        "odoo sign-in for {login} failed ({status}): {body:#}\nrepro: curl -X POST {base}/admin/auth/odoo/login"
    );
    let code = body["authorization_code"].as_str().expect("code minted");

    let discovery: serde_json::Value = http
        .get(format!("{base}/.well-known/oauth-authorization-server"))
        .send()
        .await
        .expect("discovery answers")
        .json()
        .await
        .expect("discovery is json");
    let token_endpoint = discovery["token_endpoint"]
        .as_str()
        .expect("token_endpoint advertised");

    let mut form: Vec<(&str, &str)> = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("client_id", "marketplace-admin"),
        ("redirect_uri", "/admin/login"),
        ("code_verifier", &verifier),
    ];
    if let Some(resource) = resource {
        form.push(("resource", resource));
    }
    let token: serde_json::Value = http
        .post(token_endpoint)
        .form(&form)
        .send()
        .await
        .expect("token endpoint answers")
        .json()
        .await
        .expect("token response is json");
    token["access_token"]
        .as_str()
        .unwrap_or_else(|| panic!("token exchange failed for {login}: {token:#}"))
        .to_owned()
}

async fn manifest_skills(http: &reqwest::Client, base: &str, bearer: &str) -> BTreeSet<String> {
    manifest_ids(http, base, bearer, "skills").await
}

async fn manifest_ids(
    http: &reqwest::Client,
    base: &str,
    bearer: &str,
    key: &str,
) -> BTreeSet<String> {
    let envelope: serde_json::Value = http
        .get(format!("{base}/v1/bridge/manifest"))
        .bearer_auth(bearer)
        .send()
        .await
        .expect("manifest answers")
        .json()
        .await
        .expect("manifest is json");
    let payload: serde_json::Value =
        serde_json::from_str(envelope["payload"].as_str().expect("payload present"))
            .expect("payload parses");
    payload[key]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|e| {
                    e.get("id")
                        .or_else(|| e.get("name"))
                        .and_then(|v| v.as_str())
                        .map(str::to_owned)
                })
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn live_stack_walks_the_two_role_journey() {
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

    step("odoo health + admin session");
    let db = env_or("E2E_ODOO_DB", "odoo_local");
    let admin_login = env_or("E2E_ODOO_ADMIN", "admin");
    let admin_pw = env_or("E2E_ODOO_ADMIN_PW", "admin");
    let mut odoo = Odoo {
        http: http.clone(),
        url: odoo_url.clone(),
        db: db.clone(),
        admin_uid: 0,
        admin_pw: admin_pw.clone(),
    };
    let uid = odoo
        .rpc(
            "common",
            "authenticate",
            serde_json::json!([db, admin_login, admin_pw, {}]),
        )
        .await;
    odoo.admin_uid = uid.as_i64().unwrap_or_else(|| {
        panic!("no Odoo at {odoo_url} or bad admin credential — `just db-up local && just odoo-local-init`")
    });

    step("seed e2e users in odoo (idempotent)");
    let group_system = odoo.group_id("base", "group_system").await;
    let group_user = odoo.group_id("base", "group_user").await;
    let group_salesman = odoo.group_id("sales_team", "group_sale_salesman").await;
    let group_sale_manager = odoo.group_id("sales_team", "group_sale_manager").await;
    // Why a Sales group on the admin, and the MANAGER one: `group_system` is
    // platform administration, not Sales access, so without it Odoo's own
    // record rules refuse a `mail.message` create on a crm.lead ("Operation:
    // create, Document type: Message") — which is the chatter round-trip below.
    // Salesman is not enough either: it sees only its own documents, and the
    // lead belongs to the salesperson. A real admin running this workspace
    // holds Sales Manager.
    odoo.ensure_user(ADMIN_LOGIN, &[group_system, group_user, group_sale_manager])
        .await;
    let sales_uid = odoo
        .ensure_user(SALES_LOGIN, &[group_user, group_salesman])
        .await;
    let lead_id = odoo.ensure_lead("E2E Demo Lead", sales_uid).await;

    step("sign in as both roles (JIT + PKCE exchange)");
    let admin_bearer = sign_in(&http, &base, ADMIN_LOGIN, None).await;
    let sales_bearer = sign_in(&http, &base, SALES_LOGIN, None).await;

    step("per-role manifest diff");
    let admin_skills = manifest_skills(&http, &base, &admin_bearer).await;
    let sales_skills = manifest_skills(&http, &base, &sales_bearer).await;
    assert!(
        admin_skills.contains("systemprompt_setup_admin"),
        "the Odoo administrator's manifest carries the admin skills — if this set has no admin \
         entries, the RUNNING server predates the plugin-cascade core change (skills inherit \
         their plugin's rule): rebuild and restart it, then re-run. admin skills: \
         {admin_skills:?}"
    );
    assert!(
        !sales_skills.contains("systemprompt_setup_admin"),
        "installing dashboards is admin-only: systemprompt_setup_admin carries no rule of its \
         own; the systemprompt-admin plugin rule must close it to the salesperson, and the \
         salesperson holds no setup skill at all: {sales_skills:?}"
    );

    step("the admin CLI server is enabled and admin-only");
    let admin_servers = manifest_ids(&http, &base, &admin_bearer, "managed_mcp_servers").await;
    let sales_servers = manifest_ids(&http, &base, &sales_bearer, "managed_mcp_servers").await;
    assert!(
        admin_servers.contains("systemprompt"),
        "services/mcp/systemprompt.yaml is enabled and granted to [admin]: {admin_servers:?}"
    );
    assert!(
        !sales_servers.contains("systemprompt"),
        "the admin CLI server must not reach the salesperson: {sales_servers:?}"
    );
    assert!(
        !sales_skills.is_empty(),
        "the salesperson still gets the workspace skills"
    );

    // Why the admin and not the salesperson: `note_add` is named in
    // `require_approval.patterns`, so a salesperson's call is HELD for a second
    // human — it blocks for `hold_seconds` and then comes back as an MRTR
    // `input_required` round, which is correct behaviour and not something this
    // smoke test can resolve (nobody is watching the approvals queue). The
    // stage carries `exempt_scopes: [admin]`, so the admin's call runs
    // unattended and gives the real chatter round-trip this step is for.
    step("chatter round-trip through the MCP proxy as the admin");
    let mcp_resource = format!("{base}/api/v1/mcp/odoo/mcp");
    let admin_mcp_bearer = sign_in(&http, &base, ADMIN_LOGIN, Some(&mcp_resource)).await;
    let note = format!("E2E live note {}", uuid::Uuid::new_v4().simple());
    let added = crate::harness::mcp::call_tool_at(
        &format!("{base}/api/v1/mcp/odoo/mcp"),
        &admin_mcp_bearer,
        "note_add",
        serde_json::json!({ "model": "crm.lead", "res_id": lead_id, "body": note }),
    )
    .await
    .expect("note_add through the proxy succeeds");
    assert!(added.contains("Note posted"), "note_add: {added}");

    let searched = crate::harness::mcp::call_tool_at(
        &format!("{base}/api/v1/mcp/odoo/mcp"),
        &admin_mcp_bearer,
        "note_search",
        serde_json::json!({ "query": "%", "limit": 50 }),
    )
    .await
    .expect("note_search through the proxy succeeds");
    assert!(
        searched.contains(&note),
        "a wildcard search finds the note just posted: {searched}"
    );

    step("done — two-role journey green");
}
