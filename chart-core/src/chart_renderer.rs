use vello::kurbo::{Affine, Line, Rect as KurboRect, Stroke};
use vello::peniko::Color;
use vello::Scene;

use crate::chart_model::Rect;
use crate::chart_state::ChartState;
use crate::tick_marks::{generate_price_ticks, generate_time_ticks, TickMark};

// Colors
const BG_COLOR: Color = Color::new([0.07, 0.07, 0.10, 1.0]);
const GRID_COLOR: Color = Color::new([0.15, 0.15, 0.20, 1.0]);
const AXIS_COLOR: Color = Color::new([0.4, 0.4, 0.5, 1.0]);
const BULL_COLOR: Color = Color::new([0.15, 0.65, 0.60, 1.0]);
const BEAR_COLOR: Color = Color::new([0.94, 0.33, 0.31, 1.0]);
const TEXT_COLOR: Color = Color::new([0.6, 0.6, 0.7, 1.0]);
const CROSSHAIR_COLOR: Color = Color::new([0.5, 0.5, 0.6, 0.8]);

/// Render the entire chart from ChartState into a Vello Scene
pub fn render_chart(scene: &mut Scene, state: &ChartState) {
    if state.data.bars.is_empty() {
        return;
    }

    let layout = &state.layout;
    let price_ticks = generate_price_ticks(&state.price_scale, &layout.plot_area);
    let time_ticks = generate_time_ticks(&state.data.bars, &state.time_scale, &layout.plot_area);

    let t = Affine::scale(layout.scale_factor);

    draw_background(scene, layout, t);
    draw_grid(scene, &price_ticks, &time_ticks, &layout.plot_area, t);
    draw_ohlc_bars(scene, state, t);
    draw_y_axis(scene, &price_ticks, layout, t);
    draw_x_axis(scene, &time_ticks, layout, t);

    if state.crosshair.visible {
        draw_crosshair(scene, state, t);
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

fn draw_crosshair(scene: &mut Scene, state: &ChartState, t: Affine) {
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

    // Price label background on Y-axis
    if let Some(price) = state.crosshair.price {
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

        // Price text (placeholder)
        let label = format!("{:.2}", price);
        draw_text_label(
            scene,
            &label,
            label_x + 4.0,
            label_y + 13.0,
            10.0,
            Color::WHITE,
            t,
        );
    }

    // Bar info label at top when hovering a bar
    if let Some(idx) = state.crosshair.bar_index {
        if idx < state.data.bars.len() {
            let bar = &state.data.bars[idx];
            let info = format!(
                "O:{:.2} H:{:.2} L:{:.2} C:{:.2}",
                bar.open, bar.high, bar.low, bar.close
            );

            // Background
            let info_w = info.len() as f64 * 6.5 + 12.0;
            let info_x = plot.x as f64 + 8.0;
            let info_y = plot.y as f64 + 4.0;
            scene.fill(
                vello::peniko::Fill::NonZero,
                t,
                Color::new([0.12, 0.12, 0.18, 0.9]),
                None,
                &KurboRect::new(info_x, info_y, info_x + info_w, info_y + 20.0),
            );

            draw_text_label(
                scene,
                &info,
                info_x + 6.0,
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

        draw_text_label(
            scene,
            &tick.label,
            x_start as f64,
            (tick.coord + 4.0) as f64,
            11.0,
            TEXT_COLOR,
            t,
        );
    }
}

fn draw_x_axis(
    scene: &mut Scene,
    time_ticks: &[TickMark],
    layout: &crate::chart_model::ChartLayout,
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

        let label_width = tick.label.len() as f64 * 6.0;
        draw_text_label(
            scene,
            &tick.label,
            tick.coord as f64 - label_width / 2.0,
            (y_start + 12.0) as f64,
            11.0,
            TEXT_COLOR,
            t,
        );
    }
}

/// Simple placeholder text rendering (rectangles per character)
fn draw_text_label(
    scene: &mut Scene,
    text: &str,
    x: f64,
    y: f64,
    font_size: f64,
    color: Color,
    t: Affine,
) {
    let char_width = font_size * 0.6;
    let char_height = font_size;

    for (i, ch) in text.chars().enumerate() {
        if ch == ' ' {
            continue;
        }
        let cx = x + i as f64 * char_width;

        match ch {
            '0'..='9' | 'A'..='Z' | 'a'..='z' => {
                let glyph_rect = KurboRect::new(
                    cx + 1.0,
                    y - char_height + 2.0,
                    cx + char_width - 1.0,
                    y - 1.0,
                );
                scene.fill(vello::peniko::Fill::NonZero, t, color, None, &glyph_rect);
            }
            '.' | ',' => {
                let dot = KurboRect::new(cx + 2.0, y - 3.0, cx + 4.0, y - 1.0);
                scene.fill(vello::peniko::Fill::NonZero, t, color, None, &dot);
            }
            '-' | ':' => {
                scene.stroke(
                    &Stroke::new(1.0),
                    t,
                    color,
                    None,
                    &Line::new(
                        (cx + 1.0, y - char_height / 2.0),
                        (cx + char_width - 1.0, y - char_height / 2.0),
                    ),
                );
            }
            _ => {}
        }
    }
}
