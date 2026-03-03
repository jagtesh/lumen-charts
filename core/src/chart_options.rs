/// Chart configuration options — mirrors LWC's ChartOptions / TimeChartOptions.
///
/// All colors use the `Color` newtype (RGBA f32, 0.0–1.0).
use crate::draw_backend::{Color, ColorName};
use serde::{Deserialize, Serialize};

/// Price formatting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PriceFormatOptions {
    /// Number of decimal places
    pub precision: u8,
    /// Optional prefix (e.g. "$")
    pub prefix: String,
    /// Optional suffix (e.g. "%")
    pub suffix: String,
}

impl Default for PriceFormatOptions {
    fn default() -> Self {
        PriceFormatOptions {
            precision: 2,
            prefix: String::new(),
            suffix: String::new(),
        }
    }
}

impl PriceFormatOptions {
    /// Format a price value according to this configuration.
    pub fn format(&self, price: f64) -> String {
        format!(
            "{}{:.prec$}{}",
            self.prefix,
            price,
            self.suffix,
            prec = self.precision as usize
        )
    }
}

/// Time label format
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TimeFormat {
    /// "Jan 02"
    MonthDay,
    /// "Jan 02 2024"
    MonthDayYear,
    /// "15:30"
    HourMinute,
    /// "Jan 02 15:30"
    Full,
}

impl Default for TimeFormat {
    fn default() -> Self {
        TimeFormat::MonthDay
    }
}

/// Grid line options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GridOptions {
    pub visible: bool,
    pub color: Color,
}

impl Default for GridOptions {
    fn default() -> Self {
        GridOptions {
            visible: true,
            color: ColorName::DarkSlate.color(),
        }
    }
}

/// Crosshair options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CrosshairOptions {
    pub visible: bool,
    pub color: Color,
    pub line_width: f32,
}

impl Default for CrosshairOptions {
    fn default() -> Self {
        CrosshairOptions {
            visible: true,
            color: ColorName::LightGray.color().with_alpha(0.8),
            line_width: 1.0,
        }
    }
}

/// Price scale (Y-axis) options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PriceScaleOptions {
    pub visible: bool,
    /// Auto-scale to visible data
    pub auto_scale: bool,
    /// Price format for labels
    pub format: PriceFormatOptions,
    /// Text color
    pub text_color: Color,
    /// Price scale mode: "normal" or "logarithmic"
    pub mode: String,
}

impl Default for PriceScaleOptions {
    fn default() -> Self {
        PriceScaleOptions {
            visible: true,
            auto_scale: true,
            format: PriceFormatOptions::default(),
            text_color: ColorName::SlateGray.color(),
            mode: "normal".to_string(),
        }
    }
}

/// Time scale (X-axis) options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeScaleOptions {
    pub visible: bool,
    pub time_format: TimeFormat,
    /// Text color
    pub text_color: Color,
    /// Minimum bar spacing (pixels)
    pub min_bar_spacing: f32,
    /// Maximum bar spacing (pixels)
    pub max_bar_spacing: f32,
    /// Right offset (bars of empty space at the right edge)
    pub right_offset: f32,
    /// Whether bar spacing is fixed (prevents zooming)
    pub fix_left_edge: bool,
    /// Whether to lock the visible range to the right edge
    pub lock_visible_time_range_on_resize: bool,
}

impl Default for TimeScaleOptions {
    fn default() -> Self {
        TimeScaleOptions {
            visible: true,
            time_format: TimeFormat::default(),
            text_color: ColorName::SlateGray.color(),
            min_bar_spacing: 1.0,
            max_bar_spacing: 30.0,
            right_offset: 0.0,
            fix_left_edge: false,
            lock_visible_time_range_on_resize: false,
        }
    }
}

/// Bar/candlestick color options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SeriesColors {
    pub bull_color: Color,
    pub bear_color: Color,
}

impl Default for SeriesColors {
    fn default() -> Self {
        SeriesColors {
            bull_color: ColorName::Teal.color(),
            bear_color: ColorName::Red.color(),
        }
    }
}

/// Layout options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutOptions {
    pub background_color: Color,
    pub text_color: Color,
    pub font_size: f32,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        LayoutOptions {
            background_color: ColorName::DarkerCharcoal.color(),
            text_color: ColorName::SlateGray.color(),
            font_size: 11.0,
        }
    }
}

/// Localization options — configures default date/time/price formatters.
///
/// Custom formatting callbacks (like LWC's `localization.priceFormatter`)
/// are registered via dedicated C-ABI functions rather than through this
/// serializable config, since closures can't be serialized.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalizationOptions {
    /// Locale identifier (e.g., "en-US", "de-DE")
    pub locale: String,
    /// Date format pattern for chrono (e.g., "%Y-%m-%d")
    pub date_format: String,
    /// Time format pattern for chrono (e.g., "%H:%M:%S")
    pub time_format: String,
}

impl Default for LocalizationOptions {
    fn default() -> Self {
        LocalizationOptions {
            locale: "en-US".to_string(),
            date_format: "%Y-%m-%d".to_string(),
            time_format: "%H:%M:%S".to_string(),
        }
    }
}

/// Top-level chart options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ChartOptions {
    pub layout: LayoutOptions,
    pub grid: GridOptions,
    pub crosshair: CrosshairOptions,
    pub price_scale: PriceScaleOptions,
    pub time_scale: TimeScaleOptions,
    pub series_colors: SeriesColors,
    pub localization: LocalizationOptions,
}
/// Merge a partial JSON object into a full JSON object recursively.
pub fn merge_json(target: &mut serde_json::Value, source: serde_json::Value) {
    match (target, source) {
        (&mut serde_json::Value::Object(ref mut t), serde_json::Value::Object(s)) => {
            for (k, v) in s {
                merge_json(t.entry(k).or_insert(serde_json::Value::Null), v);
            }
        }
        (t, s) => {
            *t = s;
        }
    }
}

impl ChartOptions {
    /// Applies a JSON string of partial options to the current options.
    pub fn apply_json(&mut self, json_str: &str) -> bool {
        if let Ok(partial) = serde_json::from_str::<serde_json::Value>(json_str) {
            if let Ok(mut full) = serde_json::to_value(&*self) {
                merge_json(&mut full, partial);
                if let Ok(new_opts) = serde_json::from_value(full) {
                    *self = new_opts;
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_format_default() {
        let fmt = PriceFormatOptions::default();
        assert_eq!(fmt.format(123.456), "123.46");
    }

    #[test]
    fn test_price_format_with_prefix() {
        let fmt = PriceFormatOptions {
            precision: 2,
            prefix: "$".to_string(),
            suffix: String::new(),
        };
        assert_eq!(fmt.format(1234.5), "$1234.50");
    }

    #[test]
    fn test_price_format_with_suffix() {
        let fmt = PriceFormatOptions {
            precision: 1,
            prefix: String::new(),
            suffix: "%".to_string(),
        };
        assert_eq!(fmt.format(42.789), "42.8%");
    }

    #[test]
    fn test_price_format_zero_precision() {
        let fmt = PriceFormatOptions {
            precision: 0,
            prefix: String::new(),
            suffix: String::new(),
        };
        assert_eq!(fmt.format(99.9), "100");
    }

    #[test]
    fn test_chart_options_default() {
        let opts = ChartOptions::default();
        assert!(opts.grid.visible);
        assert!(opts.crosshair.visible);
        assert!(opts.price_scale.visible);
        assert!(opts.time_scale.visible);
        assert!(opts.price_scale.auto_scale);
        assert_eq!(opts.price_scale.format.precision, 2);
    }
}
