//! The router under test, mounted at the prefixes the server mounts it at.
//!
//! This mirrors `extensions/web/src/extension_impl.rs` rather than calling the
//! per-group constructors directly: prefix handling is part of the contract.
//! `non_admin_gate_middleware` matches on `/admin/...` paths, and the SSR
//! router is attached with `nest_service`, so testing the groups in isolation
//! would exercise a path shape no request ever has.

use std::sync::Arc;

use systemprompt::database::Database;
use systemprompt::oauth::SessionCreationService;
use systemprompt::users::{SessionRepository, UserRepository, UserService};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt as _;
use sqlx::PgPool;
use systemprompt_web_admin as admin;
use tower::ServiceExt;

use crate::globals;
use crate::principal::{Credentials, Principal};

pub(crate) const ADMIN_API_PREFIX: &str = "/api/public/admin";
pub(crate) const SSR_PREFIX: &str = "/admin";

pub(crate) struct App {
    router: Router,
    credentials: Credentials,
}

fn session_service(pool: &Arc<PgPool>) -> Arc<SessionCreationService> {
    let db = Arc::new(Database::from_pools(
        Arc::clone(pool),
        Some(Arc::clone(pool)),
    ));
    let user = UserService::new(Arc::new(
        UserRepository::new(&db).expect("build the user repository"),
    ));
    let sessions = SessionRepository::new(&db).expect("build the session repository");
    Arc::new(SessionCreationService::new(
        Arc::new(sessions),
        Arc::new(user),
    ))
}

impl App {
    pub(crate) fn new(pool: &Arc<PgPool>, credentials: Credentials) -> Self {
        Self::build(pool, credentials, admin::AdfsConfig::disabled())
    }

    fn build(pool: &Arc<PgPool>, credentials: Credentials, adfs: admin::AdfsConfig) -> Self {
        let database = Arc::new(Database::from_pools(
            Arc::clone(pool),
            Some(Arc::clone(pool)),
        ));
        let admin_dir = globals::repo_root().join("storage/files/admin");
        // Handlebars strict mode: templates read `branding.*`, so an engine
        // without branding 500s on every page the server renders fine.
        let branding = systemprompt_web_extension::branding_config();
        let engine = admin::templates::AdminTemplateEngine::new(&admin_dir)
            .expect("build the admin template engine from storage/files/admin")
            .with_branding(branding);

        let api = Router::new()
            .nest(
                "/admin",
                admin::admin_router(Arc::clone(pool), pool, credentials.admin_user_id.clone()),
            )
            .merge(admin::connector_api_router(Arc::clone(pool)))
            .merge(admin::bridge_identity_router(Arc::clone(pool)))
            .merge(admin::salesforce_api_router(admin::SalesforceDeps {
                config: Arc::new(admin::SalesforceConfig::disabled()),
                write_pool: Arc::clone(pool),
            }));
        let sso_deps = admin::AdfsDeps {
            config: Arc::new(adfs),
            write_pool: Arc::clone(pool),
            session_service: session_service(pool),
        };
        // Core layers `Option<Arc<AiService>>` onto every extension router;
        // `None` is an instance without inference configured.
        let ssr = admin::admin_ssr_router(
            Arc::clone(pool),
            pool,
            engine.clone(),
            sso_deps,
            credentials.admin_user_id.clone(),
        )
        .layer(axum::Extension(None::<Arc<systemprompt::ai::AiService>>));
        let bridge_auth = admin::bridge_auth_ssr_router(Arc::clone(pool), engine);

        // Core layers the governance engine and artifact ingest onto every
        // extension router; hooks, secrets and share sit outside the route
        // modules the contract table is derived from.
        let governance = Arc::new(
            systemprompt_security::policy::GovernanceEngine::from_config(
                &systemprompt_security::policy::GovernanceConfig::defaults(),
            )
            .expect("build the default governance chain"),
        );
        let artifact_ingest = Arc::new(
            systemprompt::mcp::ArtifactIngest::from_db(&database, None)
                .expect("build the artifact ingest"),
        );
        let hooks = admin::hooks_webhook_router(Arc::clone(pool), session_service(pool))
            .layer(axum::Extension(governance))
            .layer(axum::Extension(artifact_ingest));

        let secrets = admin::secrets_router(Arc::clone(pool));
        let share = admin::share_manifest_router(Arc::clone(pool));

        let router = Router::new()
            .nest_service(SSR_PREFIX, ssr)
            .nest_service("/bridge-auth", bridge_auth)
            .nest("/api/public", api)
            .merge(hooks)
            .merge(secrets)
            .merge(share);

        Self {
            router,
            credentials,
        }
    }

    pub(crate) async fn send(
        &self,
        method: &str,
        path: &str,
        principal: Principal,
    ) -> (StatusCode, Option<String>) {
        let mut builder = Request::builder()
            .method(method.to_uppercase().as_str())
            .uri(path);
        if matches!(method, "post" | "put" | "patch" | "delete") {
            let profile = systemprompt::config::ProfileBootstrap::get().expect("fixture profile");
            let origin = url::Url::parse(&profile.server.api_external_url)
                .expect("fixture origin")
                .origin()
                .ascii_serialization();
            builder = builder.header("origin", origin);
        }
        if let Some(token) = self.credentials.token_for(principal) {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        // `{}` is the most benign well-formed body: a validation 4xx is a
        // legitimate outcome, a 500 is not.
        let request = builder
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .expect("build request");

        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("router is infallible");
        let status = response.status();
        if !status.is_server_error() {
            return (status, None);
        }

        let bytes = response
            .into_body()
            .collect()
            .await
            .map(http_body_util::Collected::to_bytes)
            .unwrap_or_default();
        let snippet: String = String::from_utf8_lossy(&bytes).chars().take(300).collect();
        (status, Some(snippet))
    }

    pub(crate) async fn call(&self, call: Call<'_>) -> (StatusCode, String) {
        self.dispatch(call, None, &[]).await
    }

    // Hook tokens carry `aud=hook` and a `plugin_id` claim no principal in
    // `Credentials` holds, so the bearer is passed per call.
    pub(crate) async fn call_with_bearer(
        &self,
        call: Call<'_>,
        token: &str,
    ) -> (StatusCode, String) {
        self.dispatch(call, Some(token), &[]).await
    }

    pub(crate) async fn redirect_of(&self, call: Call<'_>) -> (StatusCode, String) {
        self.redirect_with_headers(call, &[]).await
    }

    pub(crate) async fn redirect_with_headers(
        &self,
        call: Call<'_>,
        headers: &[(&str, &str)],
    ) -> (StatusCode, String) {
        let (status, headers) = self.response_headers_with(call, headers).await;
        (status, headers.location.unwrap_or_default())
    }

    pub(crate) async fn response_headers(&self, call: Call<'_>) -> (StatusCode, ResponseHeaders) {
        self.response_headers_with(call, &[]).await
    }

    pub(crate) async fn response_headers_with(
        &self,
        call: Call<'_>,
        extra_headers: &[(&str, &str)],
    ) -> (StatusCode, ResponseHeaders) {
        let request = self.build_request(call, None, extra_headers);
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("router is infallible");
        let status = response.status();
        let headers = response.headers();
        let location = headers
            .get("location")
            .and_then(|v| v.to_str().ok())
            .map(ToOwned::to_owned);
        let set_cookie = headers
            .get_all("set-cookie")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .map(ToOwned::to_owned)
            .collect();
        (
            status,
            ResponseHeaders {
                location,
                set_cookie,
            },
        )
    }

    async fn dispatch(
        &self,
        call: Call<'_>,
        bearer: Option<&str>,
        extra_headers: &[(&str, &str)],
    ) -> (StatusCode, String) {
        let request = self.build_request(call, bearer, extra_headers);
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("router is infallible");
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .map(http_body_util::Collected::to_bytes)
            .unwrap_or_default();
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    fn build_request(
        &self,
        call: Call<'_>,
        bearer: Option<&str>,
        extra_headers: &[(&str, &str)],
    ) -> Request<Body> {
        let mut builder = Request::builder()
            .method(call.method.to_uppercase().as_str())
            .uri(call.path);
        match bearer {
            Some(token) => {
                builder = builder.header("authorization", format!("Bearer {token}"));
            },
            None => {
                if let Some(token) = self.credentials.token_for(call.principal) {
                    builder = builder.header("authorization", format!("Bearer {token}"));
                }
            },
        }
        for (name, value) in extra_headers {
            builder = builder.header(*name, *value);
        }
        if let Some(content_type) = call.content_type {
            builder = builder.header("content-type", content_type);
        }
        let body = call
            .body
            .map_or_else(Body::empty, |b| Body::from(b.to_owned()));
        builder.body(body).expect("build request")
    }
}

pub(crate) struct ResponseHeaders {
    pub location: Option<String>,
    pub set_cookie: Vec<String>,
}

#[derive(Clone, Copy)]
pub(crate) struct Call<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub principal: Principal,
    // axum's `Json` extractor refuses a body with no `content-type`, so
    // `None` drives that rejection path.
    pub content_type: Option<&'a str>,
    pub body: Option<&'a str>,
}

impl<'a> Call<'a> {
    pub(crate) const fn get(path: &'a str, principal: Principal) -> Self {
        Self {
            method: "get",
            path,
            principal,
            content_type: None,
            body: None,
        }
    }

    pub(crate) const fn json(
        method: &'a str,
        path: &'a str,
        principal: Principal,
        body: &'a str,
    ) -> Self {
        Self {
            method,
            path,
            principal,
            content_type: Some("application/json"),
            body: Some(body),
        }
    }
}
