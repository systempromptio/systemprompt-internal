//! Which `governance_decisions` rows are a verdict on a tool call.
//!
//! Two shapes share the table. A real verdict names the tool and carries the
//! `plugin_id` of the caller. The per-request server authorization writes a row
//! per MCP server instead: `policy` is `authz` or `authz_rule_based`, the
//! `tool_name` is the *server* name (`odoo`, `systemprompt`), and `plugin_id`
//! is NULL. Counting the second shape as an allowed tool call inflates every
//! total by roughly one row per request.
//!
//! Excluding by policy alone does not separate them, because `authz_rule_based`
//! is also the policy of 15701 genuine per-tool verdicts that do carry a
//! `plugin_id`. The discriminator is the pair.
//!
//! Tool names are written both ways: the MCP proxy records the bare name
//! (`crm_lead_search`) and the Claude Code govern hook records the wire name
//! (`mcp__odoo__crm_lead_search`). Any join to hook events must normalise.

pub const REAL_VERDICT_PREDICATE: &str =
    "g.policy <> 'authz' AND NOT (g.policy = 'authz_rule_based' AND g.plugin_id IS NULL)";

pub const BARE_TOOL_NAME_SQL: &str = "CASE WHEN g.tool_name LIKE 'mcp\\_\\_%' THEN substr(g.tool_name, length('mcp__' || split_part(g.tool_name, '__', 2) || '__') + 1) ELSE g.tool_name END";
