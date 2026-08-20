//! Subject dimensions this extension adds to core's authorization resolver.
//!
//! Core resolves `user` and `role` and deliberately knows nothing else. Every
//! other dimension an operator wants to write rules against is a tenant
//! concept, declared here: a [`SubjectDimension`] describing where it sits in
//! the precedence ladder, and a [`SubjectAttributeProvider`][p] that looks up
//! the values a user holds for it.
//!
//! We declare two: [`department`] and [`organization`]. They form a ladder
//! with core's — user (0), department (100), role (200), organization (300) —
//! where a lower number is the narrower, higher-priority scope. Adding a third
//! — cost centre, clearance, jurisdiction — means writing a provider beside
//! them and one `register_subject_attribute_provider!` call; no core change,
//! and no edit to the resolve call sites, because they all read the registry
//! through [`subject_attributes_for`] and [`dimensions`].
//!
//! [p]: systemprompt_security::authz::SubjectAttributeProvider

pub mod department;
pub mod organization;

use std::sync::{Arc, OnceLock};

use sqlx::PgPool;
use systemprompt::identifiers::UserId;
use systemprompt_security::authz::{
    AuthzHookContext, NullAuditSink, SharedSubjectAttributeProvider, SubjectAttributes,
    SubjectDimension, dimensions_of, discover_subject_providers, gather_subject_attributes,
};

use crate::authz::department::DepartmentAttributeProvider;
use crate::authz::organization::OrganizationAttributeProvider;

systemprompt_security::register_subject_attribute_provider!(|ctx| {
    let provider: SharedSubjectAttributeProvider =
        Arc::new(DepartmentAttributeProvider::new(Arc::clone(&ctx.pool)));
    provider
});

systemprompt_security::register_subject_attribute_provider!(|ctx| {
    let provider: SharedSubjectAttributeProvider =
        Arc::new(OrganizationAttributeProvider::new(Arc::clone(&ctx.pool)));
    provider
});

struct Registry {
    providers: Vec<SharedSubjectAttributeProvider>,
    dimensions: Vec<SubjectDimension>,
}

static REGISTRY: OnceLock<Registry> = OnceLock::new();

fn registry(pool: &PgPool) -> &'static Registry {
    REGISTRY.get_or_init(|| {
        let providers = discover_subject_providers(&AuthzHookContext {
            pool: Arc::new(pool.clone()),
            sink: Arc::new(NullAuditSink),
        });
        Registry {
            dimensions: dimensions_of(&providers),
            providers,
        }
    })
}

pub fn dimensions(pool: &PgPool) -> &'static [SubjectDimension] {
    &registry(pool).dimensions
}

pub async fn subject_attributes_for(pool: &PgPool, user_id: &UserId) -> SubjectAttributes {
    gather_subject_attributes(&registry(pool).providers, user_id).await
}
