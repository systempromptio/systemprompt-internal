//! Creating the local account behind a first federated sign-in.
//!
//! Why this is separate from the resolution in [`super`]: provisioning is the
//! one path here that brings a new account into existence, and it is also
//! where a provider's role claim has the least established authority to draw
//! on. Reading it on its own is easier than reading it interleaved with the
//! lookup that decides whether provisioning should happen at all.

use sqlx::PgPool;
use systemprompt::identifiers::UserId;
use systemprompt_web_shared::error::MarketplaceError;

use crate::repositories::organizations;

use super::roles::strip_gated_grants;
use super::{FederatedClaims, ResolvedFederatedUser};

pub(super) async fn create_federated(
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
    let mut roles = desired_roles.map_or_else(|| vec!["user".to_owned()], <[String]>::to_vec);
    // Why: a row being created holds nothing yet, so every gated role in `desired`
    // is a new grant. See [`strip_gated_grants`].
    strip_gated_grants(issuer, &[], &mut roles);
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
