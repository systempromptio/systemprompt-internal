//! The per-request bundle every tool handler is built with.
//!
//! A handler gets the shared HTTP client and *this* caller's Odoo credential
//! together, and cannot be constructed without both. That is the invariant
//! worth enforcing in a type: there is no path through this server where a
//! tool runs without a resolved acting user.

use std::sync::Arc;

use crate::client::{Credentials, OdooClient};

#[derive(Clone, Debug)]
pub struct OdooCall {
    pub client: Arc<OdooClient>,
    pub creds: Credentials,
}

pub const LEAD_FIELDS: [&str; 10] = [
    "id",
    "name",
    "partner_name",
    "email_from",
    "phone",
    "stage_id",
    "user_id",
    "expected_revenue",
    "probability",
    "create_date",
];

pub const PARTNER_FIELDS: [&str; 9] = [
    "id",
    "name",
    "email",
    "phone",
    "mobile",
    "city",
    "country_id",
    "is_company",
    "category_id",
];

#[must_use]
pub fn lead_fields() -> Vec<String> {
    LEAD_FIELDS.iter().map(|f| (*f).to_owned()).collect()
}

#[must_use]
pub fn partner_fields() -> Vec<String> {
    PARTNER_FIELDS.iter().map(|f| (*f).to_owned()).collect()
}
