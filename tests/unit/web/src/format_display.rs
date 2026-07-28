//! Display formatting bands: a cost or duration must render identically on
//! every admin page, and zero/negative cost renders as an em-dash rather
//! than `$0` (no billed traffic is not the same as free traffic).

use systemprompt_web_admin::test_support::{format_cost, format_duration_ms, short_num};

#[test]
fn cost_zero_and_negative_render_as_dash() {
    assert_eq!(format_cost(0), "\u{2014}");
    assert_eq!(format_cost(-5), "\u{2014}");
}

#[test]
fn cost_precision_bands() {
    assert_eq!(format_cost(2_500_000), "$2.50");
    assert_eq!(format_cost(50_000), "$0.0500");
    assert_eq!(format_cost(900), "$0.000900");
}

#[test]
fn duration_bands() {
    assert_eq!(format_duration_ms(800), "800 ms");
    assert_eq!(format_duration_ms(1_200), "1.20 s");
    assert_eq!(format_duration_ms(90_000), "1.5 min");
    assert_eq!(format_duration_ms(5_400_000), "1.5 h");
}

#[test]
fn short_num_bands() {
    assert_eq!(short_num(999), "999");
    assert_eq!(short_num(1_500), "1.5k");
    assert_eq!(short_num(2_500_000), "2.5M");
}
