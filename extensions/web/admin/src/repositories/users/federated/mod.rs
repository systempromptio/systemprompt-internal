//! Federated-identity resolution for external SSO.
//!
//! Bridges an externally-issued identity (`issuer`, `external_sub`) to a local
//! `users` row, honouring the "merge by verified email" rule that core's own
//! `find_or_create_federated` deliberately omits.
//!
//! Resolution order (the first match wins):
//! 1. **Existing mapping** — the `(issuer, external_sub)` pair already points
//!    at a user (a returning SSO login).
//! 2. **Email link** — an active local account already owns this email. We
//!    attach the federated identity to it instead of minting a duplicate. This
//!    is the account *merge*. The caller MUST have verified `email_verified`
//!    and an allow-listed domain before reaching this path — linking an
//!    unverified address would let a hostile `IdP` claim arbitrary accounts.
//! 3. **Odoo credential link** (odoo issuers only) — an active local account
//!    already holds this Odoo identity in `odoo_identity` (the profile-page
//!    "Connect Odoo" flow), under a *different* platform email. Resolving to
//!    that account instead of provisioning is what keeps an admin who linked
//!    Odoo before Odoo became a sign-in door from being split into a duplicate,
//!    role-less account.
//! 4. **Create** — no mapping and no local account: provision a fresh user, the
//!    mapping, and — when the email's domain is claimed by a customer
//!    organization — its membership row, in a single transaction.
//!
//! Just-in-time provisioning is one of the two doors a seat can be minted
//! through, so the seat limit is checked here as well as on operator-created
//! users. A limit enforced on only one door is not a limit, and this is the
//! door an enterprise customer's users actually arrive through.

use sqlx::PgPool;
use systemprompt::identifiers::UserId;
use systemprompt_web_shared::error::MarketplaceError;

use crate::repositories::organizations;

mod lookup;

use lookup::{
    find_active_user_by_email, find_active_user_by_odoo_identity, find_mapping, link_existing,
    load_user,
};

/// Outcome of [`resolve_federated_user`]: a local user the caller can mint a
/// session for.
#[derive(Debug, Clone)]
pub struct ResolvedFederatedUser {
    pub user_id: UserId,
    pub email: String,
    pub display_name: String,
    pub roles: Vec<String>,
}

/// The verified external-identity claims carried into resolution. The caller
/// must have already enforced the `email_verified` + allow-listed-domain gate.
#[derive(Debug, Clone, Copy)]
pub struct FederatedClaims<'a> {
    pub issuer: &'a str,
    pub external_sub: &'a str,
    pub email: &'a str,
    pub display_name: &'a str,
}

/// Outcome of an explicit profile-driven link attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkOutcome {
    Linked,
    AlreadyLinkedElsewhere,
}

pub async fn link_identity_to_user(
    pool: &PgPool,
    issuer: &str,
    external_sub: &str,
    user_id: &UserId,
) -> Result<LinkOutcome, sqlx::Error> {
    let inserted = sqlx::query!(
        "INSERT INTO federated_identities (issuer, external_sub, user_id) \
         VALUES ($1, $2, $3) ON CONFLICT (issuer, external_sub) DO NOTHING",
        issuer,
        external_sub,
        user_id.as_str()
    )
    .execute(pool)
    .await?
    .rows_affected();
    if inserted > 0 {
        return Ok(LinkOutcome::Linked);
    }
    match find_mapping(pool, issuer, external_sub).await? {
        Some(owner) if owner == *user_id => Ok(LinkOutcome::Linked),
        _ => Ok(LinkOutcome::AlreadyLinkedElsewhere),
    }
}

pub async fn delete_federated_identities_for_issuer(
    pool: &PgPool,
    user_id: &UserId,
    issuer: &str,
) -> Result<u64, sqlx::Error> {
    let deleted = sqlx::query!(
        "DELETE FROM federated_identities WHERE user_id = $1 AND issuer = $2",
        user_id.as_str(),
        issuer
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(deleted)
}

// Why: the identity provider is also the role authority when it can be — a
// sign-in that carries freshly-computed roles overwrites the stored set, so
// flipping a user's groups at the provider changes their platform roles at
// the next sign-in. `None` means the caller could not compute roles this
// time; the stored set stands.
async fn apply_roles(
    pool: &PgPool,
    user_id: &UserId,
    stored: Vec<String>,
    desired: Option<&[String]>,
) -> Result<Vec<String>, MarketplaceError> {
    let Some(desired) = desired else {
        return Ok(stored);
    };
    if stored == desired {
        return Ok(stored);
    }
    sqlx::query!(
        "UPDATE users SET roles = $2, updated_at = CURRENT_TIMESTAMP WHERE id = $1",
        user_id.as_str(),
        desired,
    )
    .execute(pool)
    .await?;
    tracing::info!(
        user_id = %user_id,
        roles = ?desired,
        "Federated sign-in updated platform roles"
    );
    Ok(desired.to_vec())
}

async fn create_federated(
    pool: &PgPool,
    claims: &FederatedClaims<'_>,
    desired_roles: Option<&[String]>,
) -> Result<ResolvedFederatedUser, MarketplaceError> {
    let FederatedClaims {
        issuer,
        external_sub,
        email,
        display_name,
    } = *claims;
    // Why: the seat check runs before the user exists, so a full plan rejects
    // the login rather than creating an orphaned account that cannot reach
    // anything. An unclaimed email domain is not an error — that arrival is
    // not on anyone's contract and lands unattached.
    let org_id = organizations::crud::find_organization_for_email(pool, email).await?;
    if let Some(org_id) = org_id.as_deref() {
        organizations::seats::assert_seat_available(pool, org_id).await?;
    }

    let user_id = uuid::Uuid::new_v4().to_string();
    let roles = desired_roles.map_or_else(|| vec!["user".to_owned()], <[String]>::to_vec);
    let mut tx = pool.begin().await?;

    sqlx::query!(
        r#"
        INSERT INTO users (id, name, email, display_name, status, email_verified, roles)
        VALUES ($1, $2, $3, $4, 'active', true, $5)
        "#,
        &user_id,
        display_name,
        email,
        display_name,
        &roles,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "INSERT INTO federated_identities (issuer, external_sub, user_id) VALUES ($1, $2, $3)",
        issuer,
        external_sub,
        &user_id,
    )
    .execute(&mut *tx)
    .await?;

    if let Some(org_id) = org_id.as_deref() {
        sqlx::query!(
            "INSERT INTO organization_members (user_id, org_id, org_role)
             VALUES ($1, $2, 'member')",
            &user_id,
            org_id,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(ResolvedFederatedUser {
        user_id: UserId::new(user_id),
        email: email.to_owned(),
        display_name: display_name.to_owned(),
        roles,
    })
}

pub async fn resolve_federated_user(
    pool: &PgPool,
    claims: &FederatedClaims<'_>,
    auto_provision: bool,
    desired_roles: Option<&[String]>,
) -> Result<Option<ResolvedFederatedUser>, MarketplaceError> {
    let FederatedClaims {
        issuer,
        external_sub,
        email,
        display_name,
    } = *claims;
    if let Some(user_id) = find_mapping(pool, issuer, external_sub).await?
        && let Some(user) = load_user(pool, &user_id).await?
    {
        let roles = apply_roles(pool, &user.id, user.roles, desired_roles).await?;
        return Ok(Some(ResolvedFederatedUser {
            user_id: user.id,
            email: email.to_owned(),
            display_name: user.display_name,
            roles,
        }));
    }

    if let Some(user) = find_active_user_by_email(pool, email).await? {
        link_existing(pool, issuer, external_sub, &user.id).await?;
        let roles = apply_roles(pool, &user.id, user.roles, desired_roles).await?;
        return Ok(Some(ResolvedFederatedUser {
            user_id: user.id,
            email: email.to_owned(),
            display_name: user.display_name,
            roles,
        }));
    }

    // Why: an Odoo credential proves the same identity the profile-page link
    // flow recorded, so a user who linked Odoo under another platform email
    // resolves to that account rather than being duplicated (see module doc,
    // step 3).
    if issuer.starts_with("odoo:")
        && let Some(user) = find_active_user_by_odoo_identity(pool, external_sub, email).await?
    {
        link_existing(pool, issuer, external_sub, &user.id).await?;
        let roles = apply_roles(pool, &user.id, user.roles, desired_roles).await?;
        return Ok(Some(ResolvedFederatedUser {
            user_id: user.id,
            email: email.to_owned(),
            display_name: user.display_name,
            roles,
        }));
    }

    if !auto_provision {
        return Ok(None);
    }

    create_federated(pool, claims, desired_roles).await.map(Some)
}
