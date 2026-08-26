//! Boot the full production router in-process against a throwaway database.
//!
//! Order matters and is the same dance the server does at startup, minus the
//! process manager: create the database and install schemas, install the
//! fixture profile whose `database_url` names that database, seed the system
//! admin the context builder insists on, then let `AppContextBuilder::build`
//! assemble the same `AppContext` production runs with — including the
//! inventory-registered role-aware marketplace filter from
//! `systemprompt-web-admin` — and hand it to `setup_api_server`.
//!
//! `paths.services` points at this checkout's real `services/` tree, so the
//! marketplaces, plugins, skills, and access-control YAML the assertions read
//! are the shipped ones, not fixtures.

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use sqlx::PgPool;
use systemprompt::api::services::server::setup_api_server;
use systemprompt::identifiers::{SessionId, UserId};
use systemprompt::system::AppContextBuilder;
use systemprompt_security::{AdminTokenParams, JwtService};
use tower::ServiceExt;

use super::db::TempDb;

const FIXTURE_PROFILE: &str = include_str!("../../fixtures/profile.yaml");
const MASTER_KEY_HEX: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

pub struct Stack {
    pub router: Router,
    pub db: TempDb,
    pub odoo: super::odoo_mock::OdooMock,
    pub admin_token: String,
    pub user_token: String,
}

fn repo_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate sits two levels below the repository root")
        .to_path_buf()
}

pub fn profile_path() -> PathBuf {
    // Per-process: nextest runs tests as parallel processes, each with its own
    // throwaway database URL baked into the secrets file.
    repo_root().join(format!("tests/target/e2e-profile-{}", std::process::id()))
}

fn install_profile(database_url: &str, odoo_url: &str, govern_port: u16) {
    let root = repo_root();
    let dir = profile_path();
    std::fs::create_dir_all(&dir).expect("create fixture profile directory");
    let yaml = FIXTURE_PROFILE
        .replace("__REPO__", &root.to_string_lossy())
        .replace("__PROFILE_DIR__", &dir.to_string_lossy())
        .replace("__GOVERN_PORT__", &govern_port.to_string());
    std::fs::write(dir.join("profile.yaml"), yaml).expect("write fixture profile");
    // odoo_url / odoo_db ride along as custom secret keys — the same channel
    // `just setup-local` uses — so the Odoo login handler resolves the mock
    // without touching process env.
    let secrets = format!(
        r#"{{
  "database_url": "{database_url}",
  "oauth_at_rest_pepper": "e2e-suite-pepper-not-a-real-secret",
  "manifest_signing_secret_seed": "ZTJlLXN1aXRlLXNlZWQtbm90LXJlYWwtMDAwMDAwMDA=",
  "encryption_master_key": "{MASTER_KEY_HEX}",
  "odoo_url": "{odoo_url}",
  "odoo_db": "e2e_odoo"
}}"#
    );
    std::fs::write(dir.join("secrets.json"), secrets).expect("write fixture secrets");

    systemprompt::config::ProfileBootstrap::init_from_path(&dir.join("profile.yaml"))
        .expect("initialise the e2e fixture profile");
    systemprompt::config::SecretsBootstrap::try_init().expect("load the fixture secrets");
    systemprompt::config::try_init_config().expect("build config from the fixture profile");

    // Why: the key is written to the profile's signing_key_path AND installed
    // in-process — a spawned MCP subprocess bootstraps from the same profile
    // and must validate the tokens this process mints.
    let key = systemprompt_security::keys::RsaSigningKey::generate()
        .expect("generate an ephemeral RSA signing key");
    key.write_pem_file(&dir.join("signing_key.pem"))
        .expect("write the signing key beside the profile");
    systemprompt_security::keys::authority::install_for_test(key);
}

fn jwt_issuer() -> String {
    systemprompt::models::Config::get()
        .expect("config installed")
        .jwt_issuer
        .clone()
}

async fn seed_user(pool: &PgPool, name: &str, email: &str, roles: &[&str]) -> UserId {
    let user_id = UserId::new(format!("{name}-{}", uuid::Uuid::new_v4().simple()));
    sqlx::query(
        "INSERT INTO users (id, name, email, roles, email_verified, status)
         VALUES ($1, $2, $3, $4, true, 'active')",
    )
    .bind(user_id.as_str())
    .bind(name)
    .bind(email)
    .bind(roles.iter().map(|r| (*r).to_owned()).collect::<Vec<_>>())
    .execute(pool)
    .await
    .expect("seed user");
    user_id
}

// Why: the gateway's session attestation rejects any JWT whose session_id has
// no live `user_sessions` row — a minted token alone is "missing or revoked".
async fn mint_token(pool: &PgPool, user_id: &UserId, email: &str) -> String {
    let session_id = SessionId::new(uuid::Uuid::new_v4().to_string());
    sqlx::query(
        "INSERT INTO user_sessions (session_id, user_id, session_source) VALUES ($1, $2, 'bridge')",
    )
    .bind(session_id.as_str())
    .bind(user_id.as_str())
    .execute(pool)
    .await
    .expect("seed user session");
    JwtService::generate_admin_token(&AdminTokenParams {
        user_id,
        session_id: &session_id,
        email,
        issuer: &jwt_issuer(),
        duration: chrono::Duration::hours(1),
        client_id: None,
    })
    .expect("mint a session token")
    .as_str()
    .to_owned()
}

async fn reapply_seeds(pool: &Arc<PgPool>) {
    let database =
        systemprompt::database::Database::from_pools(Arc::clone(pool), Some(Arc::clone(pool)));
    let registry =
        systemprompt::ExtensionRegistry::discover().expect("discover extension registrations");
    systemprompt::database::install_extension_schemas(&registry, database.write())
        .await
        .expect("re-apply extension seeds after provisioning the admin");
}

impl Stack {
    pub async fn create() -> Option<Self> {
        let db = TempDb::create().await?;
        let odoo = super::odoo_mock::OdooMock::start().await;
        // Why: the profile's authz hook URL must be live before any config is
        // built, so the port is reserved first and the assembled router is
        // served on it below — a spawned MCP subprocess fail-closes on an
        // unreachable hook.
        let govern_listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("reserve the governance hook port");
        let govern_port = govern_listener
            .local_addr()
            .expect("read the governance hook port")
            .port();
        install_profile(&db.url, &odoo.url(), govern_port);

        // Why: `AppContextBuilder::build` resolves the profile's system_admin
        // by name and refuses to boot without an active admin row — the same
        // startup requirement the real server has.
        let admin_id = seed_user(&db.pool, "admin", "e2e-admin@e2e.test", &["admin", "user"]).await;
        let user_id = seed_user(&db.pool, "e2e-user", "e2e-user@e2e.test", &["user"]).await;
        reapply_seeds(&db.pool).await;

        // Why: the manifest filter resolves grants from access_control_rules,
        // which the governance bootstrap job ingests from roles.yaml on every
        // server start. No job scheduler runs here, so ingest directly — the
        // same loader, the same shipped YAML.
        systemprompt_web_admin::repositories::config::acl_yaml_loader::load_from_yaml(
            &db.pool,
            &repo_root().join("services"),
        )
        .await
        .expect("ingest services/access-control into the throwaway database");

        let admin_token = mint_token(&db.pool, &admin_id, "e2e-admin@e2e.test").await;
        let user_token = mint_token(&db.pool, &user_id, "e2e-user@e2e.test").await;

        let ctx = Arc::new(
            AppContextBuilder::new()
                .build()
                .await
                .expect("assemble the production AppContext"),
        );
        let router = setup_api_server(&ctx, None).expect("assemble the full API router");

        govern_listener
            .set_nonblocking(true)
            .expect("switch the governance hook listener to non-blocking");
        let hook_listener = tokio::net::TcpListener::from_std(govern_listener)
            .expect("adopt the governance hook listener");
        let hook_router = router.clone();
        tokio::spawn(async move {
            let _ = axum::serve(hook_listener, hook_router).await;
        });

        Some(Self {
            router,
            db,
            odoo,
            admin_token,
            user_token,
        })
    }

    pub async fn send(
        &self,
        method: &str,
        path: &str,
        bearer: Option<&str>,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, String) {
        let mut req = Request::builder().method(method).uri(path);
        if let Some(token) = bearer {
            req = req.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let req = match body {
            Some(json) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json.to_string())),
            None => req.body(Body::empty()),
        }
        .expect("build request");

        let response = self
            .router
            .clone()
            .oneshot(req)
            .await
            .expect("router answers");
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("read body")
            .to_bytes();
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    // Drive the real Odoo sign-in endpoint against the wiremock Odoo, using
    // the seeded `marketplace-admin` OAuth client.
    pub async fn odoo_login(&self, login: &str, credential: &str) -> (StatusCode, String) {
        self.send(
            "POST",
            "/admin/auth/odoo/login",
            None,
            Some(serde_json::json!({
                "login": login,
                "credential": credential,
                "client_id": "marketplace-admin",
                "redirect_uri": "/admin/login",
                "code_challenge": "e2e-code-challenge-not-real",
                "code_challenge_method": "S256",
            })),
        )
        .await
    }

    // A session bearer for an already-provisioned user, looked up by email.
    pub async fn token_for_email(&self, email: &str) -> String {
        let user_id: String = sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
            .bind(email)
            .fetch_one(&*self.db.pool)
            .await
            .expect("the user exists");
        mint_token(&self.db.pool, &UserId::new(user_id), email).await
    }

    // The decoded SignedManifest payload for one bearer. The envelope carries
    // the manifest as a raw JSON string under `payload`.
    pub async fn manifest(&self, bearer: &str) -> serde_json::Value {
        let (status, json) = self.get_json("/v1/bridge/manifest", Some(bearer)).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "manifest fetch failed: {}",
            serde_json::to_string(&json).unwrap_or_default()
        );
        let payload = json["payload"]
            .as_str()
            .expect("envelope carries a payload string");
        serde_json::from_str(payload).expect("payload is the SignedManifest JSON")
    }

    pub async fn get_json(
        &self,
        path: &str,
        bearer: Option<&str>,
    ) -> (StatusCode, serde_json::Value) {
        let (status, body) = self.send("GET", path, bearer, None).await;
        let json = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
        (status, json)
    }
}
