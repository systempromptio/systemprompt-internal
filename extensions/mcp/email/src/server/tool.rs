//! The two MRTR rounds, and the send behind them.
//!
//! # The shape of a call
//!
//! ```text
//! round 1  no inputResponses        -> preview artifact + InputRequired{inputRequests}
//! round 2  confirm accepted         -> enforce_approval -> Proceed | Held | Refused
//! send     Proceed                  -> SMTP -> Odoo chatter -> sent artifact
//! ```
//!
//! Two independent humans can be involved, and they are answering different
//! questions. Round 1 asks the person who drafted the mail whether the text is
//! right. `require_approval` then asks a *different* person whether it should
//! go out at all — which is why `exempt_scopes: [admin]` is set on that policy:
//! an admin approving their own send is a rubber stamp, not a control.
//!
//! # Why the send reads the draft from `arguments`, not from the confirm round
//!
//! The confirm round carries only a boolean. Everything sent is re-parsed from
//! `request.arguments`, which is also what `derive_call_id` digests — so the
//! approval a human granted is bound to the exact bytes that get sent. A client
//! that alters the body on the retry changes the call id, which correctly
//! re-opens approval rather than riding the previous decision.

use rmcp::ErrorData as McpError;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, InputRequiredResult,
};
use std::future::Future;
use systemprompt::database::DbPool;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::{ClientProfile, McpToolExecutor, McpToolHandler};
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;
use systemprompt_mcp_shared::approval::{GateOutcome, enforce_approval};

use super::send::SendHandler;
use crate::draft::{Confirmation, SendEmailInput, confirmation};
use crate::tools::{SERVER_NAME, TOOL_EMAIL_SEND};

#[derive(Debug)]
pub struct Dispatch<'a> {
    pub db_pool: &'a DbPool,
    pub executor: &'a McpToolExecutor,
    pub request: &'a CallToolRequestParams,
    pub request_context: &'a RequestContext,
    pub client: &'a ClientProfile,
}

// Why: Routes one `tools/call` to whichever round it belongs to.
pub async fn dispatch_tool(
    dispatch: &Dispatch<'_>,
    tool_name: &str,
) -> Result<CallToolResponse, McpError> {
    if tool_name != TOOL_EMAIL_SEND {
        return Err(McpError::invalid_params(
            format!("Unknown tool: {tool_name}"),
            None,
        ));
    }

    let draft = parse_draft(dispatch.request)?;
    draft.validate().map_err(McpError::from)?;

    match confirmation(dispatch.request.input_responses.as_ref()) {
        // Why: Round 1: nothing has been confirmed yet. Show the draft and ask.
        Confirmation::NotAsked => round_one(dispatch, &draft).await,

        // Why: The human said no. Not an error — a refusal is a valid outcome, and
        // reporting it as one would make the model retry.
        Confirmation::Declined => Ok(CallToolResponse::Complete(not_sent(
            "The email was not sent: the draft was declined.",
        ))),

        Confirmation::Confirmed => round_two(dispatch, &draft).await,
    }
}

// Why: Round 1 — persist the preview and hand back the confirmation request.
async fn round_one(
    dispatch: &Dispatch<'_>,
    draft: &SendEmailInput,
) -> Result<CallToolResponse, McpError> {
    // Why: Why run the preview through the executor rather than building a result
    // by hand: it persists the artifact and mints the `ui://email/artifact/{id}`
    // resource the client renders, and it records the round in
    // `mcp_tool_executions` so a draft that was shown and abandoned is still
    // visible in the audit trail.
    let preview = PreviewHandler {
        artifact: draft.preview_card(),
        summary: format!(
            "Draft email to {} — awaiting confirmation",
            draft.to.join(", ")
        ),
    };
    let preview_result = dispatch
        .executor
        .execute(
            &preview,
            dispatch.request,
            dispatch.request_context,
            dispatch.client,
        )
        .await?;

    let mut result = InputRequiredResult::from_input_requests(draft.approval_request()?);
    // Why: Carry the rendered preview's metadata through, so a UI-capable client
    // can show the card alongside the confirmation prompt rather than only the
    // plain-text message inside it.
    if let Some(meta) = preview_result.meta {
        result = result.with_meta(meta);
    }
    Ok(CallToolResponse::InputRequired(result))
}

// Why: Round 2 — the drafter has confirmed. Now governance decides.
async fn round_two(
    dispatch: &Dispatch<'_>,
    draft: &SendEmailInput,
) -> Result<CallToolResponse, McpError> {
    // Why: Placed before anything with a side effect: a held or refused call must
    // not open an SMTP connection or resolve an Odoo credential.
    match enforce_approval(
        dispatch.db_pool,
        SERVER_NAME,
        TOOL_EMAIL_SEND,
        dispatch.request,
        dispatch.request_context,
    )
    .await
    {
        GateOutcome::Proceed => {},
        GateOutcome::Held(result) => return Ok(CallToolResponse::InputRequired(*result)),
        GateOutcome::Refused(result) => return Ok((*result).into()),
    }

    let send = SendHandler {
        db_pool: std::sync::Arc::<systemprompt::database::Database>::clone(dispatch.db_pool),
        draft: draft.clone(),
    };
    dispatch
        .executor
        .execute(
            &send,
            dispatch.request,
            dispatch.request_context,
            dispatch.client,
        )
        .await
        .map(Into::into)
}

fn parse_draft(request: &CallToolRequestParams) -> Result<SendEmailInput, McpError> {
    let arguments = request
        .arguments
        .clone()
        .map_or(serde_json::Value::Null, serde_json::Value::Object);
    serde_json::from_value(arguments)
        .map_err(|e| McpError::invalid_params(format!("Invalid email arguments: {e}"), None))
}

fn not_sent(message: &str) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(message.to_owned())])
}

// Why: Round 1's handler: it renders the draft and does nothing else.
#[derive(Debug)]
struct PreviewHandler {
    artifact: CliArtifact,
    summary: String,
}

impl McpToolHandler for PreviewHandler {
    type Input = SendEmailInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_EMAIL_SEND
    }

    fn description(&self) -> &'static str {
        "Preview an email draft before sending."
    }

    fn handle(
        &self,
        _input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let output = (self.artifact.clone(), self.summary.clone());
        std::future::ready(Ok(output))
    }
}

// Why: Authenticates the caller and records the access, exactly as the other
// three servers do.
pub(super) async fn authenticate_tool_request(
    db_pool: &DbPool,
    tool_name: &str,
    service_id: &str,
    ctx: &rmcp::service::RequestContext<rmcp::service::RoleServer>,
    authz_hook: &systemprompt::security::authz::SharedAuthzHook,
) -> Result<RequestContext, McpError> {
    use systemprompt::mcp::middleware::enforce_rbac_from_registry;
    use systemprompt_mcp_shared::{record_mcp_access, record_mcp_access_rejected};

    match enforce_rbac_from_registry(ctx, service_id, authz_hook).await {
        Ok(result) => {
            match result.expect_authenticated("BUG: email requires OAuth but auth was not enforced")
            {
                Ok(authenticated) => {
                    record_mcp_access(
                        db_pool,
                        authenticated.context.user_id(),
                        service_id,
                        tool_name,
                        "authenticated",
                    )
                    .await;
                    Ok(authenticated.context.clone())
                },
                Err(e) => {
                    record_mcp_access_rejected(db_pool, service_id, tool_name, e.message.as_ref())
                        .await;
                    Err(e)
                },
            }
        },
        Err(e) => {
            record_mcp_access_rejected(db_pool, service_id, tool_name, &format!("{e}")).await;
            Err(e)
        },
    }
}
