//! The send itself: relay, ledger, and the CRM write-back behind it.
//!
//! Split from `tool.rs` because the two halves answer different questions.
//! That module decides *whether* a call may send; this one carries out a send
//! that has already been authorised, and its whole shape follows from one
//! fact: once the relay has accepted the message, nothing here may turn the
//! call into a failure. The mail is gone. The honest outcomes are "sent" and
//! "sent but not logged", and the ledger is what tells them apart.

use rmcp::ErrorData as McpError;
use std::future::Future;
use systemprompt::database::DbPool;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::{CliArtifact, TextArtifact};
use systemprompt::models::execution::context::RequestContext;
use systemprompt_email::{EmailService, OutboundMessage, mint_message_id};

use crate::draft::SendEmailInput;
use crate::error::EmailToolError;
use crate::odoo_log::{SentMail, log_sent_mail};
use crate::outbox::{self, OutboxEntry};
use crate::tools::TOOL_EMAIL_SEND;

// Why: The handler that actually sends. Only ever reached with
// `GateOutcome::Proceed`.
#[derive(Debug)]
pub struct SendHandler {
    pub db_pool: DbPool,
    pub draft: SendEmailInput,
}

impl McpToolHandler for SendHandler {
    type Input = SendEmailInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_EMAIL_SEND
    }

    fn description(&self) -> &'static str {
        "Send an approved email draft."
    }

    fn handle(
        &self,
        _input: Self::Input,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let db_pool = std::sync::Arc::<systemprompt::database::Database>::clone(&self.db_pool);
        let draft = self.draft.clone();
        let user_id = ctx.user_id().clone();
        async move {
            send(&db_pool, &draft, &user_id)
                .await
                .map_err(McpError::from)
        }
    }
}

async fn send(
    db_pool: &DbPool,
    draft: &SendEmailInput,
    user_id: &systemprompt::identifiers::UserId,
) -> Result<(CliArtifact, String), EmailToolError> {
    let service = EmailService::from_env()?;
    let rfc5322_id = mint_message_id(service.sender_domain());

    outbox::claim(
        db_pool,
        &OutboxEntry {
            rfc5322_id: &rfc5322_id,
            user_id,
            recipients: &draft.to,
            subject: &draft.subject,
            res_model: draft.res_model.as_deref(),
            res_id: draft.res_id,
        },
    )
    .await;

    if let Err(e) = service
        .send_plain(&OutboundMessage {
            to: &draft.to,
            subject: &draft.subject,
            body: &draft.body,
            reply_to: draft.reply_to.as_deref(),
            rfc5322_id: &rfc5322_id,
        })
        .await
    {
        outbox::mark_failed(db_pool, &rfc5322_id, &e.to_string()).await;
        return Err(e.into());
    }
    outbox::mark_sent(db_pool, &rfc5322_id).await;

    // Why: Past this point the email has really been delivered. Nothing below may
    // turn the call into an error: the worst honest outcome is "sent, but not
    // logged", and the outbox row carries that for reconciliation.
    let logged = write_back(db_pool, draft, &service, &rfc5322_id, user_id).await;

    let mut summary = format!(
        "Email sent to {} (Message-ID {rfc5322_id})",
        draft.to.join(", ")
    );
    match logged {
        WriteBack::NotAnchored => {},
        WriteBack::Logged { odoo_message_id } => {
            let (model, id) = draft.anchor().unwrap_or(("", 0));
            summary.push_str(&format!(
                "; logged on {model} #{id} as message {odoo_message_id}"
            ));
        },
        WriteBack::Failed(ref error) => {
            summary.push_str(&format!(
                "; WARNING: the email was sent but could NOT be logged on its Odoo record ({error}). \
                 It is recorded in email_outbox for reconciliation."
            ));
        },
    }

    Ok((
        CliArtifact::text(TextArtifact::new(&summary).with_title("Email sent")),
        summary,
    ))
}

enum WriteBack {
    NotAnchored,
    Logged { odoo_message_id: i64 },
    Failed(String),
}

async fn write_back(
    db_pool: &DbPool,
    draft: &SendEmailInput,
    service: &EmailService,
    rfc5322_id: &str,
    user_id: &systemprompt::identifiers::UserId,
) -> WriteBack {
    let Some((res_model, res_id)) = draft.anchor() else {
        return WriteBack::NotAnchored;
    };

    let attempt = async {
        let creds = systemprompt_mcp_odoo::identity::resolve_credentials(db_pool, user_id).await?;
        let client = systemprompt_mcp_odoo::client::OdooClient::from_env()?;
        log_sent_mail(
            &client,
            &creds,
            &SentMail {
                res_model,
                res_id,
                subject: &draft.subject,
                body: &draft.body,
                email_from: &service.mailbox().to_string(),
                reply_to: draft.reply_to.as_deref(),
                rfc5322_id,
            },
        )
        .await
    }
    .await;

    match attempt {
        Ok(odoo_message_id) => {
            outbox::mark_logged(db_pool, rfc5322_id, odoo_message_id).await;
            WriteBack::Logged { odoo_message_id }
        },
        Err(e) => {
            let error = e.to_string();
            tracing::error!(
                error = %error,
                rfc5322_id,
                res_model,
                res_id,
                "email was sent but could not be logged on its Odoo record"
            );
            outbox::mark_log_failed(db_pool, rfc5322_id, &error).await;
            WriteBack::Failed(error)
        },
    }
}
