//! Tool inputs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_factsheet::FactsheetDoc;

/// Takes no arguments. The braces are load-bearing: a unit struct serialises
/// to a schema that is not an object, and MCP tool inputs must be objects.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[expect(
    clippy::empty_structs_with_brackets,
    reason = "the tool's input schema must be a JSON object"
)]
pub struct ListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetInput {
    /// Sheet id, as returned by `factsheet_list` — the filename stem, e.g. `ceo`.
    pub id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct RenderInput {
    /// Render one of the sheets this instance ships, by id.
    #[serde(default)]
    pub sheet_id: Option<String>,

    /// Render a sheet supplied inline. This is how a factsheet for a specific
    /// lead is produced: fetch a shipped sheet with `factsheet_get`, change the
    /// blocks that should differ, and pass the result back here. Takes
    /// precedence over `sheet_id`.
    #[serde(default)]
    pub doc: Option<FactsheetDoc>,
}
