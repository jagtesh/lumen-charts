use lumen_charts_core::chart_model::OhlcBar;
/// Parity tests from LWC: tests/unittests/get-series-data-creator.spec.ts
///                    and: tests/unittests/get-series-plot-row-creator.spec.ts
use lumen_charts_core::series::{Series, SeriesCollection, SeriesData, SeriesType};

fn sample_ohlc() -> Vec<OhlcBar> {
    vec![
        OhlcBar {
            time: 1000,
            open: 10.0,
            high: 15.0,
            low: 5.0,
            close: 11.0,
        },
        OhlcBar {
            time: 2000,
            open: 20.0,
            high: 25.0,
            low: 15.0,
            close: 21.0,
        },
    ]
}

/// LWC: getSeriesDataCreator — Line
/// File: get-series-data-creator.spec.ts
#[test]
fn lwc_series_data_creator_line() {
    let series = Series::line(0, vec![]);
    assert_eq!(series.series_type, SeriesType::Line);
}

/// LWC: getSeriesDataCreator — Bar (OHLC)
#[test]
fn lwc_series_data_creator_bar() {
    let series = Series::ohlc(0, sample_ohlc());
    assert_eq!(series.series_type, SeriesType::Ohlc);
}

/// LWC: getSeriesDataCreator — Candlestick
#[test]
fn lwc_series_data_creator_candlestick() {
    let series = Series::candlestick(0, sample_ohlc());
    assert_eq!(series.series_type, SeriesType::Candlestick);
}

/// LWC: getSeriesDataCreator — Area
#[test]
fn lwc_series_data_creator_area() {
    let series = Series::area(0, vec![]);
    assert_eq!(series.series_type, SeriesType::Area);
}

/// LWC: getSeriesDataCreator — Baseline
#[test]
fn lwc_series_data_creator_baseline() {
    let series = Series::baseline(0, vec![], 100.0);
    assert_eq!(series.series_type, SeriesType::Baseline);
    assert_eq!(series.baseline_options.base_value, 100.0);
}

/// LWC: getSeriesDataCreator — Histogram
#[test]
fn lwc_series_data_creator_histogram() {
    let series = Series::histogram(0, vec![]);
    assert_eq!(series.series_type, SeriesType::Histogram);
}

/// LWC: getSeriesPlotRowCreator — OHLC min/max
/// File: get-series-plot-row-creator.spec.ts
#[test]
fn lwc_series_plot_row_ohlc_min_max() {
    let data = SeriesData::Ohlc(sample_ohlc());
    let (min, max) = data.min_max().expect("should have min/max");
    assert_eq!(min, 5.0);
    assert_eq!(max, 25.0);
}

/// LWC: getSeriesPlotRowCreator — Line value_at
#[test]
fn lwc_series_plot_row_line_value() {
    let data = SeriesData::Ohlc(sample_ohlc());
    assert_eq!(data.value_at(0), Some(11.0));
    assert_eq!(data.value_at(1), Some(21.0));
}

/// LWC: SeriesCollection — add and remove
#[test]
fn lwc_series_collection_add_remove() {
    let mut coll = SeriesCollection::new();
    let id = coll.add(Series::line(0, vec![]));
    assert_eq!(coll.len(), 1);
    assert!(coll.remove(id));
    assert_eq!(coll.len(), 0);
}
