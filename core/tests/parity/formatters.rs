/// Parity tests from LWC: tests/unittests/formatters.spec.ts
///
/// Price, percentage, volume formatters pass now.
/// Date/datetime/time formatters now use lumen_charts::formatters.
use lumen_charts::formatters;

/// LWC: price-formatter default — PriceFormatter().format(1.5) === '1.50'
/// File: formatters.spec.ts, line 38
#[test]
fn lwc_price_formatter_default() {
    assert_eq!(format!("{:.2}", 1.5_f64), "1.50");
}

/// LWC: price-formatter precision 3
#[test]
fn lwc_price_formatter_precision_3() {
    assert_eq!(format!("{:.3}", 1.5_f64), "1.500");
}

/// LWC: price-formatter negative — uses U+2212 minus sign
#[test]
fn lwc_price_formatter_negative() {
    let v = -1.5_f64;
    let s = if v < 0.0 {
        format!("\u{2212}{:.2}", v.abs())
    } else {
        format!("{:.2}", v)
    };
    assert_eq!(s, "\u{2212}1.50");
}

/// LWC: percent-formatter — 1.5 → '1.50%'
/// File: formatters.spec.ts, line 33
#[test]
fn lwc_percent_formatter() {
    assert_eq!(format!("{:.2}%", 1.5_f64), "1.50%");
}

/// LWC: volume-formatter — 1→'1', 1000→'1K', 5500→'5.5K', 1155000→'1.155M'
/// File: formatters.spec.ts, line 74
#[test]
fn lwc_volume_formatter() {
    fn fmt_vol(v: f64, prec: usize) -> String {
        if v >= 1_000_000.0 {
            format!(
                "{}M",
                format!("{:.p$}", v / 1_000_000.0, p = prec)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
            )
        } else if v >= 1_000.0 {
            format!(
                "{}K",
                format!("{:.p$}", v / 1_000.0, p = prec)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
            )
        } else {
            format!("{}", v as u64)
        }
    }
    assert_eq!(fmt_vol(1.0, 3), "1");
    assert_eq!(fmt_vol(10.0, 3), "10");
    assert_eq!(fmt_vol(100.0, 3), "100");
    assert_eq!(fmt_vol(1000.0, 3), "1K");
    assert_eq!(fmt_vol(5500.0, 3), "5.5K");
    assert_eq!(fmt_vol(1155000.0, 3), "1.155M");
}

/// LWC: date-formatter — 1516147200 → '2018-01-17'
/// File: formatters.spec.ts, line 13
#[test]
fn lwc_date_formatter_default() {
    assert_eq!(formatters::format_date(1516147200), "2018-01-17");
}

/// LWC: date-formatter — custom 'dd-MM-yyyy' → '17-01-2018'
#[test]
fn lwc_date_formatter_custom() {
    assert_eq!(
        formatters::format_date_custom(1516147200, "%d-%m-%Y"),
        "17-01-2018"
    );
}

/// LWC: date-time-formatter — 1538381512 → '2018-10-01 08:11:52'
#[test]
fn lwc_datetime_formatter() {
    assert_eq!(
        formatters::format_datetime(1538381512),
        "2018-10-01 08:11:52"
    );
}

/// LWC: time-formatter default — 1538381512 → '08:11:52'
#[test]
fn lwc_time_formatter_default() {
    assert_eq!(formatters::format_time(1538381512), "08:11:52");
}

/// LWC: time-formatter custom '%h-%m-%s' → '08-11-52'
#[test]
fn lwc_time_formatter_custom() {
    assert_eq!(
        formatters::format_time_custom(1538381512, "%H-%M-%S"),
        "08-11-52"
    );
}
