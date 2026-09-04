//! Quote-to-cash: `sale.order` and `account.move`.
//!
//! Orders are writable, invoices are not. Raising a quotation belongs to
//! whoever owns the deal; posting an invoice is an accounting act with its own
//! approval path, and putting it behind a tool call here would place it one
//! model mistake away from the ledger. Reading invoices is what the pipeline
//! actually needs — whether the money arrived.
//!
//! Invoice reads live in [`invoice`].
//!
//! `sale_order_create` writes a DRAFT. Nothing here confirms an order or sends
//! it to a customer: a person does that in Odoo, having read it.

mod invoice;

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use super::call::OdooCall;
use super::sales_shape::ORDER_FIELDS;
pub use super::sales_shape::{
    InvoiceRow, OrderRow, invoice_domain, invoice_fields, invoice_rows, invoice_table,
    order_domain, order_fields, order_rows, order_table,
};
use crate::client::SearchOptions;
use crate::format::{detail_lines, empty_result, text_artifact};
use crate::tools::inputs::{
    SaleOrderCreateInput, SaleOrderGetInput, SaleOrderListInput, resolve_limit,
};
use crate::tools::{TOOL_SALE_ORDER_CREATE, TOOL_SALE_ORDER_GET, TOOL_SALE_ORDER_LIST};
pub use invoice::{InvoiceGetHandler, InvoiceListHandler};

const ORDER_MODEL: &str = "sale.order";

const ORDER_LABELS: [(&str, &str); 6] = [
    ("name", "Reference"),
    ("partner_id", "Customer"),
    ("state", "State"),
    ("date_order", "Ordered"),
    ("amount_total", "Total"),
    ("origin", "Source"),
];


#[derive(Debug)]
pub struct SaleOrderListHandler {
    pub call: OdooCall,
}

impl McpToolHandler for SaleOrderListHandler {
    type Input = SaleOrderListInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_SALE_ORDER_LIST
    }

    fn description(&self) -> &'static str {
        "List Odoo quotations and sales orders."
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
                fields: order_fields(),
                limit: resolve_limit(input.limit),
                order: Some("date_order desc, id desc".to_owned()),
            };
            let records = call
                .client
                .search_read(&call.creds, ORDER_MODEL, order_domain(&input), &options)
                .await?;
            let rows = order_rows(&records);

            let summary = if rows.is_empty() {
                empty_result("quotations or orders")
            } else {
                format!("{} order(s) matched in Odoo", rows.len())
            };
            Ok((CliArtifact::table(order_table(&rows)), summary))
        }
    }
}

#[derive(Debug)]
pub struct SaleOrderGetHandler {
    pub call: OdooCall,
}

impl McpToolHandler for SaleOrderGetHandler {
    type Input = SaleOrderGetInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_SALE_ORDER_GET
    }

    fn description(&self) -> &'static str {
        "Read one Odoo sales order, including its lines."
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
            fields.extend_from_slice(&ORDER_FIELDS);
            fields.push("order_line");
            let records = call
                .client
                .read(&call.creds, ORDER_MODEL, &[input.id], &fields)
                .await?;
            let Some(record) = records.first() else {
                return Err(McpError::invalid_params(
                    format!("No Odoo sales order with id {}.", input.id),
                    None,
                ));
            };

            let mut body = detail_lines(record, &ORDER_LABELS);

            // Why: order lines are a many2one list of ids on the order, so the
            // detail view is two reads. Without the second one the answer to
            // "what is on this quote" is a list of integers.
            let line_ids: Vec<i64> = record
                .get("order_line")
                .and_then(serde_json::Value::as_array)
                .map(|ids| ids.iter().filter_map(serde_json::Value::as_i64).collect())
                .unwrap_or_default();
            if !line_ids.is_empty() {
                let lines = call
                    .client
                    .read(
                        &call.creds,
                        "sale.order.line",
                        &line_ids,
                        &["name", "product_uom_qty", "price_unit", "price_subtotal"],
                    )
                    .await?;
                body.push_str("\n\n**Lines**\n\n");
                for line in &lines {
                    body.push_str(&format!(
                        "- {} — qty {} @ {} = {}\n",
                        crate::format::field_or_dash(line, "name"),
                        crate::format::field_or_dash(line, "product_uom_qty"),
                        crate::format::field_or_dash(line, "price_unit"),
                        crate::format::field_or_dash(line, "price_subtotal"),
                    ));
                }
            }

            let summary = format!("Odoo sales order {}", input.id);
            Ok((text_artifact("Sales Order", &body), summary))
        }
    }
}

#[derive(Debug)]
pub struct SaleOrderCreateHandler {
    pub call: OdooCall,
}

impl McpToolHandler for SaleOrderCreateHandler {
    type Input = SaleOrderCreateInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_SALE_ORDER_CREATE
    }

    fn description(&self) -> &'static str {
        "Create a draft quotation in Odoo."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            if input.lines.is_empty() {
                return Err(McpError::invalid_params(
                    "A quotation needs at least one line — pass `lines` with a `product_id`."
                        .to_owned(),
                    None,
                ));
            }

            // Odoo's x2many write command: (0, 0, values) creates a new line.
            let lines: Vec<serde_json::Value> = input
                .lines
                .iter()
                .map(|line| {
                    let mut values = serde_json::Map::new();
                    values.insert("product_id".to_owned(), serde_json::json!(line.product_id));
                    if let Some(qty) = line.quantity {
                        values.insert("product_uom_qty".to_owned(), serde_json::json!(qty));
                    }
                    if let Some(price) = line.price_unit {
                        values.insert("price_unit".to_owned(), serde_json::json!(price));
                    }
                    serde_json::json!([0, 0, serde_json::Value::Object(values)])
                })
                .collect();

            let mut values = serde_json::Map::new();
            values.insert("partner_id".to_owned(), serde_json::json!(input.partner_id));
            values.insert("order_line".to_owned(), serde_json::Value::Array(lines));
            if let Some(origin) = input
                .origin
                .as_deref()
                .map(str::trim)
                .filter(|o| !o.is_empty())
            {
                values.insert("origin".to_owned(), serde_json::json!(origin));
            }

            let id = call
                .client
                .create(&call.creds, ORDER_MODEL, serde_json::Value::Object(values))
                .await?;

            let summary = format!("Created Odoo quotation {id} as {}", call.creds.login);
            let body = format!(
                "Created **draft quotation [{id}]** for partner {} with {} line(s), by `{}`.\n\n\
                 It is a draft: nobody has sent or confirmed it.",
                input.partner_id,
                input.lines.len(),
                call.creds.login
            );
            Ok((text_artifact("Quotation Created", &body), summary))
        }
    }
}
