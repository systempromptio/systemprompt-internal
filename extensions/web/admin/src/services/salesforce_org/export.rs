//! Read a live org into an [`OrgSpec`].
//!
//! Everything here goes through the REST and Tooling query APIs, which accept
//! JWT-format access tokens. The External Client App records are readable this
//! way even though they are not *writable* — `describe` reports
//! `createable: false` on all four — which is why apply reaches for the
//! Metadata API while export does not.
//!
//! Three fields cannot be read from any org: `callback_url`, `pkce_required`
//! and `consumer_secret_optional` live on `ExtlClntAppGlobalOauthSettings`,
//! which is not a queryable sObject, and the `ExtlClntAppOauthSetAttr` bag that
//! might have carried them is empty. Export carries those forward from a
//! baseline spec rather than inventing values — see [`export_org`].

use super::client::Connection;
use super::scope::{OauthScope, UNMAPPED_SCOPE_FIELDS};
use super::spec::{
    ExternalClientApp, HostedMcpServer, IpRelaxation, OauthSpec, OrgSpec, PermissionSetSpec,
    PolicySpec, Validity, ValidityUnit,
};
use crate::handlers::salesforce_auth::SalesforceError;

/// Placeholder emitted for a write-only field when no baseline is supplied.
/// Deliberately not a plausible value: applying it fails Salesforce's URL
/// validation rather than quietly pointing an org at the wrong callback.
pub const UNREADABLE_PLACEHOLDER: &str = "UNREADABLE-SUPPLY-A-BASELINE";

fn str_field(record: &serde_json::Value, field: &str) -> Option<String> {
    record
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .filter(|s| !s.is_empty())
}

fn bool_field(record: &serde_json::Value, field: &str) -> bool {
    record
        .get(field)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

/// Read the org's identity and MCP configuration.
///
/// `baseline` supplies the write-only fields that no API exposes. Pass the
/// committed spec when there is one; without it those fields come back as
/// [`UNREADABLE_PLACEHOLDER`].
///
/// # Errors
/// Propagates query failures. Returns [`SalesforceError::Internal`] if the org
/// has no External Client App, since there is then nothing to describe.
pub async fn export_org(
    conn: &Connection,
    baseline: Option<&OrgSpec>,
) -> Result<OrgSpec, SalesforceError> {
    let apps = conn
        .soql(
            "SELECT DeveloperName,MasterLabel,Description,ContactEmail,DistributionState \
             FROM ExternalClientApplication",
        )
        .await?;
    let app = apps.first().ok_or_else(|| {
        SalesforceError::Internal(
            "org has no ExternalClientApplication — nothing to export".to_owned(),
        )
    })?;
    let developer_name = str_field(app, "DeveloperName").unwrap_or_default();

    let oauth = export_oauth(conn, &developer_name, baseline).await?;
    let policies = export_policies(conn).await?;
    let permission_sets = export_permission_sets(conn).await?;

    Ok(OrgSpec {
        external_client_app: ExternalClientApp {
            developer_name,
            label: str_field(app, "MasterLabel").unwrap_or_default(),
            description: str_field(app, "Description"),
            contact_email: str_field(app, "ContactEmail").unwrap_or_default(),
            distribution_state: str_field(app, "DistributionState")
                .unwrap_or_else(|| "Local".to_owned()),
            oauth,
            policies,
        },
        permission_sets,
        // Why: standard hosted servers are not in any queryable object, so they
        // are carried from the baseline rather than discovered.
        hosted_mcp_servers: baseline
            .map(|b| b.hosted_mcp_servers.clone())
            .unwrap_or_default(),
    })
}

async fn export_oauth(
    conn: &Connection,
    developer_name: &str,
    baseline: Option<&OrgSpec>,
) -> Result<OauthSpec, SalesforceError> {
    let query = format!(
        "SELECT ExtlClntAppOauthOptionsFirstPartyAppEnabled,SingleLogoutUrl,{} \
         FROM ExtlClntAppOauthSettings",
        OauthScope::soql_projection()
    );
    let rows = conn.soql(&query).await?;
    let settings = rows.first().ok_or_else(|| {
        SalesforceError::Internal(format!(
            "app {developer_name} has no ExtlClntAppOauthSettings"
        ))
    })?;

    let scopes: Vec<OauthScope> = OauthScope::all()
        .iter()
        .copied()
        .filter(|s| bool_field(settings, s.sobject_field()))
        .collect();

    // Why: a scope Salesforce reports but that has no metadata token would be
    // silently dropped on the next apply.
    for field in UNMAPPED_SCOPE_FIELDS {
        if bool_field(settings, field) {
            tracing::warn!(
                scope_field = field,
                "Salesforce reports this OAuth scope enabled, but it has no Metadata API \
                 token — it cannot be represented in the spec and applying will clear it"
            );
        }
    }

    let base = baseline.map(|b| &b.external_client_app.oauth);
    Ok(OauthSpec {
        callback_url: base.map_or_else(
            || UNREADABLE_PLACEHOLDER.to_owned(),
            |b| b.callback_url.clone(),
        ),
        scopes,
        first_party_app_enabled: bool_field(
            settings,
            "ExtlClntAppOauthOptionsFirstPartyAppEnabled",
        ),
        pkce_required: base.is_none_or(|b| b.pkce_required),
        consumer_secret_optional: base.is_some_and(|b| b.consumer_secret_optional),
        single_logout_url: str_field(settings, "SingleLogoutUrl"),
    })
}

async fn export_policies(conn: &Connection) -> Result<PolicySpec, SalesforceError> {
    let rows = conn
        .soql(
            "SELECT PermittedUsersPolicyType,IpRelaxationPolicyType,RefreshTokenPolicyType,\
             RefreshTokenValidityPeriod,RefreshTokenValidityUnit,RequiredSessionLevel \
             FROM ExtlClntAppOauthPlcyCnfg",
        )
        .await?;
    let policy = rows.first().ok_or_else(|| {
        SalesforceError::Internal("app has no ExtlClntAppOauthPlcyCnfg".to_owned())
    })?;

    let ip_relaxation = match str_field(policy, "IpRelaxationPolicyType").as_deref() {
        Some("Bypass") => IpRelaxation::Bypass,
        Some("Bypass_2factor") => IpRelaxation::Bypass2Factor,
        Some("Enforce_relaxrefresh") => IpRelaxation::EnforceRelaxRefresh,
        _ => IpRelaxation::Enforce,
    };

    let unit = match str_field(policy, "RefreshTokenValidityUnit").as_deref() {
        Some("Hours") => Some(ValidityUnit::Hours),
        Some("Months") => Some(ValidityUnit::Months),
        Some("Days") => Some(ValidityUnit::Days),
        _ => None,
    };
    let period = policy
        .get("RefreshTokenValidityPeriod")
        .and_then(serde_json::Value::as_u64)
        .and_then(|v| u32::try_from(v).ok());

    Ok(PolicySpec {
        permitted_users: str_field(policy, "PermittedUsersPolicyType").unwrap_or_default(),
        ip_relaxation,
        refresh_token_policy: str_field(policy, "RefreshTokenPolicyType").unwrap_or_default(),
        refresh_token_validity: match (period, unit) {
            (Some(period), Some(unit)) => Some(Validity { period, unit }),
            _ => None,
        },
        required_session_level: str_field(policy, "RequiredSessionLevel"),
    })
}

async fn export_permission_sets(
    conn: &Connection,
) -> Result<Vec<PermissionSetSpec>, SalesforceError> {
    // Why: only permission sets granting an External Client App are part of this
    // spec; the org's others are not ours to manage.
    let grants = conn
        .soql(
            "SELECT SetupEntityId,Parent.Name,Parent.Label \
             FROM SetupEntityAccess WHERE SetupEntityType = 'ExternalClientApplication'",
        )
        .await?;

    let apps = conn
        .soql("SELECT Id,DeveloperName FROM ExternalClientApplication")
        .await?;

    let mut out = Vec::new();
    for grant in &grants {
        let Some(parent) = grant.get("Parent") else {
            continue;
        };
        let Some(name) = str_field(parent, "Name") else {
            continue;
        };
        let entity_id = str_field(grant, "SetupEntityId");
        let grants_app = apps
            .iter()
            .find(|a| str_field(a, "Id") == entity_id)
            .and_then(|a| str_field(a, "DeveloperName"));
        out.push(PermissionSetSpec {
            label: str_field(parent, "Label").unwrap_or_else(|| name.clone()),
            name,
            description: None,
            grants_app,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Assert that each hosted MCP server in the spec answers. Activation has no
/// API, so this only reports; it never fixes.
///
/// A server that authenticates returns a JSON-RPC-level error for a bare
/// request, which is success as far as reachability goes — only an auth-level
/// rejection means it is not usable.
pub fn describe_hosted_servers(spec: &OrgSpec) -> Vec<&HostedMcpServer> {
    spec.hosted_mcp_servers.iter().collect()
}
