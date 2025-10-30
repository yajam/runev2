use engine_core::{Brush, ColorLinPremul, Rect, RoundedRadii, RoundedRect, Stroke, HitResult, HitShape};
use engine_core::{DisplayList, Painter, PassManager, Viewport};

use super::{Scene, SceneKind};

pub struct CenteredRectScene {
    viewport: Viewport,
    hovered: Option<HitShape>,
}

impl Default for CenteredRectScene {
    fn default() -> Self {
        Self {
            viewport: Viewport {
                width: 800,
                height: 600,
            },
            hovered: None,
        }
    }
}

// First impl removed (consolidated below)

impl CenteredRectScene {
    /// Helper to pre-calculate blended color for macOS/Metal workaround
    /// Blends foreground color with alpha over background color
    fn blend_colors_srgb(fg: [u8; 3], alpha: f32, bg: [u8; 3]) -> [u8; 4] {
        let r = (fg[0] as f32 * alpha + bg[0] as f32 * (1.0 - alpha)).round() as u8;
        let g = (fg[1] as f32 * alpha + bg[1] as f32 * (1.0 - alpha)).round() as u8;
        let b = (fg[2] as f32 * alpha + bg[2] as f32 * (1.0 - alpha)).round() as u8;
        [r, g, b, 255]
    }

    /// Convert perceived alpha (like in browsers) to linear space alpha
    /// Linear blending makes colors appear more opaque, so we need lower values
    fn srgb_alpha_to_linear(alpha: f32) -> f32 {
        // Approximate compensation for linear space blending
        // This is a heuristic - may need tuning
        alpha.powf(2.2) // Apply gamma correction to alpha
    }

    fn build_display_list(&self) -> DisplayList {
        let mut painter = Painter::begin_frame(self.viewport);

        // First, draw background as a full-screen rectangle (original dark)
        let bg_color = ColorLinPremul::from_srgba_u8([0x0b, 0x12, 0x20, 0xff]);
        painter.rect(
            Rect {
                x: 0.0,
                y: 0.0,
                w: self.viewport.width as f32,
                h: self.viewport.height as f32,
            },
            Brush::Solid(bg_color),
            0,
        );

        // Calculate centered position for a 400x300 rectangle
        let rect_width = 400.0;
        let rect_height = 300.0;
        let x = (self.viewport.width as f32 - rect_width) / 2.0;
        let y = (self.viewport.height as f32 - rect_height) / 2.0;

        // Draw a small red rectangle behind (to test if we can see through)
        let red = ColorLinPremul::from_srgba_u8([255, 0, 0, 255]);
        painter.rect(
            Rect {
                x: x + 50.0,
                y: y + 50.0,
                w: 100.0,
                h: 100.0,
            },
            Brush::Solid(red),
            1,
        );

        // For a bordered rectangle with transparency, we need to draw it as a single shape
        // with the fill alpha, since layering transparent shapes causes compounding
        // Just draw the fill - we'll add proper border support later
        let white_fill = ColorLinPremul::from_srgba(255, 255, 255, 0.04);

        painter.rounded_rect(
            RoundedRect {
                rect: Rect {
                    x,
                    y,
                    w: rect_width,
                    h: rect_height,
                },
                radii: RoundedRadii {
                    tl: 16.0,
                    tr: 16.0,
                    br: 16.0,
                    bl: 16.0,
                },
            },
            Brush::Solid(white_fill),
            2,
        );

        // Add a subtle border stroke
        let border_color = ColorLinPremul::from_srgba(255, 255, 255, 0.08);
        painter.stroke_rounded_rect(
            RoundedRect {
                rect: Rect {
                    x,
                    y,
                    w: rect_width,
                    h: rect_height,
                },
                radii: RoundedRadii {
                    tl: 16.0,
                    tr: 16.0,
                    br: 16.0,
                    bl: 16.0,
                },
            },
            engine_core::Stroke { width: 2.0 },
            Brush::Solid(border_color),
            3,
        );

        // Optional hover highlight overlay (simple stroke on top)
        if let Some(ref shape) = self.hovered {
            let highlight = Brush::Solid(ColorLinPremul::from_srgba(0, 255, 255, 0.85));
            match shape {
                HitShape::Rect(r) => {
                    painter.stroke_rect(*r, Stroke { width: 2.0 }, highlight.clone(), 10);
                }
                HitShape::RoundedRect(rr) => {
                    painter.stroke_rounded_rect(*rr, Stroke { width: 2.0 }, highlight.clone(), 10);
                }
                HitShape::StrokeRect { rect, .. } => {
                    painter.stroke_rect(*rect, Stroke { width: 2.0 }, highlight.clone(), 10);
                }
                HitShape::StrokeRoundedRect { rrect, .. } => {
                    painter.stroke_rounded_rect(*rrect, Stroke { width: 2.0 }, highlight.clone(), 10);
                }
                HitShape::Ellipse { center, radii } => {
                    painter.ellipse(*center, *radii, Brush::Solid(ColorLinPremul::from_srgba(0, 255, 255, 0.12)), 10);
                }
                HitShape::Text | HitShape::BoxShadow { .. } => {}
            }
        }

        painter.finish()
    }
}

impl Scene for CenteredRectScene {
    fn kind(&self) -> SceneKind {
        SceneKind::Geometry
    }

    fn init_display_list(&mut self, viewport: Viewport) -> Option<DisplayList> {
        self.viewport = viewport;
        Some(self.build_display_list())
    }

    fn on_resize(&mut self, viewport: Viewport) -> Option<DisplayList> {
        self.viewport = viewport;
        Some(self.build_display_list())
    }

    fn paint_root_background(
        &self,
        passes: &mut PassManager,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        _queue: &wgpu::Queue,
        _width: u32,
        _height: u32,
    ) {
        // Background color #0b1220
        passes.paint_root_color(
            encoder,
            surface_view,
            ColorLinPremul::from_srgba_u8([0x0b, 0x12, 0x20, 0xff]),
            _queue,
        );
    }

    fn on_pointer_move(&mut self, _pos: [f32; 2], hit: Option<&HitResult>) -> Option<DisplayList> {
        let new_hover = hit.map(|h| h.shape.clone());
        if new_hover != self.hovered {
            self.hovered = new_hover;
            return Some(self.build_display_list());
        }
        None
    }

    fn on_pointer_down(&mut self, _pos: [f32; 2], _hit: Option<&HitResult>) -> Option<DisplayList> { None }
    fn on_pointer_up(&mut self, _pos: [f32; 2], _hit: Option<&HitResult>) -> Option<DisplayList> { None }
    fn on_click(&mut self, _pos: [f32; 2], _hit: Option<&HitResult>) -> Option<DisplayList> { None }
    fn on_drag(&mut self, _pos: [f32; 2], _hit: Option<&HitResult>) -> Option<DisplayList> { None }
}
