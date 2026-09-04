//! The read-only `account.move` tools: list customer invoices, read one.
//!
//! Invoices sit in [`super`]'s plane but carry no write path — posting one is
//! an accounting act with its own approval, so nothing here creates or
//! modifies a move.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use crate::client::SearchOptions;
use crate::format::{detail_lines, empty_result, text_artifact};
use crate::server::call::OdooCall;
use crate::server::sales_shape::{
    INVOICE_FIELDS, invoice_domain, invoice_fields, invoice_rows, invoice_table,
};
use crate::tools::inputs::{InvoiceGetInput, InvoiceListInput, resolve_limit};
use crate::tools::{TOOL_INVOICE_GET, TOOL_INVOICE_LIST};

const INVOICE_MODEL: &str = "account.move";

const INVOICE_LABELS: [(&str, &str); 7] = [
    ("name", "Invoice"),
    ("partner_id", "Customer"),
    ("invoice_date", "Invoiced"),
    ("invoice_date_due", "Due"),
    ("payment_state", "Payment"),
    ("amount_total", "Total"),
    ("amount_residual", "Outstanding"),
];


#[derive(Debug)]
pub struct InvoiceListHandler {
    pub call: OdooCall,
}

impl McpToolHandler for InvoiceListHandler {
    type Input = InvoiceListInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_INVOICE_LIST
    }

    fn description(&self) -> &'static str {
        "List Odoo customer invoices and what is outstanding."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let options = SearchOptions {
                fields: invoice_fields(),
                limit: resolve_limit(input.limit),
                order: Some("invoice_date desc, id desc".to_owned()),
            };
            let records = call
                .client
                .search_read(&call.creds, INVOICE_MODEL, invoice_domain(&input), &options)
                .await?;
            let rows = invoice_rows(&records);

            let summary = if rows.is_empty() {
                empty_result("customer invoices")
            } else {
                let outstanding: f64 = rows.iter().filter_map(|r| r.amount_residual).sum();
                format!("{} invoice(s), {outstanding:.2} outstanding", rows.len())
            };
            Ok((CliArtifact::table(invoice_table(&rows)), summary))
        }
    }
}

#[derive(Debug)]
pub struct InvoiceGetHandler {
    pub call: OdooCall,
}

impl McpToolHandler for InvoiceGetHandler {
    type Input = InvoiceGetInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_INVOICE_GET
    }

    fn description(&self) -> &'static str {
        "Read one Odoo customer invoice, including its lines."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let mut fields: Vec<&str> = vec!["id"];
            fields.extend_from_slice(&INVOICE_FIELDS);
            fields.push("invoice_line_ids");
            let records = call
                .client
                .read(&call.creds, INVOICE_MODEL, &[input.id], &fields)
                .await?;
            let Some(record) = records.first() else {
                return Err(McpError::invalid_params(
                    format!("No Odoo customer invoice with id {}.", input.id),
                    None,
                ));
            };

            let mut body = detail_lines(record, &INVOICE_LABELS);

            let line_ids: Vec<i64> = record
                .get("invoice_line_ids")
                .and_then(serde_json::Value::as_array)
                .map(|ids| ids.iter().filter_map(serde_json::Value::as_i64).collect())
                .unwrap_or_default();
            if !line_ids.is_empty() {
                let lines = call
                    .client
                    .read(
                        &call.creds,
                        "account.move.line",
                        &line_ids,
                        &["name", "quantity", "price_unit", "price_subtotal"],
                    )
                    .await?;
                body.push_str("\n\n**Lines**\n\n");
                for line in &lines {
                    body.push_str(&format!(
                        "- {} — qty {} @ {} = {}\n",
                        crate::format::field_or_dash(line, "name"),
                        crate::format::field_or_dash(line, "quantity"),
                        crate::format::field_or_dash(line, "price_unit"),
                        crate::format::field_or_dash(line, "price_subtotal"),
                    ));
                }
            }

            let summary = format!("Odoo customer invoice {}", input.id);
            Ok((text_artifact("Customer Invoice", &body), summary))
        }
    }
}
