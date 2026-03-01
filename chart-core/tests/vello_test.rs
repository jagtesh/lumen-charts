use vello::peniko::{Brush, Color, Gradient, ColorStops};
fn test() {
    let g = Gradient::new_linear((0.0, 0.0), (0.0, 100.0)).with_stops([
        (0.0, Color::RED),
        (1.0, Color::BLUE),
    ]);
    let b = Brush::Gradient(g);
}
