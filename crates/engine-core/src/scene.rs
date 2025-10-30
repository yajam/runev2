use palette::{FromColor, LinSrgba, Srgba};

#[derive(Clone, Copy, Debug, Default)]
pub struct Transform2D {
    // Affine 2D: [a, b, c, d, e, f] for matrix [[a c e],[b d f],[0 0 1]]
    pub m: [f32; 6],
}

impl Transform2D {
    pub fn identity() -> Self { Self { m: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] } }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ColorLinPremul {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl ColorLinPremul {
    pub fn from_srgba_u8(c: [u8; 4]) -> Self {
        let s = Srgba::new(
            c[0] as f32 / 255.0,
            c[1] as f32 / 255.0,
            c[2] as f32 / 255.0,
            c[3] as f32 / 255.0,
        );
        let lin: LinSrgba = LinSrgba::from_color(s);
        Self { r: lin.red * lin.alpha, g: lin.green * lin.alpha, b: lin.blue * lin.alpha, a: lin.alpha }
    }

    /// Create color from sRGB u8 values with float alpha (like CSS rgba)
    /// Example: `ColorLinPremul::from_srgba(255, 255, 255, 0.08)` for white at 8% alpha
    pub fn from_srgba(r: u8, g: u8, b: u8, a: f32) -> Self {
        let s = Srgba::new(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a,
        );
        let lin: LinSrgba = LinSrgba::from_color(s);
        Self { r: lin.red * lin.alpha, g: lin.green * lin.alpha, b: lin.blue * lin.alpha, a: lin.alpha }
    }

    pub fn from_lin_rgba(r: f32, g: f32, b: f32, a: f32) -> Self { Self { r: r * a, g: g * a, b: b * a, a } }
}

#[derive(Clone, Debug)]
pub enum Brush {
    Solid(ColorLinPremul),
    LinearGradient {
        start: [f32; 2],
        end: [f32; 2],
        stops: Vec<(f32, ColorLinPremul)>,
    },
    RadialGradient {
        center: [f32; 2],
        radius: f32,
        stops: Vec<(f32, ColorLinPremul)>,
    },
    // Pattern, RadialGradient etc. can be added later.
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RoundedRadii {
    pub tl: f32,
    pub tr: f32,
    pub br: f32,
    pub bl: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundedRect {
    pub rect: Rect,
    pub radii: RoundedRadii,
}

#[derive(Clone, Copy, Debug)]
pub struct ClipRect(pub Rect);

#[derive(Clone, Copy, Debug)]
pub struct Stroke {
    pub width: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct BoxShadowSpec {
    pub offset: [f32; 2],
    pub spread: f32,
    pub blur_radius: f32,
    pub color: ColorLinPremul,
}

#[derive(Clone, Debug)]
pub enum Shape {
    Rect(Rect),
    RoundedRect(RoundedRect),
}

#[derive(Clone, Debug)]
pub struct TextRun {
    pub text: String,
    pub pos: [f32; 2],
    pub size: f32,
    pub color: ColorLinPremul,
}

// --- Path geometry (for SVG import / lyon) ---

#[derive(Clone, Copy, Debug)]
pub enum FillRule { NonZero, EvenOdd }

#[derive(Clone, Debug)]
pub enum PathCmd {
    MoveTo([f32; 2]),
    LineTo([f32; 2]),
    QuadTo([f32; 2], [f32; 2]),
    CubicTo([f32; 2], [f32; 2], [f32; 2]),
    Close,
}

#[derive(Clone, Debug)]
pub struct Path {
    pub cmds: Vec<PathCmd>,
    pub fill_rule: FillRule,
}
