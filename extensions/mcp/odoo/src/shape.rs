//! Serde adapters for Odoo's wire idioms.
//!
//! Usable by any record struct in any crate that types Odoo rows: `false`
//! where a field is empty, a many2one that arrives as `[id, "Display Name"]`,
//! and a many2many that arrives as a bare list of ids.

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

pub fn many2many_ids<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<i64>, D::Error> {
    // JSON: protocol boundary — `[id, id, ...]`, or `false` when empty.
    let v = serde_json::Value::deserialize(d)?;
    Ok(v.as_array().map_or_else(Vec::new, |ids| {
        ids.iter().filter_map(serde_json::Value::as_i64).collect()
    }))
}

pub fn number<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(v.as_f64())
}
