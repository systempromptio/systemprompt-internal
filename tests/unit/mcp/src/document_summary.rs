//! `document_summary` renders search hits as the markdown body handed back to
//! the model: an explicit sentinel when nothing matched (never an empty
//! string, which would read as a broken tool), otherwise one `##` section per
//! document carrying its provenance — type, project, date — ahead of the text.

use systemprompt_mcp_knowledge_bank::server::tool::document_summary;
use systemprompt_mcp_knowledge_bank::store::Document;

fn document(id: &str, title: &str, content: &str) -> Document {
    Document {
        id: id.to_owned(),
        doc_type: "jira".to_owned(),
        project: "acme-storefront".to_owned(),
        title: title.to_owned(),
        date: "2026-07-03".to_owned(),
        content: content.to_owned(),
    }
}

#[test]
fn empty_slice_returns_the_no_match_sentinel() {
    let out = document_summary(&[]);
    assert_eq!(out, "No matching documents in the knowledge bank.");
    assert!(!out.is_empty());
}

#[test]
fn a_single_document_renders_a_heading_with_its_provenance() {
    let out = document_summary(&[document("jira-1", "ACME-1: Guest checkout", "Fix agreed.")]);
    assert_eq!(
        out,
        "## ACME-1: Guest checkout (jira, acme-storefront, 2026-07-03)\n\nFix agreed."
    );
}

#[test]
fn multiple_documents_are_separated_by_a_blank_line_and_keep_input_order() {
    let out = document_summary(&[
        document("jira-1", "First", "Body one."),
        document("jira-2", "Second", "Body two."),
    ]);
    let sections: Vec<&str> = out.split("\n\n## ").collect();
    assert_eq!(sections.len(), 2);
    assert!(sections[0].starts_with("## First "));
    assert!(sections[1].starts_with("Second "));
    assert!(
        out.find("First").expect("first present") < out.find("Second").expect("second present")
    );
}
