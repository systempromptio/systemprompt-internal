//! `KnowledgeStore::search` ranking, beyond the "does it find anything" cases
//! in the `knowledge_store` module: a document's score is the total number of
//! term occurrences across title and content, terms of two characters or fewer
//! contribute nothing at all, matching is case-insensitive, `limit` truncates
//! from the bottom of the ranking, and the project filter is applied before
//! scoring.

use systemprompt_mcp_knowledge_bank::store::KnowledgeStore;

fn store() -> KnowledgeStore {
    KnowledgeStore::seeded().expect("bundled fixtures must parse")
}

#[test]
fn ranks_by_occurrence_count_descending() {
    // "checkout" occurs twice in the workshop transcript (title + content) and
    // once in ACME-1042's title, so the transcript must rank first.
    let results = store().search("checkout", None, 10);
    let ids: Vec<&str> = results.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["transcript-2026-06-12-checkout-workshop", "jira-ACME-1042"]
    );
}

#[test]
fn a_short_term_contributes_nothing_alongside_a_long_one() {
    let plain = store().search("validation", None, 10);
    let padded = store().search("validation of to", None, 10);
    let plain_ids: Vec<&str> = plain.iter().map(|d| d.id.as_str()).collect();
    let padded_ids: Vec<&str> = padded.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(plain_ids, vec!["jira-ACME-1042"]);
    assert_eq!(plain_ids, padded_ids);
}

#[test]
fn matching_is_case_insensitive_on_the_query_side() {
    let upper = store().search("CHECKOUT", None, 10);
    let lower = store().search("checkout", None, 10);
    assert!(!upper.is_empty());
    let upper_ids: Vec<&str> = upper.iter().map(|d| d.id.as_str()).collect();
    let lower_ids: Vec<&str> = lower.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(upper_ids, lower_ids);
}

#[test]
fn limit_truncates_from_the_bottom_of_the_ranking() {
    let unlimited = store().search("checkout", None, 10);
    assert_eq!(unlimited.len(), 2);
    let limited = store().search("checkout", None, 1);
    assert_eq!(limited.len(), 1);
    assert_eq!(limited[0].id, unlimited[0].id);
}

#[test]
fn project_filter_is_applied_before_scoring() {
    assert!(
        !store()
            .search("checkout", Some("acme-storefront"), 10)
            .is_empty()
    );
    assert!(
        store()
            .search("checkout", Some("no-such-project"), 10)
            .is_empty()
    );
}
