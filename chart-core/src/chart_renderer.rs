use vello::kurbo::{Affine, Line, Rect as KurboRect, Stroke};
use vello::peniko::{Color, Font};
use vello::Scene;

use crate::chart_model::Rect;
use crate::chart_state::ChartState;
use crate::overlays::{LineStyle, MarkerPosition, MarkerShape};
use crate::series::{SeriesData, SeriesType};
use crate::text_render::{chart_font, draw_text, measure_text};
use crate::tick_marks::{generate_price_ticks, generate_time_ticks, TickMark};

// Colors
const BG_COLOR: Color = Color::new([0.07, 0.07, 0.10, 1.0]);
const GRID_COLOR: Color = Color::new([0.15, 0.15, 0.20, 1.0]);
const AXIS_COLOR: Color = Color::new([0.4, 0.4, 0.5, 1.0]);
const BULL_COLOR: Color = Color::new([0.15, 0.65, 0.60, 1.0]);
const BEAR_COLOR: Color = Color::new([0.94, 0.33, 0.31, 1.0]);
const TEXT_COLOR: Color = Color::new([0.6, 0.6, 0.7, 1.0]);
const CROSSHAIR_COLOR: Color = Color::new([0.5, 0.5, 0.6, 0.8]);
const LABEL_FONT_SIZE: f32 = 11.0;

/// Render the entire chart from ChartState into a Vello Scene
pub fn render_chart(scene: &mut Scene, state: &ChartState) {
    if state.data.bars.is_empty() {
        return;
    }

    let layout = &state.layout;
    let font = chart_font();
    let price_ticks = generate_price_ticks(&state.price_scale, &layout.plot_area);
    let time_ticks = generate_time_ticks(&state.data.bars, &state.time_scale, &layout.plot_area);

    let t = Affine::scale(layout.scale_factor);

    draw_background(scene, layout, t);
    draw_grid(scene, &price_ticks, &time_ticks, &layout.plot_area, t);

    // Watermark behind bars
    if state.overlays.watermark.visible {
        draw_watermark(scene, state, &font, t);
    }

    // Draw primary series (from state.data, using active_series_type)
    match state.active_series_type {
        SeriesType::Ohlc => draw_ohlc_bars(scene, state, t),
        SeriesType::Candlestick => draw_candlestick_bars(scene, state, t),
        SeriesType::Line => draw_line_series_from_ohlc(scene, state, t),
    }

    // Draw additional series from the collection
    for series in &state.series.series {
        if !series.visible {
            continue;
        }
        match (&series.series_type, &series.data) {
            (SeriesType::Line, SeriesData::Line(pts)) => {
                draw_line_series(scene, state, pts, &series.line_options, t);
            }
            (SeriesType::Candlestick, SeriesData::Ohlc(bars)) => {
                draw_candlestick_bars_data(scene, state, bars, &series.candlestick_options, t);
            }
            _ => {} // Other combos use default OHLC
        }
    }

    // Overlays on top of bars
    draw_price_lines(scene, state, &font, t);
    draw_series_markers(scene, state, t);
    draw_last_value_marker(scene, state, &font, t);

    draw_y_axis(scene, &price_ticks, layout, &font, t);
    draw_x_axis(scene, &time_ticks, layout, &font, t);

    if state.crosshair.visible {
        draw_crosshair(scene, state, &font, t);
    }
}

fn draw_background(scene: &mut Scene, layout: &crate::chart_model::ChartLayout, t: Affine) {
    scene.fill(
        vello::peniko::Fill::NonZero,
        t,
        BG_COLOR,
        None,
        &KurboRect::new(0.0, 0.0, layout.width as f64, layout.height as f64),
    );
}

fn draw_grid(
    scene: &mut Scene,
    price_ticks: &[TickMark],
    time_ticks: &[TickMark],
    plot_area: &Rect,
    t: Affine,
) {
    let stroke = Stroke::new(1.0);

    for tick in price_ticks {
        let y = tick.coord as f64;
        scene.stroke(
            &stroke,
            t,
            GRID_COLOR,
            None,
            &Line::new(
                (plot_area.x as f64, y),
                ((plot_area.x + plot_area.width) as f64, y),
            ),
        );
    }

    for tick in time_ticks {
        let x = tick.coord as f64;
        scene.stroke(
            &stroke,
            t,
            GRID_COLOR,
            None,
            &Line::new(
                (x, plot_area.y as f64),
                (x, (plot_area.y + plot_area.height) as f64),
            ),
        );
    }

    scene.stroke(
        &Stroke::new(1.0),
        t,
        AXIS_COLOR,
        None,
        &KurboRect::new(
            plot_area.x as f64,
            plot_area.y as f64,
            (plot_area.x + plot_area.width) as f64,
            (plot_area.y + plot_area.height) as f64,
        ),
    );
}

fn draw_ohlc_bars(scene: &mut Scene, state: &ChartState, t: Affine) {
    let bar_width = state.time_scale.bar_spacing * 0.3;
    let line_width = 1.5;
    let plot_area = &state.layout.plot_area;

    // Only draw visible bars
    let (first, last) = state.time_scale.visible_range(plot_area.width);
    let first = first.saturating_sub(1); // draw one extra on each side for partial visibility
    let last = (last + 1).min(state.data.bars.len());

    for i in first..last {
        let bar = &state.data.bars[i];
        let x = state.time_scale.index_to_x(i, plot_area) as f64;

        // Skip bars fully outside plot area
        if x < (plot_area.x - bar_width) as f64
            || x > (plot_area.x + plot_area.width + bar_width) as f64
        {
            continue;
        }

        let high_y = state.price_scale.price_to_y(bar.high, plot_area) as f64;
        let low_y = state.price_scale.price_to_y(bar.low, plot_area) as f64;
        let open_y = state.price_scale.price_to_y(bar.open, plot_area) as f64;
        let close_y = state.price_scale.price_to_y(bar.close, plot_area) as f64;

        let color = if bar.close >= bar.open {
            BULL_COLOR
        } else {
            BEAR_COLOR
        };
        let stroke = Stroke::new(line_width);

        // High-Low line
        scene.stroke(&stroke, t, color, None, &Line::new((x, high_y), (x, low_y)));
        // Open tick (left)
        scene.stroke(
            &stroke,
            t,
            color,
            None,
            &Line::new((x - bar_width as f64, open_y), (x, open_y)),
        );
        // Close tick (right)
        scene.stroke(
            &stroke,
            t,
            color,
            None,
            &Line::new((x, close_y), (x + bar_width as f64, close_y)),
        );
    }
}

fn draw_crosshair(scene: &mut Scene, state: &ChartState, font: &Font, t: Affine) {
    let plot = &state.layout.plot_area;
    let x = state.crosshair.x as f64;
    let y = state.crosshair.y as f64;

    let dash_stroke = Stroke::new(1.0).with_dashes(0.0, &[4.0, 4.0]);

    // Vertical line
    scene.stroke(
        &dash_stroke,
        t,
        CROSSHAIR_COLOR,
        None,
        &Line::new((x, plot.y as f64), (x, (plot.y + plot.height) as f64)),
    );

    // Horizontal line
    scene.stroke(
        &dash_stroke,
        t,
        CROSSHAIR_COLOR,
        None,
        &Line::new((plot.x as f64, y), ((plot.x + plot.width) as f64, y)),
    );

    // Price label on Y-axis
    if let Some(price) = state.crosshair.price {
        let label = format!("{:.2}", price);
        let label_x = (plot.x + plot.width + 2.0) as f64;
        let label_w = (state.layout.margins.right - 4.0) as f64;
        let label_h = 18.0;
        let label_y = y - label_h / 2.0;

        scene.fill(
            vello::peniko::Fill::NonZero,
            t,
            Color::new([0.2, 0.2, 0.3, 0.9]),
            None,
            &KurboRect::new(label_x, label_y, label_x + label_w, label_y + label_h),
        );

        draw_text(
            scene,
            font,
            &label,
            label_x + 4.0,
            label_y + 13.0,
            10.0,
            Color::WHITE,
            t,
        );
    }

    // OHLC info tooltip at top of chart
    if let Some(idx) = state.crosshair.bar_index {
        if idx < state.data.bars.len() {
            let bar = &state.data.bars[idx];
            let info = format!(
                "O:{:.2}  H:{:.2}  L:{:.2}  C:{:.2}",
                bar.open, bar.high, bar.low, bar.close
            );

            let text_width = measure_text(font, &info, 10.0);
            let info_w = text_width as f64 + 16.0;
            let info_x = plot.x as f64 + 8.0;
            let info_y = plot.y as f64 + 4.0;
            scene.fill(
                vello::peniko::Fill::NonZero,
                t,
                Color::new([0.12, 0.12, 0.18, 0.9]),
                None,
                &KurboRect::new(info_x, info_y, info_x + info_w, info_y + 20.0),
            );

            draw_text(
                scene,
                font,
                &info,
                info_x + 8.0,
                info_y + 14.0,
                10.0,
                TEXT_COLOR,
                t,
            );
        }
    }
}

fn draw_y_axis(
    scene: &mut Scene,
    price_ticks: &[TickMark],
    layout: &crate::chart_model::ChartLayout,
    font: &Font,
    t: Affine,
) {
    let x_start = layout.plot_area.x + layout.plot_area.width + 5.0;

    for tick in price_ticks {
        scene.stroke(
            &Stroke::new(1.0),
            t,
            AXIS_COLOR,
            None,
            &Line::new(
                (
                    (layout.plot_area.x + layout.plot_area.width) as f64,
                    tick.coord as f64,
                ),
                (
                    (layout.plot_area.x + layout.plot_area.width + 4.0) as f64,
                    tick.coord as f64,
                ),
            ),
        );

        draw_text(
            scene,
            font,
            &tick.label,
            x_start as f64,
            (tick.coord + 4.0) as f64,
            LABEL_FONT_SIZE,
            TEXT_COLOR,
            t,
        );
    }
}

fn draw_x_axis(
    scene: &mut Scene,
    time_ticks: &[TickMark],
    layout: &crate::chart_model::ChartLayout,
    font: &Font,
    t: Affine,
) {
    let y_start = layout.plot_area.y + layout.plot_area.height + 5.0;

    for tick in time_ticks {
        scene.stroke(
            &Stroke::new(1.0),
            t,
            AXIS_COLOR,
            None,
            &Line::new(
                (
                    tick.coord as f64,
                    (layout.plot_area.y + layout.plot_area.height) as f64,
                ),
                (
                    tick.coord as f64,
                    (layout.plot_area.y + layout.plot_area.height + 4.0) as f64,
                ),
            ),
        );

        // Center the label under the tick mark
        let label_width = measure_text(font, &tick.label, LABEL_FONT_SIZE);
        draw_text(
            scene,
            font,
            &tick.label,
            tick.coord as f64 - label_width as f64 / 2.0,
            (y_start + 12.0) as f64,
            LABEL_FONT_SIZE,
            TEXT_COLOR,
            t,
        );
    }
}

// ---------------------------------------------------------------------------
// Overlay rendering
// ---------------------------------------------------------------------------

fn draw_price_lines(scene: &mut Scene, state: &ChartState, font: &Font, t: Affine) {
    let plot = &state.layout.plot_area;
    let stroke = Stroke::new(1.0);

    for line in &state.overlays.price_lines {
        let y = state.price_scale.price_to_y(line.price, plot);
        if y < plot.y || y > plot.y + plot.height {
            continue; // Off-screen
        }

        let color = Color::new(line.color);

        // Draw the line
        match line.line_style {
            LineStyle::Dashed => {
                // Draw dashed line as segments
                let dash_len = 6.0;
                let gap_len = 4.0;
                let mut x = plot.x;
                while x < plot.x + plot.width {
                    let end = (x + dash_len).min(plot.x + plot.width);
                    scene.stroke(
                        &stroke,
                        t,
                        color,
                        None,
                        &Line::new((x as f64, y as f64), (end as f64, y as f64)),
                    );
                    x += dash_len + gap_len;
                }
            }
            LineStyle::Dotted => {
                let dot_spacing = 4.0;
                let mut x = plot.x;
                while x < plot.x + plot.width {
                    scene.stroke(
                        &stroke,
                        t,
                        color,
                        None,
                        &Line::new((x as f64, y as f64), ((x + 1.0) as f64, y as f64)),
                    );
                    x += dot_spacing;
                }
            }
            LineStyle::Solid => {
                scene.stroke(
                    &stroke,
                    t,
                    color,
                    None,
                    &Line::new(
                        (plot.x as f64, y as f64),
                        ((plot.x + plot.width) as f64, y as f64),
                    ),
                );
            }
        }

        // Label on Y-axis
        if line.label_visible {
            let label_x = plot.x + plot.width + 4.0;
            draw_text(
                scene,
                font,
                &line.label,
                label_x as f64,
                (y + 3.0) as f64,
                LABEL_FONT_SIZE,
                color,
                t,
            );
        }
    }
}

fn draw_series_markers(scene: &mut Scene, state: &ChartState, t: Affine) {
    let plot = &state.layout.plot_area;

    for marker in &state.overlays.markers {
        // Find the bar index for this marker's time
        let idx = match state
            .data
            .bars
            .binary_search_by_key(&marker.time, |b| b.time)
        {
            Ok(i) => i,
            Err(_) => continue, // No matching bar
        };

        let bar = &state.data.bars[idx];
        let x = state.time_scale.index_to_x(idx, plot);
        if x < plot.x || x > plot.x + plot.width {
            continue; // Off-screen
        }

        let price = marker.y_price(bar);
        let y = state.price_scale.price_to_y(price, plot);
        let offset = match marker.position {
            MarkerPosition::AboveBar => -(marker.size + 4.0),
            MarkerPosition::BelowBar => marker.size + 4.0,
            MarkerPosition::AtPrice => 0.0,
        };
        let y = y + offset;
        let color = Color::new(marker.color);
        let half = marker.size / 2.0;

        match marker.shape {
            MarkerShape::ArrowUp => {
                // Triangle pointing up
                let path = vello::kurbo::BezPath::from_vec(vec![
                    vello::kurbo::PathEl::MoveTo((x as f64, (y - half) as f64).into()),
                    vello::kurbo::PathEl::LineTo(((x - half) as f64, (y + half) as f64).into()),
                    vello::kurbo::PathEl::LineTo(((x + half) as f64, (y + half) as f64).into()),
                    vello::kurbo::PathEl::ClosePath,
                ]);
                scene.fill(vello::peniko::Fill::NonZero, t, color, None, &path);
            }
            MarkerShape::ArrowDown => {
                let path = vello::kurbo::BezPath::from_vec(vec![
                    vello::kurbo::PathEl::MoveTo((x as f64, (y + half) as f64).into()),
                    vello::kurbo::PathEl::LineTo(((x - half) as f64, (y - half) as f64).into()),
                    vello::kurbo::PathEl::LineTo(((x + half) as f64, (y - half) as f64).into()),
                    vello::kurbo::PathEl::ClosePath,
                ]);
                scene.fill(vello::peniko::Fill::NonZero, t, color, None, &path);
            }
            MarkerShape::Circle => {
                let circle = vello::kurbo::Circle::new((x as f64, y as f64), half as f64);
                scene.fill(vello::peniko::Fill::NonZero, t, color, None, &circle);
            }
            MarkerShape::Square => {
                let rect = KurboRect::new(
                    (x - half) as f64,
                    (y - half) as f64,
                    (x + half) as f64,
                    (y + half) as f64,
                );
                scene.fill(vello::peniko::Fill::NonZero, t, color, None, &rect);
            }
        }
    }
}

fn draw_watermark(scene: &mut Scene, state: &ChartState, font: &Font, t: Affine) {
    let wm = &state.overlays.watermark;
    let plot = &state.layout.plot_area;
    let color = Color::new(wm.color);

    let text_w = measure_text(font, &wm.text, wm.font_size);
    let x = match wm.h_align {
        crate::overlays::HAlign::Left => plot.x + 10.0,
        crate::overlays::HAlign::Center => plot.x + (plot.width - text_w) / 2.0,
        crate::overlays::HAlign::Right => plot.x + plot.width - text_w - 10.0,
    };
    let y = match wm.v_align {
        crate::overlays::VAlign::Top => plot.y + wm.font_size + 10.0,
        crate::overlays::VAlign::Center => plot.y + plot.height / 2.0,
        crate::overlays::VAlign::Bottom => plot.y + plot.height - 10.0,
    };

    draw_text(
        scene,
        font,
        &wm.text,
        x as f64,
        y as f64,
        wm.font_size,
        color,
        t,
    );
}

fn draw_last_value_marker(scene: &mut Scene, state: &ChartState, font: &Font, t: Affine) {
    let lv = &state.overlays.last_value;
    if !lv.visible || state.data.bars.is_empty() {
        return;
    }

    let last_bar = state.data.bars.last().unwrap();
    let plot = &state.layout.plot_area;
    let y = state.price_scale.price_to_y(last_bar.close, plot);

    if y < plot.y || y > plot.y + plot.height {
        return;
    }

    let color = Color::new(lv.color);
    let label = state.format_price(last_bar.close);
    let label_w = measure_text(font, &label, LABEL_FONT_SIZE);

    // Background rectangle on Y-axis
    let label_x = plot.x + plot.width + 2.0;
    let bg_rect = KurboRect::new(
        label_x as f64,
        (y - 8.0) as f64,
        (label_x + label_w + 8.0) as f64,
        (y + 8.0) as f64,
    );
    scene.fill(vello::peniko::Fill::NonZero, t, color, None, &bg_rect);

    // White text on colored background
    let text_color = Color::new([1.0, 1.0, 1.0, 1.0]);
    draw_text(
        scene,
        font,
        &label,
        (label_x + 4.0) as f64,
        (y + 3.0) as f64,
        LABEL_FONT_SIZE,
        text_color,
        t,
    );

    // Dashed line from plot area to Y-axis
    let stroke = Stroke::new(1.0);
    let dash_len = 4.0;
    let gap_len = 3.0;
    let mut x = plot.x;
    while x < plot.x + plot.width {
        let end = (x + dash_len).min(plot.x + plot.width);
        scene.stroke(
            &stroke,
            t,
            color,
            None,
            &Line::new((x as f64, y as f64), (end as f64, y as f64)),
        );
        x += dash_len + gap_len;
    }
}

// ---------------------------------------------------------------------------
// Candlestick renderer
// ---------------------------------------------------------------------------

fn draw_candlestick_bars(scene: &mut Scene, state: &ChartState, t: Affine) {
    let plot_area = &state.layout.plot_area;
    let (first, last) = state.time_scale.visible_range(plot_area.width);
    let first = first.saturating_sub(1);
    let last = (last + 1).min(state.data.bars.len());
    let body_width = (state.time_scale.bar_spacing * 0.6).max(1.0);

    for i in first..last {
        let bar = &state.data.bars[i];
        let x = state.time_scale.index_to_x(i, plot_area);
        if x < plot_area.x - body_width || x > plot_area.x + plot_area.width + body_width {
            continue;
        }

        let high_y = state.price_scale.price_to_y(bar.high, plot_area);
        let low_y = state.price_scale.price_to_y(bar.low, plot_area);
        let open_y = state.price_scale.price_to_y(bar.open, plot_area);
        let close_y = state.price_scale.price_to_y(bar.close, plot_area);
        let bullish = bar.close >= bar.open;

        let fill_color = if bullish { BULL_COLOR } else { BEAR_COLOR };
        let wick_color = fill_color;

        // Wick (high-low line)
        let wick_stroke = Stroke::new(1.0);
        scene.stroke(
            &wick_stroke,
            t,
            wick_color,
            None,
            &Line::new((x as f64, high_y as f64), (x as f64, low_y as f64)),
        );

        // Body (filled rectangle)
        let top_y = open_y.min(close_y);
        let bot_y = open_y.max(close_y);
        let body_h = (bot_y - top_y).max(1.0); // minimum 1px body
        let body_rect = KurboRect::new(
            (x - body_width / 2.0) as f64,
            top_y as f64,
            (x + body_width / 2.0) as f64,
            (top_y + body_h) as f64,
        );

        scene.fill(
            vello::peniko::Fill::NonZero,
            t,
            fill_color,
            None,
            &body_rect,
        );
    }
}

/// Draw candlestick bars from arbitrary bar data (for multi-series support)
fn draw_candlestick_bars_data(
    scene: &mut Scene,
    state: &ChartState,
    bars: &[crate::chart_model::OhlcBar],
    opts: &crate::series::CandlestickOptions,
    t: Affine,
) {
    let plot_area = &state.layout.plot_area;
    let body_width = (state.time_scale.bar_spacing * 0.6).max(1.0);

    for (i, bar) in bars.iter().enumerate() {
        let x = state.time_scale.index_to_x(i, plot_area);
        if x < plot_area.x - body_width || x > plot_area.x + plot_area.width + body_width {
            continue;
        }

        let high_y = state.price_scale.price_to_y(bar.high, plot_area);
        let low_y = state.price_scale.price_to_y(bar.low, plot_area);
        let open_y = state.price_scale.price_to_y(bar.open, plot_area);
        let close_y = state.price_scale.price_to_y(bar.close, plot_area);
        let bullish = bar.close >= bar.open;

        let fill_color = Color::new(if bullish {
            opts.up_color
        } else {
            opts.down_color
        });
        let wick_color = Color::new(if bullish {
            opts.wick_up_color
        } else {
            opts.wick_down_color
        });

        let wick_stroke = Stroke::new(1.0);
        scene.stroke(
            &wick_stroke,
            t,
            wick_color,
            None,
            &Line::new((x as f64, high_y as f64), (x as f64, low_y as f64)),
        );

        let top_y = open_y.min(close_y);
        let bot_y = open_y.max(close_y);
        let body_h = (bot_y - top_y).max(1.0);
        let body_rect = KurboRect::new(
            (x - body_width / 2.0) as f64,
            top_y as f64,
            (x + body_width / 2.0) as f64,
            (top_y + body_h) as f64,
        );

        scene.fill(
            vello::peniko::Fill::NonZero,
            t,
            fill_color,
            None,
            &body_rect,
        );
    }
}

// ---------------------------------------------------------------------------
// Line series renderers
// ---------------------------------------------------------------------------

/// Draw a line series from OHLC close prices (for primary series)
fn draw_line_series_from_ohlc(scene: &mut Scene, state: &ChartState, t: Affine) {
    let plot_area = &state.layout.plot_area;
    let (first, last) = state.time_scale.visible_range(plot_area.width);
    let first = first.saturating_sub(1);
    let last = (last + 1).min(state.data.bars.len());

    if last <= first + 1 {
        return;
    }

    let color = Color::new([0.26, 0.52, 0.96, 1.0]); // Blue line
    let stroke = Stroke::new(2.0);

    for i in first..last.saturating_sub(1) {
        let x1 = state.time_scale.index_to_x(i, plot_area);
        let x2 = state.time_scale.index_to_x(i + 1, plot_area);
        let y1 = state
            .price_scale
            .price_to_y(state.data.bars[i].close, plot_area);
        let y2 = state
            .price_scale
            .price_to_y(state.data.bars[i + 1].close, plot_area);

        scene.stroke(
            &stroke,
            t,
            color,
            None,
            &Line::new((x1 as f64, y1 as f64), (x2 as f64, y2 as f64)),
        );
    }
}

/// Draw a line series from LineDataPoint data
fn draw_line_series(
    scene: &mut Scene,
    state: &ChartState,
    points: &[crate::series::LineDataPoint],
    opts: &crate::series::LineSeriesOptions,
    t: Affine,
) {
    if points.len() < 2 {
        return;
    }

    let plot_area = &state.layout.plot_area;
    let color = Color::new(opts.color);
    let stroke = Stroke::new(opts.line_width as f64);

    for i in 0..points.len() - 1 {
        let x1 = state.time_scale.index_to_x(i, plot_area);
        let x2 = state.time_scale.index_to_x(i + 1, plot_area);
        let y1 = state.price_scale.price_to_y(points[i].value, plot_area);
        let y2 = state.price_scale.price_to_y(points[i + 1].value, plot_area);

        scene.stroke(
            &stroke,
            t,
            color,
            None,
            &Line::new((x1 as f64, y1 as f64), (x2 as f64, y2 as f64)),
        );
    }

    // Optional point markers
    if opts.point_markers_visible {
        for (i, pt) in points.iter().enumerate() {
            let x = state.time_scale.index_to_x(i, plot_area);
            let y = state.price_scale.price_to_y(pt.value, plot_area);
            let circle =
                vello::kurbo::Circle::new((x as f64, y as f64), opts.point_markers_radius as f64);
            scene.fill(vello::peniko::Fill::NonZero, t, color, None, &circle);
        }
    }
}
