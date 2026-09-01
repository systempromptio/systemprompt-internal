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
//!    already holds this Odoo uid in `odoo_identity` (the profile-page "Connect
//!    Odoo" flow). Resolving to that account instead of provisioning is what
//!    keeps an admin who linked Odoo before Odoo became a sign-in door from
//!    being split into a duplicate, role-less account.
//!
//!    This step used to resolve **silently across differing platform emails**,
//!    and to match on the Odoo login as well as the uid. Both were wrong. It
//!    bound an Odoo login onto whatever row held that identity — in one case a
//!    service principal whose email was a placeholder — and the resulting
//!    session was then displayed under that row's fabricated address on the
//!    bridge device-link consent screen, immediately above a control that mints
//!    a durable personal access token. Now: uid only, and if the resolved
//!    account's email differs from the claim, resolution **fails with a
//!    conflict** rather than linking, applying roles, or falling through to
//!    provisioning. The duplicate-account outcome step 3 exists to prevent is
//!    still prevented — no duplicate is created — but re-pointing an Odoo login
//!    at a differently-addressed account is now an explicit act performed from
//!    the profile page, not a silent one performed at sign-in.
//! 4. **Create** — no mapping and no local account: provision a fresh user, the
//!    mapping, and — when the email's domain is claimed by a customer
//!    organization — its membership row, in a single transaction.
//!
//! **Role authority.** A federated sign-in can carry freshly-computed roles,
//! and the provider is the role authority *once the binding is established* —
//! that is what makes flipping a user's Odoo groups change their platform
//! roles. It is not the authority at the moment it first attaches to a
//! pre-existing row: a first bind that could *grant* would let control of an
//! external account escalate a local one. So authority is scoped by which step
//! resolved (see `RoleAuthority`), and adding the `admin` role from a
//! federated claim is gated separately again — on provisioning too, where the
//! grant would otherwise be least visible.
//!
//! Just-in-time provisioning is one of the two doors a seat can be minted
//! through, so the seat limit is checked here as well as on operator-created
//! users. A limit enforced on only one door is not a limit, and this is the
//! door an enterprise customer's users actually arrive through.

use sqlx::PgPool;
use systemprompt::identifiers::UserId;
use systemprompt_web_shared::error::MarketplaceError;


mod lookup;
mod provisioning;
mod roles;

use lookup::{
    FederatedIdentitySummary, find_active_user_by_email, find_active_user_by_odoo_uid,
    find_mapping, link_existing, list_federated_identities, load_user,
};
use provisioning::create_federated;
use roles::{RoleAuthority, RoleUpdate, apply_roles};

// Why: lists the external identities a user can currently sign in through.
//
// Public face of [`lookup::list_federated_identities`], for screens that must
// state *how* a session authenticated rather than only which row it resolved
// to.
pub async fn list_federated_identities_for_user(
    pool: &PgPool,
    user_id: &UserId,
) -> Result<Vec<FederatedIdentitySummary>, sqlx::Error> {
    list_federated_identities(pool, user_id).await
}

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

// Why: masks an address enough to be recognised by its owner and useless to
// anyone else.
//
// The conflict this appears in is shown to whoever just authenticated against
// the *external* system, who may not hold the platform account named. They
// need enough to recognise it if it is theirs; they are owed nothing more.
fn mask_email(email: &str) -> String {
    let Some((local, domain)) = email.split_once('@') else {
        return "***".to_owned();
    };
    let head = local.chars().next().map_or_else(String::new, String::from);
    format!("{head}***@{domain}")
}

// Why: the binding already exists, which is the only case that earns
// `RoleAuthority::Sync` — the provider has been trusted with this row before,
// so it may raise as well as lower.
async fn resolve_established_binding(
    pool: &PgPool,
    issuer: &str,
    external_sub: &str,
    desired_roles: Option<&[String]>,
) -> Result<Option<ResolvedFederatedUser>, MarketplaceError> {
    let Some(user_id) = find_mapping(pool, issuer, external_sub).await? else {
        return Ok(None);
    };
    let Some(user) = load_user(pool, &user_id).await? else {
        return Ok(None);
    };
    let roles = apply_roles(
        pool,
        RoleUpdate {
            user_id: &user.id,
            issuer,
            stored: user.roles,
            desired: desired_roles,
            authority: RoleAuthority::Sync,
        },
    )
    .await?;
    Ok(Some(ResolvedFederatedUser {
        user_id: user.id,
        email: user.email,
        display_name: user.display_name,
        roles,
    }))
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
        display_name: _,
    } = *claims;
    if let Some(resolved) =
        resolve_established_binding(pool, issuer, external_sub, desired_roles).await?
    {
        return Ok(Some(resolved));
    }

    if let Some(user) = find_active_user_by_email(pool, email).await? {
        link_existing(pool, issuer, external_sub, &user.id).await?;
        let roles = apply_roles(
            pool,
            RoleUpdate {
                user_id: &user.id,
                issuer,
                stored: user.roles,
                desired: desired_roles,
                authority: RoleAuthority::DowngradeOnly,
            },
        )
        .await?;
        return Ok(Some(ResolvedFederatedUser {
            user_id: user.id,
            email: user.email,
            display_name: user.display_name,
            roles,
        }));
    }

    if let Some(resolved) =
        resolve_by_odoo_uid(pool, issuer, external_sub, email, desired_roles).await?
    {
        return Ok(Some(resolved));
    }

    if !auto_provision {
        return Ok(None);
    }

    create_federated(pool, claims, desired_roles)
        .await
        .map(Some)
}

// Why: an Odoo uid proves the same identity the profile-page link flow
// recorded, so a user who linked Odoo under another platform email resolves to
// that account rather than being duplicated (see module doc, step 3). `None`
// means no such binding — the caller carries on to provisioning.
async fn resolve_by_odoo_uid(
    pool: &PgPool,
    issuer: &str,
    external_sub: &str,
    email: &str,
    desired_roles: Option<&[String]>,
) -> Result<Option<ResolvedFederatedUser>, MarketplaceError> {
    if !issuer.starts_with("odoo:") {
        return Ok(None);
    }
    let Some(user) = find_active_user_by_odoo_uid(pool, external_sub).await? else {
        return Ok(None);
    };
    // Why: refuse rather than link: the addresses disagreeing means this Odoo
    // login is about to sign somebody in as an account that does not carry
    // their name. Whether that is correct is a judgement only a person holding
    // both accounts can make, and every screen downstream — the bridge consent
    // page above all — would show the resolved row's address as the operator's
    // identity. Refusing keeps the duplicate-account outcome step 3 exists to
    // prevent (nothing is created here) while making the re-point explicit.
    if !user.email.eq_ignore_ascii_case(email) {
        tracing::warn!(
            user_id = %user.id,
            issuer = %issuer,
            odoo_uid = %external_sub,
            claim_email = %email,
            account_email = %user.email,
            "Odoo identity resolves to an account with a different platform email; refusing"
        );
        return Err(MarketplaceError::Conflict(format!(
            "This Odoo account is already connected to the platform account {}. Sign in to \
             that account and re-confirm the Odoo connection from your profile, or ask an \
             administrator.",
            mask_email(&user.email)
        )));
    }

    link_existing(pool, issuer, external_sub, &user.id).await?;
    let roles = apply_roles(
        pool,
        RoleUpdate {
            user_id: &user.id,
            issuer,
            stored: user.roles,
            desired: desired_roles,
            authority: RoleAuthority::DowngradeOnly,
        },
    )
    .await?;
    Ok(Some(ResolvedFederatedUser {
        user_id: user.id,
        email: user.email,
        display_name: user.display_name,
        roles,
    }))
}
