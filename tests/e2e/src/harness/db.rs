//! Throwaway database with the full extension schema applied.
//!
//! Same contract as the admin contract suite's `tempdb`: created on the
//! server named by `SYSTEMPROMPT_TEST_DATABASE_URL` / `DATABASE_URL`,
//! self-skips without one — except under `CI`, where a missing database is a
//! failure, because a gate that quietly never ran reports success.

use std::sync::Arc;

use sqlx::{AssertSqlSafe, PgPool};
use systemprompt::ExtensionRegistry;
use systemprompt::database::{Database, install_extension_schemas};
use url::Url;

use systemprompt_web_admin as _;
use systemprompt_web_extension as _;

pub struct TempDb {
    pub pool: Arc<PgPool>,
    pub url: String,
    admin_url: String,
    db_name: String,
}

fn server_url() -> Option<String> {
    let url = std::env::var("SYSTEMPROMPT_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok();
    assert!(
        !(url.is_none() && std::env::var("CI").is_ok()),
        "no SYSTEMPROMPT_TEST_DATABASE_URL or DATABASE_URL in CI — the e2e suite must not be \
         skipped there"
    );
    url
}

fn with_database(base: &str, db_name: &str) -> String {
    let mut url = Url::parse(base).expect("DATABASE_URL is a valid URL");
    url.set_path(&format!("/{db_name}"));
    url.into()
}

impl TempDb {
    pub async fn create() -> Option<Self> {
        let base = server_url()?;
        let admin_url = with_database(&base, "postgres");
        let db_name = format!("e2e_{}", uuid::Uuid::new_v4().simple());

        let admin = PgPool::connect(&admin_url)
            .await
            .expect("connect to maintenance database");
        sqlx::query(AssertSqlSafe(format!("CREATE DATABASE \"{db_name}\"")))
            .execute(&admin)
            .await
            .expect("create throwaway database");
        admin.close().await;

        let url = with_database(&base, &db_name);
        let pool = Arc::new(
            PgPool::connect(&url)
                .await
                .expect("connect to throwaway database"),
        );

        let database = Database::from_pools(Arc::clone(&pool), Some(Arc::clone(&pool)));
        let registry = ExtensionRegistry::discover().expect("discover extension registrations");
        assert!(
            !registry.is_empty(),
            "no extensions registered — the e2e binary must link the crates whose \
             `register_extension!` supplies the migrations"
        );
        install_extension_schemas(&registry, database.write())
            .await
            .expect("install extension schemas");

        Some(Self {
            pool,
            url,
            admin_url,
            db_name,
        })
    }

    pub async fn cleanup(self) {
        self.pool.close().await;
        let admin = PgPool::connect(&self.admin_url)
            .await
            .expect("reconnect maintenance database for drop");
        sqlx::query(AssertSqlSafe(format!(
            "DROP DATABASE IF EXISTS \"{}\" WITH (FORCE)",
            self.db_name
        )))
        .execute(&admin)
        .await
        .expect("drop throwaway database");
        admin.close().await;
    }
}
