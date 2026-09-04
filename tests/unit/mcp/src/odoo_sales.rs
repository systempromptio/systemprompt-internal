//! The quote-to-cash domains and typed rows.
//!
//! The invoice domain carries the one condition that cannot be omitted:
//! `account.move` holds vendor bills and journal entries in the same table as
//! customer invoices, so a missing `move_type` filter answers "what do
//! customers owe us" with the company's own purchase ledger. That is a wrong
//! answer, not a failed call, which is why it is pinned here.

use systemprompt_mcp_odoo::server::sales::{
    InvoiceRow, OrderRow, invoice_domain, invoice_rows, invoice_table, order_domain, order_rows,
    order_table,
};
use systemprompt_mcp_odoo::tools::inputs::{InvoiceListInput, SaleOrderListInput};

fn order_input() -> SaleOrderListInput {
    SaleOrderListInput {
        partner_id: None,
        state: None,
        date_from: None,
        date_to: None,
        limit: None,
    }
}

fn invoice_input() -> InvoiceListInput {
    InvoiceListInput {
        partner_id: None,
        unpaid_only: None,
        date_from: None,
        date_to: None,
        limit: None,
    }
}

#[test]
fn order_domain_is_empty_when_nothing_is_filtered() {
    assert_eq!(
        order_domain(&order_input()),
        serde_json::json!([]),
        "an unfiltered order search must not smuggle in a condition"
    );
}

#[test]
fn order_domain_filters_by_partner_state_and_dates() {
    let input = SaleOrderListInput {
        partner_id: Some(7),
        state: Some("draft".to_owned()),
        date_from: Some("2026-01-01".to_owned()),
        date_to: Some("2026-01-31".to_owned()),
        limit: None,
    };

    assert_eq!(
        order_domain(&input),
        serde_json::json!([
            ["partner_id", "=", 7],
            ["state", "=", "draft"],
            ["date_order", ">=", "2026-01-01"],
            ["date_order", "<=", "2026-01-31"]
        ])
    );
}

#[test]
fn order_domain_ignores_blank_state_and_dates() {
    let input = SaleOrderListInput {
        partner_id: None,
        state: Some("   ".to_owned()),
        date_from: Some(String::new()),
        date_to: Some("  ".to_owned()),
        limit: None,
    };

    assert_eq!(
        order_domain(&input),
        serde_json::json!([]),
        "whitespace is not a filter"
    );
}

#[test]
fn invoice_domain_always_restricts_to_customer_invoices() {
    assert_eq!(
        invoice_domain(&invoice_input()),
        serde_json::json!([["move_type", "=", "out_invoice"]]),
        "without move_type the tool would return vendor bills as receivables"
    );
}

#[test]
fn invoice_domain_unpaid_only_asks_for_a_positive_balance() {
    let input = InvoiceListInput {
        unpaid_only: Some(true),
        ..invoice_input()
    };

    assert_eq!(
        invoice_domain(&input),
        serde_json::json!([
            ["move_type", "=", "out_invoice"],
            ["amount_residual", ">", 0]
        ])
    );
}

#[test]
fn invoice_domain_unpaid_only_false_does_not_filter_the_balance() {
    let input = InvoiceListInput {
        unpaid_only: Some(false),
        ..invoice_input()
    };

    assert_eq!(
        invoice_domain(&input),
        serde_json::json!([["move_type", "=", "out_invoice"]]),
        "explicitly asking for all invoices must not filter to unpaid ones"
    );
}

#[test]
fn order_rows_absorb_odoo_wire_idioms() {
    let records = vec![serde_json::json!({
        "id": 3,
        "name": "S00003",
        // many2one: [id, "Display Name"]
        "partner_id": [11, "Acme Ltd"],
        "state": "draft",
        "date_order": "2026-02-01 09:00:00",
        "amount_total": 1500.0,
        // Odoo sends `false` for an empty field, not null.
        "origin": false
    })];

    let rows: Vec<OrderRow> = order_rows(&records);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, 3);
    assert_eq!(rows[0].partner_id.as_deref(), Some("Acme Ltd"));
    assert_eq!(rows[0].amount_total, Some(1500.0));
    assert_eq!(
        rows[0].origin, None,
        "Odoo's `false` must arrive as an absent field, not the string \"false\""
    );
}

#[test]
fn invoice_rows_absorb_odoo_wire_idioms() {
    let records = vec![serde_json::json!({
        "id": 9,
        "name": "INV/2026/0009",
        "partner_id": [11, "Acme Ltd"],
        "invoice_date": "2026-02-01",
        "invoice_date_due": false,
        "payment_state": "not_paid",
        "amount_total": 2400.0,
        "amount_residual": 2400.0
    })];

    let rows: Vec<InvoiceRow> = invoice_rows(&records);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].partner_id.as_deref(), Some("Acme Ltd"));
    assert_eq!(rows[0].amount_residual, Some(2400.0));
    assert_eq!(rows[0].invoice_date_due, None);
}

#[test]
fn a_record_that_does_not_type_is_dropped_not_half_parsed() {
    let records = vec![
        serde_json::json!({ "id": 1, "name": "S00001" }),
        // No id: cannot be an order.
        serde_json::json!({ "name": "S00002" }),
    ];

    assert_eq!(
        order_rows(&records).len(),
        1,
        "an untypeable record is dropped rather than shipped with a guessed id"
    );
}

#[test]
fn tables_carry_every_row_as_structured_data() {
    let orders = order_rows(&[serde_json::json!({ "id": 1, "name": "S00001" })]);
    let invoices = invoice_rows(&[serde_json::json!({ "id": 2, "name": "INV/1" })]);

    assert_eq!(order_table(&orders).rows.len(), 1);
    assert_eq!(invoice_table(&invoices).rows.len(), 1);
}

#[test]
fn the_invoice_table_exposes_the_outstanding_balance_column() {
    let table = invoice_table(&[]);

    assert!(
        table.columns.iter().any(|c| c.key == "amount_residual"),
        "the outstanding balance is the reason this dashboard exists"
    );
}
