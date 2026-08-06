//! The knowledge bank's query-shaping decisions, made before any SQL runs:
//! which of the three search modes a query earns, how a caller's limit is
//! clamped, how an `ILIKE` pattern is escaped, and the two gates upload has to
//! pass. Each is a pure function precisely so it can be pinned without a
//! database.

use systemprompt_mcp_knowledge_bank::store::{
    DEFAULT_SEARCH_LIMIT, MAX_CONTENT_BYTES, MAX_SEARCH_LIMIT, SearchMode, check_content_size,
    clamp_search_limit, like_pattern, normalize_optional, require_non_empty, search_mode,
};

#[test]
fn an_empty_or_whitespace_query_falls_back_to_the_newest_documents() {
    for query in ["", "   ", "\n\t "] {
        assert_eq!(
            search_mode(query),
            SearchMode::Newest,
            "a query of {query:?} has nothing to match on"
        );
    }
}

#[test]
fn a_single_character_query_is_treated_as_empty() {
    // `ILIKE '%a%'` matches nearly every document, which is a worse answer
    // than "here is what is newest".
    assert_eq!(search_mode("a"), SearchMode::Newest);
    assert_eq!(search_mode("  x  "), SearchMode::Newest);
    assert_eq!(search_mode("ab"), SearchMode::FullText);
}

#[test]
fn a_real_query_goes_to_full_text() {
    for query in ["checkout", "guest checkout", "\"exact phrase\" -excluded"] {
        assert_eq!(search_mode(query), SearchMode::FullText);
    }
}

#[test]
fn an_absent_limit_uses_the_default() {
    assert_eq!(clamp_search_limit(None), i64::from(DEFAULT_SEARCH_LIMIT));
}

#[test]
fn an_out_of_range_limit_is_clamped_rather_than_refused() {
    // An agent asking for 100_000 results wants "as many as you have".
    assert_eq!(clamp_search_limit(Some(0)), 1);
    assert_eq!(clamp_search_limit(Some(1)), 1);
    assert_eq!(clamp_search_limit(Some(100_000)), i64::from(MAX_SEARCH_LIMIT));
    assert_eq!(
        clamp_search_limit(Some(MAX_SEARCH_LIMIT)),
        i64::from(MAX_SEARCH_LIMIT)
    );
}

#[test]
fn an_in_range_limit_is_passed_through() {
    assert_eq!(clamp_search_limit(Some(7)), 7);
}

#[test]
fn the_like_pattern_wraps_the_trimmed_query_in_wildcards() {
    assert_eq!(like_pattern("  checkout  "), "%checkout%");
}

#[test]
fn the_like_pattern_escapes_the_wildcards_the_caller_typed() {
    // Without escaping, a query of "%" would match every document and a "_"
    // would silently match any single character.
    assert_eq!(like_pattern("100%"), r"%100\%%");
    assert_eq!(like_pattern("a_b"), r"%a\_b%");
    assert_eq!(like_pattern(r"back\slash"), r"%back\\slash%");
}

#[test]
fn content_within_the_cap_is_accepted() {
    assert!(check_content_size("a short note").is_ok());
    assert!(check_content_size(&"x".repeat(MAX_CONTENT_BYTES)).is_ok());
}

#[test]
fn content_over_the_cap_is_refused_and_the_message_says_by_how_much() {
    let oversized = "x".repeat(MAX_CONTENT_BYTES + 1);

    let error = check_content_size(&oversized).expect_err("one byte over the cap is refused");

    let message = error.to_string();
    assert!(
        message.contains(&(MAX_CONTENT_BYTES + 1).to_string()),
        "the refusal reports the actual size: {message}"
    );
    assert!(
        message.contains(&MAX_CONTENT_BYTES.to_string()),
        "the refusal reports the permitted size: {message}"
    );
    assert!(
        message.contains("Split the document"),
        "the refusal says what to do next: {message}"
    );
}

#[test]
fn the_cap_counts_bytes_not_characters() {
    // Multi-byte text must not slip past a byte cap by counting chars.
    let multibyte = "é".repeat(MAX_CONTENT_BYTES / 2 + 1);
    assert!(multibyte.chars().count() < MAX_CONTENT_BYTES);
    assert!(check_content_size(&multibyte).is_err());
}

#[test]
fn a_blank_optional_filter_is_the_same_as_an_absent_one() {
    // The MCP wire cannot distinguish "omitted" from "empty string", and the
    // two mean the same thing to a project filter.
    assert_eq!(normalize_optional(None), None);
    assert_eq!(normalize_optional(Some(String::new())), None);
    assert_eq!(normalize_optional(Some("   ".to_owned())), None);
}

#[test]
fn a_populated_optional_filter_is_trimmed_and_kept() {
    assert_eq!(
        normalize_optional(Some("  acme-storefront ".to_owned())),
        Some("acme-storefront".to_owned())
    );
}

#[test]
fn a_required_upload_field_is_trimmed() {
    assert_eq!(
        require_non_empty("title", "  Kickoff Notes  ").expect("a populated field passes"),
        "Kickoff Notes"
    );
}

#[test]
fn a_blank_required_upload_field_is_refused_by_name() {
    let error = require_non_empty("title", "   ").expect_err("a blank title is refused");
    assert!(
        error.to_string().contains("title"),
        "the refusal names the offending field: {error}"
    );
}
