//! Make an org match an [`OrgSpec`].
//!
//! Work splits by what Salesforce actually permits, which is not uniform:
//!
//! - **Metadata deploy** for the External Client App and its OAuth settings and
//!   policies. Those four sObjects report `createable: false`, so there is no
//!   REST write path — the Metadata API is the only way in.
//! - **REST writes** for permission sets and the `SetupEntityAccess` grants
//!   that pre-authorize the app. These are ordinary createable sObjects.
//! - **Assertions** for the standard hosted MCP servers. No API to activate one
//!   was found, so an inactive server is reported with instructions rather than
//!   silently skipped.
//!
//! The metadata element names below were read back from a live org by
//! submitting deliberately invalid packages under `checkOnly` and reading the
//! validation errors, which name every rejected element. They are not guesses.

use super::client::Connection;
use super::deploy::DeployResult;
use super::spec::OrgSpec;
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
pub fn build_package(spec: &OrgSpec) -> Vec<(String, String)> {
    let app = &spec.external_client_app;
    let name = &app.developer_name;
    let oauth_name = format!("{name}_oauth");
    let global_name = format!("{name}_glbloauth");
    let policy_name = format!("{name}_oauthPlcy");

    let mut eca = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ExternalClientApplication xmlns=\"{METADATA_NS}\">\n"
    );
    eca.push_str(&element("contactEmail", &app.contact_email));
    if let Some(description) = &app.description {
        eca.push_str(&element("description", description));
    }
    eca.push_str(&element("distributionState", &app.distribution_state));
    eca.push_str(&element("label", &app.label));
    eca.push_str("</ExternalClientApplication>\n");

    let mut global = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ExtlClntAppGlobalOauthSettings xmlns=\"{METADATA_NS}\">\n"
    );
    global.push_str(&element("callbackUrl", &app.oauth.callback_url));
    global.push_str(&element("externalClientApplication", name));
    global.push_str(&element(
        "isConsumerSecretOptional",
        &app.oauth.consumer_secret_optional.to_string(),
    ));
    global.push_str(&element(
        "isPkceRequired",
        &app.oauth.pkce_required.to_string(),
    ));
    global.push_str(&element("label", &global_name));
    global.push_str("</ExtlClntAppGlobalOauthSettings>\n");

    let scopes = app
        .oauth
        .scopes
        .iter()
        .map(|s| s.metadata_token())
        .collect::<Vec<_>>()
        .join(",");
    let mut settings = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ExtlClntAppOauthSettings xmlns=\"{METADATA_NS}\">\n"
    );
    settings.push_str(&element("commaSeparatedOauthScopes", &scopes));
    settings.push_str(&element("externalClientApplication", name));
    settings.push_str(&element(
        "isFirstPartyAppEnabled",
        &app.oauth.first_party_app_enabled.to_string(),
    ));
    settings.push_str(&element("label", &oauth_name));
    if let Some(url) = &app.oauth.single_logout_url {
        settings.push_str(&element("singleLogoutUrl", url));
    }
    settings.push_str("</ExtlClntAppOauthSettings>\n");

    let policies = &app.policies;
    let mut policy = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ExtlClntAppOauthConfigurablePolicies xmlns=\"{METADATA_NS}\">\n"
    );
    policy.push_str(&element("externalClientApplication", name));
    policy.push_str(&element(
        "ipRelaxationPolicyType",
        policies.ip_relaxation.metadata_token(),
    ));
    policy.push_str(&element("label", &policy_name));
    policy.push_str(&element(
        "permittedUsersPolicyType",
        &policies.permitted_users,
    ));
    policy.push_str(&element(
        "refreshTokenPolicyType",
        &policies.refresh_token_policy,
    ));
    if let Some(validity) = &policies.refresh_token_validity {
        policy.push_str(&element(
            "refreshTokenValidityPeriod",
            &validity.period.to_string(),
        ));
        policy.push_str(&element(
            "refreshTokenValidityUnit",
            validity.unit.metadata_token(),
        ));
    }
    if let Some(level) = &policies.required_session_level {
        policy.push_str(&element("requiredSessionLevel", level));
    }
    policy.push_str("</ExtlClntAppOauthConfigurablePolicies>\n");

    let package = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Package xmlns=\"{METADATA_NS}\">\n\
         {}{}{}{}    <version>{}</version>\n</Package>\n",
        types_block("ExternalClientApplication", &[name.clone()]),
        types_block("ExtlClntAppGlobalOauthSettings", &[global_name.clone()]),
        types_block("ExtlClntAppOauthSettings", &[oauth_name.clone()]),
        types_block(
            "ExtlClntAppOauthConfigurablePolicies",
            &[policy_name.clone()]
        ),
        super::client::API_VERSION,
    );

    vec![
        ("package.xml".to_owned(), package),
        (format!("externalClientApps/{name}.eca"), eca),
        (
            format!("extlClntAppGlobalOauthSets/{global_name}.ecaGlblOauth"),
            global,
        ),
        (
            format!("extlClntAppOauthSettings/{oauth_name}.ecaOauth"),
            settings,
        ),
        (
            format!("extlClntAppOauthPolicies/{policy_name}.ecaOauthPlcy"),
            policy,
        ),
    ]
}

fn types_block(name: &str, members: &[String]) -> String {
    let mut out = String::from("    <types>\n");
    for member in members {
        out.push_str(&format!(
            "        <members>{}</members>\n",
            xml_escape(member)
        ));
    }
    out.push_str(&format!("        <name>{name}</name>\n    </types>\n"));
    out
}

/// What an apply did, or would do.
#[derive(Debug, Default)]
pub struct ApplyReport {
    pub deploy: Option<DeployResult>,
    pub permission_sets_created: Vec<String>,
    pub app_grants_created: Vec<String>,
    /// Hosted MCP servers the spec requires that could not be verified. These
    /// need a human in Setup — there is no API to activate them.
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
    check_only: bool,
) -> Result<DeployResult, SalesforceError> {
    let package = build_package(spec);
    conn.deploy(&package, check_only).await
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
        let permset_id = match existing
            .iter()
            .find(|p| field(p, "Name").as_ref() == Some(&wanted.name))
        {
            Some(found) => field(found, "Id"),
            None => {
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
            },
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

/// Record the steps that have no API and must be done in Setup.
pub fn note_manual_steps(spec: &OrgSpec, report: &mut ApplyReport) {
    for server in &spec.hosted_mcp_servers {
        report.manual_followups.push(format!(
            "verify hosted MCP server '{}' is Active in Setup -> MCP Servers ({}). \
             Activation has no API.",
            server.name, server.endpoint
        ));
    }
}
