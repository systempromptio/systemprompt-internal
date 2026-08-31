//! Tool declarations.

use rmcp::model::Tool;
use schemars::schema_for;

use super::{TOOL_GET, TOOL_LIST, TOOL_RENDER, ToolDef, create_tool};
use crate::tools::inputs::{GetInput, ListInput, RenderInput};

pub fn factsheet_tools() -> Vec<Tool> {
    vec![
        create_tool(&ToolDef {
            name: TOOL_LIST,
            title: "List factsheets",
            description: "List the factsheets this instance ships, by id. Every one of them is \
                          data rendered through the same template and the same design system, so \
                          any of them is a usable starting point for a new sheet.",
            input_schema: schema_for!(ListInput).to_value(),
            read_only: true,
        }),
        create_tool(&ToolDef {
            name: TOOL_GET,
            title: "Get factsheet data",
            description: "Return a factsheet's full document model — pages, masthead entries and \
                          typed content blocks. This is the editable form of the sheet. To build \
                          a sheet for a particular customer or lead, get the closest existing \
                          one, change the blocks that should differ, and pass the result to \
                          factsheet_render.",
            input_schema: schema_for!(GetInput).to_value(),
            read_only: true,
        }),
        create_tool(&ToolDef {
            name: TOOL_RENDER,
            title: "Render a factsheet to PDF",
            description: "Render a factsheet to a branded PDF, stored as a file and returned with \
                          a page-image preview. Pass `sheet_id` to render a sheet as it ships, or \
                          `doc` to render one you have edited. The house style is a two-page \
                          document: if the copy overruns that budget the render fails and says \
                          so, rather than quietly producing a longer sheet — shorten the prose \
                          and render again.",
            input_schema: schema_for!(RenderInput).to_value(),
            read_only: false,
        }),
    ]
}
