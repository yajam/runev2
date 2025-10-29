use engine_core::{DisplayList, Painter, PassManager, Viewport};
use engine_core::{Brush, ColorLinPremul, Rect};

use super::{Scene, SceneKind};

pub struct CenteredRectScene {
    viewport: Viewport,
}

impl Default for CenteredRectScene {
    fn default() -> Self {
        Self {
            viewport: Viewport { width: 800, height: 600 },
        }
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
        );
    }
}

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

        // First, draw background as a full-screen rectangle
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

        // White with 4% alpha
        let white_alpha = ColorLinPremul::from_srgba_u8([255, 255, 255, 10]);

        painter.rect(
            Rect {
                x,
                y,
                w: rect_width,
                h: rect_height,
            },
            Brush::Solid(white_alpha),
            2, // Higher z to render on top
        );

        painter.finish()
    }
}
