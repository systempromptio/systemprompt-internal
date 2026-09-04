//! Pins the attribution window into the two queries that implement it.
//!
//! The rule is documented once in `repositories::demo::attribution`, but it is
//! executed twice, in SQL, where nothing type-checks it. These assertions fail
//! the moment one query's window drifts from the documented predicate.

use systemprompt_web_admin::repositories::demo::attribution::{
    ATTRIBUTION_PAD_MINUTES, MCP_WINDOW_PREDICATE, SKILL_WINDOW_PREDICATE,
};
use systemprompt_web_admin::repositories::demo::policy::{
    BARE_TOOL_NAME_SQL, REAL_VERDICT_PREDICATE,
};

const SKILL_SQL: &str =
    include_str!("../../../../extensions/web/admin/src/repositories/demo/skill_invocations.rs");
const MCP_SQL: &str =
    include_str!("../../../../extensions/web/admin/src/repositories/demo/mcp_tools/invocations.rs");

#[test]
fn the_skill_query_uses_the_documented_window() {
    assert!(
        SKILL_SQL.contains(SKILL_WINDOW_PREDICATE),
        "skill_invocations.rs no longer contains SKILL_WINDOW_PREDICATE"
    );
}

#[test]
fn the_mcp_query_uses_the_documented_window() {
    assert!(
        MCP_SQL.contains(MCP_WINDOW_PREDICATE),
        "mcp_tools/invocations.rs no longer contains MCP_WINDOW_PREDICATE"
    );
}

#[test]
fn both_queries_pad_by_the_documented_interval() {
    let pad = format!("make_interval(mins => ${}::int)", 3);
    assert!(SKILL_SQL.contains(&pad));
    assert!(MCP_SQL.contains(&pad));
    assert_eq!(ATTRIBUTION_PAD_MINUTES, 5);
}

// The server-authorization rows must be excluded by shape, never by policy name
// alone: `authz_rule_based` is also the policy of genuine per-tool verdicts,
// and excluding it wholesale zeroes every allowed count.
#[test]
fn every_decision_query_excludes_the_noise_by_shape() {
    for (name, sql) in [
        ("mcp_tools/stats.rs", STATS_SQL),
        ("logbook.rs", LOGBOOK_SQL),
        ("kpis.rs", KPIS_SQL),
    ] {
        assert!(
            sql.contains(REAL_VERDICT_PREDICATE),
            "{name} no longer contains REAL_VERDICT_PREDICATE"
        );
        assert!(
            !sql.contains("policy NOT IN ('authz'"),
            "{name} still excludes decisions by policy name alone"
        );
    }
}

// Hook-path decisions carry the wire name, proxy-path decisions the bare name.
#[test]
fn the_governance_join_normalises_the_tool_name() {
    assert!(
        STATS_SQL.contains(BARE_TOOL_NAME_SQL),
        "mcp_tools/stats.rs no longer normalises governance_decisions.tool_name"
    );
}
