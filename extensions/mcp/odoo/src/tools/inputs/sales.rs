//! Input shapes for the quote-to-cash plane: `sale.order` and `account.move`.
//!
//! Orders are writable and invoices are not. Raising a quotation is a sales
//! act and belongs to whoever owns the deal; posting an invoice is an
//! accounting one, with its own approval path outside this server. Reading
//! invoices is what a salesperson actually needs — "has this been paid?" —
//! and that is what `invoice_list` answers through `amount_residual`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SaleOrderListInput {
    /// Restrict to one customer, by partner id.
    pub partner_id: Option<i64>,
    /// Odoo's own order state: `draft` and `sent` are quotations, `sale` is a
    /// confirmed order, `cancel` a dead one.
    pub state: Option<String>,
    /// Inclusive `YYYY-MM-DD` bounds on the order date.
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct SaleOrderGetInput {
    pub id: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct SaleOrderLineInput {
    /// The product to sell, by id. `sale_order_create` refuses a line without
    /// one rather than inventing a product.
    pub product_id: i64,
    pub quantity: Option<f64>,
    /// Override the product's list price. Omit to let Odoo price the line.
    pub price_unit: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SaleOrderCreateInput {
    pub partner_id: i64,
    pub lines: Vec<SaleOrderLineInput>,
    /// Where this quotation came from — set it to the lead's name or reference
    /// so the order and the deal that produced it can be read together.
    pub origin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InvoiceListInput {
    pub partner_id: Option<i64>,
    /// Only invoices with an outstanding balance — the "who owes us" question.
    pub unpaid_only: Option<bool>,
    /// Inclusive `YYYY-MM-DD` bounds on the invoice date.
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct InvoiceGetInput {
    pub id: i64,
}
