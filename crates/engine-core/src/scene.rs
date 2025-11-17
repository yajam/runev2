#[derive(Clone, Copy, Debug, Default)]
pub struct Transform2D {
    // Affine 2D: [a, b, c, d, e, f] for matrix [[a c e],[b d f],[0 0 1]]
    pub m: [f32; 6],
}

impl Transform2D {
    pub fn identity() -> Self {
        Self {
            m: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        }
    }

    /// Compose two transforms: self ∘ other (apply `other`, then `self`).
    pub fn concat(self, other: Self) -> Self {
        let [a1, b1, c1, d1, e1, f1] = self.m;
        let [a2, b2, c2, d2, e2, f2] = other.m;
        let a = a1 * a2 + c1 * b2;
        let b = b1 * a2 + d1 * b2;
        let c = a1 * c2 + c1 * d2;
        let d = b1 * c2 + d1 * d2;
        let e = a1 * e2 + c1 * f2 + e1;
        let f = b1 * e2 + d1 * f2 + f1;
        Self {
            m: [a, b, c, d, e, f],
        }
    }

    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            m: [sx, 0.0, 0.0, sy, 0.0, 0.0],
        }
    }

    pub fn translate(tx: f32, ty: f32) -> Self {
        Self {
            m: [1.0, 0.0, 0.0, 1.0, tx, ty],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ColorLinPremul {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// Alias for the premultiplied linear color type, for a friendlier name in APIs.
pub type Color = ColorLinPremul;

// Constructors for ColorLinPremul are defined in color.rs to keep scene.rs focused

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
pub enum FillRule {
    NonZero,
    EvenOdd,
}

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
