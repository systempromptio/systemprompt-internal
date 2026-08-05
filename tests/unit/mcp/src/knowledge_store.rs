//! `KnowledgeStore` seeds from the bundled fixtures and answers keyword
//! search, filtered listing, and in-process inserts — the stub contract the
//! real RAG server replaces.

use systemprompt_mcp_knowledge_bank::store::{Document, KnowledgeStore};

fn store() -> KnowledgeStore {
    KnowledgeStore::seeded().expect("bundled fixtures must parse")
}

#[test]
fn seeds_fixture_documents() {
    let s = store();
    assert!(s.count() >= 6, "expected the seeded fixture set");
}

#[test]
fn fixtures_cover_all_three_source_shapes() {
    let s = store();
    for doc_type in ["transcript", "jira", "confluence"] {
        assert!(
            !s.list_documents(None, Some(doc_type)).is_empty(),
            "no fixture of type {doc_type}"
        );
    }
}

#[test]
fn list_filters_by_project_and_type() {
    let s = store();
    let all = s.list_documents(None, None);
    let scoped = s.list_documents(Some("acme-storefront"), Some("jira"));
    assert!(!scoped.is_empty());
    assert!(scoped.len() < all.len());
    assert!(
        scoped
            .iter()
            .all(|d| d.project == "acme-storefront" && d.doc_type == "jira")
    );
}

#[test]
fn list_with_unknown_project_is_empty() {
    assert!(
        store()
            .list_documents(Some("no-such-project"), None)
            .is_empty()
    );
}

#[test]
fn search_finds_relevant_documents() {
    let s = store();
    let results = s.search("guest checkout", None, 5);
    assert!(!results.is_empty());
    assert!(
        results[0].title.to_lowercase().contains("checkout")
            || results[0].content.to_lowercase().contains("checkout")
    );
}

#[test]
fn search_respects_limit() {
    let s = store();
    let results = s.search("the", None, 1);
    assert!(results.len() <= 1);
}

#[test]
fn search_ignores_short_stopword_like_terms() {
    let s = store();
    assert!(s.search("a an to", None, 5).is_empty());
}

#[test]
fn search_with_no_match_is_empty() {
    assert!(
        store()
            .search("zeppelin quantum walrus", None, 5)
            .is_empty()
    );
}

#[test]
fn inserted_document_is_immediately_searchable() {
    let s = store();
    let before = s.count();
    s.insert(Document {
        id: "transcript-sprint-retrospective".to_owned(),
        doc_type: "transcript".to_owned(),
        project: "acme-storefront".to_owned(),
        title: "Sprint retrospective".to_owned(),
        date: "2026-08-05".to_owned(),
        content: "Team agreed to adopt xenolith naming for feature branches.".to_owned(),
    });
    assert_eq!(s.count(), before + 1);
    let results = s.search("xenolith", None, 5);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "transcript-sprint-retrospective");
}
