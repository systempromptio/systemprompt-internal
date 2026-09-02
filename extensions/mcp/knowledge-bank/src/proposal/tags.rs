//! Tagging an Odoo record with its email category.
//!
//! A `crm.lead` is tagged through `tag_ids` (`crm.tag`); a `res.partner`
//! through `category_id` (`res.partner.category`). Both are many2many fields,
//! written with Odoo's command tuples: `[6, 0, ids]` replaces the set, `[4,
//! id]` links one more. The tuples are tuple structs here so the command code
//! and its arity are fixed at the type, not retyped per call site.

use serde::Serialize;
use systemprompt_mcp_odoo::error::OdooError;
use systemprompt_mcp_odoo::resolve;

use super::OdooAction;
use super::apply::ApplyContext;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplaceLinks(pub i64, pub i64, pub Vec<i64>);

impl ReplaceLinks {
    #[must_use]
    pub const fn new(ids: Vec<i64>) -> Self {
        Self(6, 0, ids)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LinkOne(pub i64, pub i64);

impl LinkOne {
    #[must_use]
    pub const fn new(id: i64) -> Self {
        Self(4, id)
    }
}

#[derive(Serialize)]
struct TagLeadValues {
    tag_ids: [LinkOne; 1],
}

#[derive(Serialize)]
struct TagPartnerValues {
    category_id: [LinkOne; 1],
}

pub const LEAD_TAG_MODEL: &str = "crm.tag";
pub const PARTNER_TAG_MODEL: &str = "res.partner.category";

pub(super) async fn lead_tag_ids(
    ctx: &ApplyContext<'_>,
    names: &[String],
) -> Result<Vec<i64>, OdooError> {
    let mut ids = Vec::with_capacity(names.len());
    for name in names {
        ids.push(resolve::tag_id(ctx.client, ctx.creds, LEAD_TAG_MODEL, name).await?);
    }
    Ok(ids)
}

// Why: the link command is idempotent in Odoo, so a retry after a ledger
// failure re-links the same tag rather than duplicating it.
pub(super) async fn tag_record(
    ctx: &ApplyContext<'_>,
    action: &OdooAction,
    model: &str,
    res_id: i64,
) -> Result<(i64, Option<i64>), OdooError> {
    let OdooAction::TagRecord { tag, .. } = action else {
        return Err(OdooError::Internal("not a tag_record action".to_owned()));
    };
    let values = match model {
        "crm.lead" => {
            let id = resolve::tag_id(ctx.client, ctx.creds, LEAD_TAG_MODEL, tag).await?;
            serde_json::to_value(TagLeadValues {
                tag_ids: [LinkOne::new(id)],
            })?
        },
        "res.partner" => {
            let id = resolve::tag_id(ctx.client, ctx.creds, PARTNER_TAG_MODEL, tag).await?;
            serde_json::to_value(TagPartnerValues {
                category_id: [LinkOne::new(id)],
            })?
        },
        other => {
            return Err(OdooError::Internal(format!(
                "{other} records cannot be tagged"
            )));
        },
    };
    ctx.client.write(ctx.creds, model, res_id, values).await?;
    Ok((res_id, None))
}
