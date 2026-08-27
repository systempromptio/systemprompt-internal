//! `derive_workspace` turns a session's `cwd` into the addressable half of its
//! handle. It has to be total over whatever a hook sends: the path may be
//! empty, may carry a trailing slash, and may contain characters that would
//! make a handle ambiguous to type (`@ed/my repo` reads as two words).

use systemprompt_web_admin::repositories::dashboard::session_registry::derive_workspace;

#[test]
fn the_last_path_segment_becomes_the_workspace() {
    assert_eq!(
        derive_workspace("/var/www/html/systemprompt-internal"),
        Some("systemprompt-internal".to_owned())
    );
}

#[test]
fn trailing_slashes_do_not_produce_an_empty_workspace() {
    assert_eq!(
        derive_workspace("/home/ed/project/"),
        Some("project".to_owned())
    );
    assert_eq!(
        derive_workspace("/home/ed/project///"),
        Some("project".to_owned())
    );
}

#[test]
fn paths_without_a_usable_segment_yield_none() {
    assert_eq!(derive_workspace(""), None);
    assert_eq!(derive_workspace("   "), None);
    assert_eq!(derive_workspace("/"), None);
}

#[test]
fn spaces_become_hyphens_so_a_handle_stays_one_token() {
    assert_eq!(
        derive_workspace("/home/ed/my repo"),
        Some("my-repo".to_owned())
    );
}

#[test]
fn workspaces_are_lowercased() {
    assert_eq!(
        derive_workspace("/home/ed/SystemPrompt"),
        Some("systemprompt".to_owned())
    );
}

#[test]
fn characters_that_would_break_addressing_are_dropped() {
    assert_eq!(
        derive_workspace("/home/ed/repo#2"),
        Some("repo2".to_owned())
    );
    assert_eq!(
        derive_workspace("/home/ed/repo:branch"),
        Some("repobranch".to_owned())
    );
}

#[test]
fn a_segment_of_only_punctuation_yields_none() {
    assert_eq!(derive_workspace("/home/ed/@@@"), None);
}

#[test]
fn non_ascii_segments_that_reduce_to_nothing_yield_none() {
    assert_eq!(derive_workspace("/home/ed/日本語"), None);
}
