/// Chart configuration options — mirrors LWC's ChartOptions / TimeChartOptions.
///
/// All colors are stored as `[r, g, b, a]` arrays (0.0..1.0).

/// Price formatting configuration
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone, Copy, PartialEq)]
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
#[derive(Debug, Clone)]
pub struct GridOptions {
    pub visible: bool,
    pub color: [f32; 4],
}

impl Default for GridOptions {
    fn default() -> Self {
        GridOptions {
            visible: true,
            color: [0.15, 0.15, 0.2, 1.0],
        }
    }
}

/// Crosshair options
#[derive(Debug, Clone)]
pub struct CrosshairOptions {
    pub visible: bool,
    pub color: [f32; 4],
    pub line_width: f32,
}

impl Default for CrosshairOptions {
    fn default() -> Self {
        CrosshairOptions {
            visible: true,
            color: [0.5, 0.5, 0.6, 0.8],
            line_width: 1.0,
        }
    }
}

/// Price scale (Y-axis) options
#[derive(Debug, Clone)]
pub struct PriceScaleOptions {
    pub visible: bool,
    /// Auto-scale to visible data
    pub auto_scale: bool,
    /// Price format for labels
    pub format: PriceFormatOptions,
    /// Text color
    pub text_color: [f32; 4],
}

impl Default for PriceScaleOptions {
    fn default() -> Self {
        PriceScaleOptions {
            visible: true,
            auto_scale: true,
            format: PriceFormatOptions::default(),
            text_color: [0.6, 0.6, 0.7, 1.0],
        }
    }
}

/// Time scale (X-axis) options
#[derive(Debug, Clone)]
pub struct TimeScaleOptions {
    pub visible: bool,
    pub time_format: TimeFormat,
    /// Text color
    pub text_color: [f32; 4],
    /// Minimum bar spacing (pixels)
    pub min_bar_spacing: f32,
    /// Maximum bar spacing (pixels)
    pub max_bar_spacing: f32,
}

impl Default for TimeScaleOptions {
    fn default() -> Self {
        TimeScaleOptions {
            visible: true,
            time_format: TimeFormat::default(),
            text_color: [0.6, 0.6, 0.7, 1.0],
            min_bar_spacing: 1.0,
            max_bar_spacing: 30.0,
        }
    }
}

/// Bar/candlestick color options
#[derive(Debug, Clone)]
pub struct SeriesColors {
    pub bull_color: [f32; 4],
    pub bear_color: [f32; 4],
}

impl Default for SeriesColors {
    fn default() -> Self {
        SeriesColors {
            bull_color: [0.15, 0.65, 0.60, 1.0], // Teal/green
            bear_color: [0.94, 0.33, 0.31, 1.0], // Red
        }
    }
}

/// Layout options
#[derive(Debug, Clone)]
pub struct LayoutOptions {
    pub background_color: [f32; 4],
    pub text_color: [f32; 4],
    pub font_size: f32,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        LayoutOptions {
            background_color: [0.05, 0.05, 0.07, 1.0],
            text_color: [0.6, 0.6, 0.7, 1.0],
            font_size: 11.0,
        }
    }
}

/// Top-level chart options
#[derive(Debug, Clone, Default)]
pub struct ChartOptions {
    pub layout: LayoutOptions,
    pub grid: GridOptions,
    pub crosshair: CrosshairOptions,
    pub price_scale: PriceScaleOptions,
    pub time_scale: TimeScaleOptions,
    pub series_colors: SeriesColors,
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
