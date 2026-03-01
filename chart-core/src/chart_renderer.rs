use vello::kurbo::{Affine, Line, Rect as KurboRect, Stroke};
use vello::peniko::Color;
use vello::Scene;

use crate::chart_model::{ChartData, ChartLayout, Rect};
use crate::price_scale::PriceScale;
use crate::tick_marks::{generate_price_ticks, generate_time_ticks, TickMark};
use crate::time_scale::TimeScale;

// Colors
const BG_COLOR: Color = Color::new([0.07, 0.07, 0.10, 1.0]);
const GRID_COLOR: Color = Color::new([0.15, 0.15, 0.20, 1.0]);
const AXIS_COLOR: Color = Color::new([0.4, 0.4, 0.5, 1.0]);
const BULL_COLOR: Color = Color::new([0.15, 0.65, 0.60, 1.0]); // #26a69a
const BEAR_COLOR: Color = Color::new([0.94, 0.33, 0.31, 1.0]); // #ef5350
const TEXT_COLOR: Color = Color::new([0.6, 0.6, 0.7, 1.0]);

/// Render the entire chart into a Vello Scene
pub fn render_chart(scene: &mut Scene, data: &ChartData, layout: &ChartLayout) {
    if data.bars.is_empty() {
        return;
    }

    let price_scale = PriceScale::from_data(&data.bars);
    let time_scale = TimeScale::new(data.bars.len(), &layout.plot_area);
    let price_ticks = generate_price_ticks(&price_scale, &layout.plot_area);
    let time_ticks = generate_time_ticks(&data.bars, &time_scale, &layout.plot_area);

    // HiDPI: scale all drawing from logical coordinates to physical pixels
    let t = Affine::scale(layout.scale_factor);

    draw_background(scene, layout, t);
    draw_grid(scene, &price_ticks, &time_ticks, &layout.plot_area, t);
    draw_ohlc_bars(scene, data, &time_scale, &price_scale, &layout.plot_area, t);
    draw_y_axis(scene, &price_ticks, layout, t);
    draw_x_axis(scene, &time_ticks, layout, t);
}

fn draw_background(scene: &mut Scene, layout: &ChartLayout, t: Affine) {
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

    // Horizontal grid lines (at price ticks)
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

    // Vertical grid lines (at time ticks)
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

    // Plot area border
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

fn draw_ohlc_bars(
    scene: &mut Scene,
    data: &ChartData,
    time_scale: &TimeScale,
    price_scale: &PriceScale,
    plot_area: &Rect,
    t: Affine,
) {
    let bar_width = time_scale.bar_spacing * 0.3;
    let line_width = 1.5;

    for (i, bar) in data.bars.iter().enumerate() {
        let x = time_scale.index_to_x(i, plot_area) as f64;
        let high_y = price_scale.price_to_y(bar.high, plot_area) as f64;
        let low_y = price_scale.price_to_y(bar.low, plot_area) as f64;
        let open_y = price_scale.price_to_y(bar.open, plot_area) as f64;
        let close_y = price_scale.price_to_y(bar.close, plot_area) as f64;

        let color = if bar.close >= bar.open {
            BULL_COLOR
        } else {
            BEAR_COLOR
        };

        let stroke = Stroke::new(line_width);

        // Vertical line: high to low
        scene.stroke(&stroke, t, color, None, &Line::new((x, high_y), (x, low_y)));

        // Left tick: open
        scene.stroke(
            &stroke,
            t,
            color,
            None,
            &Line::new((x - bar_width as f64, open_y), (x, open_y)),
        );

        // Right tick: close
        scene.stroke(
            &stroke,
            t,
            color,
            None,
            &Line::new((x, close_y), (x + bar_width as f64, close_y)),
        );
    }
}

fn draw_y_axis(scene: &mut Scene, price_ticks: &[TickMark], layout: &ChartLayout, t: Affine) {
    let x_start = layout.plot_area.x + layout.plot_area.width + 5.0;

    for tick in price_ticks {
        // Small tick mark
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

fn draw_x_axis(scene: &mut Scene, time_ticks: &[TickMark], layout: &ChartLayout, t: Affine) {
    let y_start = layout.plot_area.y + layout.plot_area.height + 5.0;

    for tick in time_ticks {
        // Small tick mark
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
            '-' => {
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
