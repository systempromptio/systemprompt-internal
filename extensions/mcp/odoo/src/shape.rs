//! Serde adapters for Odoo's wire idioms, usable by any record struct in any
//! crate that types Odoo rows: `false` where a field is empty, and a many2one
//! that arrives as `[id, "Display Name"]`.

use serde::{Deserialize, Deserializer};

pub fn text<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    // JSON: protocol boundary — Odoo writes `false` where a field is empty.
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::String(s) if !s.trim().is_empty() => Some(s),
        _ => None,
    })
}

pub fn many2one<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    // JSON: protocol boundary — `[id, "Display Name"]`, or `false`.
    let v = serde_json::Value::deserialize(d)?;
    Ok(v.as_array()
        .and_then(|t| t.get(1))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned))
}

pub fn many2one_id<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    // JSON: protocol boundary — the id half of `[id, "Display Name"]`.
    let v = serde_json::Value::deserialize(d)?;
    Ok(v.as_array()
        .and_then(|t| t.first())
        .and_then(serde_json::Value::as_i64))
}

pub fn number<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(v.as_f64())
}
