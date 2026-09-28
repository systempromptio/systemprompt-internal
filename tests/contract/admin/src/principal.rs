//! The seven principals every route is driven under, and the credentials that
//! distinguish them.
//!
//! Roles are not carried in the JWT: `user_context_middleware` reads
//! `users.roles` from the database, so roles are seeded as rows and the token
//! only has to validate and carry a subject.

use sqlx::PgPool;
use systemprompt::identifiers::{SessionId, UserId};
use systemprompt_security::{AdminTokenParams, JwtService};

use crate::globals;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Principal {
    Anonymous,
    NonAdmin,
    Developer,
    Admin,
    PlatformAdmin,
    ProjectManager,
    KnowledgeWorker,
}

impl Principal {
    pub(crate) const ALL: [Self; 7] = [
        Self::Anonymous,
        Self::NonAdmin,
        Self::Developer,
        Self::Admin,
        Self::PlatformAdmin,
        Self::ProjectManager,
        Self::KnowledgeWorker,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Anonymous => "anonymous",
            Self::NonAdmin => "non-admin",
            Self::Developer => "developer",
            Self::Admin => "admin",
            Self::PlatformAdmin => "platform-admin",
            Self::ProjectManager => "project-manager",
            Self::KnowledgeWorker => "knowledge-worker",
        }
    }
}

pub(crate) struct Credentials {
    pub non_admin: String,
    pub developer: String,
    pub admin: String,
    pub admin_user_id: UserId,
    pub platform_admin: String,
    pub project_manager: String,
    pub knowledge_worker: String,
    pub non_admin_user_id: UserId,
}

impl Credentials {
    pub(crate) fn token_for(&self, principal: Principal) -> Option<&str> {
        match principal {
            Principal::Anonymous => None,
            Principal::NonAdmin => Some(&self.non_admin),
            Principal::Developer => Some(&self.developer),
            Principal::Admin => Some(&self.admin),
            Principal::PlatformAdmin => Some(&self.platform_admin),
            Principal::ProjectManager => Some(&self.project_manager),
            Principal::KnowledgeWorker => Some(&self.knowledge_worker),
        }
    }
}

// Scoped listings narrow to the caller's groups; a caller in none is derived
// into `unassigned`, so a real membership keeps scoped routes non-vacuous.
const CONTRACT_GROUP: &str = "contract-group";

pub(crate) async fn provision(pool: &PgPool) -> Credentials {
    sqlx::query(
        "INSERT INTO groups (id, name, source) VALUES ($1, 'Contract group', 'dashboard')
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(CONTRACT_GROUP)
    .execute(pool)
    .await
    .expect("seed the contract group");

    let (non_admin, non_admin_user_id) =
        provision_one(pool, "contract-user", &["user"], true).await;
    let (developer, _) = provision_one(pool, "contract-dev", &["developer", "user"], true).await;
    let (admin, admin_user_id) =
        provision_one(pool, "contract-admin", &["admin", "user"], false).await;
    let (platform_admin, _) = provision_one(
        pool,
        "contract-platform",
        &["platform_admin", "admin", "user"],
        false,
    )
    .await;
    let (project_manager, _) =
        provision_one(pool, "contract-pm", &["project_manager", "user"], true).await;
    let (knowledge_worker, knowledge_worker_user_id) =
        provision_one(pool, "contract-kw", &["knowledge_worker", "user"], true).await;
    // Role recomputation reads hand-granted roles from `user_manual_roles`, so
    // seed the row an admin's grant writes.
    sqlx::query(
        "INSERT INTO user_manual_roles (user_id, role) VALUES ($1, 'knowledge_worker')
         ON CONFLICT DO NOTHING",
    )
    .bind(knowledge_worker_user_id.as_str())
    .execute(pool)
    .await
    .expect("record the knowledge worker's manual role");
    Credentials {
        non_admin,
        developer,
        admin,
        admin_user_id,
        platform_admin,
        project_manager,
        knowledge_worker,
        non_admin_user_id,
    }
}

async fn provision_one(
    pool: &PgPool,
    name: &str,
    roles: &[&str],
    in_group: bool,
) -> (String, UserId) {
    // Why: SSR browser sessions are minted for a UUID account id.
    let user_id = UserId::new(uuid::Uuid::new_v4().to_string());
    let email = format!("{name}@contract.test");

    sqlx::query(
        "INSERT INTO users (id, name, email, roles, email_verified)
         VALUES ($1, $2, $3, $4, true)",
    )
    .bind(user_id.as_str())
    .bind(format!("{name}-{}", user_id.as_str()))
    .bind(&email)
    .bind(roles.iter().map(|r| (*r).to_owned()).collect::<Vec<_>>())
    .execute(pool)
    .await
    .expect("seed contract principal");
    sqlx::query("INSERT INTO user_profile_ext (user_id) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(user_id.as_str())
        .execute(pool)
        .await
        .expect("seed the principal's profile row");
    if in_group {
        sqlx::query(
            "INSERT INTO group_members (group_id, user_id, source) VALUES ($1, $2, 'adfs')",
        )
        .bind(CONTRACT_GROUP)
        .bind(user_id.as_str())
        .execute(pool)
        .await
        .expect("place the principal in the contract group");
    }

    let session_id = SessionId::new(uuid::Uuid::new_v4().to_string());
    let token = JwtService::generate_admin_token(&AdminTokenParams {
        user_id: &user_id,
        session_id: &session_id,
        email: &email,
        issuer: &globals::jwt_issuer(),
        duration: chrono::Duration::hours(1),
        client_id: None,
    })
    .expect("mint a session token");

    (token.as_str().to_owned(), user_id)
}
