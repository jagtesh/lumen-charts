/// Chart renderer — draws all chart elements using the DrawBackend trait.
///
/// All functions are generic over `impl DrawBackend`, enabling multiple
/// rendering backends (Vello/WebGPU, Canvas 2D, WebGL/femtovg).
use crate::chart_state::ChartState;
use crate::draw_backend::{snap_x, snap_y, Color, DrawBackend, Palette};
use crate::overlays::{LineStyle, MarkerPosition, MarkerShape};
use crate::series::{SeriesData, SeriesType};
use crate::tick_marks::{generate_price_ticks, generate_time_ticks, TickMark};

const LABEL_FONT_SIZE: f64 = 11.0;

// Convenience constants resolved from the palette
const BG_COLOR: Color = Palette::Background.color();
const GRID_COLOR: Color = Palette::Grid.color();
const AXIS_COLOR: Color = Palette::Axis.color();
const BULL_COLOR: Color = Palette::Bull.color();
const BEAR_COLOR: Color = Palette::Bear.color();
const TEXT_COLOR: Color = Palette::Text.color();
const CROSSHAIR_COLOR: Color = Palette::Crosshair.color();
const WHITE: Color = Palette::White.color();

/// Render the entire chart from ChartState into a DrawBackend.
/// This is the legacy entry point — always renders everything.
pub fn render_chart(b: &mut impl DrawBackend, state: &ChartState) {
    render_bottom_scene(b, state);
    render_crosshair_scene(b, state);
}

/// Render the "bottom" scene: background, grid, series, axes, overlays.
/// This is the expensive part that should be cached when only the crosshair moves.
pub fn render_bottom_scene(b: &mut impl DrawBackend, state: &ChartState) {
    if state.data.bars.is_empty() {
        return;
    }

    let layout = &state.layout;
    let time_ticks = generate_time_ticks(&state.data.bars, &state.time_scale, &layout.plot_area);

    let sf = layout.scale_factor;
    b.set_scale(sf, sf);

    draw_background(b, layout);
    draw_grid(b, state, &time_ticks);

    // Watermark behind bars
    if state.overlays.watermark.visible {
        draw_watermark(b, state);
    }

    // Draw primary series (from state.data, using active_series_type) typically on pane 0
    match state.active_series_type {
        SeriesType::Ohlc => draw_ohlc_bars(b, 0, state),
        SeriesType::Candlestick => draw_candlestick_bars(b, 0, state),
        SeriesType::Line => draw_line_series_from_ohlc(b, 0, state),
        SeriesType::Area => {
            let points = ohlc_to_line_points(&state.data.bars);
            let opts = crate::series::AreaSeriesOptions::default();
            draw_area_series(b, 0, state, &points, &opts);
        }
        SeriesType::Baseline => {
            let points = ohlc_to_line_points(&state.data.bars);
            let opts = crate::series::BaselineSeriesOptions::default();
            draw_baseline_series(b, 0, state, &points, &opts);
        }
        SeriesType::Histogram => {
            let points: Vec<crate::series::HistogramDataPoint> = state
                .data
                .bars
                .iter()
                .map(|bar| crate::series::HistogramDataPoint {
                    time: bar.time,
                    value: bar.close,
                    color: None,
                })
                .collect();
            let opts = crate::series::HistogramSeriesOptions::default();
            draw_histogram_series(b, 0, state, &points, &opts);
        }
    }

    // Draw opaque backgrounds for additional panes
    for pane in &state.panes[1..] {
        let r = &pane.layout_rect;
        b.fill_rect(
            r.x as f64,
            r.y as f64,
            r.width as f64,
            r.height as f64,
            BG_COLOR,
        );
    }

    // Draw additional series from the collection
    for series in &state.series.series {
        if !series.visible {
            continue;
        }
        let p_idx = series.pane_index;
        match (&series.series_type, &series.data) {
            (SeriesType::Line, SeriesData::Line(pts)) => {
                draw_line_series(b, p_idx, state, pts, &series.line_options);
            }
            (SeriesType::Area, SeriesData::Line(pts)) => {
                draw_area_series(b, p_idx, state, pts, &series.area_options);
            }
            (SeriesType::Baseline, SeriesData::Line(pts)) => {
                draw_baseline_series(b, p_idx, state, pts, &series.baseline_options);
            }
            (SeriesType::Candlestick, SeriesData::Ohlc(bars)) => {
                draw_candlestick_bars_data(b, p_idx, state, bars, &series.candlestick_options);
            }
            (SeriesType::Histogram, SeriesData::Histogram(pts)) => {
                draw_histogram_series(b, p_idx, state, pts, &series.histogram_options);
            }
            _ => {}
        }
    }

    // Draw axis gutters (opaque backgrounds + grid labels) FIRST.
    // This clips series content that overflows into the gutter area.
    draw_y_axis(b, state, layout);
    draw_x_axis(b, &time_ticks, layout);

    // Overlays on top of axes — price line labels render ABOVE grid labels
    draw_price_lines(b, state);
    draw_series_markers(b, state);
    draw_last_value_marker(b, state);
}

/// Render only the crosshair layer. This is cheap — just 2 dashed lines + labels.
pub fn render_crosshair_scene(b: &mut impl DrawBackend, state: &ChartState) {
    if !state.crosshair.visible {
        return;
    }
    let sf = state.layout.scale_factor;
    b.set_scale(sf, sf);
    draw_crosshair(b, state);
}

fn draw_background(b: &mut impl DrawBackend, layout: &crate::chart_model::ChartLayout) {
    b.fill_rect(
        0.0,
        0.0,
        layout.width as f64,
        layout.height as f64,
        BG_COLOR,
    );
}

fn draw_grid(b: &mut impl DrawBackend, state: &ChartState, time_ticks: &[TickMark]) {
    let grid = &state.options.grid;
    if !grid.visible {
        return;
    }

    let grid_color = grid.color;
    let sf = state.layout.scale_factor;

    // Vertical grid lines at time tick positions
    for tick in time_ticks {
        let x = snap_x(tick.coord as f64, sf);
        let plot = &state.layout.plot_area;
        b.stroke_line(
            x,
            plot.y as f64,
            x,
            (plot.y + plot.height) as f64,
            grid_color,
            1.0,
        );
    }

    // Horizontal grid lines for all panes
    for pane in &state.panes {
        let price_ticks = generate_price_ticks(&pane.price_scale, &pane.layout_rect);
        for tick in &price_ticks {
            let y = snap_y(tick.coord as f64, sf);
            let r = &pane.layout_rect;
            b.stroke_line(r.x as f64, y, (r.x + r.width) as f64, y, grid_color, 1.0);
        }

        // Pane border
        let r = &pane.layout_rect;
        b.stroke_line(
            r.x as f64,
            r.y as f64,
            (r.x + r.width) as f64,
            r.y as f64,
            AXIS_COLOR,
            1.0,
        );
        b.stroke_line(
            r.x as f64,
            (r.y + r.height) as f64,
            (r.x + r.width) as f64,
            (r.y + r.height) as f64,
            AXIS_COLOR,
            1.0,
        );
        b.stroke_line(
            r.x as f64,
            r.y as f64,
            r.x as f64,
            (r.y + r.height) as f64,
            AXIS_COLOR,
            1.0,
        );
        b.stroke_line(
            (r.x + r.width) as f64,
            r.y as f64,
            (r.x + r.width) as f64,
            (r.y + r.height) as f64,
            AXIS_COLOR,
            1.0,
        );
    }
}

fn draw_ohlc_bars(b: &mut impl DrawBackend, pane_index: usize, state: &ChartState) {
    let pane = &state.panes[pane_index];
    let plot_area = &pane.layout_rect;
    let bar_width = state.time_scale.bar_spacing * 0.3;
    let line_width = 1.5;
    let sf = state.layout.scale_factor;

    let (first, last) = state.time_scale.visible_range(plot_area.width);
    let first = first.saturating_sub(1);
    let last = (last + 1).min(state.data.bars.len());

    for i in first..last {
        let bar = &state.data.bars[i];
        let x = snap_x(state.time_scale.index_to_x(i, plot_area) as f64, sf);

        if x < (plot_area.x - bar_width) as f64
            || x > (plot_area.x + plot_area.width + bar_width) as f64
        {
            continue;
        }

        let high_y = pane.price_scale.price_to_y(bar.high, plot_area) as f64;
        let low_y = pane.price_scale.price_to_y(bar.low, plot_area) as f64;
        let open_y = snap_y(pane.price_scale.price_to_y(bar.open, plot_area) as f64, sf);
        let close_y = snap_y(pane.price_scale.price_to_y(bar.close, plot_area) as f64, sf);

        let color = if bar.close >= bar.open {
            BULL_COLOR
        } else {
            BEAR_COLOR
        };

        // High-Low line (vertical wick — snap x only)
        b.stroke_line(x, high_y, x, low_y, color, line_width);
        // Open tick (horizontal — snap y)
        b.stroke_line(x - bar_width as f64, open_y, x, open_y, color, line_width);
        // Close tick (horizontal — snap y)
        b.stroke_line(x, close_y, x + bar_width as f64, close_y, color, line_width);
    }
}

fn draw_crosshair(b: &mut impl DrawBackend, state: &ChartState) {
    let plot = &state.layout.plot_area;
    let sf = state.layout.scale_factor;
    let x = snap_x(state.crosshair.x as f64, sf);
    let y = snap_y(state.crosshair.y as f64, sf);

    // Vertical dashed line
    b.stroke_dashed_line(
        x,
        plot.y as f64,
        x,
        (plot.y + plot.height) as f64,
        CROSSHAIR_COLOR,
        1.0,
        4.0,
        4.0,
    );

    // Horizontal dashed line
    b.stroke_dashed_line(
        plot.x as f64,
        y,
        (plot.x + plot.width) as f64,
        y,
        CROSSHAIR_COLOR,
        1.0,
        4.0,
        4.0,
    );

    // Price label on Y-axis
    if let Some(price) = state.crosshair.price {
        let label = format!("{:.2}", price);
        let label_x = (plot.x + plot.width + 2.0) as f64;
        let label_w = (state.layout.margins.right - 4.0) as f64;
        let label_h = 18.0;
        let label_y = y - label_h / 2.0;

        b.fill_rect(
            label_x,
            label_y,
            label_w,
            label_h,
            Palette::CrosshairLabelBg.color(),
        );
        b.draw_text(&label, label_x + 4.0, label_y + 13.0, 10.0, WHITE);
    }

    // OHLC info tooltip at top
    if let Some(idx) = state.crosshair.bar_index {
        if idx < state.data.bars.len() {
            let bar = &state.data.bars[idx];
            let info = format!(
                "O:{:.2}  H:{:.2}  L:{:.2}  C:{:.2}",
                bar.open, bar.high, bar.low, bar.close
            );

            let text_w = b.measure_text(&info, 10.0);
            let info_w = text_w + 16.0;
            let info_x = plot.x as f64 + 8.0;
            let info_y = plot.y as f64 + 4.0;
            b.fill_rect(
                info_x,
                info_y,
                info_w,
                20.0,
                Palette::CrosshairInfoBg.color(),
            );
            b.draw_text(&info, info_x + 8.0, info_y + 14.0, 10.0, TEXT_COLOR);
        }
    }
}

fn draw_y_axis(
    b: &mut impl DrawBackend,
    state: &ChartState,
    layout: &crate::chart_model::ChartLayout,
) {
    // Opaque background for the entire Y-axis gutter (clips series overflow)
    let gutter_x = (layout.plot_area.x + layout.plot_area.width) as f64;
    let gutter_w = layout.margins.right as f64;
    b.fill_rect(gutter_x, 0.0, gutter_w, layout.height as f64, BG_COLOR);

    let x_start = (layout.plot_area.x + layout.plot_area.width + 5.0) as f64;
    let sf = layout.scale_factor;

    for pane in &state.panes {
        let price_ticks = generate_price_ticks(&pane.price_scale, &pane.layout_rect);
        for tick in &price_ticks {
            let y = snap_y(tick.coord as f64, sf);

            // Tick mark
            let tick_x = (layout.plot_area.x + layout.plot_area.width) as f64;
            b.stroke_line(tick_x, y, tick_x + 4.0, y, AXIS_COLOR, 1.0);

            // Label
            b.draw_text(
                &format!("{:.2}", tick.value),
                x_start,
                y + 4.0,
                LABEL_FONT_SIZE,
                TEXT_COLOR,
            );
        }
    }
}

fn draw_x_axis(
    b: &mut impl DrawBackend,
    time_ticks: &[TickMark],
    layout: &crate::chart_model::ChartLayout,
) {
    let plot = &layout.plot_area;

    // Opaque background for the entire X-axis gutter (clips series overflow)
    let gutter_y = (plot.y + plot.height) as f64;
    let gutter_h = layout.margins.bottom as f64;
    b.fill_rect(0.0, gutter_y, layout.width as f64, gutter_h, BG_COLOR);

    let y_start = (plot.y + plot.height + 5.0) as f64;
    let sf = layout.scale_factor;

    for tick in time_ticks {
        let x = snap_x(tick.coord as f64, sf);

        // Tick mark
        b.stroke_line(
            x,
            (plot.y + plot.height) as f64,
            x,
            (plot.y + plot.height + 4.0) as f64,
            AXIS_COLOR,
            1.0,
        );

        // Center the label under the tick mark
        let label_w = b.measure_text(&tick.label, LABEL_FONT_SIZE);
        b.draw_text(
            &tick.label,
            x - label_w / 2.0,
            y_start + 12.0,
            LABEL_FONT_SIZE,
            TEXT_COLOR,
        );
    }
}

// ---------------------------------------------------------------------------
// Overlay rendering
// ---------------------------------------------------------------------------

fn draw_price_lines(b: &mut impl DrawBackend, state: &ChartState) {
    let pane = &state.panes[0];
    let plot = &pane.layout_rect;
    let sf = state.layout.scale_factor;

    for line in &state.overlays.price_lines {
        let y = pane.price_scale.price_to_y(line.price, plot);
        if y < plot.y || y > plot.y + plot.height {
            continue;
        }
        let y = snap_y(y as f64, sf);
        let color = line.color;

        match line.line_style {
            LineStyle::Dashed => {
                b.stroke_dashed_line(
                    plot.x as f64,
                    y,
                    (plot.x + plot.width) as f64,
                    y,
                    color,
                    line.line_width as f64,
                    6.0,
                    4.0,
                );
            }
            LineStyle::Dotted => {
                b.stroke_dashed_line(
                    plot.x as f64,
                    y,
                    (plot.x + plot.width) as f64,
                    y,
                    color,
                    line.line_width as f64,
                    2.0,
                    3.0,
                );
            }
            LineStyle::Solid => {
                b.stroke_line(
                    plot.x as f64,
                    y,
                    (plot.x + plot.width) as f64,
                    y,
                    color,
                    line.line_width as f64,
                );
            }
        }

        // Label on Y-axis
        if line.label_visible {
            let label_x = (plot.x + plot.width + 2.0) as f64;
            let label_w = b.measure_text(&line.label, LABEL_FONT_SIZE) + 8.0;
            let label_h = 16.0;
            let label_y = y - label_h / 2.0;

            b.fill_rect(label_x, label_y, label_w, label_h, color);
            b.draw_text(
                &line.label,
                label_x + 4.0,
                label_y + 12.0,
                LABEL_FONT_SIZE,
                WHITE,
            );
        }
    }
}

fn draw_series_markers(b: &mut impl DrawBackend, state: &ChartState) {
    let plot = &state.layout.plot_area;
    let price_scale = &state.panes[0].price_scale;

    for marker in &state.overlays.markers {
        let bar_idx = match state
            .data
            .bars
            .binary_search_by_key(&marker.time, |bar| bar.time)
        {
            Ok(i) => i,
            Err(_) => continue,
        };

        let x = state.time_scale.index_to_x(bar_idx, plot) as f64;
        if x < plot.x as f64 || x > (plot.x + plot.width) as f64 {
            continue;
        }

        let bar = &state.data.bars[bar_idx];
        let marker_size = marker.size as f64;
        let color = marker.color;

        let y = match marker.position {
            MarkerPosition::AboveBar => {
                price_scale.price_to_y(bar.high, plot) as f64 - marker_size - 4.0
            }
            MarkerPosition::BelowBar => {
                price_scale.price_to_y(bar.low, plot) as f64 + marker_size + 4.0
            }
            MarkerPosition::AtPrice => price_scale.price_to_y(bar.close, plot) as f64,
        };

        match marker.shape {
            MarkerShape::ArrowUp => {
                let pts = [
                    (x, y - marker_size),
                    (x - marker_size * 0.6, y + marker_size * 0.3),
                    (x + marker_size * 0.6, y + marker_size * 0.3),
                ];
                b.fill_path(&pts, color);
            }
            MarkerShape::ArrowDown => {
                let pts = [
                    (x, y + marker_size),
                    (x - marker_size * 0.6, y - marker_size * 0.3),
                    (x + marker_size * 0.6, y - marker_size * 0.3),
                ];
                b.fill_path(&pts, color);
            }
            MarkerShape::Circle => {
                b.fill_circle(x, y, marker_size * 0.5, color);
            }
            MarkerShape::Square => {
                let half = marker_size * 0.5;
                b.fill_rect(x - half, y - half, marker_size, marker_size, color);
            }
        }

        // Text label
        if !marker.text.is_empty() {
            let text_y = match marker.position {
                MarkerPosition::AboveBar => y - marker_size - 2.0,
                MarkerPosition::BelowBar | MarkerPosition::AtPrice => {
                    y + marker_size + LABEL_FONT_SIZE + 2.0
                }
            };
            let text_w = b.measure_text(&marker.text, LABEL_FONT_SIZE);
            b.draw_text(
                &marker.text,
                x - text_w / 2.0,
                text_y,
                LABEL_FONT_SIZE,
                color,
            );
        }
    }
}

fn draw_watermark(b: &mut impl DrawBackend, state: &ChartState) {
    let wm = &state.overlays.watermark;
    let plot = &state.layout.plot_area;
    let font_size = wm.font_size as f64;

    for (i, line) in wm.text.lines().enumerate() {
        let x = (plot.x + plot.width / 2.0) as f64;
        let y = (plot.y + plot.height / 2.0) as f64 + (i as f64 * font_size * 1.2);
        let text_w = b.measure_text(line, font_size);
        b.draw_text(line, x - text_w / 2.0, y, font_size, wm.color);
    }
}

fn draw_last_value_marker(b: &mut impl DrawBackend, state: &ChartState) {
    let plot = &state.layout.plot_area;
    let pane = &state.panes[0];

    if let Some(last_bar) = state.data.bars.last() {
        let price = last_bar.close;
        let y = pane.price_scale.price_to_y(price, plot) as f64;

        if y < plot.y as f64 || y > (plot.y + plot.height) as f64 {
            return;
        }

        let color = if last_bar.close >= last_bar.open {
            BULL_COLOR
        } else {
            BEAR_COLOR
        };

        // Background rectangle on the price axis
        let label = format!("{:.2}", price);
        let label_w = b.measure_text(&label, LABEL_FONT_SIZE) + 12.0;
        let label_h = LABEL_FONT_SIZE + 6.0;
        let label_x = (plot.x + plot.width) as f64 + 2.0;
        let label_y = y - label_h / 2.0;

        b.fill_rect(label_x, label_y, label_w, label_h, color);
        b.draw_text(
            &label,
            label_x + 6.0,
            label_y + LABEL_FONT_SIZE,
            LABEL_FONT_SIZE,
            WHITE,
        );

        // Dashed line across the chart
        b.stroke_dashed_line(
            plot.x as f64,
            y,
            (plot.x + plot.width) as f64,
            y,
            color,
            1.0,
            4.0,
            3.0,
        );
    }
}

// ---------------------------------------------------------------------------
// Candlestick renderer
// ---------------------------------------------------------------------------

fn draw_candlestick_bars(b: &mut impl DrawBackend, pane_index: usize, state: &ChartState) {
    draw_candlestick_bars_data(
        b,
        pane_index,
        state,
        &state.data.bars,
        &crate::series::CandlestickOptions::default(),
    );
}

/// Draw candlestick bars from arbitrary bar data (for multi-series support)
fn draw_candlestick_bars_data(
    b: &mut impl DrawBackend,
    pane_index: usize,
    state: &ChartState,
    bars: &[crate::chart_model::OhlcBar],
    opts: &crate::series::CandlestickOptions,
) {
    let pane = &state.panes[pane_index];
    let plot_area = &pane.layout_rect;
    let bar_width = (state.time_scale.bar_spacing * 0.7).max(1.0);

    let (first, last) = state.time_scale.visible_range(plot_area.width);
    let first = first.saturating_sub(1);
    let last = (last + 1).min(bars.len());

    for i in first..last {
        let bar = &bars[i];
        let x = state.time_scale.index_to_x(i, plot_area) as f64;

        if x < (plot_area.x - bar_width) as f64
            || x > (plot_area.x + plot_area.width + bar_width) as f64
        {
            continue;
        }

        let open_y = pane.price_scale.price_to_y(bar.open, plot_area) as f64;
        let close_y = pane.price_scale.price_to_y(bar.close, plot_area) as f64;
        let high_y = pane.price_scale.price_to_y(bar.high, plot_area) as f64;
        let low_y = pane.price_scale.price_to_y(bar.low, plot_area) as f64;

        let is_bull = bar.close >= bar.open;
        let body_top = open_y.min(close_y);
        let body_bottom = open_y.max(close_y);
        let body_height = (body_bottom - body_top).max(1.0);
        let half_w = bar_width as f64 / 2.0;

        let body_color = if is_bull {
            opts.up_color
        } else {
            opts.down_color
        };
        let wick_color = if is_bull {
            opts.wick_up_color
        } else {
            opts.wick_down_color
        };

        // Wick
        b.stroke_line(x, high_y, x, low_y, wick_color, 1.0);

        // Body
        if opts.hollow && is_bull {
            b.stroke_line(x - half_w, body_top, x + half_w, body_top, body_color, 1.0);
            b.stroke_line(
                x - half_w,
                body_bottom,
                x + half_w,
                body_bottom,
                body_color,
                1.0,
            );
            b.stroke_line(
                x - half_w,
                body_top,
                x - half_w,
                body_bottom,
                body_color,
                1.0,
            );
            b.stroke_line(
                x + half_w,
                body_top,
                x + half_w,
                body_bottom,
                body_color,
                1.0,
            );
        } else {
            b.fill_rect(
                x - half_w,
                body_top,
                bar_width as f64,
                body_height,
                body_color,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Line series renderers
// ---------------------------------------------------------------------------

/// Draw a line series from OHLC close prices (for primary series)
fn draw_line_series_from_ohlc(b: &mut impl DrawBackend, pane_index: usize, state: &ChartState) {
    let pane = &state.panes[pane_index];
    let plot_area = &pane.layout_rect;

    let (first, last) = state.time_scale.visible_range(plot_area.width);
    let first = first.saturating_sub(1);
    let last = (last + 1).min(state.data.bars.len());

    if first >= last {
        return;
    }

    let mut points: Vec<(f64, f64)> = Vec::with_capacity(last - first);
    for i in first..last {
        let bar = &state.data.bars[i];
        let x = state.time_scale.index_to_x(i, plot_area) as f64;
        let y = pane.price_scale.price_to_y(bar.close, plot_area) as f64;
        points.push((x, y));
    }

    b.stroke_path(&points, BULL_COLOR, 2.0);
}

/// Draw a line series from LineDataPoint data
fn draw_line_series(
    b: &mut impl DrawBackend,
    pane_index: usize,
    state: &ChartState,
    line_points: &[crate::series::LineDataPoint],
    opts: &crate::series::LineSeriesOptions,
) {
    let pane = &state.panes[pane_index];
    let plot_area = &pane.layout_rect;

    let (first, last) = state.time_scale.visible_range(plot_area.width);
    let first = first.saturating_sub(1);
    let last = (last + 1).min(line_points.len());

    if first >= last {
        return;
    }

    let mut points: Vec<(f64, f64)> = Vec::with_capacity(last - first);
    for i in first..last {
        let pt = &line_points[i];
        let x = state.time_scale.index_to_x(i, plot_area) as f64;
        let y = pane.price_scale.price_to_y(pt.value, plot_area) as f64;
        points.push((x, y));
    }

    let color = opts.color;
    let width = opts.line_width as f64;
    b.stroke_path(&points, color, width);

    // Draw circles at each data point if few enough
    if opts.point_markers_visible && points.len() < 200 {
        let radius = opts.point_markers_radius as f64;
        for &(px, py) in &points {
            b.fill_circle(px, py, radius, color);
        }
    }
}

/// Helper to convert OHLC to LineDataPoints
pub fn ohlc_to_line_points(
    bars: &[crate::chart_model::OhlcBar],
) -> Vec<crate::series::LineDataPoint> {
    bars.iter()
        .map(|b| crate::series::LineDataPoint {
            time: b.time,
            value: b.close,
        })
        .collect()
}

/// Draw an area series (filled gradient below a line)
fn draw_area_series(
    b: &mut impl DrawBackend,
    pane_index: usize,
    state: &ChartState,
    line_points: &[crate::series::LineDataPoint],
    opts: &crate::series::AreaSeriesOptions,
) {
    let pane = &state.panes[pane_index];
    let plot_area = &pane.layout_rect;

    let (first, last) = state.time_scale.visible_range(plot_area.width);
    let first = first.saturating_sub(1);
    let last = (last + 1).min(line_points.len());

    if first >= last {
        return;
    }

    // 1. Build line points
    let mut line_pts: Vec<(f64, f64)> = Vec::with_capacity(last - first);
    for i in first..last {
        let pt = &line_points[i];
        let x = state.time_scale.index_to_x(i, plot_area) as f64;
        let y = pane.price_scale.price_to_y(pt.value, plot_area) as f64;
        line_pts.push((x, y));
    }

    // 2. Build the fill polygon (line + bottom edge)
    let bottom_y = (plot_area.y + plot_area.height) as f64;
    let mut fill_pts = line_pts.clone();
    if let Some(&(last_x, _)) = fill_pts.last() {
        fill_pts.push((last_x, bottom_y));
    }
    if let Some(&(first_x, _)) = line_pts.first() {
        fill_pts.push((first_x, bottom_y));
    }

    let lowest_y = line_pts
        .iter()
        .map(|(_, y)| *y)
        .fold(f64::INFINITY, f64::min);

    // 3. Fill with gradient
    b.fill_path_gradient(
        &fill_pts,
        lowest_y,
        bottom_y,
        &[(opts.top_color, 0.0), (opts.bottom_color, 1.0)],
    );

    // 4. Stroke the line on top
    b.stroke_path(&line_pts, opts.line_color, opts.line_width as f64);
}

/// Draw a baseline series (filled areas above and below a baseline)
fn draw_baseline_series(
    b: &mut impl DrawBackend,
    pane_index: usize,
    state: &ChartState,
    line_points: &[crate::series::LineDataPoint],
    opts: &crate::series::BaselineSeriesOptions,
) {
    let pane = &state.panes[pane_index];
    let plot_area = &pane.layout_rect;

    let (first, last) = state.time_scale.visible_range(plot_area.width);
    let first = first.saturating_sub(1);
    let last = (last + 1).min(line_points.len());

    if first >= last {
        return;
    }

    let base_y = pane.price_scale.price_to_y(opts.base_value, plot_area) as f64;

    // Build line points
    let mut line_pts: Vec<(f64, f64)> = Vec::with_capacity(last - first);
    for i in first..last {
        let pt = &line_points[i];
        let x = state.time_scale.index_to_x(i, plot_area) as f64;
        let y = pane.price_scale.price_to_y(pt.value, plot_area) as f64;
        line_pts.push((x, y));
    }

    // Top fill (above baseline → clamped)
    let mut top_fill: Vec<(f64, f64)> = Vec::new();
    let mut bottom_fill: Vec<(f64, f64)> = Vec::new();

    for &(x, y) in &line_pts {
        top_fill.push((x, y.min(base_y)));
        bottom_fill.push((x, y.max(base_y)));
    }

    // Close top fill polygon
    if !top_fill.is_empty() {
        let mut pts = top_fill.clone();
        if let Some(&(lx, _)) = pts.last() {
            pts.push((lx, base_y));
        }
        if let Some(&(fx, _)) = pts.first() {
            pts.push((fx, base_y));
        }
        let min_y = pts.iter().map(|(_, y)| *y).fold(f64::INFINITY, f64::min);
        b.fill_path_gradient(
            &pts,
            min_y,
            base_y,
            &[
                (opts.top_fill_color, 0.0),
                (
                    Color([
                        opts.top_fill_color[0],
                        opts.top_fill_color[1],
                        opts.top_fill_color[2],
                        0.0,
                    ]),
                    1.0,
                ),
            ],
        );
    }

    // Close bottom fill polygon
    if !bottom_fill.is_empty() {
        let mut pts = Vec::new();
        if let Some(&(fx, _)) = bottom_fill.first() {
            pts.push((fx, base_y));
        }
        pts.extend_from_slice(&bottom_fill);
        if let Some(&(lx, _)) = bottom_fill.last() {
            pts.push((lx, base_y));
        }
        let max_y = pts
            .iter()
            .map(|(_, y)| *y)
            .fold(f64::NEG_INFINITY, f64::max);
        b.fill_path_gradient(
            &pts,
            base_y,
            max_y,
            &[
                (opts.bottom_fill_color, 0.0),
                (
                    Color([
                        opts.bottom_fill_color[0],
                        opts.bottom_fill_color[1],
                        opts.bottom_fill_color[2],
                        0.0,
                    ]),
                    1.0,
                ),
            ],
        );
    }

    // Stroke line segments with appropriate colors
    for i in 0..line_pts.len().saturating_sub(1) {
        let (x0, y0) = line_pts[i];
        let (x1, y1) = line_pts[i + 1];
        let mid_y = (y0 + y1) / 2.0;
        let color = if mid_y <= base_y {
            opts.top_line_color
        } else {
            opts.bottom_line_color
        };
        b.stroke_line(x0, y0, x1, y1, color, opts.line_width as f64);
    }
}

// ---------------------------------------------------------------------------
// Histogram series renderer
// ---------------------------------------------------------------------------

/// Draw a histogram series — vertical bars from base to value
fn draw_histogram_series(
    b: &mut impl DrawBackend,
    pane_index: usize,
    state: &ChartState,
    points: &[crate::series::HistogramDataPoint],
    opts: &crate::series::HistogramSeriesOptions,
) {
    let pane = &state.panes[pane_index];
    let plot_area = &pane.layout_rect;
    let bar_width = (state.time_scale.bar_spacing * 0.6).max(1.0);

    let base_y = pane.price_scale.price_to_y(opts.base, plot_area) as f64;

    let (first, last) = state.time_scale.visible_range(plot_area.width);
    let first = first.saturating_sub(1);
    let last = (last + 1).min(points.len());

    for i in first..last {
        let pt = &points[i];
        let x = state.time_scale.index_to_x(i, plot_area) as f64;
        let val_y = pane.price_scale.price_to_y(pt.value, plot_area) as f64;

        if x < plot_area.x as f64 || x > (plot_area.x + plot_area.width) as f64 {
            continue;
        }

        let color = if let Some(c) = pt.color {
            c
        } else {
            opts.color
        };

        let half_w = bar_width as f64 / 2.0;
        let top = val_y.min(base_y);
        let height = (val_y - base_y).abs().max(1.0);
        b.fill_rect(x - half_w, top, bar_width as f64, height, color);
    }
}
