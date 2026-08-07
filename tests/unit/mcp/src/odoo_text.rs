//! Chatter HTML to readable text, and query-centred snippets.
//!
//! Odoo's editor emits HTML, so every note passes through this on its way to a
//! model. The failure that matters is silent: markup that survives, or text
//! that runs together because a block tag vanished without a separator, both
//! read as plausible content.

use systemprompt_mcp_odoo::text::{SNIPPET_CHARS, html_to_text, snippet_around};

#[test]
fn tags_are_stripped_and_text_survives() {
    assert_eq!(
        html_to_text("<p>Called the customer</p>"),
        "Called the customer"
    );
}

#[test]
fn block_tags_become_a_separator_rather_than_vanishing() {
    assert_eq!(
        html_to_text("<p>First point</p><p>Second point</p>"),
        "First point Second point",
        "without a separator these two paragraphs would read as one run-on word"
    );
}

#[test]
fn inline_tags_do_not_introduce_spaces() {
    assert_eq!(
        html_to_text("that is <b>very</b> urgent"),
        "that is very urgent",
        "bold mid-sentence must not split the word it wraps"
    );
}

#[test]
fn line_breaks_separate() {
    assert_eq!(html_to_text("one<br>two<br/>three"), "one two three");
}

#[test]
fn the_entities_odoo_emits_are_decoded() {
    assert_eq!(
        html_to_text("<p>Tom&nbsp;&amp;&nbsp;Jerry said &quot;yes&quot; &#39;today&#39;</p>"),
        "Tom & Jerry said \"yes\" 'today'"
    );
}

#[test]
fn editor_indentation_collapses() {
    let html = "<div>\n    <p>\n        Spread over lines\n    </p>\n</div>";

    assert_eq!(
        html_to_text(html),
        "Spread over lines",
        "the editor's indentation is markup, not content"
    );
}

#[test]
fn attributes_never_leak_into_the_text() {
    assert_eq!(
        html_to_text(r#"<a href="https://example.com" title="x">the proposal</a>"#),
        "the proposal",
        "a URL in an attribute would read as body text to a model"
    );
}

#[test]
fn a_truncated_tag_does_not_swallow_the_text_before_it() {
    assert_eq!(html_to_text("visible text <span cla"), "visible text");
}

#[test]
fn empty_and_markup_only_bodies_come_back_empty() {
    assert_eq!(html_to_text(""), "");
    assert_eq!(html_to_text("<p></p><br>"), "");
}

#[test]
fn short_text_is_returned_whole_without_ellipses() {
    let text = "A short note.";

    let snippet = snippet_around(text, "note");

    assert_eq!(snippet, text, "nothing was dropped, so nothing is elided");
}

#[test]
fn a_long_body_is_centred_on_the_match() {
    let text = format!("{}NEEDLE{}", "a".repeat(500), "b".repeat(500));

    let snippet = snippet_around(&text, "needle");

    assert!(
        snippet.contains("NEEDLE"),
        "the whole point is to show the match: {snippet}"
    );
    assert!(snippet.starts_with('…') && snippet.ends_with('…'));
    assert!(
        snippet.contains('a') && snippet.contains('b'),
        "context on both sides"
    );
}

#[test]
fn the_snippet_respects_its_length_budget() {
    let text = "x".repeat(2000);

    let snippet = snippet_around(&text, "x");

    let content = snippet.chars().filter(|c| *c != '…').count();
    assert!(
        content <= SNIPPET_CHARS,
        "{content} chars of content exceeds the {SNIPPET_CHARS} budget"
    );
}

#[test]
fn a_match_at_the_very_start_yields_a_full_width_snippet() {
    let text = format!("NEEDLE{}", "z".repeat(1000));

    let snippet = snippet_around(&text, "needle");

    assert!(
        snippet.starts_with("NEEDLE"),
        "no leading ellipsis is needed"
    );
    let content = snippet.chars().filter(|c| *c != '…').count();
    assert_eq!(
        content, SNIPPET_CHARS,
        "clamping at the start must not shrink the window"
    );
}

#[test]
fn a_query_that_does_not_appear_falls_back_to_the_head() {
    let text = "y".repeat(1000);

    let snippet = snippet_around(&text, "absent");

    assert!(
        !snippet.starts_with('…'),
        "a subject-only match still needs to show something: {}",
        &snippet[..20.min(snippet.len())]
    );
    assert!(snippet.ends_with('…'));
}

#[test]
fn multibyte_text_is_cut_on_character_boundaries() {
    let text = "é".repeat(1000);

    let snippet = snippet_around(&text, "é");

    assert!(snippet.chars().all(|c| c == 'é' || c == '…'));
}
