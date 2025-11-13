use std::sync::Arc;

use engine_core::{
    Viewport,
    Painter,
    Brush, ColorLinPremul, Path, Rect, Stroke, TextRun, Transform2D, RoundedRect,
    RasterizedGlyph, TextProvider,
};

/// Builder for a single frame’s draw commands. Wraps `Painter` and adds canvas helpers.
pub struct Canvas {
    pub(crate) viewport: Viewport,
    pub(crate) painter: Painter,
    pub(crate) clear_color: Option<ColorLinPremul>,
    pub(crate) text_provider: Option<Arc<dyn TextProvider + Send + Sync>>, // optional high-level text shaper
    pub(crate) glyph_draws: Vec<([f32; 2], RasterizedGlyph, ColorLinPremul)>, // low-level glyph masks
    pub(crate) svg_draws: Vec<(std::path::PathBuf, [f32; 2], [f32; 2], i32)>, // (path, origin, max_size, z)
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

    /// Fill a path with a solid color.
    pub fn fill_path(&mut self, path: Path, color: ColorLinPremul, z: i32) {
        self.painter.fill_path(path, color, z);
    }

    /// Draw an ellipse (y-down coordinates).
    pub fn ellipse(&mut self, center: [f32; 2], radii: [f32; 2], brush: Brush, z: i32) {
        self.painter.ellipse(center, radii, brush, z);
    }

    /// Draw a circle (y-down coordinates).
    pub fn circle(&mut self, center: [f32; 2], radius: f32, brush: Brush, z: i32) {
        self.painter.circle(center, radius, brush, z);
    }

    /// Draw a rounded rectangle fill.
    pub fn rounded_rect(&mut self, rrect: RoundedRect, brush: Brush, z: i32) {
        self.painter.rounded_rect(rrect, brush, z);
    }

    /// Stroke a rounded rectangle.
    pub fn stroke_rounded_rect(&mut self, rrect: RoundedRect, width: f32, brush: Brush, z: i32) {
        self.painter.stroke_rounded_rect(rrect, Stroke { width }, brush, z);
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

    /// Queue an SVG to be rasterized and drawn at origin, scaled to fit within max_size.
    pub fn draw_svg<P: Into<std::path::PathBuf>>(&mut self, path: P, origin: [f32; 2], max_size: [f32; 2], z: i32) {
        self.svg_draws.push((path.into(), origin, max_size, z));
    }

    // Expose some painter helpers for advanced users
    pub fn push_clip_rect(&mut self, rect: Rect) { self.painter.push_clip_rect(rect); }
    pub fn pop_clip(&mut self) { self.painter.pop_clip(); }
    pub fn push_transform(&mut self, t: Transform2D) { self.painter.push_transform(t); }
    pub fn pop_transform(&mut self) { self.painter.pop_transform(); }
}
