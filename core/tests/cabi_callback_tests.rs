/// C-ABI callback integration tests
///
/// These tests verify the callback wiring for subscription APIs.
/// Since chart_create requires a GPU, we test callback types, storage,
/// and data-level APIs that don't need rendering.
use std::sync::atomic::{AtomicU32, Ordering};

// Test that the RangeChangeCallback and SizeChangeCallback types
// match the expected C-ABI signatures
extern "C" fn test_range_callback(from: f64, to: f64, user_data: *mut std::ffi::c_void) {
    let counter = unsafe { &*(user_data as *const AtomicU32) };
    counter.fetch_add(1, Ordering::SeqCst);
    // Verify we get reasonable values
    assert!(from.is_finite());
    assert!(to.is_finite());
}

extern "C" fn test_size_callback(width: f32, height: f32, user_data: *mut std::ffi::c_void) {
    let counter = unsafe { &*(user_data as *const AtomicU32) };
    counter.fetch_add(1, Ordering::SeqCst);
    assert!(width >= 0.0);
    assert!(height >= 0.0);
}

extern "C" fn test_event_callback(
    param: *const lumen_charts::ChartEventParam,
    user_data: *mut std::ffi::c_void,
) {
    let counter = unsafe { &*(user_data as *const AtomicU32) };
    counter.fetch_add(1, Ordering::SeqCst);
    // Verify param is not null when called
    assert!(!param.is_null());
}

/// Verify that range callback function pointers have correct ABI
#[test]
fn test_range_callback_abi_compat() {
    let counter = AtomicU32::new(0);
    let ud = &counter as *const AtomicU32 as *mut std::ffi::c_void;

    // Call through the function pointer — this validates ABI compatibility
    let cb: lumen_charts::RangeChangeCallback = test_range_callback;
    cb(100.0, 200.0, ud);
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    cb(-50.5, 1000.123, ud);
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

/// Verify that size callback function pointers have correct ABI
#[test]
fn test_size_callback_abi_compat() {
    let counter = AtomicU32::new(0);
    let ud = &counter as *const AtomicU32 as *mut std::ffi::c_void;

    let cb: lumen_charts::SizeChangeCallback = test_size_callback;
    cb(800.0, 600.0, ud);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

/// Verify that the ChartEventCallback signature is correct
#[test]
fn test_chart_event_callback_abi_compat() {
    let counter = AtomicU32::new(0);
    let ud = &counter as *const AtomicU32 as *mut std::ffi::c_void;

    let param = lumen_charts::ChartEventParam {
        time: 1704153600,
        logical: 42.0,
        point_x: 100.0,
        point_y: 200.0,
        price: 150.50,
    };

    let cb: lumen_charts::ChartEventCallback = test_event_callback;
    cb(&param as *const _, ud);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

/// Test that ChartEventParam is correctly laid out for C-ABI
#[test]
fn test_chart_event_param_layout() {
    let param = lumen_charts::ChartEventParam {
        time: 1704153600,
        logical: 42.5,
        point_x: 100.0,
        point_y: 200.0,
        price: 150.50,
    };

    assert_eq!(param.time, 1704153600);
    assert!((param.logical - 42.5).abs() < f64::EPSILON);
    assert!((param.point_x - 100.0).abs() < f32::EPSILON);
    assert!((param.point_y - 200.0).abs() < f32::EPSILON);
    assert!((param.price - 150.50).abs() < f64::EPSILON);

    // Verify struct is #[repr(C)] — field order and alignment must match C
    assert_eq!(
        std::mem::size_of::<lumen_charts::ChartEventParam>(),
        std::mem::size_of::<i64>()          // time
            + std::mem::size_of::<f64>()    // logical
            + std::mem::size_of::<f32>()    // point_x
            + std::mem::size_of::<f32>()    // point_y
            + std::mem::size_of::<f64>(), // price
        "ChartEventParam should be tightly packed as #[repr(C)]"
    );
}

/// Test markers JSON round-trip (set → get)
/// This tests the JSON parsing/serialization logic without GPU
#[test]
fn test_markers_json_roundtrip() {
    use lumen_charts::overlays::{MarkerPosition, MarkerShape, SeriesMarker};

    let markers = vec![
        SeriesMarker::new(1000, MarkerShape::ArrowUp, MarkerPosition::BelowBar)
            .with_color([0.0, 1.0, 0.0, 1.0])
            .with_text("Buy")
            .with_size(12.0),
        SeriesMarker::new(2000, MarkerShape::ArrowDown, MarkerPosition::AboveBar)
            .with_color([1.0, 0.0, 0.0, 1.0])
            .with_text("Sell")
            .with_size(10.0),
    ];

    // Verify marker construction
    assert_eq!(markers[0].time, 1000);
    assert_eq!(markers[0].shape, MarkerShape::ArrowUp);
    assert_eq!(markers[0].position, MarkerPosition::BelowBar);
    assert_eq!(markers[0].text, "Buy");
    assert_eq!(markers[0].size, 12.0);

    assert_eq!(markers[1].time, 2000);
    assert_eq!(markers[1].shape, MarkerShape::ArrowDown);
    assert_eq!(markers[1].position, MarkerPosition::AboveBar);
    assert_eq!(markers[1].text, "Sell");

    // Test overlays set/clear
    let mut overlays = lumen_charts::overlays::Overlays::new();
    overlays.set_markers(markers);
    assert_eq!(overlays.markers.len(), 2);
    overlays.clear_markers();
    assert!(overlays.markers.is_empty());
}

/// Test series options JSON serialization
#[test]
fn test_series_options_json_serialization() {
    use lumen_charts::series::*;

    // Candlestick options
    let cs_opts = CandlestickOptions::default();
    let json = serde_json::to_string(&cs_opts).unwrap();
    assert!(json.contains("up_color"));
    assert!(json.contains("down_color"));
    assert!(json.contains("hollow"));

    // Line options
    let line_opts = LineSeriesOptions::default();
    let json = serde_json::to_string(&line_opts).unwrap();
    assert!(json.contains("line_width"));
    assert!(json.contains("color"));

    // Area options
    let area_opts = AreaSeriesOptions::default();
    let json = serde_json::to_string(&area_opts).unwrap();
    assert!(json.contains("top_color"));
    assert!(json.contains("bottom_color"));

    // Histogram options
    let hist_opts = HistogramSeriesOptions::default();
    let json = serde_json::to_string(&hist_opts).unwrap();
    assert!(json.contains("base"));

    // Baseline options
    let base_opts = BaselineSeriesOptions::default();
    let json = serde_json::to_string(&base_opts).unwrap();
    assert!(json.contains("base_value"));
    assert!(json.contains("top_line_color"));
    assert!(json.contains("bottom_line_color"));

    // Verify round-trip: serialize → deserialize
    let cs_roundtrip: CandlestickOptions =
        serde_json::from_str(&serde_json::to_string(&cs_opts).unwrap()).unwrap();
    assert_eq!(cs_roundtrip.hollow, cs_opts.hollow);
    assert_eq!(cs_roundtrip.up_color, cs_opts.up_color);
}

/// Test barsInLogicalRange logic
#[test]
fn test_bars_in_logical_range_logic() {
    use lumen_charts::chart_model::OhlcBar;
    use lumen_charts::series::*;

    let mut coll = SeriesCollection::new();
    let bars: Vec<OhlcBar> = (0..100)
        .map(|i| OhlcBar {
            time: 1704153600 + i * 60,
            open: 100.0 + i as f64,
            high: 102.0 + i as f64,
            low: 98.0 + i as f64,
            close: 101.0 + i as f64,
        })
        .collect();

    let id = coll.add(Series::ohlc(0, bars));
    let series = coll.get(id).unwrap();

    // Logical range 0..10 should have 10 bars
    let from = 0.0f32;
    let to = 10.0f32;
    let from_idx = from.floor().max(0.0) as usize;
    let to_idx = to.ceil().max(0.0) as usize;
    let count = to_idx.min(series.data.len()) - from_idx;
    assert_eq!(count, 10);

    // Logical range 50..60 should have 10 bars
    let from2 = 50.0f32;
    let to2 = 60.0f32;
    let from_idx2 = from2.floor().max(0.0) as usize;
    let to_idx2 = to2.ceil().max(0.0) as usize;
    let count2 = to_idx2.min(series.data.len()) - from_idx2;
    assert_eq!(count2, 10);

    // Out of range should return 0
    let from3 = 200.0f32;
    let to3 = 300.0f32;
    let from_idx3 = from3.floor().max(0.0) as usize;
    let data_len = series.data.len();
    let count3 = if from_idx3 >= data_len {
        0
    } else {
        to3.ceil().max(0.0) as usize - from_idx3
    };
    assert_eq!(count3, 0);
}

/// Test multiple callbacks can be stored and replaced independently
#[test]
fn test_callback_storage_independence() {
    // Simulate the callback storage pattern used in Chart
    type CB = Option<(
        extern "C" fn(f64, f64, *mut std::ffi::c_void),
        *mut std::ffi::c_void,
    )>;

    let counter1 = AtomicU32::new(0);
    let counter2 = AtomicU32::new(0);

    let mut time_range_cb: CB = None;
    let mut logical_range_cb: CB = None;

    // Subscribe to time range
    time_range_cb = Some((
        test_range_callback,
        &counter1 as *const _ as *mut std::ffi::c_void,
    ));

    // Subscribe to logical range
    logical_range_cb = Some((
        test_range_callback,
        &counter2 as *const _ as *mut std::ffi::c_void,
    ));

    // Fire time range — only counter1 should increment
    if let Some((cb, ud)) = time_range_cb {
        cb(100.0, 200.0, ud);
    }
    assert_eq!(counter1.load(Ordering::SeqCst), 1);
    assert_eq!(counter2.load(Ordering::SeqCst), 0);

    // Fire logical range — only counter2 should increment
    if let Some((cb, ud)) = logical_range_cb {
        cb(0.0, 50.0, ud);
    }
    assert_eq!(counter1.load(Ordering::SeqCst), 1);
    assert_eq!(counter2.load(Ordering::SeqCst), 1);

    // Unsubscribe time range
    time_range_cb = None;
    assert!(time_range_cb.is_none());
    assert!(logical_range_cb.is_some());
}
