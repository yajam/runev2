use std::sync::Arc;

use engine_core::{
    Brush, ColorLinPremul, Painter, Path, RasterizedGlyph, Rect, RoundedRadii, RoundedRect, Stroke,
    TextProvider, TextRun, Transform2D, Viewport,
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
    pub(crate) glyph_draws: Vec<([f32; 2], RasterizedGlyph, ColorLinPremul, i32)>, // low-level glyph masks with z-index
    pub(crate) svg_draws: Vec<(
        std::path::PathBuf,
        [f32; 2],
        [f32; 2],
        Option<engine_core::SvgStyle>,
        i32,
        Transform2D,
    )>, // (path, origin, max_size, style, z, transform)
    pub(crate) image_draws: Vec<(
        std::path::PathBuf,
        [f32; 2],
        [f32; 2],
        ImageFitMode,
        i32,
        Transform2D,
    )>, // (path, origin, size, fit, z, transform)
    /// Raw pixel data draws: (pixels_rgba, src_width, src_height, origin, dst_size, z, transform)
    pub(crate) raw_image_draws: Vec<RawImageDraw>,
    pub(crate) dpi_scale: f32, // DPI scale factor for text rendering
    // Effective clip stack in device coordinates for direct text rendering.
    // Each entry is the intersection of all active clips at that depth.
    pub(crate) clip_stack: Vec<Option<Rect>>,
    // Overlay rectangles that render without depth testing (for modal scrims).
    // These are rendered in a separate pass after the main scene.
    pub(crate) overlay_draws: Vec<(Rect, ColorLinPremul)>,
    // Scrim draws that blend over content but allow z-ordered content to render on top.
    // Supports either a full-rect scrim or a scrim with a rounded-rect cutout via stencil.
    pub(crate) scrim_draws: Vec<ScrimDraw>,
}

/// Scrim drawing modes.
#[derive(Clone, Copy)]
pub enum ScrimDraw {
    Rect(Rect, ColorLinPremul),
    Cutout {
        hole: RoundedRect,
        color: ColorLinPremul,
    },
}

/// Raw image draw request for rendering pixel data directly.
#[derive(Clone)]
pub struct RawImageDraw {
    /// BGRA pixel data (4 bytes per pixel) - matches CEF native format
    pub pixels: Vec<u8>,
    /// Source image width
    pub src_width: u32,
    /// Source image height
    pub src_height: u32,
    /// Destination origin in scene coordinates
    pub origin: [f32; 2],
    /// Destination size in scene coordinates
    pub dst_size: [f32; 2],
    /// Z-index for depth ordering
    pub z: i32,
    /// Transform at draw time
    pub transform: Transform2D,
    /// Dirty rectangles for partial update (x, y, w, h) - empty = full frame
    pub dirty_rects: Vec<(u32, u32, u32, u32)>,
}

impl Canvas {
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// Get the current transform from the painter's transform stack.
    pub fn current_transform(&self) -> Transform2D {
        self.painter.current_transform()
    }

    /// Set the frame clear/background color (premultiplied linear RGBA).
    pub fn clear(&mut self, color: ColorLinPremul) {
        self.clear_color = Some(color);
    }

    /// Fill a rectangle with a brush.
    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, brush: Brush, z: i32) {
        self.painter.rect(Rect { x, y, w, h }, brush, z);
    }

    /// Fill a rectangle as an overlay (no depth testing).
    /// Use this for modal scrims and other overlays that should blend over
    /// existing content without blocking text rendered at lower z-indices.
    ///
    /// The rectangle coordinates are transformed by the current canvas transform,
    /// so they should be in local (viewport) coordinates.
    pub fn fill_overlay_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: ColorLinPremul) {
        // Apply current transform to get screen coordinates.
        // Transform all four corners and compute axis-aligned bounding box.
        let t = self.painter.current_transform();
        let [a, b, c, d, e, f] = t.m;

        // Transform corner points
        let p0 = [a * x + c * y + e, b * x + d * y + f];
        let p1 = [a * (x + w) + c * y + e, b * (x + w) + d * y + f];
        let p2 = [a * (x + w) + c * (y + h) + e, b * (x + w) + d * (y + h) + f];
        let p3 = [a * x + c * (y + h) + e, b * x + d * (y + h) + f];

        // For axis-aligned transforms (translation/scale only), the AABB works.
        // For rotation, this is an approximation but should be fine for scrims.
        let min_x = p0[0].min(p1[0]).min(p2[0]).min(p3[0]);
        let max_x = p0[0].max(p1[0]).max(p2[0]).max(p3[0]);
        let min_y = p0[1].min(p1[1]).min(p2[1]).min(p3[1]);
        let max_y = p0[1].max(p1[1]).max(p2[1]).max(p3[1]);

        self.overlay_draws.push((
            Rect {
                x: min_x,
                y: min_y,
                w: max_x - min_x,
                h: max_y - min_y,
            },
            color,
        ));
    }

    /// Fill a rectangle as a scrim (blends over all existing content but allows
    /// subsequent z-ordered draws to render on top).
    ///
    /// Unlike `fill_overlay_rect`, this uses a depth buffer attachment with:
    /// - depth_compare = Always (always passes depth test)
    /// - depth_write_enabled = false (doesn't affect depth buffer)
    ///
    /// This allows the scrim to dim background content while the modal panel
    /// (rendered at a higher z-index afterward) renders cleanly on top.
    pub fn fill_scrim_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: ColorLinPremul) {
        // Apply current transform to get screen coordinates.
        let t = self.painter.current_transform();
        let [a, b, c, d, e, f] = t.m;

        // Transform corner points
        let p0 = [a * x + c * y + e, b * x + d * y + f];
        let p1 = [a * (x + w) + c * y + e, b * (x + w) + d * y + f];
        let p2 = [a * (x + w) + c * (y + h) + e, b * (x + w) + d * (y + h) + f];
        let p3 = [a * x + c * (y + h) + e, b * x + d * (y + h) + f];

        // Compute axis-aligned bounding box
        let min_x = p0[0].min(p1[0]).min(p2[0]).min(p3[0]);
        let max_x = p0[0].max(p1[0]).max(p2[0]).max(p3[0]);
        let min_y = p0[1].min(p1[1]).min(p2[1]).min(p3[1]);
        let max_y = p0[1].max(p1[1]).max(p2[1]).max(p3[1]);

        self.scrim_draws.push(ScrimDraw::Rect(
            Rect {
                x: min_x,
                y: min_y,
                w: max_x - min_x,
                h: max_y - min_y,
            },
            color,
        ));
    }

    /// Fill a fullscreen scrim that leaves a rounded-rect hole using stencil.
    pub fn fill_scrim_with_cutout(&mut self, hole: RoundedRect, color: ColorLinPremul) {
        // Transform the hole into screen space using the current canvas transform.
        // Assumes transform is affine (translation/scale/skew); uses AABB to keep it simple.
        let t = self.painter.current_transform();
        let [a, b, c, d, e, f] = t.m;

        let rect = hole.rect;
        let corners = [
            [rect.x, rect.y],
            [rect.x + rect.w, rect.y],
            [rect.x + rect.w, rect.y + rect.h],
            [rect.x, rect.y + rect.h],
        ];

        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;

        for p in corners {
            let tx = a * p[0] + c * p[1] + e;
            let ty = b * p[0] + d * p[1] + f;
            min_x = min_x.min(tx);
            max_x = max_x.max(tx);
            min_y = min_y.min(ty);
            max_y = max_y.max(ty);
        }

        // Approximate radius scaling by average scale of the transform axes.
        let sx = (a * a + b * b).sqrt();
        let sy = (c * c + d * d).sqrt();
        let scale = if sx.is_finite() && sy.is_finite() && sx > 0.0 && sy > 0.0 {
            (sx + sy) * 0.5
        } else {
            1.0
        };

        let transformed = RoundedRect {
            rect: Rect {
                x: min_x,
                y: min_y,
                w: (max_x - min_x).max(0.0),
                h: (max_y - min_y).max(0.0),
            },
            radii: RoundedRadii {
                tl: hole.radii.tl * scale,
                tr: hole.radii.tr * scale,
                br: hole.radii.br * scale,
                bl: hole.radii.bl * scale,
            },
        };

        self.scrim_draws.push(ScrimDraw::Cutout {
            hole: transformed,
            color,
        });
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
        self.painter
            .stroke_rounded_rect(rrect, Stroke { width }, brush, z);
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
    pub fn draw_text_run(
        &mut self,
        origin: [f32; 2],
        text: String,
        size_px: f32,
        color: ColorLinPremul,
        z: i32,
    ) {
        // If we have a provider, rasterize immediately (simple, reliable)
        if let Some(ref provider) = self.text_provider {
            // Apply current transform to origin (handles zone positioning)
            let transform = self.painter.current_transform();
            let [a, b, c, d, e, f] = transform.m;
            let transformed_origin = [
                a * origin[0] + c * origin[1] + e,
                b * origin[0] + d * origin[1] + f,
            ];

            // DPI scale for converting logical to device coordinates
            let sf = if self.dpi_scale.is_finite() && self.dpi_scale > 0.0 {
                self.dpi_scale
            } else {
                1.0
            };

            // Rasterize at logical size - PassManager handles DPI scaling when rendering
            let run = TextRun {
                text,
                pos: [0.0, 0.0],
                size: size_px,
                color,
            };

            // Rasterize glyphs, using a shared cache to avoid
            // re-rasterizing identical text every frame.
            let glyphs = engine_core::rasterize_run_cached(provider.as_ref(), &run);
            // Current effective clip rect in device coordinates, if any.
            let current_clip = self.clip_stack.last().cloned().unwrap_or(None);

            let snap = |v: f32| -> f32 { (v * sf).round() / sf };

            for g in glyphs.iter() {
                // Glyph offsets are in logical pixels (from rasterization at logical size)
                // Store origins in LOGICAL PIXELS for local coordinate system
                let mut glyph_origin_logical = [
                    transformed_origin[0] + g.offset[0],
                    transformed_origin[1] + g.offset[1],
                ];

                // Snap small text to device pixels to avoid shimmering edges
                if size_px <= 15.0 {
                    glyph_origin_logical[0] = snap(glyph_origin_logical[0]);
                    glyph_origin_logical[1] = snap(glyph_origin_logical[1]);
                }

                // For clipping, convert to device pixels
                let glyph_origin_device =
                    [glyph_origin_logical[0] * sf, glyph_origin_logical[1] * sf];

                if let Some(clip) = current_clip {
                    // Clip glyph to the current rect in device coordinates.
                    if let Some((clipped_mask, clipped_origin_device)) =
                        clip_glyph_to_rect(&g.mask, glyph_origin_device, clip)
                    {
                        let clipped = RasterizedGlyph {
                            offset: [0.0, 0.0],
                            mask: clipped_mask,
                        };
                        // Convert clipped origin back to logical coordinates
                        let mut clipped_origin_logical =
                            [clipped_origin_device[0] / sf, clipped_origin_device[1] / sf];
                        if size_px <= 15.0 {
                            clipped_origin_logical[0] = snap(clipped_origin_logical[0]);
                            clipped_origin_logical[1] = snap(clipped_origin_logical[1]);
                        }
                        self.glyph_draws
                            .push((clipped_origin_logical, clipped, color, z));
                    }
                } else {
                    self.glyph_draws
                        .push((glyph_origin_logical, g.clone(), color, z));
                }
            }
        } else {
            // Fallback: use display list path (complex, but kept for compatibility)
            self.painter.text(
                TextRun {
                    text,
                    pos: origin,
                    size: size_px,
                    color,
                },
                z,
            );
        }
    }

    /// Draw text directly by rasterizing immediately (simpler, bypasses display list).
    /// This is the recommended approach - it's simpler and more reliable than draw_text_run.
    pub fn draw_text_direct(
        &mut self,
        origin: [f32; 2],
        text: &str,
        size_px: f32,
        color: ColorLinPremul,
        provider: &dyn TextProvider,
        z: i32,
    ) {
        // Apply current transform to origin (handles zone positioning)
        let transform = self.painter.current_transform();
        let [a, b, c, d, e, f] = transform.m;
        let transformed_origin = [
            a * origin[0] + c * origin[1] + e,
            b * origin[0] + d * origin[1] + f,
        ];

        // Current effective clip rect in device coordinates, if any.
        let current_clip = self.clip_stack.last().cloned().unwrap_or(None);

        // DPI scale for converting logical to device coordinates
        let sf = if self.dpi_scale.is_finite() && self.dpi_scale > 0.0 {
            self.dpi_scale
        } else {
            1.0
        };

        // Rasterize at logical size - PassManager handles DPI scaling when rendering
        let run = TextRun {
            text: text.to_string(),
            pos: [0.0, 0.0],
            size: size_px,
            color,
        };

        // Rasterize glyphs, using the shared cache to avoid
        // re-rasterizing identical text every frame.
        let glyphs = engine_core::rasterize_run_cached(provider, &run);

        let snap = |v: f32| -> f32 { (v * sf).round() / sf };

        for g in glyphs.iter() {
            // Glyph offsets are in logical pixels (from rasterization at logical size)
            // Store origins in LOGICAL PIXELS for local coordinate system
            let mut glyph_origin_logical = [
                transformed_origin[0] + g.offset[0],
                transformed_origin[1] + g.offset[1],
            ];

            if size_px <= 15.0 {
                glyph_origin_logical[0] = snap(glyph_origin_logical[0]);
                glyph_origin_logical[1] = snap(glyph_origin_logical[1]);
            }

            // For clipping, convert to device pixels
            let glyph_origin_device = [glyph_origin_logical[0] * sf, glyph_origin_logical[1] * sf];

            if let Some(clip) = current_clip {
                if let Some((clipped_mask, clipped_origin_device)) =
                    clip_glyph_to_rect(&g.mask, glyph_origin_device, clip)
                {
                    let clipped = RasterizedGlyph {
                        offset: [0.0, 0.0],
                        mask: clipped_mask,
                    };
                    // Convert clipped origin back to logical coordinates
                    let mut clipped_origin_logical =
                        [clipped_origin_device[0] / sf, clipped_origin_device[1] / sf];
                    if size_px <= 15.0 {
                        clipped_origin_logical[0] = snap(clipped_origin_logical[0]);
                        clipped_origin_logical[1] = snap(clipped_origin_logical[1]);
                    }
                    self.glyph_draws
                        .push((clipped_origin_logical, clipped, color, z));
                }
            } else {
                self.glyph_draws
                    .push((glyph_origin_logical, g.clone(), color, z));
            }
        }
    }

    /// Provide a text provider used for high-level text runs in this frame.
    pub fn set_text_provider(&mut self, provider: Arc<dyn TextProvider + Send + Sync>) {
        self.text_provider = Some(provider);
    }

    /// Draw pre-rasterized glyph masks at the given origin tinted with the color.
    pub fn draw_text_glyphs(
        &mut self,
        origin: [f32; 2],
        glyphs: &[RasterizedGlyph],
        color: ColorLinPremul,
        z: i32,
    ) {
        for g in glyphs.iter().cloned() {
            self.glyph_draws.push((origin, g, color, z));
        }
    }

    /// Draw a hyperlink with text, optional underline, and URL target.
    ///
    /// # Example
    /// ```no_run
    /// # use rune_surface::Canvas;
    /// # use engine_core::{ColorLinPremul, Hyperlink};
    /// # let mut canvas: Canvas = todo!();
    /// let link = Hyperlink {
    ///     text: "Click me".to_string(),
    ///     pos: [10.0, 20.0],
    ///     size: 16.0,
    ///     color: ColorLinPremul::from_srgba_u8([0, 122, 255, 255]),
    ///     url: "https://example.com".to_string(),
    ///     underline: true,
    ///     underline_color: None,
    /// };
    /// canvas.draw_hyperlink(link, 10);
    /// ```
    pub fn draw_hyperlink(&mut self, hyperlink: engine_core::Hyperlink, z: i32) {
        self.painter.hyperlink(hyperlink, z);
    }

    /// Queue an SVG to be rasterized and drawn at origin, scaled to fit within max_size.
    /// Captures the current transform from the painter's transform stack.
    /// Optional style parameter allows overriding fill, stroke, and stroke-width.
    pub fn draw_svg<P: Into<std::path::PathBuf>>(
        &mut self,
        path: P,
        origin: [f32; 2],
        max_size: [f32; 2],
        z: i32,
    ) {
        let transform = self.painter.current_transform();
        self.svg_draws
            .push((path.into(), origin, max_size, None, z, transform));
    }

    /// Queue an SVG with style overrides to be rasterized and drawn.
    pub fn draw_svg_styled<P: Into<std::path::PathBuf>>(
        &mut self,
        path: P,
        origin: [f32; 2],
        max_size: [f32; 2],
        style: engine_core::SvgStyle,
        z: i32,
    ) {
        let path_buf = path.into();
        let transform = self.painter.current_transform();
        self.svg_draws
            .push((path_buf, origin, max_size, Some(style), z, transform));
    }

    /// Queue a raster image (PNG/JPEG/GIF/WebP) to be drawn at origin with the given size.
    /// The fit parameter controls how the image is scaled within the size bounds.
    /// Captures the current transform from the painter's transform stack.
    pub fn draw_image<P: Into<std::path::PathBuf>>(
        &mut self,
        path: P,
        origin: [f32; 2],
        size: [f32; 2],
        fit: ImageFitMode,
        z: i32,
    ) {
        let transform = self.painter.current_transform();
        self.image_draws
            .push((path.into(), origin, size, fit, z, transform));
    }

    /// Queue raw pixel data to be drawn at origin with the given size.
    /// Pixels should be in BGRA format (4 bytes per pixel) to match CEF native output.
    /// Captures the current transform from the painter's transform stack.
    pub fn draw_raw_image(
        &mut self,
        pixels: Vec<u8>,
        src_width: u32,
        src_height: u32,
        origin: [f32; 2],
        dst_size: [f32; 2],
        z: i32,
    ) {
        let transform = self.painter.current_transform();
        self.raw_image_draws.push(RawImageDraw {
            pixels,
            src_width,
            src_height,
            origin,
            dst_size,
            z,
            transform,
            dirty_rects: Vec::new(), // Full frame update
        });
    }

    /// Queue raw pixel data with dirty rects for partial update.
    /// Pixels should be in BGRA format (4 bytes per pixel) to match CEF native output.
    /// Only the dirty rectangles will be uploaded to the GPU texture.
    pub fn draw_raw_image_with_dirty_rects(
        &mut self,
        pixels: Vec<u8>,
        src_width: u32,
        src_height: u32,
        origin: [f32; 2],
        dst_size: [f32; 2],
        z: i32,
        dirty_rects: Vec<(u32, u32, u32, u32)>,
    ) {
        let transform = self.painter.current_transform();
        self.raw_image_draws.push(RawImageDraw {
            pixels,
            src_width,
            src_height,
            origin,
            dst_size,
            z,
            transform,
            dirty_rects,
        });
    }

    // Expose some painter helpers for advanced users
    pub fn push_clip_rect(&mut self, rect: Rect) {
        // Forward to Painter to keep display list behavior.
        self.painter.push_clip_rect(rect);

        // Compute device-space clip rect based on current transform and dpi.
        let t = self.painter.current_transform();
        let [a, b, c, d, e, f] = t.m;

        let x0 = rect.x;
        let y0 = rect.y;
        let x1 = rect.x + rect.w;
        let y1 = rect.y + rect.h;

        let p0 = [a * x0 + c * y0 + e, b * x0 + d * y0 + f];
        let p1 = [a * x1 + c * y0 + e, b * x1 + d * y0 + f];
        let p2 = [a * x0 + c * y1 + e, b * x0 + d * y1 + f];
        let p3 = [a * x1 + c * y1 + e, b * x1 + d * y1 + f];

        let min_x = p0[0].min(p1[0]).min(p2[0]).min(p3[0]) * self.dpi_scale;
        let max_x = p0[0].max(p1[0]).max(p2[0]).max(p3[0]) * self.dpi_scale;
        let min_y = p0[1].min(p1[1]).min(p2[1]).min(p3[1]) * self.dpi_scale;
        let max_y = p0[1].max(p1[1]).max(p2[1]).max(p3[1]) * self.dpi_scale;

        let new_clip = Rect {
            x: min_x,
            y: min_y,
            w: (max_x - min_x).max(0.0),
            h: (max_y - min_y).max(0.0),
        };

        let merged = match self.clip_stack.last().cloned().unwrap_or(None) {
            None => Some(new_clip),
            Some(prev) => intersect_rect(prev, new_clip),
        };
        self.clip_stack.push(merged);
    }

    pub fn pop_clip(&mut self) {
        self.painter.pop_clip();
        if self.clip_stack.len() > 1 {
            self.clip_stack.pop();
        }
    }
    pub fn push_transform(&mut self, t: Transform2D) {
        self.painter.push_transform(t);
    }
    pub fn pop_transform(&mut self) {
        self.painter.pop_transform();
    }

    /// Add a hit-only region (invisible, used for interaction detection)
    pub fn hit_region_rect(&mut self, id: u32, rect: Rect, z: i32) {
        self.painter.hit_region_rect(id, rect, z);
    }

    /// Get a reference to the display list for hit testing
    pub fn display_list(&self) -> &engine_core::DisplayList {
        self.painter.display_list()
    }
}

/// Intersect two rectangles (device-space); returns None if they do not overlap.
fn intersect_rect(a: Rect, b: Rect) -> Option<Rect> {
    let ax1 = a.x + a.w;
    let ay1 = a.y + a.h;
    let bx1 = b.x + b.w;
    let by1 = b.y + b.h;

    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = ax1.min(bx1);
    let y1 = ay1.min(by1);

    if x1 <= x0 || y1 <= y0 {
        None
    } else {
        Some(Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        })
    }
}

/// Clip a glyph mask to a device-space rectangle, returning a new mask and origin.
fn clip_glyph_to_rect(
    mask: &engine_core::SubpixelMask,
    origin: [f32; 2],
    clip: Rect,
) -> Option<(engine_core::SubpixelMask, [f32; 2])> {
    let glyph_x0 = origin[0];
    let glyph_y0 = origin[1];
    let glyph_x1 = glyph_x0 + mask.width as f32;
    let glyph_y1 = glyph_y0 + mask.height as f32;

    let clip_x0 = clip.x;
    let clip_y0 = clip.y;
    let clip_x1 = clip.x + clip.w;
    let clip_y1 = clip.y + clip.h;

    let ix0 = glyph_x0.max(clip_x0);
    let iy0 = glyph_y0.max(clip_y0);
    let ix1 = glyph_x1.min(clip_x1);
    let iy1 = glyph_y1.min(clip_y1);

    if ix0 >= ix1 || iy0 >= iy1 {
        return None;
    }

    let bpp = mask.bytes_per_pixel() as u32;

    // Convert intersection to pixel indices within the glyph mask.
    let start_x = ((ix0 - glyph_x0).floor().max(0.0)) as u32;
    let start_y = ((iy0 - glyph_y0).floor().max(0.0)) as u32;
    let end_x = ((ix1 - glyph_x0).ceil().min(mask.width as f32)) as u32;
    let end_y = ((iy1 - glyph_y0).ceil().min(mask.height as f32)) as u32;

    if end_x <= start_x || end_y <= start_y {
        return None;
    }

    let new_w = end_x - start_x;
    let new_h = end_y - start_y;

    let src_stride = mask.width * bpp;
    let dst_stride = new_w * bpp;
    let mut data = vec![0u8; (new_w * new_h * bpp) as usize];

    for row in 0..new_h {
        let src_y = start_y + row;
        let src_offset = (src_y * src_stride + start_x * bpp) as usize;
        let dst_offset = (row * dst_stride) as usize;
        data[dst_offset..dst_offset + dst_stride as usize]
            .copy_from_slice(&mask.data[src_offset..src_offset + dst_stride as usize]);
    }

    let clipped = engine_core::SubpixelMask {
        width: new_w,
        height: new_h,
        format: mask.format,
        data,
    };

    let new_origin = [glyph_x0 + start_x as f32, glyph_y0 + start_y as f32];
    Some((clipped, new_origin))
}
