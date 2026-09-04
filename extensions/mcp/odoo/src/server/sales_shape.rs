//! Pure query- and row-shaping for the quote-to-cash tools.
//!
//! Domains, typed rows and table columns for `sale.order` and `account.move`.
//! No I/O, so every function here is directly assertable from the external
//! test workspace — the same split [`super::crm_shape`] makes.

use systemprompt::models::artifacts::{Column, ColumnType, TableArtifact};

use crate::tools::inputs::{InvoiceListInput, SaleOrderListInput};

pub(crate) use crate::shape as odoo;

/// One `sale.order` as `search_read` returns it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OrderRow {
    pub id: i64,
    #[serde(deserialize_with = "odoo::text", default)]
    pub name: Option<String>,
    #[serde(deserialize_with = "odoo::many2one", default)]
    pub partner_id: Option<String>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub state: Option<String>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub date_order: Option<String>,
    #[serde(deserialize_with = "odoo::number", default)]
    pub amount_total: Option<f64>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub origin: Option<String>,
}

/// One `account.move` customer invoice.
///
/// `amount_residual` is the field that answers "has this been paid?", which is
/// the only question a salesperson asks of an invoice.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InvoiceRow {
    pub id: i64,
    #[serde(deserialize_with = "odoo::text", default)]
    pub name: Option<String>,
    #[serde(deserialize_with = "odoo::many2one", default)]
    pub partner_id: Option<String>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub invoice_date: Option<String>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub invoice_date_due: Option<String>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub payment_state: Option<String>,
    #[serde(deserialize_with = "odoo::number", default)]
    pub amount_total: Option<f64>,
    #[serde(deserialize_with = "odoo::number", default)]
    pub amount_residual: Option<f64>,
}

pub(super) const ORDER_FIELDS: [&str; 6] = [
    "name",
    "partner_id",
    "state",
    "date_order",
    "amount_total",
    "origin",
];

pub(super) const INVOICE_FIELDS: [&str; 7] = [
    "name",
    "partner_id",
    "invoice_date",
    "invoice_date_due",
    "payment_state",
    "amount_total",
    "amount_residual",
];

#[must_use]
pub fn order_fields() -> Vec<String> {
    std::iter::once("id".to_owned())
        .chain(ORDER_FIELDS.iter().map(|f| (*f).to_owned()))
        .collect()
}

#[must_use]
pub fn invoice_fields() -> Vec<String> {
    std::iter::once("id".to_owned())
        .chain(INVOICE_FIELDS.iter().map(|f| (*f).to_owned()))
        .collect()
}

#[doc(hidden)]
#[must_use]
pub fn order_domain(input: &SaleOrderListInput) -> serde_json::Value {
    let mut domain: Vec<serde_json::Value> = Vec::new();
    if let Some(partner_id) = input.partner_id {
        domain.push(serde_json::json!(["partner_id", "=", partner_id]));
    }
    if let Some(state) = input
        .state
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        domain.push(serde_json::json!(["state", "=", state]));
    }
    if let Some(from) = input
        .date_from
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
        domain.push(serde_json::json!(["date_order", ">=", from]));
    }
    if let Some(to) = input
        .date_to
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
        domain.push(serde_json::json!(["date_order", "<=", to]));
    }
    serde_json::Value::Array(domain)
}

#[doc(hidden)]
#[must_use]
pub fn invoice_domain(input: &InvoiceListInput) -> serde_json::Value {
    // Why: `account.move` holds vendor bills, credit notes and journal entries
    // in the same table. Without this the tool would answer "what do customers
    // owe us" with the company's own purchase ledger.
    let mut domain: Vec<serde_json::Value> = vec![serde_json::json!(["move_type", "=", "out_invoice"])];
    if let Some(partner_id) = input.partner_id {
        domain.push(serde_json::json!(["partner_id", "=", partner_id]));
    }
    if input.unpaid_only == Some(true) {
        domain.push(serde_json::json!(["amount_residual", ">", 0]));
    }
    if let Some(from) = input
        .date_from
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
        domain.push(serde_json::json!(["invoice_date", ">=", from]));
    }
    if let Some(to) = input
        .date_to
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
        domain.push(serde_json::json!(["invoice_date", "<=", to]));
    }
    serde_json::Value::Array(domain)
}

#[doc(hidden)]
#[must_use]
pub fn order_rows(records: &[serde_json::Value]) -> Vec<OrderRow> {
    // JSON: protocol boundary — records arrive as the RPC client's JSON.
    records
        .iter()
        .filter_map(
            |record| match serde_json::from_value::<OrderRow>(record.clone()) {
                Ok(row) => Some(row),
                Err(e) => {
                    tracing::warn!(error = %e, "sale.order record did not match OrderRow; dropping");
                    None
                },
            },
        )
        .collect()
}

#[doc(hidden)]
#[must_use]
pub fn invoice_rows(records: &[serde_json::Value]) -> Vec<InvoiceRow> {
    // JSON: protocol boundary — records arrive as the RPC client's JSON.
    records
        .iter()
        .filter_map(
            |record| match serde_json::from_value::<InvoiceRow>(record.clone()) {
                Ok(row) => Some(row),
                Err(e) => {
                    tracing::warn!(error = %e, "account.move record did not match InvoiceRow; dropping");
                    None
                },
            },
        )
        .collect()
}

#[doc(hidden)]
#[must_use]
pub fn order_table(rows: &[OrderRow]) -> TableArtifact {
    let columns = vec![
        Column::new("id", ColumnType::Integer),
        Column::new("name", ColumnType::String).with_header("Reference"),
        Column::new("partner_id", ColumnType::String).with_header("Customer"),
        Column::new("state", ColumnType::String).with_header("State"),
        Column::new("date_order", ColumnType::Date).with_header("Ordered"),
        Column::new("amount_total", ColumnType::Currency).with_header("Total"),
        Column::new("origin", ColumnType::String).with_header("Source"),
    ];
    // JSON: protocol boundary — TableArtifact carries rows as JSON values.
    let items = rows
        .iter()
        .filter_map(|row| match serde_json::to_value(row) {
            Ok(item) => Some(item),
            Err(e) => {
                tracing::warn!(error = %e, order_id = row.id, "order row did not serialise; dropping");
                None
            },
        })
        .collect();
    TableArtifact::new(columns)
        .with_title("Quotations & Orders")
        .with_rows(items)
}

#[doc(hidden)]
#[must_use]
pub fn invoice_table(rows: &[InvoiceRow]) -> TableArtifact {
    let columns = vec![
        Column::new("id", ColumnType::Integer),
        Column::new("name", ColumnType::String).with_header("Invoice"),
        Column::new("partner_id", ColumnType::String).with_header("Customer"),
        Column::new("invoice_date", ColumnType::Date).with_header("Invoiced"),
        Column::new("invoice_date_due", ColumnType::Date).with_header("Due"),
        Column::new("payment_state", ColumnType::String).with_header("Payment"),
        Column::new("amount_total", ColumnType::Currency).with_header("Total"),
        Column::new("amount_residual", ColumnType::Currency).with_header("Outstanding"),
    ];
    // JSON: protocol boundary — TableArtifact carries rows as JSON values.
    let items = rows
        .iter()
        .filter_map(|row| match serde_json::to_value(row) {
            Ok(item) => Some(item),
            Err(e) => {
                tracing::warn!(error = %e, invoice_id = row.id, "invoice row did not serialise; dropping");
                None
            },
        })
        .collect();
    TableArtifact::new(columns)
        .with_title("Customer Invoices")
        .with_rows(items)
}
