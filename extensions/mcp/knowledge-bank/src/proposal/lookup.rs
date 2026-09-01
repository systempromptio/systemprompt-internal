//! Read-only Odoo questions the planner needs answered: is this sender already
//! a partner or an open lead, and can tasks be created here at all.
//!
//! Runs as whichever user the caller resolved — the proposal job uses its
//! owner's linked account. Nothing here writes.

use serde::Deserialize;
use systemprompt_mcp_odoo::client::{Credentials, OdooClient, SearchOptions};
use systemprompt_mcp_odoo::error::OdooError;
use systemprompt_mcp_odoo::shape;

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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OdooLookup {
    pub partner: Option<PartnerRef>,
    pub lead: Option<LeadRef>,
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
    Ok(OdooLookup { partner, lead })
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
