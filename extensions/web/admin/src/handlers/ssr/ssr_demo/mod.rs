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

use crate::handlers::ssr::format::{format_cost, format_token_total};
use crate::repositories::demo::kpis::DemoKpis;
use view::{KpiView, ScenarioCard};

pub(crate) use logbook::demo_logbook_page;
pub(crate) use me::demo_me_page;
pub(crate) use skills::demo_skills_page;
pub(crate) use tools::demo_tools_page;

const CHART_DAYS: i32 = 14;

const ATTRIBUTION_NOTE: &str = "Tokens and cost are attributed, not \
     measured: an AI request counts toward an invocation when it belongs to the \
     same user and falls between that invocation and the next one of its kind in \
     the session, or the session's last event plus five minutes.";

fn kpi(label: &'static str, value: String) -> KpiView {
    KpiView {
        label,
        value,
        testid: "",
        variant: "",
    }
}

fn kpi_tagged(label: &'static str, value: String, testid: &'static str) -> KpiView {
    KpiView {
        label,
        value,
        testid,
        variant: "",
    }
}

fn kpi_variant(label: &'static str, value: String, variant: &'static str) -> KpiView {
    KpiView {
        label,
        value,
        testid: "",
        variant,
    }
}

// Why: one strip on all four pages, so the two screenshot test ids exist
// wherever a reader lands and the same number never appears twice under two
// different labels.
fn kpi_strip(kpis: &DemoKpis) -> Vec<KpiView> {
    vec![
        kpi_tagged(
            "Skill invocations",
            kpis.skill_invocations.to_string(),
            "demo-kpi-skill-invocations",
        ),
        kpi_tagged(
            "MCP tool calls",
            kpis.mcp_calls.to_string(),
            "demo-kpi-mcp-calls",
        ),
        kpi_variant("MCP failures", kpis.mcp_failures.to_string(), "danger"),
        kpi_variant("Held for approval", kpis.held.to_string(), "warning"),
        kpi_variant("Secrets refused", kpis.refused.to_string(), "danger"),
        kpi_variant("Tools blocked", kpis.blocked.to_string(), "danger"),
        kpi_variant("Approved", kpis.approved.to_string(), "success"),
        kpi(
            "Attributed tokens",
            format_token_total(kpis.attributed_tokens),
        ),
        kpi(
            "Attributed cost",
            format_cost(kpis.attributed_cost_microdollars),
        ),
    ]
}

// Why: the three scenarios are the demo script (DEMO.md) — each card links at
// the policy stage that produced its count, so a reader can go from "three held
// calls" to the three rows without knowing the query parameter.
fn scenario_cards(kpis: &DemoKpis) -> Vec<ScenarioCard> {
    vec![
        ScenarioCard {
            letter: "A",
            title: "A held call",
            description: "A tool call parked on a named human. Nothing ran until \
                          someone answered it at the approvals queue.",
            count: kpis.held,
            href: "/admin/governance/decisions?policy=require_approval",
            tone: "warning",
        },
        ScenarioCard {
            letter: "B",
            title: "A refused secret",
            description: "The secret scanner matched credential-shaped input and \
                          refused the call before it reached the tool.",
            count: kpis.refused,
            href: "/admin/governance/decisions?policy=secret_scan",
            tone: "danger",
        },
        ScenarioCard {
            letter: "C",
            title: "A blocked tool",
            description: "The blocklist refused a tool this caller is not entitled \
                          to invoke, whatever the arguments were.",
            count: kpis.blocked,
            href: "/admin/governance/decisions?policy=tool_blocklist",
            tone: "danger",
        },
    ]
}
