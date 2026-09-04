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
//!
//! Every resolver returns [`OdooError`] rather than an MCP error so the same
//! lookups serve a scheduled job as well as a tool call; the MCP layer maps
//! `Unresolved` to `invalid_params` at its boundary.

use serde::{Deserialize, Serialize};

use crate::client::{Credentials, OdooClient, SearchOptions};
use crate::error::OdooError;

#[derive(Deserialize)]
struct IdRow {
    id: i64,
}

#[derive(Serialize)]
struct NamedValues<'a> {
    name: &'a str,
}

async fn first_id_by_name(
    client: &OdooClient,
    creds: &Credentials,
    model: &str,
    name: &str,
) -> Result<Option<i64>, OdooError> {
    let options = SearchOptions {
        fields: vec!["id".to_owned()],
        limit: 1,
        order: Some("id asc".to_owned()),
    };
    // JSON: protocol boundary — an Odoo search domain.
    let domain = serde_json::json!([["name", "=ilike", name.trim()]]);
    let rows = client.search_read(creds, model, domain, &options).await?;
    Ok(rows
        .first()
        .and_then(|r| serde_json::from_value::<IdRow>(r.clone()).ok())
        .map(|r| r.id))
}

// Why: a stage the model guessed and Odoo lacks must not sink the lead; the
// lead lands in the default stage instead.
pub async fn stage_id(
    client: &OdooClient,
    creds: &Credentials,
    name: &str,
) -> Result<Option<i64>, OdooError> {
    first_id_by_name(client, creds, "crm.stage", name).await
}

// Why: search-then-create keeps one row per tag name even when two proposals
// carrying the same new tag are approved in sequence.
pub async fn tag_id(
    client: &OdooClient,
    creds: &Credentials,
    model: &str,
    name: &str,
) -> Result<i64, OdooError> {
    if let Some(id) = first_id_by_name(client, creds, model, name).await? {
        return Ok(id);
    }
    let values = serde_json::to_value(NamedValues { name: name.trim() })?;
    client.create(creds, model, values).await
}

// Why: enough candidates to make a useful "did you mean", few enough that the
// list itself does not become the problem.
const SUGGESTION_LIMIT: u32 = 25;

async fn names(
    client: &OdooClient,
    creds: &Credentials,
    model: &str,
    field: &str,
) -> Result<Vec<String>, OdooError> {
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

pub async fn user_id(
    client: &OdooClient,
    creds: &Credentials,
    who: &str,
) -> Result<i64, OdooError> {
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
            .ok_or_else(|| OdooError::Odoo("res.users row has no id".to_owned())),
        // Why: two matches is a real fork — assigning work to the wrong
        // colleague is worse than asking which one.
        n if n > 1 => Err(OdooError::Unresolved(format!(
            "\"{who}\" matches more than one Odoo user. Use a full login to disambiguate."
        ))),
        _ => {
            let available = names(client, creds, "res.users", "login").await?;
            Err(OdooError::Unresolved(format!(
                "No Odoo user matches \"{who}\". Available logins: {}.",
                available.join(", ")
            )))
        },
    }
}

pub async fn project_id(
    client: &OdooClient,
    creds: &Credentials,
    project: &str,
) -> Result<i64, OdooError> {
    let options = SearchOptions {
        fields: vec!["id".to_owned(), "name".to_owned()],
        limit: 1,
        order: Some("id asc".to_owned()),
    };
    let domain = serde_json::json!([["name", "ilike", format!("%{}%", project.trim())]]);
    let matches = client
        .search_read(creds, "project.project", domain, &options)
        .await?;

    if let Some(id) = matches
        .first()
        .and_then(|r| r.get("id"))
        .and_then(serde_json::Value::as_i64)
    {
        return Ok(id);
    }
    let available = names(client, creds, "project.project", "name").await?;
    Err(OdooError::Unresolved(format!(
        "No Odoo project matches \"{project}\". Available projects: {}.",
        if available.is_empty() {
            "none are visible to your account".to_owned()
        } else {
            available.join(", ")
        }
    )))
}

pub async fn activity_type_id(client: &OdooClient, creds: &Credentials) -> Result<i64, OdooError> {
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
    if let Some(id) = preferred
        .first()
        .and_then(|r| r.get("id"))
        .and_then(serde_json::Value::as_i64)
    {
        return Ok(id);
    }

    let any = client
        .search_read(creds, "mail.activity.type", serde_json::json!([]), &options)
        .await?;
    any.first()
        .and_then(|r| r.get("id"))
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| {
            OdooError::Unresolved(
                "This Odoo instance defines no activity types, so nothing can be scheduled."
                    .to_owned(),
            )
        })
}

/// Resolve an activity type by the name a person would say ("Call").
///
/// Falls back to [`activity_type_id`]'s default when `named` is absent, so a
/// caller that does not care still gets a to-do rather than an error.
pub async fn activity_type_id_named(
    client: &OdooClient,
    creds: &Credentials,
    named: Option<&str>,
) -> Result<i64, OdooError> {
    let Some(name) = named.map(str::trim).filter(|n| !n.is_empty()) else {
        return activity_type_id(client, creds).await;
    };
    if let Some(id) = first_id_by_name(client, creds, "mail.activity.type", name).await? {
        return Ok(id);
    }
    let available = names(client, creds, "mail.activity.type", "name").await?;
    Err(OdooError::Unresolved(format!(
        "No Odoo activity type named \"{name}\". Available types: {}.",
        available.join(", ")
    )))
}

pub async fn model_id(
    client: &OdooClient,
    creds: &Credentials,
    model: &str,
) -> Result<i64, OdooError> {
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
            OdooError::Unresolved(format!("Odoo does not know a model called \"{model}\"."))
        })
}
