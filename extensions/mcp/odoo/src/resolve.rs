//! Turning the names a person would say into the ids Odoo needs.
//!
//! Every tool that takes a user, a project or an activity type accepts a name,
//! not an id. That is a deliberate cost: it means an extra lookup per call, and
//! it means ambiguity has to be handled. The alternative is worse — a caller
//! forced to run `partner_search` before every `task_create` will either skip
//! it and guess an id, or give up and ask the user for one.
//!
//! When a name matches nothing, the error lists what *is* available. A bare
//! "not found" invites a model to try three more spellings; a list ends the
//! search in one turn.

use rmcp::ErrorData as McpError;

use crate::client::{Credentials, OdooClient, SearchOptions};

// Why: enough candidates to make a useful "did you mean", few enough that the
// list itself does not become the problem.
const SUGGESTION_LIMIT: u32 = 25;

async fn names(
    client: &OdooClient,
    creds: &Credentials,
    model: &str,
    field: &str,
) -> Result<Vec<String>, McpError> {
    let options = SearchOptions {
        fields: vec![field.to_owned()],
        limit: SUGGESTION_LIMIT,
        order: Some(format!("{field} asc")),
    };
    let records = client
        .search_read(creds, model, serde_json::json!([]), &options)
        .await?;
    Ok(records
        .iter()
        .filter_map(|r| crate::format::field(r, field))
        .collect())
}

/// Resolve an Odoo user by login or display name.
///
/// # Errors
/// No match, with the logins this account can see listed in the message.
pub async fn user_id(
    client: &OdooClient,
    creds: &Credentials,
    who: &str,
) -> Result<i64, McpError> {
    let pattern = format!("%{}%", who.trim());
    let options = SearchOptions {
        fields: vec!["id".to_owned(), "login".to_owned(), "name".to_owned()],
        limit: 2,
        order: Some("id asc".to_owned()),
    };
    let domain = serde_json::json!(["|", ["login", "ilike", pattern], ["name", "ilike", pattern]]);
    let matches = client
        .search_read(creds, "res.users", domain, &options)
        .await?;

    match matches.len() {
        1 => matches[0]
            .get("id")
            .and_then(serde_json::Value::as_i64)
            .ok_or_else(|| McpError::internal_error("res.users row has no id".to_owned(), None)),
        // Why: two matches is a real fork — assigning work to the wrong
        // colleague is worse than asking which one.
        n if n > 1 => Err(McpError::invalid_params(
            format!(
                "\"{who}\" matches more than one Odoo user. Use a full login to disambiguate."
            ),
            None,
        )),
        _ => {
            let available = names(client, creds, "res.users", "login").await?;
            Err(McpError::invalid_params(
                format!(
                    "No Odoo user matches \"{who}\". Available logins: {}.",
                    available.join(", ")
                ),
                None,
            ))
        },
    }
}

/// Resolve a project by name.
///
/// # Errors
/// No match, with the visible project names listed in the message.
pub async fn project_id(
    client: &OdooClient,
    creds: &Credentials,
    project: &str,
) -> Result<i64, McpError> {
    let options = SearchOptions {
        fields: vec!["id".to_owned(), "name".to_owned()],
        limit: 1,
        order: Some("id asc".to_owned()),
    };
    let domain = serde_json::json!([["name", "ilike", format!("%{}%", project.trim())]]);
    let matches = client
        .search_read(creds, "project.project", domain, &options)
        .await?;

    if let Some(id) = matches.first().and_then(|r| r.get("id")).and_then(serde_json::Value::as_i64)
    {
        return Ok(id);
    }
    let available = names(client, creds, "project.project", "name").await?;
    Err(McpError::invalid_params(
        format!(
            "No Odoo project matches \"{project}\". Available projects: {}.",
            if available.is_empty() {
                "none are visible to your account".to_owned()
            } else {
                available.join(", ")
            }
        ),
        None,
    ))
}

/// The `mail.activity.type` to schedule under.
///
/// Prefers a type named like "To Do", which is Odoo's default and what a plain
/// "follow this up" means. Falls back to whichever type the instance lists
/// first: an instance may use its own vocabulary, and scheduling under the
/// wrong type beats refusing to schedule at all.
///
/// # Errors
/// The instance has no activity types at all.
pub async fn activity_type_id(
    client: &OdooClient,
    creds: &Credentials,
) -> Result<i64, McpError> {
    let options = SearchOptions {
        fields: vec!["id".to_owned(), "name".to_owned()],
        limit: 1,
        order: Some("id asc".to_owned()),
    };
    let preferred = client
        .search_read(
            creds,
            "mail.activity.type",
            serde_json::json!([["name", "ilike", "to do"]]),
            &options,
        )
        .await?;
    if let Some(id) = preferred.first().and_then(|r| r.get("id")).and_then(serde_json::Value::as_i64)
    {
        return Ok(id);
    }

    let any = client
        .search_read(
            creds,
            "mail.activity.type",
            serde_json::json!([]),
            &options,
        )
        .await?;
    any.first()
        .and_then(|r| r.get("id"))
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| {
            McpError::internal_error(
                "This Odoo instance defines no activity types, so nothing can be scheduled."
                    .to_owned(),
                None,
            )
        })
}

/// The `ir.model` id for a model name, which `mail.activity` requires
/// alongside the record reference.
///
/// # Errors
/// Odoo does not know the model — usually a typo, or an app that is not
/// installed.
pub async fn model_id(
    client: &OdooClient,
    creds: &Credentials,
    model: &str,
) -> Result<i64, McpError> {
    let options = SearchOptions {
        fields: vec!["id".to_owned()],
        limit: 1,
        order: Some("id asc".to_owned()),
    };
    let matches = client
        .search_read(
            creds,
            "ir.model",
            serde_json::json!([["model", "=", model]]),
            &options,
        )
        .await?;
    matches
        .first()
        .and_then(|r| r.get("id"))
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| {
            McpError::invalid_params(
                format!("Odoo does not know a model called \"{model}\"."),
                None,
            )
        })
}
