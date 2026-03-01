/// Parity tests from LWC: tests/unittests/default-tick-mark-formatter.spec.ts
///
/// Tests for time axis tick mark formatting at different granularities.
/// All #[ignore] — tick formatting not yet implemented.

/// LWC: correct format year — '2019', '2020'
/// File: default-tick-mark-formatter.spec.ts, line 15
/// Input: 2019-01-01 UTC → '2019'
#[test]
#[ignore]
fn lwc_tick_format_year() {
    // timestamp for 2019-01-01 00:00:00 UTC = 1546300800
    // timestamp for 2020-01-01 00:00:00 UTC = 1577836800
    // Expected: "2019", "2020"
    todo!("Implement tick mark formatter for year");
}

/// LWC: correct format month — 'Jan', 'Dec'
/// File: default-tick-mark-formatter.spec.ts, line 20
#[test]
#[ignore]
fn lwc_tick_format_month() {
    // 2019-01-01 → 'Jan', 2019-12-01 → 'Dec'
    todo!("Implement tick mark formatter for month");
}

/// LWC: correct format day of month — '1', '31'
/// File: default-tick-mark-formatter.spec.ts, line 28
#[test]
#[ignore]
fn lwc_tick_format_day() {
    // 2019-01-01 → '1', 2019-01-31 → '31'
    todo!("Implement tick mark formatter for day");
}

/// LWC: correct format time without seconds — '01:10', '17:59'
/// File: default-tick-mark-formatter.spec.ts, line 33
#[test]
#[ignore]
fn lwc_tick_format_time() {
    // 2019-01-01T01:10:00 → '01:10'
    // 2019-01-01T17:59:00 → '17:59'
    // 2019-01-01T18:59:59 → '18:59' (no seconds)
    todo!("Implement tick mark formatter for time");
}

/// LWC: correct format time with seconds — '01:10:10', '17:59:44'
/// File: default-tick-mark-formatter.spec.ts, line 39
#[test]
#[ignore]
fn lwc_tick_format_time_with_seconds() {
    // 2019-01-01T01:10:10 → '01:10:10'
    // 2019-01-01T17:59:44 → '17:59:44'
    todo!("Implement tick mark formatter for time with seconds");
}
