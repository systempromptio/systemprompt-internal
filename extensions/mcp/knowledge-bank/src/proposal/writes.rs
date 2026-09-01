//! The typed Odoo writes an approved proposal performs.
//!
//! Every value struct here is a `Serialize` struct rather than a `json!`
//! literal (repository rule 8) so a load-bearing field — `partner_ids` on the
//! chatter post above all — cannot be quietly omitted at a call site. Chatter
//! is posted as `mail.mt_note` with no recipients: this email has already
//! been delivered once, to brain@, and Odoo must not deliver it again. The
//! inbound Message-ID goes on the `mail.message`, and is searched for before
//! posting, so the same email is never logged twice on one record.

use serde::{Deserialize, Serialize};
use systemprompt_mcp_odoo::client::{ModelCall, SearchOptions};
use systemprompt_mcp_odoo::error::OdooError;
use systemprompt_mcp_odoo::resolve;

use super::OdooAction;
use super::apply::{ApplyContext, ApplySource};

#[derive(Serialize)]
struct CreateLeadValues<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    contact_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    partner_name: Option<&'a str>,
    email_from: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    partner_id: Option<i64>,
    description: &'a str,
}

#[derive(Serialize)]
struct LogInboundMail<'a> {
    body: &'a str,
    subject: &'a str,
    message_type: &'static str,
    subtype_xmlid: &'static str,
    // Why: always empty — any partner listed here is emailed the message.
    partner_ids: [i64; 0],
    email_from: &'a str,
    #[serde(rename = "message_id")]
    rfc5322_id: &'a str,
}

#[derive(Serialize)]
struct CreateActivityValues<'a> {
    res_model_id: i64,
    res_model: &'a str,
    res_id: i64,
    summary: &'a str,
    note: &'a str,
    date_deadline: &'a str,
    user_id: i64,
    activity_type_id: i64,
}

#[derive(Serialize)]
struct CreateTaskValues<'a> {
    name: &'a str,
    project_id: i64,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    date_deadline: Option<&'a str>,
}

#[derive(Deserialize)]
struct MessageIdRow {
    id: i64,
}

pub(super) async fn create_lead(
    ctx: &ApplyContext<'_>,
    action: &OdooAction,
) -> Result<(i64, Option<i64>), OdooError> {
    let OdooAction::CreateLead {
        title,
        contact_name,
        partner_name,
        email_from,
        partner_id,
        description,
    } = action
    else {
        return Err(OdooError::Internal("not a create_lead action".to_owned()));
    };
    let values = serde_json::to_value(CreateLeadValues {
        name: title,
        contact_name: contact_name.as_deref(),
        partner_name: partner_name.as_deref(),
        email_from,
        partner_id: *partner_id,
        description,
    })?;
    let id = ctx.client.create(ctx.creds, "crm.lead", values).await?;
    Ok((id, None))
}

pub(super) async fn post_chatter(
    ctx: &ApplyContext<'_>,
    source: &ApplySource<'_>,
    model: &str,
    res_id: i64,
) -> Result<(i64, Option<i64>), OdooError> {
    if let Some(existing) = find_logged_message(ctx, source.rfc5322_id, model, res_id).await? {
        return Ok((res_id, Some(existing)));
    }
    let kwargs = serde_json::to_value(LogInboundMail {
        body: source.body_html,
        subject: source.subject,
        message_type: "email",
        subtype_xmlid: "mail.mt_note",
        partner_ids: [],
        email_from: source.email_from,
        rfc5322_id: source.rfc5322_id,
    })?;
    let posted = ctx
        .client
        .execute_kw(
            ctx.creds,
            ModelCall {
                model,
                method: "message_post",
                // JSON: protocol boundary — execute_kw positional args.
                args: serde_json::json!([[res_id]]),
                kwargs,
            },
        )
        .await?;
    Ok((res_id, posted.as_i64()))
}

async fn find_logged_message(
    ctx: &ApplyContext<'_>,
    rfc5322_id: &str,
    model: &str,
    res_id: i64,
) -> Result<Option<i64>, OdooError> {
    let options = SearchOptions {
        fields: vec!["id".to_owned()],
        limit: 1,
        order: None,
    };
    // JSON: protocol boundary — an Odoo search domain.
    let domain = serde_json::json!([
        ["message_id", "=", rfc5322_id],
        ["model", "=", model],
        ["res_id", "=", res_id]
    ]);
    let rows = ctx
        .client
        .search_read(ctx.creds, "mail.message", domain, &options)
        .await?;
    Ok(rows
        .first()
        .and_then(|r| serde_json::from_value::<MessageIdRow>(r.clone()).ok())
        .map(|r| r.id))
}

pub(super) async fn create_activity(
    ctx: &ApplyContext<'_>,
    action: &OdooAction,
    model: &str,
    res_id: i64,
) -> Result<(i64, Option<i64>), OdooError> {
    let OdooAction::CreateActivity {
        summary,
        note,
        date_deadline,
        ..
    } = action
    else {
        return Err(OdooError::Internal(
            "not a create_activity action".to_owned(),
        ));
    };
    let (activity_type_id, res_model_id) = tokio::try_join!(
        resolve::activity_type_id(ctx.client, ctx.creds),
        resolve::model_id(ctx.client, ctx.creds, model),
    )?;
    let values = serde_json::to_value(CreateActivityValues {
        res_model_id,
        res_model: model,
        res_id,
        summary,
        note,
        date_deadline,
        user_id: i64::from(ctx.creds.uid),
        activity_type_id,
    })?;
    let id = ctx
        .client
        .create(ctx.creds, "mail.activity", values)
        .await?;
    Ok((id, None))
}

pub(super) async fn create_task(
    ctx: &ApplyContext<'_>,
    action: &OdooAction,
    model: &str,
    res_id: i64,
) -> Result<(i64, Option<i64>), OdooError> {
    let OdooAction::CreateTask {
        project,
        name,
        description,
        date_deadline,
        ..
    } = action
    else {
        return Err(OdooError::Internal("not a create_task action".to_owned()));
    };
    let project_id = resolve::project_id(ctx.client, ctx.creds, project).await?;
    let record = ctx.client.connection().record_url(model, res_id);
    let values = serde_json::to_value(CreateTaskValues {
        name,
        project_id,
        description: format!(
            "<p>{}</p><p><a href=\"{record}\">Source record</a></p>",
            super::body::escape(description)
        ),
        date_deadline: date_deadline.as_deref(),
    })?;
    let id = ctx.client.create(ctx.creds, "project.task", values).await?;
    Ok((id, None))
}
