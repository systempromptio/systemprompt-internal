//! The router under test, mounted at the prefixes the server mounts it at.
//!
//! This mirrors `extensions/web/src/extension_impl.rs` rather than calling the
//! per-group constructors directly: prefix handling is part of the contract.
//! `non_admin_gate_middleware` matches on `/admin/...` paths, and the SSR
//! router is attached with `nest_service`, so testing the groups in isolation
//! would exercise a path shape no request ever has.

use std::sync::Arc;

use systemprompt::analytics::AnalyticsService;
use systemprompt::database::Database;
use systemprompt::oauth::SessionCreationService;
use systemprompt::users::UserService;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt as _;
use sqlx::PgPool;
use systemprompt_web_admin as admin;
use tower::ServiceExt;

use crate::globals;
use crate::principal::{Credentials, Principal};

// Mount prefixes, kept next to the router build so the contract table and the
// exhaustiveness check agree on where each route module lands.
pub const ADMIN_API_PREFIX: &str = "/api/public/admin";
pub const SSR_PREFIX: &str = "/admin";

pub struct App {
    router: Router,
    credentials: Credentials,
}

fn session_service(pool: &Arc<PgPool>) -> Arc<SessionCreationService> {
    let db = Arc::new(Database::from_pools(
        Arc::clone(pool),
        Some(Arc::clone(pool)),
    ));
    let user = UserService::new(&db).expect("build the user service");
    let analytics = AnalyticsService::new(&db, None, None).expect("build the analytics service");
    Arc::new(SessionCreationService::new(
        Arc::new(analytics),
        Arc::new(user),
    ))
}

impl App {
    pub fn new(pool: &Arc<PgPool>, credentials: Credentials) -> Self {
        let admin_dir = globals::repo_root().join("storage/files/admin");
        // Branding is not decoration here: the templates read `branding.*`
        // under strict mode, so an engine built without it 500s on every page
        // the server renders fine.
        let branding = systemprompt_web_extension::branding_config();
        let engine = admin::templates::AdminTemplateEngine::new(&admin_dir)
            .expect("build the admin template engine from storage/files/admin")
            .with_branding(branding);

        let api = Router::new().nest("/admin", admin::admin_router(Arc::clone(pool)));
        // Salesforce SSO is disabled here: the suite drives routes, and a
        // configured IdP would make every sign-in path depend on a live org.
        let sf_deps = admin::SalesforceDeps {
            config: Arc::new(admin::SalesforceConfig::disabled()),
            write_pool: Arc::clone(pool),
            session_service: session_service(pool),
        };
        let ssr = admin::admin_ssr_router(Arc::clone(pool), engine.clone(), sf_deps.clone());
        let bridge_auth = admin::bridge_auth_ssr_router(Arc::clone(pool), engine);

        let router = Router::new()
            .nest_service(SSR_PREFIX, ssr)
            .nest_service("/bridge-auth", bridge_auth)
            .nest("/api/public", api);

        Self {
            router,
            credentials,
        }
    }

    // Issue one request, returning its status and — only when the status is a
    // server error — a snippet of the body.
    //
    // A contract failure that reports `500` and nothing else is barely
    // actionable, and the whole point of the suite is that a 5xx is a defect
    // someone has to go and fix.
    pub async fn send(
        &self,
        method: &str,
        path: &str,
        principal: Principal,
    ) -> (StatusCode, Option<String>) {
        // HTTP methods are case-sensitive; the route source spells them
        // lowercase after axum's constructors.
        let mut builder = Request::builder()
            .method(method.to_uppercase().as_str())
            .uri(path);
        if let Some(token) = self.credentials.token_for(principal) {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        // Every write route takes JSON; an empty object is the most benign
        // well-formed body, and a 4xx from validation is a legitimate contract
        // outcome. What must not happen is a 500.
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
}
