//! Turning a `FactsheetDoc` into what the Handlebars templates can consume.
//!
//! Two jobs that both exist because the templates are deliberately dumb: the
//! block walk that resolves values a template cannot compute (column spans,
//! comparison highlighting), and the helpers that turn data into markup at the
//! one point where that conversion is allowed. Split from `render`, which is
//! about the engine and the `WeasyPrint` subprocess rather than the data.

use handlebars::{
    Context, Handlebars, Helper, HelperResult, Output, RenderContext, RenderErrorReason,
};

use crate::inline::Inline;

#[derive(Debug, serde::Deserialize)]
pub(crate) struct RenderReport {
    pub(crate) page_count: usize,
    pub(crate) page_images: Vec<String>,
}

// Why: Walk the serialised document, resolving what the templates cannot
// compute.
pub(crate) fn prepare_blocks(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                prepare_blocks(item);
            }
        },
        serde_json::Value::Object(object) => {
            match object.get("type").and_then(serde_json::Value::as_str) {
                Some("compare") => rewrite_compare(object),
                Some("ctable") => insert_span(object),
                Some("cuts") => {
                    if let Some(panels) = object
                        .get_mut("panels")
                        .and_then(serde_json::Value::as_array_mut)
                    {
                        for panel in panels {
                            if let Some(panel) = panel.as_object_mut() {
                                insert_span(panel);
                            }
                        }
                    }
                },
                _ => {},
            }
            for (_, child) in object.iter_mut() {
                prepare_blocks(child);
            }
        },
        _ => {},
    }
}

// Why: A banded row spans the whole table, so it needs the column count.
fn insert_span(block: &mut serde_json::Map<String, serde_json::Value>) {
    let span = block
        .get("headers")
        .and_then(serde_json::Value::as_array)
        .map_or(1, Vec::len);
    block.insert("span".to_owned(), span.into());
}

// Why: Resolve a comparison table's highlighted column into per-cell flags.
//
// The template could walk back up the context stack to find `highlight`, but
// the path depth differs between the header row and a body cell, so a small
// change to the markup silently highlights the wrong column. Computing the
// flags here keeps that decision in one place and the template dumb.
fn rewrite_compare(block: &mut serde_json::Map<String, serde_json::Value>) {
    let highlight = block
        .get("highlight")
        .and_then(serde_json::Value::as_u64)
        .map(|index| index as usize);

    let column_count = block
        .get("columns")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    // Why: The stub column has no heading of its own but still spans the band rows.
    block.insert("colspan".to_owned(), (column_count + 1).into());

    if let Some(columns) = block
        .get_mut("columns")
        .and_then(serde_json::Value::as_array_mut)
    {
        for (index, column) in columns.iter_mut().enumerate() {
            let text = column.take();
            *column = serde_json::json!({
                "text": text,
                "spcol": highlight == Some(index),
            });
        }
    }

    let Some(sections) = block
        .get_mut("sections")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    for section in sections {
        let Some(rows) = section
            .get_mut("rows")
            .and_then(serde_json::Value::as_array_mut)
        else {
            continue;
        };
        for row in rows {
            let Some(cells) = row
                .get_mut("cells")
                .and_then(serde_json::Value::as_array_mut)
            else {
                continue;
            };
            for (index, cell) in cells.iter_mut().enumerate() {
                let value = cell.take();
                *cell = serde_json::json!({
                    "value": value,
                    "spcol": highlight == Some(index),
                });
            }
        }
    }
}

pub(crate) fn css_string(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}

// Why: `{{{lines field}}}` — escape text, rendering newlines as line breaks.
//
// Table cells in the partner sheet stack a calculation over two lines. Putting
// a literal `<br>` in the data would mean accepting markup from the caller;
// a newline is data, and this is where it becomes markup.
pub(crate) fn lines_helper(
    helper: &Helper<'_>,
    _: &Handlebars<'_>,
    _: &Context,
    _: &mut RenderContext<'_, '_>,
    out: &mut dyn Output,
) -> HelperResult {
    let raw = helper
        .param(0)
        .ok_or_else(|| RenderErrorReason::ParamNotFoundForIndex("lines", 0))?
        .value()
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let rendered = raw
        .split('\n')
        .map(crate::inline::escape)
        .collect::<Vec<_>>()
        .join("<br/>");
    out.write(&rendered)?;
    Ok(())
}

// Why: `{{{inline field}}}` — render an [`Inline`] to safe HTML.
pub(crate) fn inline_helper(
    helper: &Helper<'_>,
    _: &Handlebars<'_>,
    _: &Context,
    _: &mut RenderContext<'_, '_>,
    out: &mut dyn Output,
) -> HelperResult {
    let value = helper
        .param(0)
        .ok_or_else(|| RenderErrorReason::ParamNotFoundForIndex("inline", 0))?
        .value();
    let inline: Inline = serde_json::from_value(value.clone())
        .map_err(|e| RenderErrorReason::Other(format!("inline: {e}")))?;
    out.write(&inline.to_html())?;
    Ok(())
}
