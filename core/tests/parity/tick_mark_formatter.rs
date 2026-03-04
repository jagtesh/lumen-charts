/// Parity tests from LWC: tests/unittests/default-tick-mark-formatter.spec.ts
///
/// Tests for time axis tick mark formatting at different granularities.
/// Uses lumen_charts_core::formatters for all formatting.
use lumen_charts_core::formatters;

/// LWC: correct format year — '2019', '2020'
/// File: default-tick-mark-formatter.spec.ts, line 15
/// Input: 2019-01-01 UTC → '2019'
#[test]
fn lwc_tick_format_year() {
    // 2019-01-01 00:00:00 UTC = 1546300800
    // 2020-01-01 00:00:00 UTC = 1577836800
    assert_eq!(formatters::format_tick_year(1546300800), "2019");
    assert_eq!(formatters::format_tick_year(1577836800), "2020");
}

/// LWC: correct format month — 'Jan', 'Dec'
/// File: default-tick-mark-formatter.spec.ts, line 20
#[test]
fn lwc_tick_format_month() {
    // 2019-01-01 → 'Jan'
    assert_eq!(formatters::format_tick_month(1546300800), "Jan");
    // 2019-12-01 → 'Dec' (1575158400)
    assert_eq!(formatters::format_tick_month(1575158400), "Dec");
}

/// LWC: correct format day of month — '1', '31'
/// File: default-tick-mark-formatter.spec.ts, line 28
#[test]
fn lwc_tick_format_day() {
    // 2019-01-01 → '1'
    assert_eq!(formatters::format_tick_day(1546300800), "1");
    // 2019-01-31 → '31' (1548892800)
    assert_eq!(formatters::format_tick_day(1548892800), "31");
}

/// LWC: correct format time without seconds — '01:10', '17:59'
/// File: default-tick-mark-formatter.spec.ts, line 33
#[test]
fn lwc_tick_format_time() {
    // 2019-01-01T01:10:00 UTC = 1546300800 + 3600 + 600 = 1546305000
    assert_eq!(formatters::format_tick_time(1546305000), "01:10");
    // 2019-01-01T17:59:00 UTC = 1546300800 + 17*3600 + 59*60 = 1546365540
    assert_eq!(formatters::format_tick_time(1546365540), "17:59");
    // 2019-01-01T18:59:59 → '18:59' (no seconds in this format)
    assert_eq!(formatters::format_tick_time(1546369199), "18:59");
}

/// LWC: correct format time with seconds — '01:10:10', '17:59:44'
/// File: default-tick-mark-formatter.spec.ts, line 39
#[test]
fn lwc_tick_format_time_with_seconds() {
    // 2019-01-01T01:10:10 UTC = 1546305010
    assert_eq!(
        formatters::format_tick_time_with_seconds(1546305010),
        "01:10:10"
    );
    // 2019-01-01T17:59:44 UTC = 1546365584
    assert_eq!(
        formatters::format_tick_time_with_seconds(1546365584),
        "17:59:44"
    );
}
