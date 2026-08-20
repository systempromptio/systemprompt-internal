//! Organization and membership rows.

use sqlx::PgPool;
use systemprompt::identifiers::UserId;
use systemprompt_web_shared::error::MarketplaceError;

use crate::authz::organization;

#[derive(Debug, Clone)]
pub struct OrganizationSummary {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub plan_id: Option<String>,
    pub plan_name: Option<String>,
    pub status: String,
    pub is_platform: bool,
    pub seat_limit: Option<i32>,
    pub seats_used: i64,
}

#[derive(Debug, Clone)]
pub struct OrganizationMember {
    pub user_id: UserId,
    pub email: String,
    pub display_name: Option<String>,
    pub org_role: String,
    pub department: Option<String>,
    pub is_active: bool,
}

pub async fn list_organizations(
    pool: &PgPool,
) -> Result<Vec<OrganizationSummary>, MarketplaceError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            o.id AS "id!",
            o.slug AS "slug!",
            o.name AS "name!",
            o.plan_id,
            p.name AS "plan_name?",
            o.status AS "status!",
            o.is_platform AS "is_platform!",
            COALESCE(o.seat_limit_override, p.seat_limit) AS "seat_limit?",
            (SELECT COUNT(*) FROM organization_members m
              JOIN users u ON u.id = m.user_id
             WHERE m.org_id = o.id AND u.status = 'active') AS "seats_used!"
        FROM organizations o
        LEFT JOIN plans p ON p.id = o.plan_id
        ORDER BY o.name
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| OrganizationSummary {
            id: r.id,
            slug: r.slug,
            name: r.name,
            plan_id: r.plan_id,
            plan_name: r.plan_name,
            status: r.status,
            is_platform: r.is_platform,
            seat_limit: r.seat_limit,
            seats_used: r.seats_used,
        })
        .collect())
}

pub async fn find_organization_by_slug(
    pool: &PgPool,
    slug: &str,
) -> Result<Option<OrganizationSummary>, MarketplaceError> {
    Ok(list_organizations(pool)
        .await?
        .into_iter()
        .find(|o| o.slug == slug))
}

pub async fn find_organization_for_user(
    pool: &PgPool,
    user_id: &UserId,
) -> Result<Option<String>, MarketplaceError> {
    let slug = sqlx::query_scalar!(
        "SELECT o.slug FROM organization_members m
         JOIN organizations o ON o.id = m.org_id
         WHERE m.user_id = $1",
        user_id.as_str()
    )
    .fetch_optional(pool)
    .await?;
    Ok(slug)
}

pub async fn get_platform_membership(
    pool: &PgPool,
    user_id: &UserId,
) -> Result<bool, MarketplaceError> {
    let found = sqlx::query_scalar!(
        "SELECT 1 AS present FROM organization_members m
         JOIN organizations o ON o.id = m.org_id
         WHERE m.user_id = $1 AND o.is_platform",
        user_id.as_str()
    )
    .fetch_optional(pool)
    .await?;
    Ok(found.is_some())
}

pub async fn list_members(
    pool: &PgPool,
    org_id: &str,
) -> Result<Vec<OrganizationMember>, MarketplaceError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            u.id AS "user_id!: UserId",
            u.email AS "email!",
            u.display_name,
            m.org_role AS "org_role!",
            NULLIF(e.department, '') AS "department?",
            (u.status = 'active') AS "is_active!"
        FROM organization_members m
        JOIN users u ON u.id = m.user_id
        LEFT JOIN user_profile_ext e ON e.user_id = u.id
        WHERE m.org_id = $1
        ORDER BY u.email
        "#,
        org_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| OrganizationMember {
            user_id: r.user_id,
            email: r.email,
            display_name: r.display_name,
            org_role: r.org_role,
            department: r.department,
            is_active: r.is_active,
        })
        .collect())
}

pub async fn set_membership(
    pool: &PgPool,
    user_id: &UserId,
    org_id: &str,
    org_role: &str,
) -> Result<(), MarketplaceError> {
    sqlx::query!(
        "INSERT INTO organization_members (user_id, org_id, org_role)
         VALUES ($1, $2, $3)
         ON CONFLICT (user_id) DO UPDATE
            SET org_id = EXCLUDED.org_id,
                org_role = EXCLUDED.org_role",
        user_id.as_str(),
        org_id,
        org_role,
    )
    .execute(pool)
    .await?;

    organization::invalidate(user_id).await;
    Ok(())
}

pub async fn find_organization_for_email(
    pool: &PgPool,
    email: &str,
) -> Result<Option<String>, MarketplaceError> {
    let Some((_, domain)) = email.rsplit_once('@').filter(|(_, d)| !d.is_empty()) else {
        return Ok(None);
    };
    let domain = domain.to_lowercase();

    let id = sqlx::query_scalar!(
        "SELECT id FROM organizations
         WHERE status = 'active' AND NOT is_platform AND $1 = ANY(email_domains)
         LIMIT 1",
        domain
    )
    .fetch_optional(pool)
    .await?;
    Ok(id)
}
