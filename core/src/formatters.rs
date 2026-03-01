//! Date/time formatting utilities for chart labels and tick marks.
//!
//! Provides LWC-compatible timestamp formatting functions.

use chrono::{DateTime, Datelike, Timelike, Utc};

/// Format a unix timestamp as a date string: "YYYY-MM-DD"
pub fn format_date(ts: i64) -> String {
    if let Some(dt) = DateTime::<Utc>::from_timestamp(ts, 0) {
        format!("{:04}-{:02}-{:02}", dt.year(), dt.month(), dt.day())
    } else {
        String::new()
    }
}

/// Format a unix timestamp with a custom date pattern.
/// Supported placeholders: %Y (year), %m (month), %d (day)
/// For LWC parity: "dd-MM-yyyy" → "%d-%m-%Y"
pub fn format_date_custom(ts: i64, pattern: &str) -> String {
    if let Some(dt) = DateTime::<Utc>::from_timestamp(ts, 0) {
        dt.format(pattern).to_string()
    } else {
        String::new()
    }
}

/// Format a unix timestamp as datetime: "YYYY-MM-DD HH:MM:SS"
pub fn format_datetime(ts: i64) -> String {
    if let Some(dt) = DateTime::<Utc>::from_timestamp(ts, 0) {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            dt.year(),
            dt.month(),
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.second()
        )
    } else {
        String::new()
    }
}

/// Format a unix timestamp as time only: "HH:MM:SS"
pub fn format_time(ts: i64) -> String {
    if let Some(dt) = DateTime::<Utc>::from_timestamp(ts, 0) {
        format!("{:02}:{:02}:{:02}", dt.hour(), dt.minute(), dt.second())
    } else {
        String::new()
    }
}

/// Format a unix timestamp with a custom time pattern.
/// Supported chrono placeholders: %H (hour), %M (minute), %S (second)
pub fn format_time_custom(ts: i64, pattern: &str) -> String {
    if let Some(dt) = DateTime::<Utc>::from_timestamp(ts, 0) {
        dt.format(pattern).to_string()
    } else {
        String::new()
    }
}

/// Format a tick mark label at year granularity: "2019"
pub fn format_tick_year(ts: i64) -> String {
    if let Some(dt) = DateTime::<Utc>::from_timestamp(ts, 0) {
        format!("{}", dt.year())
    } else {
        String::new()
    }
}

/// Format a tick mark label at month granularity: "Jan", "Dec"
pub fn format_tick_month(ts: i64) -> String {
    if let Some(dt) = DateTime::<Utc>::from_timestamp(ts, 0) {
        dt.format("%b").to_string()
    } else {
        String::new()
    }
}

/// Format a tick mark label at day granularity: "1", "31"
pub fn format_tick_day(ts: i64) -> String {
    if let Some(dt) = DateTime::<Utc>::from_timestamp(ts, 0) {
        format!("{}", dt.day())
    } else {
        String::new()
    }
}

/// Format a tick mark label at time granularity (no seconds): "01:10", "17:59"
pub fn format_tick_time(ts: i64) -> String {
    if let Some(dt) = DateTime::<Utc>::from_timestamp(ts, 0) {
        format!("{:02}:{:02}", dt.hour(), dt.minute())
    } else {
        String::new()
    }
}

/// Format a tick mark label at time granularity with seconds: "01:10:10"
pub fn format_tick_time_with_seconds(ts: i64) -> String {
    format_time(ts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_date_basic() {
        // 2018-01-17 00:00:00 UTC = 1516147200
        assert_eq!(format_date(1516147200), "2018-01-17");
    }

    #[test]
    fn test_format_datetime_basic() {
        // 2018-10-01 08:11:52 UTC = 1538381512
        assert_eq!(format_datetime(1538381512), "2018-10-01 08:11:52");
    }

    #[test]
    fn test_format_time_basic() {
        assert_eq!(format_time(1538381512), "08:11:52");
    }

    #[test]
    fn test_format_tick_year() {
        assert_eq!(format_tick_year(1546300800), "2019"); // 2019-01-01
        assert_eq!(format_tick_year(1577836800), "2020"); // 2020-01-01
    }

    #[test]
    fn test_format_tick_month() {
        assert_eq!(format_tick_month(1546300800), "Jan"); // 2019-01-01
        assert_eq!(format_tick_month(1575158400), "Dec"); // 2019-12-01
    }

    #[test]
    fn test_format_tick_day() {
        assert_eq!(format_tick_day(1546300800), "1"); // 2019-01-01
        assert_eq!(format_tick_day(1548892800), "31"); // 2019-01-31
    }

    #[test]
    fn test_format_tick_time() {
        // 2019-01-01T01:10:00 UTC = 1546304400 + 600 = 1546305000... let me compute
        // 1546300800 = 2019-01-01 00:00:00
        // + 1*3600 + 10*60 = 4200 → 1546305000
        assert_eq!(format_tick_time(1546305000), "01:10");
        // 1546300800 + 17*3600 + 59*60 = 1546300800 + 64740 = 1546365540
        assert_eq!(format_tick_time(1546365540), "17:59");
    }

    #[test]
    fn test_format_tick_time_with_seconds() {
        // 1546300800 + 1*3600 + 10*60 + 10 = 1546305010
        assert_eq!(format_tick_time_with_seconds(1546305010), "01:10:10");
        // 1546300800 + 17*3600 + 59*60 + 44 = 1546365584
        assert_eq!(format_tick_time_with_seconds(1546365584), "17:59:44");
    }
}
