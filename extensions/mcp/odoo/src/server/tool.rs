//! Per-call logic: authenticate the platform caller, resolve their Odoo
//! credential, then route to a handler.
//!
//! The two authentications are distinct and both required. The MCP transport
//! proves *which platform user* is calling; `odoo_identity` turns that into an
//! Odoo credential, and Odoo then decides what that credential may do. This
//! server enforces no data-access policy of its own — it has none to enforce
//! that Odoo does not already own.

use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::{RequestContext, RoleServer};
use std::sync::Arc;
use systemprompt::database::DbPool;
use systemprompt::mcp::middleware::enforce_rbac_from_registry;
use systemprompt::mcp::{ClientProfile, McpToolExecutor, McpToolHandler};
use systemprompt::models::execution::context::RequestContext as SysRequestContext;
use systemprompt::security::authz::SharedAuthzHook;
use systemprompt_mcp_shared::{record_mcp_access, record_mcp_access_rejected};

use super::call::OdooCall;
use super::{
    activity, attachments, calendar, channels, crm, crm_delete, discovery, notes, overview,
    partner, partner_write, report, sales, tasks,
};
use crate::client::OdooClient;
use crate::error::OdooError;
use crate::identity::resolve_credentials;
use crate::tools::{
    ALL_TOOLS, TOOL_ACTIVITY_COMPLETE, TOOL_ACTIVITY_CREATE, TOOL_ACTIVITY_LIST,
    TOOL_ACTIVITY_TYPE_LIST, TOOL_ATTACHMENT_ADD, TOOL_ATTACHMENT_GET, TOOL_ATTACHMENT_LIST,
    TOOL_CALENDAR_EVENT_CREATE, TOOL_CALENDAR_EVENT_LIST, TOOL_CHANNEL_LIST, TOOL_CHANNEL_POST,
    TOOL_INVOICE_GET, TOOL_INVOICE_LIST, TOOL_LEAD_CONVERT, TOOL_LEAD_CREATE, TOOL_LEAD_DELETE,
    TOOL_LEAD_GET, TOOL_LEAD_MARK_LOST, TOOL_LEAD_MARK_WON, TOOL_LEAD_REPORT, TOOL_LEAD_SEARCH,
    TOOL_LEAD_UPDATE, TOOL_NOTE_ADD, TOOL_NOTE_LIST, TOOL_NOTE_SEARCH, TOOL_OVERVIEW,
    TOOL_PARTNER_CREATE, TOOL_PARTNER_GET, TOOL_PARTNER_SEARCH, TOOL_PARTNER_UPDATE,
    TOOL_SALE_ORDER_CREATE, TOOL_SALE_ORDER_GET, TOOL_SALE_ORDER_LIST, TOOL_STAGE_LIST,
    TOOL_TASK_CREATE, TOOL_TASK_LIST, TOOL_TASK_UPDATE, TOOL_USER_LIST,
};

pub(super) async fn authenticate_tool_request(
    db_pool: &DbPool,
    tool_name: &str,
    service_id: &str,
    ctx: &RequestContext<RoleServer>,
    authz_hook: &SharedAuthzHook,
) -> Result<SysRequestContext, McpError> {
    let rbac_result = enforce_rbac_from_registry(ctx, service_id, authz_hook).await;

    match rbac_result {
        Ok(result) => {
            match result.expect_authenticated("BUG: odoo requires OAuth but auth was not enforced")
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

pub(super) async fn build_call(
    db_pool: &DbPool,
    client: &Arc<OdooClient>,
    request_context: &SysRequestContext,
) -> Result<OdooCall, OdooError> {
    let creds = resolve_credentials(db_pool, request_context.user_id()).await?;
    Ok(OdooCall {
        client: Arc::clone(client),
        creds,
    })
}

#[doc(hidden)]
pub async fn dispatch_tool(
    ctx: &Dispatch<'_>,
    call: OdooCall,
    tool_name: &str,
) -> Result<CallToolResult, McpError> {
    // Why: split by plane rather than one 25-arm match. Every arm names a
    // distinct handler type, and holding all of them in one stack frame costs
    // half a megabyte — over clippy's frame ceiling, and a real cost on a
    // server that runs one of these per request.
    if let Some(result) = crm_tools(ctx, call.clone(), tool_name).await {
        return result;
    }
    if let Some(result) = closing_tools(ctx, call.clone(), tool_name).await {
        return result;
    }
    if let Some(result) = sales_tools(ctx, call.clone(), tool_name).await {
        return result;
    }
    if let Some(result) = knowledge_tools(ctx, call.clone(), tool_name).await {
        return result;
    }
    if let Some(result) = work_tools(ctx, call, tool_name).await {
        return result;
    }
    Err(unknown_tool(tool_name))
}

#[doc(hidden)]
#[derive(Debug)]
pub struct Dispatch<'a> {
    pub executor: &'a McpToolExecutor,
    pub request: &'a CallToolRequestParams,
    pub request_context: &'a SysRequestContext,
    pub client: &'a ClientProfile,
}

async fn crm_tools(
    ctx: &Dispatch<'_>,
    call: OdooCall,
    tool_name: &str,
) -> Option<Result<CallToolResult, McpError>> {
    Some(match tool_name {
        TOOL_LEAD_SEARCH => ctx.run(&crm::LeadSearchHandler { call }).await,
        TOOL_LEAD_GET => ctx.run(&crm::LeadGetHandler { call }).await,
        TOOL_LEAD_CREATE => ctx.run(&crm::LeadCreateHandler { call }).await,
        TOOL_LEAD_UPDATE => ctx.run(&crm::LeadUpdateHandler { call }).await,
        TOOL_LEAD_DELETE => ctx.run(&crm_delete::LeadDeleteHandler { call }).await,
        TOOL_LEAD_REPORT => ctx.run(&report::LeadReportHandler { call }).await,
        TOOL_PARTNER_SEARCH => ctx.run(&partner::PartnerSearchHandler { call }).await,
        TOOL_PARTNER_GET => ctx.run(&partner::PartnerGetHandler { call }).await,
        TOOL_OVERVIEW => ctx.run(&overview::OverviewHandler { call }).await,
        _ => return None,
    })
}

/// Closing a deal, writing the customer, and the id lookups.
///
/// A plane of its own for the same reason the others are: each handler is a
/// distinct type and they cannot all share one stack frame.
async fn closing_tools(
    ctx: &Dispatch<'_>,
    call: OdooCall,
    tool_name: &str,
) -> Option<Result<CallToolResult, McpError>> {
    Some(match tool_name {
        TOOL_LEAD_MARK_WON => ctx.run(&crm::LeadMarkWonHandler { call }).await,
        TOOL_LEAD_MARK_LOST => ctx.run(&crm::LeadMarkLostHandler { call }).await,
        TOOL_LEAD_CONVERT => ctx.run(&crm::LeadConvertHandler { call }).await,
        TOOL_PARTNER_CREATE => ctx.run(&partner_write::PartnerCreateHandler { call }).await,
        TOOL_PARTNER_UPDATE => ctx.run(&partner_write::PartnerUpdateHandler { call }).await,
        TOOL_STAGE_LIST => ctx.run(&discovery::StageListHandler { call }).await,
        TOOL_USER_LIST => ctx.run(&discovery::UserListHandler { call }).await,
        TOOL_ACTIVITY_TYPE_LIST => ctx.run(&discovery::ActivityTypeListHandler { call }).await,
        _ => return None,
    })
}

async fn sales_tools(
    ctx: &Dispatch<'_>,
    call: OdooCall,
    tool_name: &str,
) -> Option<Result<CallToolResult, McpError>> {
    Some(match tool_name {
        TOOL_SALE_ORDER_LIST => ctx.run(&sales::SaleOrderListHandler { call }).await,
        TOOL_SALE_ORDER_GET => ctx.run(&sales::SaleOrderGetHandler { call }).await,
        TOOL_SALE_ORDER_CREATE => ctx.run(&sales::SaleOrderCreateHandler { call }).await,
        TOOL_INVOICE_LIST => ctx.run(&sales::InvoiceListHandler { call }).await,
        TOOL_INVOICE_GET => ctx.run(&sales::InvoiceGetHandler { call }).await,
        _ => return None,
    })
}

async fn knowledge_tools(
    ctx: &Dispatch<'_>,
    call: OdooCall,
    tool_name: &str,
) -> Option<Result<CallToolResult, McpError>> {
    Some(match tool_name {
        TOOL_NOTE_ADD => ctx.run(&notes::NoteAddHandler { call }).await,
        TOOL_NOTE_LIST => ctx.run(&notes::NoteListHandler { call }).await,
        TOOL_NOTE_SEARCH => ctx.run(&notes::NoteSearchHandler { call }).await,
        TOOL_ATTACHMENT_ADD => ctx.run(&attachments::AttachmentAddHandler { call }).await,
        TOOL_ATTACHMENT_LIST => ctx.run(&attachments::AttachmentListHandler { call }).await,
        TOOL_ATTACHMENT_GET => ctx.run(&attachments::AttachmentGetHandler { call }).await,
        _ => return None,
    })
}

async fn work_tools(
    ctx: &Dispatch<'_>,
    call: OdooCall,
    tool_name: &str,
) -> Option<Result<CallToolResult, McpError>> {
    Some(match tool_name {
        TOOL_ACTIVITY_LIST => ctx.run(&activity::ActivityListHandler { call }).await,
        TOOL_ACTIVITY_CREATE => ctx.run(&activity::ActivityCreateHandler { call }).await,
        TOOL_ACTIVITY_COMPLETE => ctx.run(&activity::ActivityCompleteHandler { call }).await,
        TOOL_CALENDAR_EVENT_LIST => ctx.run(&calendar::CalendarEventListHandler { call }).await,
        TOOL_CALENDAR_EVENT_CREATE => {
            ctx.run(&calendar::CalendarEventCreateHandler { call })
                .await
        },
        TOOL_TASK_LIST => ctx.run(&tasks::TaskListHandler { call }).await,
        TOOL_TASK_CREATE => ctx.run(&tasks::TaskCreateHandler { call }).await,
        TOOL_TASK_UPDATE => ctx.run(&tasks::TaskUpdateHandler { call }).await,
        TOOL_CHANNEL_LIST => ctx.run(&channels::ChannelListHandler { call }).await,
        TOOL_CHANNEL_POST => ctx.run(&channels::ChannelPostHandler { call }).await,
        _ => return None,
    })
}

impl Dispatch<'_> {
    async fn run<H: McpToolHandler>(&self, handler: &H) -> Result<CallToolResult, McpError> {
        self.executor
            .execute(handler, self.request, self.request_context, self.client)
            .await
    }
}

#[doc(hidden)]
#[must_use]
pub fn unknown_tool(tool_name: &str) -> McpError {
    McpError::invalid_params(
        format!(
            "Unknown tool: '{tool_name}'. Available tools: {}.",
            ALL_TOOLS.join(", ")
        ),
        None,
    )
}
