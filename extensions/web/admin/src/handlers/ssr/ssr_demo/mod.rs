//! The Demo section: what the enterprise demo actually did, as the platform
//! recorded it.
//!
//! Four pages over one data layer — a merged governance logbook, skill
//! adoption, MCP tool usage, and the signed-in person's own slice. The first
//! three are admin-only; `/admin/demo/me` is scoped to the caller and is the
//! one admin page a non-admin may open.
//!
//! Token and cost figures are *attributed*, never measured: see
//! [`crate::repositories::demo::attribution`] for the window rule that
//! [`ATTRIBUTION_NOTE`] states on every page.

mod context;
mod logbook;
mod me;
mod skills;
mod tools;
mod view;

use crate::handlers::ssr::format::format_token_total;
use crate::repositories::demo::kpis::DemoKpis;
use view::{AttributedTotals, KpiView, ScenarioCard, ToolVerdictTotals, format_demo_cost};

pub(crate) use logbook::demo_logbook_page;
pub(crate) use me::demo_me_page;
pub(crate) use skills::demo_skills_page;
pub(crate) use tools::demo_tools_page;

const CHART_DAYS: i32 = 14;

const ATTRIBUTION_NOTE: &str = "Tokens and cost are attributed, not \
     measured: an AI request counts toward an invocation when it belongs to the \
     same user and falls between that invocation and the next one of its kind in \
     the session, or the session's last event plus five minutes.";

const fn kpi(label: &'static str, value: String) -> KpiView {
    KpiView {
        label,
        value,
        testid: "",
        variant: "",
    }
}

const fn kpi_tagged(label: &'static str, value: String, testid: &'static str) -> KpiView {
    KpiView {
        label,
        value,
        testid,
        variant: "",
    }
}

const fn kpi_variant(label: &'static str, value: String, variant: &'static str) -> KpiView {
    KpiView {
        label,
        value,
        testid: "",
        variant,
    }
}

// Why: the two screenshot test ids ride on these three builders, so a page that
// prints skill invocations or MCP calls always tags them, and a page that
// prints neither does not invent a card to carry a tag.
fn skill_invocations_kpi(kpis: &DemoKpis) -> KpiView {
    kpi_tagged(
        "Skill invocations",
        kpis.skill_invocations.to_string(),
        "demo-kpi-skill-invocations",
    )
}

fn mcp_calls_kpi(kpis: &DemoKpis) -> KpiView {
    kpi_tagged(
        "MCP tool calls",
        kpis.mcp_calls.to_string(),
        "demo-kpi-mcp-calls",
    )
}

// Why: the two figures are folded from the rows the page's own table lists, so
// a strip can never disagree with the table under it. Only the logbook and the
// personal page, which list every kind of row, pass the combined figure.
fn attributed_kpis(usage: &AttributedTotals) -> [KpiView; 2] {
    [
        kpi("Attributed tokens", format_token_total(usage.total_tokens)),
        kpi("Attributed cost", format_demo_cost(usage.cost_microdollars)),
    ]
}

fn combined_usage(kpis: &DemoKpis) -> AttributedTotals {
    AttributedTotals {
        total_tokens: kpis.attributed_tokens,
        cost_microdollars: kpis.attributed_cost_microdollars,
    }
}

fn logbook_kpi_strip(kpis: &DemoKpis) -> Vec<KpiView> {
    let mut strip = vec![
        skill_invocations_kpi(kpis),
        mcp_calls_kpi(kpis),
        kpi_variant("MCP failures", kpis.mcp_failures.to_string(), "danger"),
        kpi_variant("Held for approval", kpis.held.to_string(), "warning"),
        kpi_variant("Secrets refused", kpis.refused.to_string(), "danger"),
        kpi_variant("Tools blocked", kpis.blocked.to_string(), "danger"),
        kpi_variant("Allowed", kpis.allowed.to_string(), "success"),
        kpi_variant("Approved", kpis.approved.to_string(), "success"),
    ];
    strip.extend(attributed_kpis(&combined_usage(kpis)));
    strip
}

fn skill_kpi_strip(
    kpis: &DemoKpis,
    distinct_skills: i64,
    distinct_users: i64,
    usage: &AttributedTotals,
) -> Vec<KpiView> {
    let mut strip = vec![
        skill_invocations_kpi(kpis),
        kpi("Distinct skills", distinct_skills.to_string()),
        kpi("Distinct users", distinct_users.to_string()),
    ];
    strip.extend(attributed_kpis(usage));
    strip
}

fn tool_kpi_strip(
    kpis: &DemoKpis,
    verdicts: &ToolVerdictTotals,
    usage: &AttributedTotals,
) -> Vec<KpiView> {
    let mut strip = vec![
        mcp_calls_kpi(kpis),
        kpi_variant("Failures", kpis.mcp_failures.to_string(), "danger"),
        kpi_variant("Allowed", verdicts.allowed.to_string(), "success"),
        kpi_variant("Denied", verdicts.denied.to_string(), "danger"),
        kpi_variant("Held", verdicts.held.to_string(), "warning"),
        kpi_variant("Approved", verdicts.approved.to_string(), "success"),
    ];
    strip.extend(attributed_kpis(usage));
    strip
}

// Why: the three scenarios are the demo script (DEMO.md) — each card links at
// the policy stage that produced its count, so a reader can go from "three held
// calls" to the three rows without knowing the query parameter.
fn scenario_cards(kpis: &DemoKpis) -> Vec<ScenarioCard> {
    vec![
        ScenarioCard {
            heading: "A · The held call",
            description: "A tool call parked on a named human. Nothing ran until \
                          someone answered it at the approvals queue.",
            count: kpis.held,
            href: "/admin/governance/decisions?policy=require_approval",
            tone: "warning",
        },
        ScenarioCard {
            heading: "B · The refused secret",
            description: "The secret scanner matched credential-shaped input and \
                          refused the call before it reached the tool.",
            count: kpis.refused,
            href: "/admin/governance/decisions?policy=secret_scan",
            tone: "danger",
        },
        ScenarioCard {
            heading: "C · The blocked tool",
            description: "The blocklist refused a tool this caller is not entitled \
                          to invoke, whatever the arguments were.",
            count: kpis.blocked,
            href: "/admin/governance/decisions?policy=tool_blocklist",
            tone: "danger",
        },
    ]
}
