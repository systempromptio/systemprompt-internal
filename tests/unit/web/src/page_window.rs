//! `PageWindow::new` ceiling division: the empty result set is the one case
//! the arithmetic cannot produce ("page 1 of 1", bounds (0, 0)).

use systemprompt_web_admin::test_support::PageWindow;

#[test]
fn empty_result_is_one_page() {
    let w = PageWindow::new(0, 50, 0, 0, "rows");
    assert_eq!(w.total_pages, 1);
    assert_eq!(w.bounds(), (0, 0));
}

#[test]
fn exact_multiple_has_no_phantom_page() {
    let w = PageWindow::new(0, 50, 100, 50, "rows");
    assert_eq!(w.total_pages, 2);
}

#[test]
fn remainder_rounds_up() {
    let w = PageWindow::new(1, 50, 54, 4, "rows");
    assert_eq!(w.total_pages, 2);
    assert_eq!(w.bounds(), (51, 54));
}
