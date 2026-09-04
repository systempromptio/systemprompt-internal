//! Pins the SQL rules that nothing else type-checks.
//!
//! The pad is documented once in `repositories::demo::attribution` and the
//! real-verdict predicate once in `repositories::demo::policy`, and each is
//! executed by several queries. These assertions fail the moment a query
//! drifts from the documented form. The attribution *window* is no longer
//! among them: it lives in the `skill_invocation_events` view, where the
//! database holds it once for all four readers.

use systemprompt_web_admin::repositories::demo::attribution::ATTRIBUTION_PAD_MINUTES;
use systemprompt_web_admin::repositories::demo::policy::{
    BARE_TOOL_NAME_SQL, REAL_VERDICT_PREDICATE,
};

const SKILL_SQL: &str =
    include_str!("../../../../extensions/web/admin/src/repositories/demo/skill_invocations.rs");
const MCP_SQL: &str =
    include_str!("../../../../extensions/web/admin/src/repositories/demo/mcp_tools/invocations.rs");
const STATS_SQL: &str =
    include_str!("../../../../extensions/web/admin/src/repositories/demo/mcp_tools/stats.rs");
const LOGBOOK_SQL: &str =
    include_str!("../../../../extensions/web/admin/src/repositories/demo/logbook.rs");
const KPIS_SQL: &str =
    include_str!("../../../../extensions/web/admin/src/repositories/demo/kpis.rs");

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
