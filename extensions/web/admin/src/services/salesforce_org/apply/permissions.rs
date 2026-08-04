//! Permission sets, the `SetupEntityAccess` grants that pre-authorize the app,
//! and the `PermissionSetAssignment` rows that put users inside it.
//!
//! All three are ordinary createable sObjects, so this half of apply is plain
//! REST writes. Everything here is additive: it creates what is missing and
//! never deletes. Removing a permission set revokes access for everyone holding
//! it, which is not something a config apply should do implicitly.

use super::ApplyReport;
use crate::handlers::salesforce_auth::SalesforceError;
use crate::services::salesforce_org::client::Connection;
use crate::services::salesforce_org::spec::OrgSpec;

fn str_field(value: &serde_json::Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

/// Create the permission sets and app-access grants the spec calls for.
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

    for wanted in &spec.permission_sets {
        let found = existing
            .iter()
            .find(|p| str_field(p, "Name").as_ref() == Some(&wanted.name));
        let permset_id = if let Some(found) = found {
            str_field(found, "Id")
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
            .find(|a| str_field(a, "DeveloperName").as_ref() == Some(app_name))
            .and_then(|a| str_field(a, "Id"))
        else {
            report.manual_followups.push(format!(
                "permission set {} grants app {app_name}, which does not exist in this org",
                wanted.name
            ));
            continue;
        };

        let already = grants.iter().any(|g| {
            str_field(g, "ParentId").as_ref() == Some(&permset_id)
                && str_field(g, "SetupEntityId").as_deref() == Some(app_id.as_str())
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
/// A username with no matching active Salesforce user is recorded as a
/// follow-up rather than failing the apply — one stale row in the platform
/// database should not block configuring the org.
///
/// This must run *before*
/// [`apply_metadata`](crate::services::salesforce_org::apply::apply_metadata)
/// flips the app to `AdminApprovedPreAuthorized`. See the module docs.
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

    for username in usernames {
        let Some(user_id) = users
            .iter()
            .find(|u| str_field(u, "Username").as_ref() == Some(username))
            .and_then(|u| str_field(u, "Id"))
        else {
            report.manual_followups.push(format!(
                "no active Salesforce user with username {username} — cannot assign a \
                 permission set to them"
            ));
            continue;
        };
        let who = Assignee {
            username,
            user_id: &user_id,
            permsets: &permsets,
        };
        assign_one_user(conn, spec, who, report).await?;
    }
    Ok(())
}

// Why: grouped into a struct because the three travel together and always will
// — passing them separately pushed the function past the argument limit without
// making any call site clearer.
struct Assignee<'a> {
    username: &'a str,
    user_id: &'a str,
    permsets: &'a [serde_json::Value],
}

async fn assign_one_user(
    conn: &Connection,
    spec: &OrgSpec,
    who: Assignee<'_>,
    report: &mut ApplyReport,
) -> Result<(), SalesforceError> {
    let Assignee {
        username,
        user_id,
        permsets,
    } = who;

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
                .and_then(|p| str_field(p, "Name"))
                .as_ref()
                == Some(&permset.name)
        });
        if already {
            continue;
        }
        let Some(permset_id) = permsets
            .iter()
            .find(|p| str_field(p, "Name").as_ref() == Some(&permset.name))
            .and_then(|p| str_field(p, "Id"))
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
