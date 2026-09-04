//! Renders every artifact type to HTML on disk, so the branded output can be
//! rasterized and looked at.
//!
//! This is the functional half of `just artifact-gallery`: it asserts the
//! things a machine can judge — that the brand theme actually reached the
//! renderer, and that all twelve registered artifact types produce HTML — and
//! writes `target/artifact-gallery/<type>.html` plus a `manifest.json` for the
//! Playwright spec to screenshot.
//!
//! It lives in this repository rather than core because the brand
//! `ArtifactTheme` is registered in `systemprompt-mcp-shared`; only a binary
//! linking that crate renders branded output at all.

use std::path::{Path, PathBuf};

use systemprompt::identifiers::{ArtifactId, ContextId};
use systemprompt::mcp::services::ui_renderer::{RenderTarget, artifact_ui_resource};

// Why: `inventory` registrations live in a static, and the linker drops an
// rlib nothing references. Naming a real item from the theme's host crate is
// the anchor that guarantees the registration is in this test binary — the
// same rule the crate's own module docs spell out for the MCP servers.
const _THEME_CRATE_ANCHOR: usize = systemprompt_mcp_shared::MAX_REASON_LEN;

// The brand accent, verbatim from
// `extensions/mcp/shared/src/artifact_theme.rs`. Core ships cool-blue neutral
// tokens; if this string is missing the theme did not arrive and every artifact
// Cowork shows is unbranded.
pub const BRAND_ACCENT: &str = "oklch(0.67 0.18 50)";
// The notched top-right corner — the other half of the brand, and the one a
// colour-only assertion would not catch.
const BRAND_RADIUS: &str = "--mcpui-radius-card:  0.75rem 0.25rem 0.75rem 0.75rem;";
// The compressed spacing scale. These artifacts render inside Cowork's own
// padded tool-call card, so the theme re-declares core's page-sized scale; if
// this is missing they are back to paying core's padding on top of Cowork's.
const BRAND_SPACING: &str = "--mcpui-space-3: 0.5rem;";
// `extra_css` is appended after every renderer stylesheet and is what
// suppresses the artifact's own <h1> — the one Cowork's header already
// duplicates. A theme that lost its extra_css would still be branded and still
// be twice as tall.
const BRAND_EXTRA_CSS: &str = ".mcp-app-title {\n  display: none;\n}";

pub fn gallery_dir() -> PathBuf {
    // `tests/target/…` sits two levels below the repository root, the same
    // anchor the MCP binary lookup uses.
    let root = crate::harness::stack::profile_path()
        .parent()
        .expect("profile dir has a parent")
        .ancestors()
        .nth(2)
        .expect("tests/target sits two levels below the repository root")
        .to_path_buf();
    root.join("target/artifact-gallery")
}

pub fn write_gallery_entry(dir: &Path, name: &str, html: &str) -> PathBuf {
    std::fs::create_dir_all(dir).expect("create the gallery directory");
    let path = dir.join(format!("{name}.html"));
    std::fs::write(&path, html).expect("write the gallery entry");
    path
}

struct Case {
    artifact_type: &'static str,
    title: &'static str,
    payload: serde_json::Value,
}

// Mirrors core's `CLI_ARTIFACT_TYPES` plus `form`, which the default registry
// registers but the CLI union does not name. A type added to the registry
// without an entry here renders nowhere in the gallery — which is the point of
// asserting the count below.
fn cases() -> Vec<Case> {
    vec![
        Case {
            artifact_type: "table",
            title: "Pipeline by owner",
            // Currency, percentage and date columns together: the three
            // formatters that render differently from a plain string.
            payload: serde_json::json!({
                "x-artifact-type": "table",
                "title": "Pipeline by owner",
                "columns": [
                    {"name": "name", "column_type": "string", "label": "Opportunity"},
                    {"name": "owner", "column_type": "string", "label": "Owner"},
                    {"name": "value", "column_type": "currency", "label": "Expected revenue", "align": "right"},
                    // Why 65.0 and not 0.65: the `percentage` column type takes a
                    // number already on the 0-100 scale — the same shape Odoo's
                    // own `probability` field uses — and appends the sign. A
                    // fraction here renders as "0.65%".
                    {"name": "probability", "column_type": "percentage", "label": "Probability", "align": "right"},
                    {"name": "close", "column_type": "date", "label": "Expected close"},
                    {"name": "link", "column_type": "link", "label": "Record"}
                ],
                "items": [
                    {"name": "Northwind — platform licence", "owner": "Ana Ruiz", "value": 48250.0, "probability": 65.0, "close": "2026-09-30", "link": "https://example.invalid/leads/1"},
                    {"name": "Beaumont Group — pilot", "owner": "Tom Iwu", "value": 7400.5, "probability": 20.0, "close": "2026-10-14", "link": "https://example.invalid/leads/2"},
                    {"name": "Very long opportunity name that has no spaces at all in it: Northwind-platform-licence-renewal-2026", "owner": "Ana Ruiz", "value": 1200000.0, "probability": 90.0, "close": "2026-12-01", "link": "https://example.invalid/leads/3"}
                ]
            }),
        },
        Case {
            artifact_type: "list",
            title: "Empty list",
            // Deliberately empty: the empty state is the case that renders
            // blank when a renderer regresses.
            payload: serde_json::json!({
                "x-artifact-type": "list",
                "items": [],
                "count": 0
            }),
        },
        Case {
            artifact_type: "text",
            title: "Handover note",
            payload: serde_json::json!({
                "x-artifact-type": "text",
                "title": "Handover note",
                "content": "The pilot closes on 30 September.\n\nTwo blockers remain:\n\n- **[23] SSO mapping** for the sales group — `res.groups`, due 2026-09-08\n- **[24] Signed DPA** from legal — due 2026-09-11\n\nEverything else is agreed."
            }),
        },
        Case {
            artifact_type: "copy_paste_text",
            title: "services/governance/config.yaml",
            payload: serde_json::json!({
                "x-artifact-type": "copy_paste_text",
                "title": "services/governance/config.yaml",
                "language": "yaml",
                "content": "governance:\n  enabled: true\n  stages:\n    - scope_check\n    - secret_scan\n    - blocklist\n    - rate_limit\n    - require_approval\n"
            }),
        },
        Case {
            artifact_type: "dashboard",
            title: "Governance overview",
            payload: serde_json::json!({
                "x-artifact-type": "dashboard",
                "title": "Governance overview",
                "description": "Every tool call in the last 24 hours.",
                "sections": [
                    {
                        "section_id": "metrics",
                        "title": "Today",
                        "section_type": "metrics_cards",
                        "layout": {"width": "full", "order": 0},
                        "data": {"cards": [
                            {"title": "Tool calls", "value": "1,482", "subtitle": "+12% vs yesterday", "status": "success"},
                            {"title": "Held for approval", "value": "7", "subtitle": "median wait 4m", "status": "warning"},
                            {"title": "Denied", "value": "3", "subtitle": "all secret_scan", "status": "error"}
                        ]}
                    },
                    {
                        "section_id": "by-hour",
                        "title": "Calls by hour",
                        "section_type": "chart",
                        "layout": {"width": "twothirds", "order": 1},
                        "data": {
                            "chart_type": "bar",
                            "labels": ["00", "04", "08", "12", "16", "20"],
                            "datasets": [{"label": "Calls", "data": [12.0, 8.0, 240.0, 610.0, 480.0, 132.0]}],
                            "x_axis_label": "Hour",
                            "y_axis_label": "Calls"
                        }
                    },
                    {
                        "section_id": "services",
                        "title": "Servers",
                        "section_type": "status",
                        "layout": {"width": "third", "order": 2},
                        "data": {"services": [
                            {"name": "odoo", "status": "healthy", "uptime": "12d"},
                            {"name": "knowledge-bank", "status": "healthy", "uptime": "12d"}
                        ]}
                    },
                    {
                        "section_id": "malformed",
                        "title": "Deliberately malformed",
                        "section_type": "table",
                        "layout": {"width": "half", "order": 3},
                        // Why: a section whose `data` does not match its
                        // `section_type` must degrade to something legible
                        // rather than blanking the whole dashboard.
                        "data": {"not_a_table": true}
                    }
                ],
                "hints": {"layout": "grid", "refreshable": false, "refresh_interval_seconds": null, "drill_down_enabled": false}
            }),
        },
        Case {
            artifact_type: "chart",
            title: "Tool-call latency",
            // One series, logarithmic axis: the two chart shapes most likely to
            // divide by zero or collapse to a flat line.
            payload: serde_json::json!({
                "x-artifact-type": "chart",
                "title": "Tool-call latency",
                "chart_type": "line",
                "labels": ["p50", "p75", "p90", "p95", "p99"],
                "datasets": [{"label": "milliseconds", "data": [21.0, 48.0, 190.0, 640.0, 4100.0]}],
                "x_axis_label": "Percentile",
                "y_axis_label": "Latency (ms)",
                "x_axis_type": "category",
                "y_axis_type": "logarithmic"
            }),
        },
        Case {
            artifact_type: "audio",
            title: "Discovery call — Northwind",
            payload: serde_json::json!({
                "x-artifact-type": "audio",
                "src": "data:audio/wav;base64,UklGRiQAAABXQVZFZm10IBAAAAABAAEAESsAACJWAAACABAAZGF0YQAAAAA=",
                "mime_type": "audio/wav",
                "title": "Discovery call — Northwind",
                "artist": "Ana Ruiz",
                "controls": true,
                "autoplay": false,
                "loop": false
            }),
        },
        Case {
            artifact_type: "image",
            title: "Architecture sketch",
            payload: serde_json::json!({
                "x-artifact-type": "image",
                "src": "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='320' height='180'><rect width='320' height='180' fill='%23f0a05a'/><text x='24' y='96' font-size='22' fill='%23241a12'>governance spine</text></svg>",
                "alt": "A block diagram of the governance spine",
                "caption": "Five stages, one audit row per call.",
                "width": 320,
                "height": 180
            }),
        },
        Case {
            artifact_type: "video",
            title: "Demo clip",
            payload: serde_json::json!({
                "x-artifact-type": "video",
                "src": "https://example.invalid/demo.mp4",
                "mime_type": "video/mp4",
                "caption": "Approval held, then resolved by an admin.",
                "controls": true,
                "autoplay": false,
                "loop": false,
                "muted": true
            }),
        },
        Case {
            artifact_type: "presentation_card",
            title: "Approval requested",
            // A `secondary` CTA and a long unbroken URL: the two things that
            // used to overflow the card on a narrow viewport.
            payload: serde_json::json!({
                "x-artifact-type": "presentation_card",
                "title": "Approval requested",
                "subtitle": "crm_lead_write · held by require_approval",
                "theme": "gradient",
                "sections": [
                    {"heading": "Requested by", "content": "ana.ruiz@systemprompt.io"},
                    {"heading": "Record", "content": "https://internal.systemprompt.io/admin/governance/approvals/9f2c1ad4e7b64c0fa1d38e5b7c092def?from=mcp&trace=01JQ0Z8XK3M4N5P6R7S8T9V0W1"}
                ],
                "ctas": [
                    {"id": "approve", "label": "Approve", "message": "approve 9f2c1ad4", "variant": "primary"},
                    {"id": "deny", "label": "Deny", "message": "deny 9f2c1ad4", "variant": "secondary"}
                ]
            }),
        },
        Case {
            artifact_type: "message",
            title: "Denied",
            payload: serde_json::json!({
                "x-artifact-type": "message",
                "messages": [
                    {"level": "error", "text": "Denied by secret_scan: the arguments contained an AWS access key id."},
                    {"level": "warn", "text": "This is the third denial from this session."},
                    {"level": "info", "text": "Audited as trace 01JQ0Z8XK3M4N5P6R7S8T9V0W1."}
                ]
            }),
        },
        Case {
            artifact_type: "form",
            title: "Log a follow-up",
            payload: serde_json::json!({
                "x-artifact-type": "form",
                "fields": [
                    {"name": "summary", "label": "Summary", "type": "text", "required": true, "placeholder": "What was agreed?"},
                    {"name": "stage", "label": "Stage", "type": "select", "options": [
                        {"value": "qualified", "label": "Qualified"},
                        {"value": "proposition", "label": "Proposition"},
                        {"value": "won", "label": "Won"}
                    ]},
                    {"name": "follow_up", "label": "Follow up on", "type": "date"}
                ]
            }),
        },
    ]
}

async fn render(case: &Case) -> String {
    let artifact_id = ArtifactId::generate();
    let target = RenderTarget {
        artifact_id: &artifact_id,
        artifact_type: case.artifact_type,
        payload: &case.payload,
        context_id: ContextId::generate(),
        title: Some(case.title.to_owned()),
    };
    artifact_ui_resource(&target)
        .await
        .unwrap_or_else(|e| panic!("{} renders: {e}", case.artifact_type))
        .html
}

#[tokio::test]
async fn every_artifact_type_renders_with_the_brand_theme() {
    let dir = gallery_dir();
    let cases = cases();
    assert_eq!(
        cases.len(),
        12,
        "the gallery must cover every renderer the default registry registers"
    );

    let mut manifest = Vec::new();
    for case in &cases {
        let html = render(case).await;

        assert!(
            html.contains(BRAND_ACCENT),
            "{}: the brand accent is missing — the ArtifactTheme registration \
             did not reach this binary, so Cowork is being served core's \
             unbranded neutral tokens",
            case.artifact_type
        );
        assert!(
            html.contains(BRAND_RADIUS),
            "{}: the notched card radius is missing — half the brand is colour \
             and half is this corner",
            case.artifact_type
        );
        assert!(
            html.contains(BRAND_SPACING),
            "{}: the compressed spacing scale is missing — this artifact is \
             rendering at core's page-sized padding inside Cowork's card",
            case.artifact_type
        );
        assert!(
            html.contains(BRAND_EXTRA_CSS),
            "{}: the theme's extra_css did not reach the document, so the \
             artifact restates the title Cowork's own header already shows",
            case.artifact_type
        );
        assert!(
            html.contains(case.title),
            "{}: the title did not reach the output",
            case.artifact_type
        );

        let path = write_gallery_entry(&dir, case.artifact_type, &html);
        manifest.push(serde_json::json!({
            "type": case.artifact_type,
            "title": case.title,
            "file": path.file_name().and_then(|n| n.to_str()).unwrap_or_default(),
        }));
    }

    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_string_pretty(&serde_json::json!({ "entries": manifest }))
            .expect("serialize the manifest"),
    )
    .expect("write the manifest");
}
