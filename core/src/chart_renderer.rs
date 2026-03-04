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

    // Watermark behind bars (global, on pane 0)
    if state.overlays.watermark.visible {
        draw_watermark(b, state);
    }

    // ── Per-pane rendering: each pane is a clipped virtual surface ──
    for (pane_idx, pane) in state.panes.iter().enumerate() {
        let r = &pane.layout_rect;

        // Clip to this pane's bounds
        b.clip_rect(r.x as f64, r.y as f64, r.width as f64, r.height as f64);

        // Opaque background for non-primary panes (pane 0 uses the global background)
        if pane_idx > 0 {
            b.fill_rect(
                r.x as f64,
                r.y as f64,
                r.width as f64,
                r.height as f64,
                BG_COLOR,
            );
        }

        // Draw primary series on pane 0
        if pane_idx == 0 {
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
        }

        // Draw additional series assigned to this pane
        for series in &state.series.series {
            if !series.visible || series.pane_index != pane_idx {
                continue;
            }
            match (&series.series_type, &series.data) {
                (SeriesType::Line, SeriesData::Line(pts)) => {
                    draw_line_series(b, pane_idx, state, pts, &series.line_options);
                }
                (SeriesType::Area, SeriesData::Line(pts)) => {
                    draw_area_series(b, pane_idx, state, pts, &series.area_options);
                }
                (SeriesType::Baseline, SeriesData::Line(pts)) => {
                    draw_baseline_series(b, pane_idx, state, pts, &series.baseline_options);
                }
                (SeriesType::Candlestick, SeriesData::Ohlc(bars)) => {
                    draw_candlestick_bars_data(
                        b,
                        pane_idx,
                        state,
                        bars,
                        &series.candlestick_options,
                    );
                }
                (SeriesType::Histogram, SeriesData::Histogram(pts)) => {
                    draw_histogram_series(b, pane_idx, state, pts, &series.histogram_options);
                }
                _ => {}
            }
        }

        // Per-pane overlays (price lines and last value marker for this pane)
        if pane_idx == 0 {
            draw_price_lines(b, state, pane_idx);
            draw_series_markers(b, state);
            draw_last_value_marker(b, state, pane_idx);
        }

        b.restore_clip();
    }

    // ── Post-clip: gutters and borders on top of all panes ──
    draw_y_axis(b, state, layout);
    draw_x_axis(b, &time_ticks, layout);

    // Price line labels render in the gutter (ABOVE grid labels, outside clip)
    draw_price_line_labels(b, state);
    draw_last_value_label(b, state);
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

        // Pane border — extends full width (no left border)
        let r = &pane.layout_rect;
        let right_edge = state.layout.width as f64;
        // Top border (from left surface edge to right edge)
        b.stroke_line(0.0, r.y as f64, right_edge, r.y as f64, AXIS_COLOR, 1.0);
        // Bottom border
        b.stroke_line(
            0.0,
            (r.y + r.height) as f64,
            right_edge,
            (r.y + r.height) as f64,
            AXIS_COLOR,
            1.0,
        );
        // Right border (at right edge of Y-axis gutter)
        b.stroke_line(
            right_edge,
            r.y as f64,
            right_edge,
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

    // Horizontal dashed line — only within the active pane
    let active_rect = &state.panes[state.active_pane].layout_rect;
    b.stroke_dashed_line(
        active_rect.x as f64,
        y,
        (active_rect.x + active_rect.width) as f64,
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

            // Label (no tick dash — cleaner, especially for negative numbers)
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

        // Center the label under the tick mark (no dash — cleaner)
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

/// Draw price line indicators (horizontal lines) within a specific pane.
/// Called inside clip_rect — lines are clipped to the pane's bounds.
fn draw_price_lines(b: &mut impl DrawBackend, state: &ChartState, pane_idx: usize) {
    let pane = &state.panes[pane_idx];
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
            LineStyle::LargeDashed => {
                b.stroke_dashed_line(
                    plot.x as f64,
                    y,
                    (plot.x + plot.width) as f64,
                    y,
                    color,
                    line.line_width as f64,
                    6.0,
                    6.0,
                );
            }
            LineStyle::SparseDotted => {
                b.stroke_dashed_line(
                    plot.x as f64,
                    y,
                    (plot.x + plot.width) as f64,
                    y,
                    color,
                    line.line_width as f64,
                    1.0,
                    4.0,
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
    }
}

/// Draw price line labels in the Y-axis gutter (outside pane clip).
fn draw_price_line_labels(b: &mut impl DrawBackend, state: &ChartState) {
    let pane = &state.panes[0];
    let plot = &pane.layout_rect;
    let sf = state.layout.scale_factor;

    for line in &state.overlays.price_lines {
        if !line.label_visible {
            continue;
        }
        let y = pane.price_scale.price_to_y(line.price, plot);
        if y < plot.y || y > plot.y + plot.height {
            continue;
        }
        let y = snap_y(y as f64, sf);
        let color = line.color;

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

/// Draw last value dashed line within the specified pane (inside clip).
fn draw_last_value_marker(b: &mut impl DrawBackend, state: &ChartState, pane_idx: usize) {
    let pane = &state.panes[pane_idx];
    let plot = &pane.layout_rect;

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

        // Dashed line across the chart (clipped to pane)
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

/// Draw last value label in the Y-axis gutter (outside pane clip).
fn draw_last_value_label(b: &mut impl DrawBackend, state: &ChartState) {
    let pane = &state.panes[0];
    let plot = &pane.layout_rect;

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

        // Background rectangle on the price axis (gutter)
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

    let (vis_first, vis_last) = state.time_scale.visible_range(plot_area.width);

    for bar in bars {
        let bar_idx = match state.time_index_map.get(&bar.time) {
            Some(&idx) => idx,
            None => continue,
        };
        if bar_idx + 1 < vis_first || bar_idx > vis_last + 1 {
            continue;
        }
        let x = state.time_scale.index_to_x(bar_idx, plot_area) as f64;

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

    let (vis_first, vis_last) = state.time_scale.visible_range(plot_area.width);

    // Collect points with their bar index for gap detection
    let mut indexed_points: Vec<(usize, f64, f64)> = Vec::with_capacity(line_points.len());
    for pt in line_points {
        let bar_idx = match state.time_index_map.get(&pt.time) {
            Some(&idx) => idx,
            None => continue,
        };
        if bar_idx + 1 < vis_first || bar_idx > vis_last + 1 {
            continue;
        }
        let x = state.time_scale.index_to_x(bar_idx, plot_area) as f64;
        let y = pane.price_scale.price_to_y(pt.value, plot_area) as f64;
        indexed_points.push((bar_idx, x, y));
    }

    let color = opts.color;
    let width = opts.line_width as f64;
    let line_type = opts.line_type;

    // Break the line into segments at gaps (non-adjacent bar indices).
    // A gap is when the bar index jumps by more than 1 between consecutive points.
    let mut segment: Vec<(f64, f64)> = Vec::new();
    let mut prev_idx: Option<usize> = None;

    for &(bar_idx, x, y) in &indexed_points {
        if let Some(prev) = prev_idx {
            if bar_idx > prev + 1 {
                // Gap detected — flush current segment
                flush_line_segment(b, &segment, color, width, line_type);
                segment.clear();
            }
        }
        segment.push((x, y));
        prev_idx = Some(bar_idx);
    }
    // Flush final segment
    flush_line_segment(b, &segment, color, width, line_type);

    // Draw circles at each data point if few enough
    if opts.point_markers_visible && indexed_points.len() < 200 {
        let radius = opts.point_markers_radius as f64;
        for &(_, px, py) in &indexed_points {
            b.fill_circle(px, py, radius, color);
        }
    }
}

/// Flush a line segment with the appropriate interpolation (LineType).
fn flush_line_segment(
    b: &mut impl DrawBackend,
    segment: &[(f64, f64)],
    color: Color,
    width: f64,
    line_type: crate::series::LineType,
) {
    if segment.len() < 2 {
        return;
    }
    match line_type {
        crate::series::LineType::Simple => {
            b.stroke_path(segment, color, width);
        }
        crate::series::LineType::WithSteps => {
            // Staircase: horizontal then vertical between each pair
            let mut stepped: Vec<(f64, f64)> = Vec::with_capacity(segment.len() * 2);
            stepped.push(segment[0]);
            for i in 1..segment.len() {
                // Horizontal line to x of next point at current y
                stepped.push((segment[i].0, segment[i - 1].1));
                // Vertical line to next point
                stepped.push(segment[i]);
            }
            b.stroke_path(&stepped, color, width);
        }
        crate::series::LineType::Curved => {
            // Catmull-Rom spline approximation: generate intermediate points
            let mut curved: Vec<(f64, f64)> = Vec::with_capacity(segment.len() * 8);
            for i in 0..segment.len() - 1 {
                let p0 = if i > 0 { segment[i - 1] } else { segment[i] };
                let p1 = segment[i];
                let p2 = segment[i + 1];
                let p3 = if i + 2 < segment.len() {
                    segment[i + 2]
                } else {
                    segment[i + 1]
                };

                // Add start point for first segment
                if i == 0 {
                    curved.push(p1);
                }

                // Generate 8 intermediate points per span
                let steps = 8;
                for s in 1..=steps {
                    let t = s as f64 / steps as f64;
                    let t2 = t * t;
                    let t3 = t2 * t;
                    // Catmull-Rom basis (alpha=0.5)
                    let x = 0.5
                        * (2.0 * p1.0
                            + (-p0.0 + p2.0) * t
                            + (2.0 * p0.0 - 5.0 * p1.0 + 4.0 * p2.0 - p3.0) * t2
                            + (-p0.0 + 3.0 * p1.0 - 3.0 * p2.0 + p3.0) * t3);
                    let y = 0.5
                        * (2.0 * p1.1
                            + (-p0.1 + p2.1) * t
                            + (2.0 * p0.1 - 5.0 * p1.1 + 4.0 * p2.1 - p3.1) * t2
                            + (-p0.1 + 3.0 * p1.1 - 3.0 * p2.1 + p3.1) * t3);
                    curved.push((x, y));
                }
            }
            b.stroke_path(&curved, color, width);
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

    let (vis_first, vis_last) = state.time_scale.visible_range(plot_area.width);

    // Collect points with bar index for gap detection
    let mut indexed_points: Vec<(usize, f64, f64)> = Vec::with_capacity(line_points.len());
    for pt in line_points {
        let bar_idx = match state.time_index_map.get(&pt.time) {
            Some(&idx) => idx,
            None => continue,
        };
        if bar_idx + 1 < vis_first || bar_idx > vis_last + 1 {
            continue;
        }
        let x = state.time_scale.index_to_x(bar_idx, plot_area) as f64;
        let y = pane.price_scale.price_to_y(pt.value, plot_area) as f64;
        indexed_points.push((bar_idx, x, y));
    }

    let bottom_y = (plot_area.y + plot_area.height) as f64;

    // Helper: render one continuous segment
    let render_segment = |b: &mut dyn DrawBackend, segment: &[(f64, f64)]| {
        if segment.len() < 2 {
            return;
        }
        // Build fill polygon
        let mut fill_pts: Vec<(f64, f64)> = segment.to_vec();
        if let Some(&(last_x, _)) = fill_pts.last() {
            fill_pts.push((last_x, bottom_y));
        }
        if let Some(&(first_x, _)) = segment.first() {
            fill_pts.push((first_x, bottom_y));
        }
        let lowest_y = segment
            .iter()
            .map(|(_, y)| *y)
            .fold(f64::INFINITY, f64::min);

        b.fill_path_gradient(
            &fill_pts,
            lowest_y,
            bottom_y,
            &[(opts.top_color, 0.0), (opts.bottom_color, 1.0)],
        );
        b.stroke_path(segment, opts.line_color, opts.line_width as f64);
    };

    // Split into segments at gaps
    let mut segment: Vec<(f64, f64)> = Vec::new();
    let mut prev_idx: Option<usize> = None;

    for &(bar_idx, x, y) in &indexed_points {
        if let Some(prev) = prev_idx {
            if bar_idx > prev + 1 {
                render_segment(b, &segment);
                segment.clear();
            }
        }
        segment.push((x, y));
        prev_idx = Some(bar_idx);
    }
    render_segment(b, &segment);
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

    let (vis_first, vis_last) = state.time_scale.visible_range(plot_area.width);

    let base_y = pane.price_scale.price_to_y(opts.base_value, plot_area) as f64;

    // Collect points with bar index for gap detection
    let mut indexed_points: Vec<(usize, f64, f64)> = Vec::with_capacity(line_points.len());
    for pt in line_points {
        let bar_idx = match state.time_index_map.get(&pt.time) {
            Some(&idx) => idx,
            None => continue,
        };
        if bar_idx + 1 < vis_first || bar_idx > vis_last + 1 {
            continue;
        }
        let x = state.time_scale.index_to_x(bar_idx, plot_area) as f64;
        let y = pane.price_scale.price_to_y(pt.value, plot_area) as f64;
        indexed_points.push((bar_idx, x, y));
    }

    // Render one segment's fill and stroke
    let render_baseline_segment = |b: &mut dyn DrawBackend, seg: &[(f64, f64)]| {
        if seg.len() < 2 {
            return;
        }

        let mut top_fill: Vec<(f64, f64)> = Vec::new();
        let mut bottom_fill: Vec<(f64, f64)> = Vec::new();
        for &(x, y) in seg {
            top_fill.push((x, y.min(base_y)));
            bottom_fill.push((x, y.max(base_y)));
        }

        // Top fill
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

        // Bottom fill
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

        // Stroke line segments
        for i in 0..seg.len().saturating_sub(1) {
            let (x0, y0) = seg[i];
            let (x1, y1) = seg[i + 1];
            let mid_y = (y0 + y1) / 2.0;
            let color = if mid_y <= base_y {
                opts.top_line_color
            } else {
                opts.bottom_line_color
            };
            b.stroke_line(x0, y0, x1, y1, color, opts.line_width as f64);
        }
    };

    // Split into segments at gaps
    let mut segment: Vec<(f64, f64)> = Vec::new();
    let mut prev_idx: Option<usize> = None;

    for &(bar_idx, x, y) in &indexed_points {
        if let Some(prev) = prev_idx {
            if bar_idx > prev + 1 {
                render_baseline_segment(b, &segment);
                segment.clear();
            }
        }
        segment.push((x, y));
        prev_idx = Some(bar_idx);
    }
    render_baseline_segment(b, &segment);
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

    let (vis_first, vis_last) = state.time_scale.visible_range(plot_area.width);

    for pt in points {
        let bar_idx = match state.time_index_map.get(&pt.time) {
            Some(&idx) => idx,
            None => continue,
        };
        if bar_idx + 1 < vis_first || bar_idx > vis_last + 1 {
            continue;
        }
        let x = state.time_scale.index_to_x(bar_idx, plot_area) as f64;
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
