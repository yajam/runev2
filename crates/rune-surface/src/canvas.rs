use std::sync::Arc;

use engine_core::{
    Viewport,
    Painter,
    Brush, ColorLinPremul, Path, Rect, Stroke, TextRun, Transform2D,
    RasterizedGlyph, TextProvider,
};

/// Builder for a single frame’s draw commands. Wraps `Painter` and adds canvas helpers.
pub struct Canvas {
    pub(crate) viewport: Viewport,
    pub(crate) painter: Painter,
    pub(crate) clear_color: Option<ColorLinPremul>,
    pub(crate) text_provider: Option<Arc<dyn TextProvider + Send + Sync>>, // optional high-level text shaper
    pub(crate) glyph_draws: Vec<([f32; 2], RasterizedGlyph, ColorLinPremul)>, // low-level glyph masks
}

impl Canvas {
    pub fn viewport(&self) -> Viewport { self.viewport }

    /// Set the frame clear/background color (premultiplied linear RGBA).
    pub fn clear(&mut self, color: ColorLinPremul) { self.clear_color = Some(color); }

    /// Fill a rectangle with a brush.
    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, brush: Brush, z: i32) {
        self.painter.rect(Rect { x, y, w, h }, brush, z);
    }

    /// Stroke a path with uniform width and solid color.
    pub fn stroke_path(&mut self, path: Path, width: f32, color: ColorLinPremul, z: i32) {
        self.painter.stroke_path(path, Stroke { width }, color, z);
    }

    /// Push a high-level text run into the display list. Requires a text provider at end_frame.
    pub fn draw_text_run(&mut self, origin: [f32; 2], text: String, size_px: f32, color: ColorLinPremul, z: i32) {
        self.painter.text(TextRun { text, pos: origin, size: size_px, color }, z);
    }

    /// Provide a text provider used for high-level text runs in this frame.
    pub fn set_text_provider(&mut self, provider: Arc<dyn TextProvider + Send + Sync>) {
        self.text_provider = Some(provider);
    }

    /// Draw pre-rasterized glyph masks at the given origin tinted with the color.
    pub fn draw_text_glyphs(&mut self, origin: [f32; 2], glyphs: &[RasterizedGlyph], color: ColorLinPremul, z: i32) {
        let _ = z; // z currently not used for low-level masks; they are composited after solids
        for g in glyphs.iter().cloned() {
            self.glyph_draws.push((origin, g, color));
        }
    }

    // Expose some painter helpers for advanced users
    pub fn push_clip_rect(&mut self, rect: Rect) { self.painter.push_clip_rect(rect); }
    pub fn pop_clip(&mut self) { self.painter.pop_clip(); }
    pub fn push_transform(&mut self, t: Transform2D) { self.painter.push_transform(t); }
    pub fn pop_transform(&mut self) { self.painter.pop_transform(); }
}
