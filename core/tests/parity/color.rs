/// Parity tests from LWC: tests/unittests/color.spec.ts
///
/// Contrast color generation and gradient interpolation.
/// Inline implementations — will be moved to lumen_charts::color module in Slice 6.

#[derive(Debug, Clone, PartialEq)]
struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: f32,
}

fn contrast_foreground(bg: &Rgba) -> &'static str {
    let lum = 0.2126 * bg.r as f64 + 0.7152 * bg.g as f64 + 0.0722 * bg.b as f64;
    if lum > 160.0 {
        "black"
    } else {
        "white"
    }
}

fn gradient_at(from: &Rgba, to: &Rgba, pct: f32) -> Rgba {
    Rgba {
        r: (from.r as f32 + (to.r as f32 - from.r as f32) * pct).round() as u8,
        g: (from.g as f32 + (to.g as f32 - from.g as f32) * pct).round() as u8,
        b: (from.b as f32 + (to.b as f32 - from.b as f32) * pct).round() as u8,
        a: from.a + (to.a - from.a) * pct,
    }
}

/// LWC: generateContrastColors — white bg → black fg
/// File: color.spec.ts, line 45
#[test]
fn lwc_contrast_white() {
    assert_eq!(
        contrast_foreground(&Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 1.0
        }),
        "black"
    );
}

/// LWC: generateContrastColors — black bg → white fg
#[test]
fn lwc_contrast_black() {
    assert_eq!(
        contrast_foreground(&Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 1.0
        }),
        "white"
    );
}

/// LWC: correct contrast color — midtones
/// File: color.spec.ts, line 57
#[test]
fn lwc_contrast_midtones() {
    assert_eq!(
        contrast_foreground(&Rgba {
            r: 150,
            g: 150,
            b: 150,
            a: 1.0
        }),
        "white"
    );
    assert_eq!(
        contrast_foreground(&Rgba {
            r: 170,
            g: 170,
            b: 170,
            a: 1.0
        }),
        "black"
    );
    assert_eq!(
        contrast_foreground(&Rgba {
            r: 130,
            g: 140,
            b: 160,
            a: 1.0
        }),
        "white"
    );
    assert_eq!(
        contrast_foreground(&Rgba {
            r: 190,
            g: 180,
            b: 160,
            a: 1.0
        }),
        "black"
    );
}

/// LWC: gradientColorAtPercent 0%
/// File: color.spec.ts, line 68
#[test]
fn lwc_gradient_0() {
    let r = gradient_at(
        &Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 1.0,
        },
        &Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 1.0,
        },
        0.0,
    );
    assert_eq!(
        r,
        Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 1.0
        }
    );
}

/// LWC: gradientColorAtPercent 50%
/// File: color.spec.ts, line 75
#[test]
fn lwc_gradient_50() {
    let r = gradient_at(
        &Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 1.0,
        },
        &Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 1.0,
        },
        0.5,
    );
    assert_eq!(
        r,
        Rgba {
            r: 128,
            g: 128,
            b: 128,
            a: 1.0
        }
    );
}

/// LWC: gradientColorAtPercent 50% with alpha
#[test]
fn lwc_gradient_50_alpha() {
    let r = gradient_at(
        &Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 1.0,
        },
        &Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 0.0,
        },
        0.5,
    );
    assert_eq!(
        r,
        Rgba {
            r: 128,
            g: 128,
            b: 128,
            a: 0.5
        }
    );
}

/// LWC: gradientColorAtPercent 100%
/// File: color.spec.ts, line 82
#[test]
fn lwc_gradient_100() {
    let r = gradient_at(
        &Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 1.0,
        },
        &Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 1.0,
        },
        1.0,
    );
    assert_eq!(
        r,
        Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 1.0
        }
    );
}
