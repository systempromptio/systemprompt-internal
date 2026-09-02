//! Read-only Odoo questions the planner needs answered: is this sender already
//! a partner or an open lead, can tasks be created here at all, which partner
//! is the job owner's own, and which colleagues the named assignees are.
//!
//! Runs as whichever user the caller resolved — the proposal job uses its
//! owner's linked account. Nothing here writes.

use std::collections::HashMap;

use serde::Deserialize;
use systemprompt_mcp_odoo::client::{Credentials, OdooClient, SearchOptions};
use systemprompt_mcp_odoo::error::OdooError;
use systemprompt_mcp_odoo::{resolve, shape};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartnerRef {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeadRef {
    pub id: i64,
    pub name: String,
    pub partner_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserRef {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OdooLookup {
    pub partner: Option<PartnerRef>,
    pub lead: Option<LeadRef>,
    pub owner_partner: Option<PartnerRef>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OdooCapabilities {
    pub project: bool,
}

#[derive(Deserialize)]
struct PartnerRow {
    id: i64,
    #[serde(deserialize_with = "shape::text", default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct LeadRow {
    id: i64,
    #[serde(deserialize_with = "shape::text", default)]
    name: Option<String>,
    #[serde(deserialize_with = "shape::many2one_id", default)]
    partner_id: Option<i64>,
}

pub async fn lookup_sender(
    client: &OdooClient,
    creds: &Credentials,
    email: &str,
) -> Result<OdooLookup, OdooError> {
    let partner = first_partner(client, creds, email).await?;
    let lead = first_lead(client, creds, email, partner.as_ref().map(|p| p.id)).await?;
    Ok(OdooLookup {
        partner,
        lead,
        owner_partner: None,
    })
}

#[derive(Deserialize)]
struct UserPartnerRow {
    #[serde(deserialize_with = "many2one_link", default)]
    partner_id: Option<Many2OneLink>,
}

#[derive(Deserialize)]
struct Many2OneLink(i64, String);

fn many2one_link<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Many2OneLink>, D::Error> {
    // JSON: protocol boundary — `[id, "Display Name"]`, or `false` when unset.
    let v = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(v).ok())
}

pub async fn owner_partner(
    client: &OdooClient,
    creds: &Credentials,
) -> Result<Option<PartnerRef>, OdooError> {
    let rows = client
        .read(creds, "res.users", &[i64::from(creds.uid)], &["partner_id"])
        .await?;
    Ok(rows.first().and_then(|r| {
        let row: UserPartnerRow = serde_json::from_value(r.clone()).ok()?;
        row.partner_id
            .map(|Many2OneLink(id, name)| PartnerRef { id, name })
    }))
}

#[must_use]
pub fn assignee_key(name: &str) -> String {
    name.trim().to_lowercase()
}

// Why: an assignee Odoo cannot name unambiguously is dropped here, not
// errored, so the follow-up still lands — on the approver — rather than the
// whole proposal stalling on one misspelt colleague.
pub async fn resolve_assignees(
    client: &OdooClient,
    creds: &Credentials,
    names: &[String],
) -> HashMap<String, UserRef> {
    let mut resolved = HashMap::new();
    for name in names {
        let key = assignee_key(name);
        if key.is_empty() || resolved.contains_key(&key) {
            continue;
        }
        match resolve::user_id(client, creds, name).await {
            Ok(id) => {
                resolved.insert(
                    key,
                    UserRef {
                        id,
                        name: name.trim().to_owned(),
                    },
                );
            },
            Err(e) => {
                tracing::warn!(assignee = %name, error = %e, "knowledge_proposal: assignee not resolved; follow-up stays with the approver");
            },
        }
    }
    resolved
}

async fn first_partner(
    client: &OdooClient,
    creds: &Credentials,
    email: &str,
) -> Result<Option<PartnerRef>, OdooError> {
    let options = SearchOptions {
        fields: vec!["id".to_owned(), "name".to_owned()],
        limit: 1,
        order: Some("id asc".to_owned()),
    };
    // JSON: protocol boundary — an Odoo search domain.
    let domain = serde_json::json!([["email", "=ilike", email]]);
    let rows = client
        .search_read(creds, "res.partner", domain, &options)
        .await?;
    Ok(rows.first().and_then(|r| {
        let row: PartnerRow = serde_json::from_value(r.clone()).ok()?;
        Some(PartnerRef {
            id: row.id,
            name: row.name.unwrap_or_else(|| email.to_owned()),
        })
    }))
}

async fn first_lead(
    client: &OdooClient,
    creds: &Credentials,
    email: &str,
    partner_id: Option<i64>,
) -> Result<Option<LeadRef>, OdooError> {
    let options = SearchOptions {
        fields: vec!["id".to_owned(), "name".to_owned(), "partner_id".to_owned()],
        limit: 1,
        order: Some("write_date desc".to_owned()),
    };
    // JSON: protocol boundary — an Odoo search domain. The partner match is
    // included so a lead created from a contact form, whose email_from is
    // blank but whose partner is set, is still found.
    let domain = partner_id.map_or_else(
        || serde_json::json!([["active", "=", true], ["email_from", "=ilike", email]]),
        |pid| {
            serde_json::json!([
                ["active", "=", true],
                "|",
                ["email_from", "=ilike", email],
                ["partner_id", "=", pid]
            ])
        },
    );
    let rows = client
        .search_read(creds, "crm.lead", domain, &options)
        .await?;
    Ok(rows.first().and_then(|r| {
        let row: LeadRow = serde_json::from_value(r.clone()).ok()?;
        Some(LeadRef {
            id: row.id,
            name: row.name.unwrap_or_else(|| format!("lead #{}", row.id)),
            partner_id: row.partner_id,
        })
    }))
}

pub async fn capabilities(
    client: &OdooClient,
    creds: &Credentials,
) -> Result<OdooCapabilities, OdooError> {
    let options = SearchOptions {
        fields: vec!["id".to_owned()],
        limit: 1,
        order: None,
    };
    // JSON: protocol boundary — an Odoo search domain.
    let domain = serde_json::json!([["name", "=", "project"], ["state", "=", "installed"]]);
    let rows = client
        .search_read(creds, "ir.module.module", domain, &options)
        .await?;
    Ok(OdooCapabilities {
        project: !rows.is_empty(),
    })
}
