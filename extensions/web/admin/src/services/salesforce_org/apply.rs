//! Make an org match an [`OrgSpec`].
//!
//! Work splits by what Salesforce actually permits, which is not uniform:
//!
//! - **Metadata deploy** for the External Client App and its OAuth settings and
//!   policies. Those four sObjects report `createable: false`, so there is no
//!   REST write path — the Metadata API is the only way in.
//! - **REST writes** for permission sets, the `SetupEntityAccess` grants that
//!   pre-authorize the app, and the `PermissionSetAssignment` rows that put
//!   users inside it. These are ordinary createable sObjects.
//! - **Tooling writes** for the standard hosted MCP servers. `McpServerAccess`
//!   is `updateable: true` from API version 67.0, so activation is a PATCH
//!   rather than the Setup click it used to be.
//!
//! # Ordering is load-bearing
//!
//! Permission sets, grants and assignments all run *before* the metadata
//! deploy. The deploy is what flips `permittedUsersPolicyType` to
//! `AdminApprovedPreAuthorized`, and from that moment only holders of the
//! permission set can authenticate. Deploying first opens a window in which
//! nobody — including the operator running this command — holds it yet.
//!
//! The metadata element names below were read back from a live org by
//! submitting deliberately invalid packages under `checkOnly` and reading the
//! validation errors, which name every rejected element. They are not guesses,
//! and they are version-specific: re-derive them when
//! [`METADATA_VERSION`](super::client::METADATA_VERSION) moves.

use super::client::Connection;
use super::deploy::DeployResult;
use super::spec::{ExternalClientApp, OrgSpec};
use crate::handlers::salesforce_auth::SalesforceError;

const METADATA_NS: &str = "http://soap.sforce.com/2006/04/metadata";

fn xml_escape(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn element(name: &str, value: &str) -> String {
    format!("    <{name}>{}</{name}>\n", xml_escape(value))
}

/// Build the deployable metadata package for an org spec.
///
/// Returned as `(path_in_zip, contents)` pairs so the caller can inspect or
/// print the package without deploying it — which is what makes `--dry-run`
/// able to show exactly what would be sent.
#[must_use]
pub fn build_package(spec: &OrgSpec, certificate: Option<&str>) -> Vec<(String, String)> {
    let app = &spec.external_client_app;
    let name = &app.developer_name;
    let oauth_name = format!("{name}_oauth");
    let global_name = format!("{name}_glbloauth");
    let policy_name = format!("{name}_oauthPlcy");

    let package = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Package xmlns=\"{METADATA_NS}\">\n\
         {}{}{}{}    <version>{}</version>\n</Package>\n",
        types_block("ExternalClientApplication", name),
        types_block("ExtlClntAppGlobalOauthSettings", &global_name),
        types_block("ExtlClntAppOauthSettings", &oauth_name),
        types_block("ExtlClntAppOauthConfigurablePolicies", &policy_name),
        super::client::METADATA_VERSION,
    );

    vec![
        ("package.xml".to_owned(), package),
        (format!("externalClientApps/{name}.eca"), build_eca(app)),
        (
            format!("extlClntAppGlobalOauthSets/{global_name}.ecaGlblOauth"),
            build_global_oauth(app, name, &global_name, certificate),
        ),
        (
            format!("extlClntAppOauthSettings/{oauth_name}.ecaOauth"),
            build_oauth_settings(app, name, &oauth_name),
        ),
        (
            format!("extlClntAppOauthPolicies/{policy_name}.ecaOauthPlcy"),
            build_policies(app, name, &policy_name),
        ),
    ]
}

fn build_eca(app: &ExternalClientApp) -> String {
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ExternalClientApplication xmlns=\"{METADATA_NS}\">\n"
    );
    out.push_str(&element("contactEmail", &app.contact_email));
    if let Some(description) = &app.description {
        out.push_str(&element("description", description));
    }
    out.push_str(&element("distributionState", &app.distribution_state));
    out.push_str(&element("label", &app.label));
    out.push_str("</ExternalClientApplication>\n");
    out
}

/// Strip a PEM certificate down to the base64 body the Metadata API wants.
///
/// Accepts a full PEM because that is what the operator has on disk next to the
/// private key; an already-bare base64 blob passes through unchanged.
fn certificate_body(pem: &str) -> String {
    pem.lines()
        .filter(|line| !line.starts_with("-----"))
        .map(str::trim)
        .collect()
}

fn build_global_oauth(
    app: &ExternalClientApp,
    name: &str,
    label: &str,
    certificate: Option<&str>,
) -> String {
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ExtlClntAppGlobalOauthSettings xmlns=\"{METADATA_NS}\">\n"
    );
    out.push_str(&element("callbackUrl", &app.oauth.callback_url));
    // Why: emitted, never omitted. `certificate` is in schema here, and a
    // declarative deploy that leaves it out clears the app's digital signature —
    // which is the credential this tool authenticates with. Omitting it once
    // cost a live org its JWT-bearer grant.
    if let Some(pem) = certificate {
        out.push_str(&element("certificate", &certificate_body(pem)));
    }
    out.push_str(&element("externalClientApplication", name));
    out.push_str(&element(
        "isConsumerSecretOptional",
        &app.oauth.consumer_secret_optional.to_string(),
    ));
    // Why: emitted explicitly rather than omitted. The deploy is declarative,
    // and this element came into schema at metadata version 67.0 — leaving it
    // out would take the default and stop the org issuing the JWT-format access
    // tokens the REST metadata deploy depends on.
    out.push_str(&element(
        "isNamedUserJwtEnabled",
        &app.oauth.named_user_jwt.to_string(),
    ));
    out.push_str(&element(
        "isPkceRequired",
        &app.oauth.pkce_required.to_string(),
    ));
    out.push_str(&element("label", label));
    out.push_str("</ExtlClntAppGlobalOauthSettings>\n");
    out
}

fn build_oauth_settings(app: &ExternalClientApp, name: &str, label: &str) -> String {
    let scopes = app
        .oauth
        .scopes
        .iter()
        .map(|s| s.metadata_token())
        .collect::<Vec<_>>()
        .join(",");
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ExtlClntAppOauthSettings xmlns=\"{METADATA_NS}\">\n"
    );
    out.push_str(&element("commaSeparatedOauthScopes", &scopes));
    out.push_str(&element("externalClientApplication", name));
    out.push_str(&element(
        "isFirstPartyAppEnabled",
        &app.oauth.first_party_app_enabled.to_string(),
    ));
    out.push_str(&element("label", label));
    if let Some(url) = &app.oauth.single_logout_url {
        out.push_str(&element("singleLogoutUrl", url));
    }
    out.push_str("</ExtlClntAppOauthSettings>\n");
    out
}

fn build_policies(app: &ExternalClientApp, name: &str, label: &str) -> String {
    let policies = &app.policies;
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ExtlClntAppOauthConfigurablePolicies xmlns=\"{METADATA_NS}\">\n"
    );
    out.push_str(&element("externalClientApplication", name));
    out.push_str(&element(
        "ipRelaxationPolicyType",
        policies.ip_relaxation.metadata_token(),
    ));
    out.push_str(&element("label", label));
    out.push_str(&element(
        "permittedUsersPolicyType",
        &policies.permitted_users,
    ));
    out.push_str(&element(
        "refreshTokenPolicyType",
        &policies.refresh_token_policy,
    ));
    if let Some(validity) = &policies.refresh_token_validity {
        out.push_str(&element(
            "refreshTokenValidityPeriod",
            &validity.period.to_string(),
        ));
        out.push_str(&element(
            "refreshTokenValidityUnit",
            validity.unit.metadata_token(),
        ));
    }
    if let Some(level) = &policies.required_session_level {
        out.push_str(&element("requiredSessionLevel", level));
    }
    out.push_str("</ExtlClntAppOauthConfigurablePolicies>\n");
    out
}

fn types_block(name: &str, member: &str) -> String {
    let mut out = String::from("    <types>\n");
    out.push_str(&format!(
        "        <members>{}</members>\n",
        xml_escape(member)
    ));
    out.push_str(&format!("        <name>{name}</name>\n    </types>\n"));
    out
}

/// What an apply did, or would do.
#[derive(Debug, Default)]
pub struct ApplyReport {
    pub deploy: Option<DeployResult>,
    pub permission_sets_created: Vec<String>,
    pub app_grants_created: Vec<String>,
    /// `username -> permission set` pairs newly assigned.
    pub assignments_created: Vec<String>,
    /// Hosted MCP servers switched from inactive to active.
    pub servers_activated: Vec<String>,
    /// Things apply could not do and a human must resolve — a server the org
    /// does not offer, a username with no matching Salesforce user, or an
    /// unreachable platform database.
    pub manual_followups: Vec<String>,
}

/// Apply the app-level metadata.
///
/// With `check_only` the deploy is validated in full and nothing is written,
/// which is what `--dry-run` uses. Salesforce reports component-level failures
/// either way.
///
/// # Errors
/// Propagates deploy failures. A deploy that runs but reports component errors
/// returns `Ok` with an unsuccessful [`DeployResult`] — inspect
/// [`DeployResult::failure_lines`].
pub async fn apply_metadata(
    conn: &Connection,
    spec: &OrgSpec,
    certificate: Option<&str>,
    check_only: bool,
) -> Result<DeployResult, SalesforceError> {
    check_certificate_present(certificate)?;
    let package = build_package(spec, certificate);
    conn.deploy(&package, check_only).await
}

/// Refuse to deploy a package that would clear the app's signing certificate.
///
/// A metadata deploy is declarative: `certificate` is in schema on
/// `ExtlClntAppGlobalOauthSettings`, so a package that omits it clears the
/// digital signature — and the JWT-bearer grant this whole tool authenticates
/// with then fails with `invalid_grant: invalid assertion`. The certificate is
/// not readable back through any API, so apply cannot preserve it by round-trip
/// and must be given it.
///
/// This is a guard, not a fix: it converts a silent, self-inflicted lockout into
/// a refusal before anything is sent.
///
/// # Errors
/// [`SalesforceError::Internal`] naming the variable to set.
pub fn check_certificate_present(certificate: Option<&str>) -> Result<(), SalesforceError> {
    if certificate.is_some_and(|c| !c.trim().is_empty()) {
        return Ok(());
    }
    Err(SalesforceError::Internal(
        "refusing to deploy: SF_TARGET_CERTIFICATE is not set. A metadata deploy is \
         declarative, so a package without <certificate> clears the External Client App's \
         digital signature and the JWT-bearer grant stops working (invalid_grant: invalid \
         assertion). Set SF_TARGET_CERTIFICATE to the PEM certificate matching \
         SF_TARGET_PRIVATE_KEY."
            .to_owned(),
    ))
}

/// Create the permission sets and app-access grants the spec calls for.
///
/// Additive by design: it creates what is missing and never deletes. Removing a
/// permission set revokes access for everyone holding it, which is not
/// something a config apply should do implicitly.
///
/// # Errors
/// Propagates query and create failures.
pub async fn apply_permission_sets(
    conn: &Connection,
    spec: &OrgSpec,
    report: &mut ApplyReport,
) -> Result<(), SalesforceError> {
    let existing = conn
        .soql("SELECT Id,Name FROM PermissionSet WHERE IsOwnedByProfile = false")
        .await?;
    let apps = conn
        .soql("SELECT Id,DeveloperName FROM ExternalClientApplication")
        .await?;
    let grants = conn
        .soql(
            "SELECT SetupEntityId,ParentId FROM SetupEntityAccess \
             WHERE SetupEntityType = 'ExternalClientApplication'",
        )
        .await?;

    let field = |v: &serde_json::Value, f: &str| {
        v.get(f)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };

    for wanted in &spec.permission_sets {
        let found = existing
            .iter()
            .find(|p| field(p, "Name").as_ref() == Some(&wanted.name));
        let permset_id = if let Some(found) = found {
            field(found, "Id")
        } else {
            let mut body = serde_json::json!({
                "Name": wanted.name,
                "Label": wanted.label,
            });
            if let Some(description) = &wanted.description {
                body["Description"] = serde_json::Value::String(description.clone());
            }
            let id = conn.create_sobject("PermissionSet", &body, false).await?;
            report.permission_sets_created.push(wanted.name.clone());
            Some(id)
        };

        let (Some(permset_id), Some(app_name)) = (permset_id, wanted.grants_app.as_ref()) else {
            continue;
        };
        let Some(app_id) = apps
            .iter()
            .find(|a| field(a, "DeveloperName").as_ref() == Some(app_name))
            .and_then(|a| field(a, "Id"))
        else {
            report.manual_followups.push(format!(
                "permission set {} grants app {app_name}, which does not exist in this org",
                wanted.name
            ));
            continue;
        };

        let already = grants.iter().any(|g| {
            field(g, "ParentId").as_ref() == Some(&permset_id)
                && field(g, "SetupEntityId").as_deref() == Some(app_id.as_str())
        });
        if !already {
            conn.create_sobject(
                "SetupEntityAccess",
                &serde_json::json!({ "ParentId": permset_id, "SetupEntityId": app_id }),
                false,
            )
            .await?;
            report
                .app_grants_created
                .push(format!("{} -> {app_name}", wanted.name));
        }
    }
    Ok(())
}

/// Assign every spec permission set to each of `usernames`.
///
/// Additive: it creates the `PermissionSetAssignment` rows that are missing and
/// never revokes one. A username with no matching active Salesforce user is
/// recorded as a follow-up rather than failing the apply — one stale row in the
/// platform database should not block configuring the org.
///
/// This must run *before* [`apply_metadata`] flips the app to
/// `AdminApprovedPreAuthorized`. See the module docs.
///
/// # Errors
/// Propagates query failures. Individual assignment creates that Salesforce
/// rejects are collected as follow-ups.
pub async fn apply_assignments(
    conn: &Connection,
    spec: &OrgSpec,
    usernames: &[String],
    report: &mut ApplyReport,
) -> Result<(), SalesforceError> {
    if usernames.is_empty() {
        return Ok(());
    }
    let wanted: Vec<&str> = spec
        .permission_sets
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    if wanted.is_empty() {
        return Ok(());
    }

    let permsets = conn
        .soql(&format!(
            "SELECT Id,Name FROM PermissionSet WHERE Name IN ({})",
            soql_list(&wanted)
        ))
        .await?;
    let users = conn
        .soql(&format!(
            "SELECT Id,Username FROM User WHERE IsActive = true AND Username IN ({})",
            soql_list(&usernames.iter().map(String::as_str).collect::<Vec<_>>())
        ))
        .await?;

    let field = |v: &serde_json::Value, f: &str| {
        v.get(f)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };

    for username in usernames {
        let Some(user_id) = users
            .iter()
            .find(|u| field(u, "Username").as_ref() == Some(username))
            .and_then(|u| field(u, "Id"))
        else {
            report.manual_followups.push(format!(
                "no active Salesforce user with username {username} — cannot assign a \
                 permission set to them"
            ));
            continue;
        };
        assign_one_user(conn, spec, &permsets, username, &user_id, report).await?;
    }
    Ok(())
}

/// Give one user every permission set the spec names that they do not hold.
async fn assign_one_user(
    conn: &Connection,
    spec: &OrgSpec,
    permsets: &[serde_json::Value],
    username: &str,
    user_id: &str,
    report: &mut ApplyReport,
) -> Result<(), SalesforceError> {
    let field = |v: &serde_json::Value, f: &str| {
        v.get(f)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };

    // Why: queried per user rather than once for everyone. The existing-
    // assignment set is small and this keeps the SOQL well inside the governor
    // limits an org with many users would otherwise breach.
    let held = conn
        .soql(&format!(
            "SELECT PermissionSet.Name FROM PermissionSetAssignment WHERE AssigneeId = '{}'",
            soql_escape(user_id)
        ))
        .await?;

    for permset in &spec.permission_sets {
        let already = held.iter().any(|h| {
            h.get("PermissionSet")
                .and_then(|p| field(p, "Name"))
                .as_ref()
                == Some(&permset.name)
        });
        if already {
            continue;
        }
        let Some(permset_id) = permsets
            .iter()
            .find(|p| field(p, "Name").as_ref() == Some(&permset.name))
            .and_then(|p| field(p, "Id"))
        else {
            report.manual_followups.push(format!(
                "permission set {} does not exist in this org — cannot assign it",
                permset.name
            ));
            continue;
        };
        match conn
            .create_sobject(
                "PermissionSetAssignment",
                &serde_json::json!({ "AssigneeId": user_id, "PermissionSetId": permset_id }),
                false,
            )
            .await
        {
            Ok(_) => report
                .assignments_created
                .push(format!("{username} -> {}", permset.name)),
            Err(e) => report.manual_followups.push(format!(
                "could not assign {} to {username}: {e}",
                permset.name
            )),
        }
    }
    Ok(())
}

/// Switch on the hosted MCP servers the spec wants active.
///
/// `McpServerAccess` is a Tooling object, `updateable: true` from API version
/// 67.0. Additive like the rest of apply: a server the spec marks inactive is
/// left alone, and a server active in the org but absent from the spec is not
/// touched.
///
/// # Errors
/// Propagates the Tooling query failure. A PATCH Salesforce rejects is recorded
/// as a follow-up rather than aborting.
pub async fn apply_hosted_mcp_servers(
    conn: &Connection,
    spec: &OrgSpec,
    report: &mut ApplyReport,
) -> Result<(), SalesforceError> {
    if spec.hosted_mcp_servers.is_empty() {
        return Ok(());
    }
    let rows = conn
        .tooling_soql("SELECT Id,DeveloperName,Active FROM McpServerAccess")
        .await?;

    for server in &spec.hosted_mcp_servers {
        let Some(row) = rows.iter().find(|r| {
            r.get("DeveloperName").and_then(serde_json::Value::as_str)
                == Some(server.developer_name.as_str())
        }) else {
            // Why: an error, not a follow-up to shrug at. Absence means the org
            // does not offer this server at all, which activation cannot fix.
            report.manual_followups.push(format!(
                "hosted MCP server '{}' ({}) is not present in this org — no \
                 McpServerAccess record named {}. The org does not offer it; \
                 activation cannot fix that.",
                server.name, server.endpoint, server.developer_name
            ));
            continue;
        };
        let active = row
            .get("Active")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if active || !server.active {
            continue;
        }
        let Some(id) = row.get("Id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        match conn
            .update_sobject(
                "McpServerAccess",
                id,
                &serde_json::json!({ "Active": true }),
                true,
            )
            .await
        {
            Ok(()) => report.servers_activated.push(server.name.clone()),
            Err(e) => report.manual_followups.push(format!(
                "could not activate hosted MCP server '{}': {e}",
                server.name
            )),
        }
    }
    Ok(())
}

// Why: SOQL string literals are single-quoted with backslash escapes. These
// values are Salesforce usernames and permission set API names rather than free
// text, but building a query by concatenation without escaping is the kind of
// thing that stops being true later.
fn soql_escape(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('\'', "\\'")
}

fn soql_list(values: &[&str]) -> String {
    values
        .iter()
        .map(|v| format!("'{}'", soql_escape(v)))
        .collect::<Vec<_>>()
        .join(",")
}
