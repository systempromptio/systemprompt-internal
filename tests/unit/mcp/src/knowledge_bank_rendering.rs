//! How search hits and listing rows reach the model.
//!
//! Both renderers return an explicit sentence when they have nothing — an
//! empty body reads as a broken tool rather than an answer — and both carry
//! the row's id, because the id is what a follow-up call needs and an agent
//! that has to guess it will guess wrong. Listing rows carry a size and never
//! the content.

use chrono::{DateTime, TimeZone as _, Utc};
use systemprompt_mcp_knowledge_bank::server::tool::{
    NO_DOCUMENTS, NO_MATCHES, listing_summary, search_summary,
};
use systemprompt_mcp_knowledge_bank::store::{DocumentSummary, SearchHit};
use uuid::Uuid;

fn created_at() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 7, 3, 9, 30, 0)
        .single()
        .expect("a valid instant")
}

fn hit(title: &str, project: Option<&str>, snippet: &str) -> SearchHit {
    SearchHit {
        id: Uuid::nil(),
        title: title.to_owned(),
        source: "meeting-transcript".to_owned(),
        project: project.map(str::to_owned),
        created_at: created_at(),
        uploaded_by: "user-7".to_owned(),
        snippet: snippet.to_owned(),
    }
}

fn row(title: &str, project: Option<&str>, size: i32) -> DocumentSummary {
    DocumentSummary {
        id: Uuid::nil(),
        title: title.to_owned(),
        source: "document".to_owned(),
        project: project.map(str::to_owned),
        created_at: created_at(),
        size,
    }
}

#[test]
fn no_hits_renders_the_sentinel_rather_than_an_empty_body() {
    let out = search_summary(&[]);
    assert_eq!(out, NO_MATCHES);
    assert!(!out.is_empty());
}

#[test]
fn a_hit_leads_with_its_title_and_provenance() {
    let out = search_summary(&[hit("Kickoff", Some("acme"), "We agreed to ship on Friday.")]);
    assert!(
        out.starts_with("## Kickoff (meeting-transcript, acme, 2026-07-03)"),
        "{out}"
    );
    assert!(out.contains("We agreed to ship on Friday."));
}

#[test]
fn a_hit_carries_the_id_a_follow_up_call_needs() {
    let out = search_summary(&[hit("Kickoff", Some("acme"), "body")]);
    assert!(
        out.contains(&Uuid::nil().to_string()),
        "the document id is rendered, not just its title: {out}"
    );
    assert!(out.contains("uploaded by: user-7"));
}

#[test]
fn an_unscoped_document_says_so_rather_than_rendering_nothing() {
    // `project` is nullable, and a blank in the provenance line would read as
    // a rendering bug.
    let out = search_summary(&[hit("Standalone note", None, "body")]);
    assert!(out.contains("(meeting-transcript, unscoped, 2026-07-03)"), "{out}");
}

#[test]
fn hits_keep_their_ranking_order_and_are_separated_by_a_blank_line() {
    let out = search_summary(&[
        hit("First", Some("acme"), "body one"),
        hit("Second", Some("acme"), "body two"),
    ]);
    assert_eq!(out.matches("\n## ").count(), 1, "two headings, one join");
    assert!(out.find("First").expect("first") < out.find("Second").expect("second"));
}

#[test]
fn an_empty_listing_renders_the_filter_sentinel() {
    assert_eq!(listing_summary(&[]), NO_DOCUMENTS);
}

#[test]
fn a_listing_row_reports_a_size_and_withholds_the_content() {
    let out = listing_summary(&[row("Architecture decision record", Some("acme"), 4096)]);
    assert!(out.contains("4096 chars"), "{out}");
    assert!(out.contains("Architecture decision record"));
    assert!(out.contains(&Uuid::nil().to_string()));
}

#[test]
fn a_listing_renders_exactly_one_line_per_document() {
    let out = listing_summary(&[
        row("One", Some("acme"), 10),
        row("Two", None, 20),
        row("Three", Some("beta"), 30),
    ]);
    assert_eq!(out.lines().count(), 3);
    assert!(
        out.lines().all(|line| line.starts_with("- ")),
        "each row is a list item: {out}"
    );
    assert!(
        out.contains("unscoped"),
        "an untagged document lists as unscoped: {out}"
    );
}
