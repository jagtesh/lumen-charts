use vello::peniko::{Brush, Color, Gradient};
fn test() {
    let g = Gradient::new_linear((0.0, 0.0), (0.0, 100.0)).with_stops([
        (0.0, Color::new([1.0, 0.0, 0.0, 1.0])),
        (1.0, Color::new([0.0, 0.0, 1.0, 1.0])),
    ]);
    let b = Brush::Gradient(g);
}
