use std::sync::Arc;

use engine_core::{
    Viewport,
    Painter,
    Brush, ColorLinPremul, Path, Rect, Stroke, TextRun, Transform2D, RoundedRect,
    RasterizedGlyph, TextProvider,
};

/// How an image should fit within its bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFitMode {
    /// Stretch to fill (may distort aspect ratio)
    Fill,
    /// Fit inside maintaining aspect ratio (letterbox/pillarbox)
    Contain,
    /// Fill maintaining aspect ratio (may crop edges)
    Cover,
}

impl Default for ImageFitMode {
    fn default() -> Self {
        Self::Contain
    }
}

/// Builder for a single frame’s draw commands. Wraps `Painter` and adds canvas helpers.
pub struct Canvas {
    pub(crate) viewport: Viewport,
    pub(crate) painter: Painter,
    pub(crate) clear_color: Option<ColorLinPremul>,
    pub(crate) text_provider: Option<Arc<dyn TextProvider + Send + Sync>>, // optional high-level text shaper
    pub(crate) glyph_draws: Vec<([f32; 2], RasterizedGlyph, ColorLinPremul)>, // low-level glyph masks
    pub(crate) svg_draws: Vec<(std::path::PathBuf, [f32; 2], [f32; 2], Option<engine_core::SvgStyle>, i32, Transform2D)>, // (path, origin, max_size, style, z, transform)
    pub(crate) image_draws: Vec<(std::path::PathBuf, [f32; 2], [f32; 2], ImageFitMode, i32, Transform2D)>, // (path, origin, size, fit, z, transform)
    pub(crate) dpi_scale: f32, // DPI scale factor for text rendering
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

    /// Draw text using direct rasterization (recommended).
    ///
    /// This method rasterizes glyphs immediately using the text provider,
    /// bypassing complex display list paths. This is simpler and more
    /// reliable than deferred rendering.
    ///
    /// # Performance
    /// - Glyphs are shaped and rasterized on each call
    /// - Use [`TextLayoutCache`](engine_core::TextLayoutCache) to cache wrapping computations
    /// - Debounce resize events to avoid excessive rasterization
    ///
    /// # Transform Stack
    /// The current transform is applied to position text correctly
    /// within zones (viewport, toolbar, etc.).
    ///
    /// # DPI Scaling
    /// Both position and size are automatically scaled by `self.dpi_scale`.
    ///
    /// # Example
    /// ```no_run
    /// # use rune_surface::Canvas;
    /// # use engine_core::ColorLinPremul;
    /// # let mut canvas: Canvas = todo!();
    /// canvas.draw_text_run(
    ///     [10.0, 20.0],
    ///     "Hello, world!".to_string(),
    ///     16.0,
    ///     ColorLinPremul::rgba(255, 255, 255, 255),
    ///     10,  // z-index
    /// );
    /// ```
    pub fn draw_text_run(&mut self, origin: [f32; 2], text: String, size_px: f32, color: ColorLinPremul, z: i32) {
        let _ = z; // z-ordering not used for direct glyph rendering
        
        // If we have a provider, rasterize immediately (simple, reliable)
        if let Some(ref provider) = self.text_provider {
            // Apply current transform to origin (handles zone positioning)
            let transform = self.painter.current_transform();
            let [a, b, c, d, e, f] = transform.m;
            let transformed_origin = [
                a * origin[0] + c * origin[1] + e,
                b * origin[0] + d * origin[1] + f,
            ];
            
            // Apply DPI scaling to both size and position
            let scaled_size = size_px * self.dpi_scale;
            let scaled_origin = [
                transformed_origin[0] * self.dpi_scale,
                transformed_origin[1] * self.dpi_scale,
            ];
            
            let run = TextRun {
                text,
                pos: [0.0, 0.0],
                size: scaled_size,
                color,
            };
            
            // Rasterize glyphs immediately
            for g in provider.rasterize_run(&run) {
                let glyph_origin = [scaled_origin[0] + g.offset[0], scaled_origin[1] + g.offset[1]];
                self.glyph_draws.push((glyph_origin, g, color));
            }
        } else {
            // Fallback: use display list path (complex, but kept for compatibility)
            self.painter.text(TextRun { text, pos: origin, size: size_px, color }, z);
        }
    }

    /// Draw text directly by rasterizing immediately (simpler, bypasses display list).
    /// This is the recommended approach - it's simpler and more reliable than draw_text_run.
    pub fn draw_text_direct(&mut self, origin: [f32; 2], text: &str, size_px: f32, color: ColorLinPremul, provider: &dyn TextProvider) {
        // Apply current transform to origin (handles zone positioning)
        let transform = self.painter.current_transform();
        let [a, b, c, d, e, f] = transform.m;
        let transformed_origin = [
            a * origin[0] + c * origin[1] + e,
            b * origin[0] + d * origin[1] + f,
        ];
        
        // Apply DPI scaling to both size and position
        let scaled_size = size_px * self.dpi_scale;
        let scaled_origin = [
            transformed_origin[0] * self.dpi_scale,
            transformed_origin[1] * self.dpi_scale,
        ];
        
        let run = TextRun {
            text: text.to_string(),
            pos: [0.0, 0.0],
            size: scaled_size,
            color,
        };
        
        // Rasterize glyphs immediately
        for g in provider.rasterize_run(&run) {
            let glyph_origin = [scaled_origin[0] + g.offset[0], scaled_origin[1] + g.offset[1]];
            self.glyph_draws.push((glyph_origin, g, color));
        }
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
    /// Captures the current transform from the painter's transform stack.
    /// Optional style parameter allows overriding fill, stroke, and stroke-width.
    pub fn draw_svg<P: Into<std::path::PathBuf>>(&mut self, path: P, origin: [f32; 2], max_size: [f32; 2], z: i32) {
        let transform = self.painter.current_transform();
        self.svg_draws.push((path.into(), origin, max_size, None, z, transform));
    }

    /// Queue an SVG with style overrides to be rasterized and drawn.
    pub fn draw_svg_styled<P: Into<std::path::PathBuf>>(&mut self, path: P, origin: [f32; 2], max_size: [f32; 2], style: engine_core::SvgStyle, z: i32) {
        let transform = self.painter.current_transform();
        self.svg_draws.push((path.into(), origin, max_size, Some(style), z, transform));
    }

    /// Queue a raster image (PNG/JPEG/GIF/WebP) to be drawn at origin with the given size.
    /// The fit parameter controls how the image is scaled within the size bounds.
    /// Captures the current transform from the painter's transform stack.
    pub fn draw_image<P: Into<std::path::PathBuf>>(&mut self, path: P, origin: [f32; 2], size: [f32; 2], fit: ImageFitMode, z: i32) {
        let transform = self.painter.current_transform();
        self.image_draws.push((path.into(), origin, size, fit, z, transform));
    }

    // Expose some painter helpers for advanced users
    pub fn push_clip_rect(&mut self, rect: Rect) { self.painter.push_clip_rect(rect); }
    pub fn pop_clip(&mut self) { self.painter.pop_clip(); }
    pub fn push_transform(&mut self, t: Transform2D) { self.painter.push_transform(t); }
    pub fn pop_transform(&mut self) { self.painter.pop_transform(); }
    
    /// Add a hit-only region (invisible, used for interaction detection)
    pub fn hit_region_rect(&mut self, id: u32, rect: Rect, z: i32) {
        self.painter.hit_region_rect(id, rect, z);
    }
    
    /// Get a reference to the display list for hit testing
    pub fn display_list(&self) -> &engine_core::DisplayList {
        self.painter.display_list()
    }
}
