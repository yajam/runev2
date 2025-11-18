use std::sync::Arc;

use anyhow::Result;

use engine_core::{
    ColorLinPremul,
    Painter,
    PassManager,
    RenderAllocator,
    Transform2D,
    Viewport,
    wgpu, // import wgpu from engine-core to keep type identity
};

use crate::canvas::{Canvas, ImageFitMode};

/// Apply a 2D affine transform to a point
fn apply_transform_to_point(point: [f32; 2], transform: Transform2D) -> [f32; 2] {
    let [a, b, c, d, e, f] = transform.m;
    let x = point[0];
    let y = point[1];
    [a * x + c * y + e, b * x + d * y + f]
}

/// Overlay callback signature: called after main rendering with full PassManager access.
/// Allows scenes to draw overlays (like SVG ticks) directly to the surface.
pub type OverlayCallback = Box<
    dyn FnMut(
        &mut PassManager,
        &mut wgpu::CommandEncoder,
        &wgpu::TextureView,
        &wgpu::Queue,
        u32,
        u32,
    ),
>;

/// Calculate the actual render origin and size for an image based on fit mode.
/// Returns (origin, size) where the image should be drawn.
fn calculate_image_fit(
    origin: [f32; 2],
    bounds: [f32; 2],
    img_w: f32,
    img_h: f32,
    fit: ImageFitMode,
) -> ([f32; 2], [f32; 2]) {
    match fit {
        ImageFitMode::Fill => {
            // Stretch to fill - use bounds as-is
            (origin, bounds)
        }
        ImageFitMode::Contain => {
            // Fit inside maintaining aspect ratio
            let bounds_aspect = bounds[0] / bounds[1];
            let img_aspect = img_w / img_h;

            let (render_w, render_h) = if img_aspect > bounds_aspect {
                // Image is wider - fit to width
                (bounds[0], bounds[0] / img_aspect)
            } else {
                // Image is taller - fit to height
                (bounds[1] * img_aspect, bounds[1])
            };

            // Center within bounds
            let offset_x = (bounds[0] - render_w) * 0.5;
            let offset_y = (bounds[1] - render_h) * 0.5;

            (
                [origin[0] + offset_x, origin[1] + offset_y],
                [render_w, render_h],
            )
        }
        ImageFitMode::Cover => {
            // Fill maintaining aspect ratio (may crop)
            let bounds_aspect = bounds[0] / bounds[1];
            let img_aspect = img_w / img_h;

            let (render_w, render_h) = if img_aspect > bounds_aspect {
                // Image is wider - fit to height
                (bounds[1] * img_aspect, bounds[1])
            } else {
                // Image is taller - fit to width
                (bounds[0], bounds[0] / img_aspect)
            };

            // Center within bounds (will be clipped)
            let offset_x = (bounds[0] - render_w) * 0.5;
            let offset_y = (bounds[1] - render_h) * 0.5;

            (
                [origin[0] + offset_x, origin[1] + offset_y],
                [render_w, render_h],
            )
        }
    }
}

/// High-level canvas-style wrapper over Painter + PassManager.
///
/// Typical flow:
/// - let mut canvas = surface.begin_frame(w, h);
/// - canvas.clear(color);
/// - canvas.draw calls ...
/// - surface.end_frame(frame, canvas);
pub struct RuneSurface {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    pass: PassManager,
    allocator: RenderAllocator,
    /// When true, render directly to the surface; otherwise render offscreen then composite.
    direct: bool,
    /// When true, preserve existing surface content (LoadOp::Load) instead of clearing.
    preserve_surface: bool,
    /// When true, render solids to an intermediate texture and blit to the surface.
    /// This matches the demo-app default and is often more robust across platforms during resize.
    use_intermediate: bool,
    /// When true, positions are interpreted as logical pixels and scaled by dpi_scale in PassManager.
    logical_pixels: bool,
    /// Current DPI scale factor (e.g., 2.0 on Retina).
    dpi_scale: f32,
    /// Additional UI scale multiplier
    ui_scale: f32,
    /// Optional overlay callback for post-render passes (e.g., SVG overlays)
    overlay: Option<OverlayCallback>,
}

impl RuneSurface {
    /// Create a new surface wrapper using an existing device/queue and the chosen surface format.
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let pass = PassManager::new(device.clone(), surface_format);
        let allocator = RenderAllocator::new(device.clone());

        Self {
            device,
            queue,
            pass,
            allocator,
            direct: false,
            preserve_surface: false,
            use_intermediate: true,
            logical_pixels: true,
            dpi_scale: 1.0,
            ui_scale: 1.0,
            overlay: None,
        }
    }

    /// Convenience: construct from shared device/queue handles.
    pub fn from_device_queue(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        Self::new(device, queue, surface_format)
    }

    pub fn device(&self) -> Arc<wgpu::Device> {
        self.device.clone()
    }
    pub fn queue(&self) -> Arc<wgpu::Queue> {
        self.queue.clone()
    }
    pub fn pass_manager(&mut self) -> &mut PassManager {
        &mut self.pass
    }
    pub fn allocator_mut(&mut self) -> &mut RenderAllocator {
        &mut self.allocator
    }

    /// Choose whether to render directly to the surface (bypass compositor).
    pub fn set_direct(&mut self, direct: bool) {
        self.direct = direct;
    }
    /// Control whether to preserve existing contents on the surface.
    pub fn set_preserve_surface(&mut self, preserve: bool) {
        self.preserve_surface = preserve;
    }
    /// Choose whether to use an intermediate texture and blit to the surface.
    pub fn set_use_intermediate(&mut self, use_it: bool) {
        self.use_intermediate = use_it;
    }
    /// Enable or disable logical pixel interpretation.
    pub fn set_logical_pixels(&mut self, on: bool) {
        self.logical_pixels = on;
    }
    /// Set current DPI scale and propagate to passes before rendering.
    pub fn set_dpi_scale(&mut self, scale: f32) {
        self.dpi_scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
    }
    /// Set a global UI scale multiplier
    pub fn set_ui_scale(&mut self, s: f32) {
        self.ui_scale = if s.is_finite() { s } else { 1.0 };
    }
    /// Set an overlay callback for post-render passes
    pub fn set_overlay(&mut self, callback: OverlayCallback) {
        self.overlay = Some(callback);
    }
    /// Clear the overlay callback
    pub fn clear_overlay(&mut self) {
        self.overlay = None;
    }

    /// Pre-allocate intermediate texture at the given size.
    /// This should be called after surface reconfiguration to avoid jitter.
    pub fn prepare_for_resize(&mut self, width: u32, height: u32) {
        self.pass
            .ensure_intermediate_texture(&mut self.allocator, width, height);
    }

    /// Begin a canvas frame of the given size (in pixels).
    pub fn begin_frame(&self, width: u32, height: u32) -> Canvas {
        let vp = Viewport { width, height };
        Canvas {
            viewport: vp,
            painter: Painter::begin_frame(vp),
            clear_color: None,
            text_provider: None,
            glyph_draws: Vec::new(),
            svg_draws: Vec::new(),
            image_draws: Vec::new(),
            dpi_scale: self.dpi_scale,
            clip_stack: vec![None],
        }
    }

    /// Finish the frame by rendering accumulated commands to the provided surface texture.
    pub fn end_frame(&mut self, frame: wgpu::SurfaceTexture, canvas: Canvas) -> Result<()> {
        // Keep passes in sync with DPI/logical settings
        self.pass.set_scale_factor(self.dpi_scale);
        self.pass.set_logical_pixels(self.logical_pixels);
        self.pass.set_ui_scale(self.ui_scale);

        // Build final display list from painter
        let mut list = canvas.painter.finish();
        let width = canvas.viewport.width.max(1);
        let height = canvas.viewport.height.max(1);

        // Sort display list by z-index to ensure proper layering
        list.sort_by_z();

        // Create target view
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rune-surface-encoder"),
            });

        // Clear color or transparent
        let clear = canvas.clear_color.unwrap_or(ColorLinPremul {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        });
        let clear_wgpu = wgpu::Color {
            r: clear.r as f64,
            g: clear.g as f64,
            b: clear.b as f64,
            a: clear.a as f64,
        };

        // Ensure depth texture is allocated for z-ordering (Phase 1 of depth buffer implementation)
        self.pass
            .ensure_depth_texture(&mut self.allocator, width, height);

        // Extract unified scene data (solids + text/image/svg draws) from the display list.
        let unified_scene = engine_core::upload_display_list_unified(
            &mut self.allocator,
            &self.queue,
            &list,
        )?;

        // Sort SVG draws by z-index
        let mut svg_draws = canvas.svg_draws.clone();
        svg_draws.sort_by_key(|(_, _, _, _, z, _)| *z);

        // Sort image draws by z-index and prepare simplified data (for unified pass)
        let mut image_draws = canvas.image_draws.clone();
        image_draws.sort_by_key(|(_, _, _, _, z, _)| *z);

        // Convert image draws to simplified format (path, origin, size, z)
        // Apply transforms and fit calculations here. We synchronously load images
        // via PassManager so that they appear on the very first frame, without
        // requiring a scroll/resize to trigger a second redraw.
        //
        // NOTE: Origins in `canvas.image_draws` are already in logical coordinates;
        // they will be scaled by PassManager via logical_pixels/dpi.
        let mut prepared_images: Vec<(std::path::PathBuf, [f32; 2], [f32; 2], i32)> =
            Vec::new();
        for (path, origin, size, fit, z, transform) in image_draws.iter() {
            // Synchronously load (or fetch from cache) to ensure the texture
            // is available for this frame. This mirrors the demo-app unified
            // path and avoids images only appearing after a later redraw.
            if let Some((tex_view, img_w, img_h)) =
                self.pass.load_image_to_view(std::path::Path::new(path), &self.queue)
            {
                drop(tex_view); // Only need dimensions here
                let transformed_origin = apply_transform_to_point(*origin, *transform);
                let (render_origin, render_size) = calculate_image_fit(
                    transformed_origin,
                    *size,
                    img_w as f32,
                    img_h as f32,
                    *fit,
                );
                prepared_images.push((path.clone(), render_origin, render_size, *z));
            }
        }

        // Merge glyphs supplied explicitly via Canvas (draw_text_run/draw_text_direct)
        // with text runs extracted from the display list for unified text rendering.
        let mut glyph_draws = canvas.glyph_draws.clone();

        if let Some(ref provider) = canvas.text_provider {
            // Reuse the logical pixel scale used by PassManager for solids so
            // text aligns with other geometry.
            let logical_scale = if self.logical_pixels {
                let s = if self.dpi_scale.is_finite() && self.dpi_scale > 0.0 {
                    self.dpi_scale
                } else {
                    1.0
                };
                let u = if self.ui_scale.is_finite() && self.ui_scale > 0.0 {
                    self.ui_scale
                } else {
                    1.0
                };
                (s * u).max(0.0001)
            } else {
                1.0
            };

            let sf = if self.dpi_scale.is_finite() && self.dpi_scale > 0.0 {
                self.dpi_scale
            } else {
                1.0
            };
            let snap = |v: f32| -> f32 { (v * sf).round() / sf };

            for text_draw in &unified_scene.text_draws {
                let run = &text_draw.run;
                let [a, b, c, d, e, f] = text_draw.transform.m;

                // Apply full affine transform (scale + translate) to the run origin
                let rx = a * run.pos[0] + c * run.pos[1] + e;
                let ry = b * run.pos[0] + d * run.pos[1] + f;

                // Infer uniform scale from the linear part of the transform
                let sx = (a * a + b * b).sqrt();
                let sy = (c * c + d * d).sqrt();
                let mut s = if sx.is_finite() && sy.is_finite() {
                    if sx > 0.0 && sy > 0.0 {
                        (sx + sy) * 0.5
                    } else {
                        sx.max(sy).max(1.0)
                    }
                } else {
                    1.0
                };
                if !s.is_finite() || s <= 0.0 {
                    s = 1.0;
                }
                s *= logical_scale;

                let scaled_size = (run.size * s).max(1.0);
                let run_for_provider = engine_core::TextRun {
                    text: run.text.clone(),
                    pos: [0.0, 0.0],
                    size: scaled_size,
                    color: run.color,
                };

                // Convert origin to physical pixels if logical mode is enabled
                let rx_px = rx * logical_scale;
                let ry_px = ry * logical_scale;
                let run_origin_x = snap(rx_px);

                let baseline_y = if let Some(m) = provider.line_metrics(scaled_size) {
                    let asc = m.ascent;
                    snap(ry_px + asc) - asc
                } else {
                    snap(ry_px)
                };

                // Rasterize glyphs for this run and push into glyph_draws
                let glyphs = engine_core::rasterize_run_cached(provider.as_ref(), &run_for_provider);
                for g in glyphs.iter() {
                    let mut origin = [run_origin_x + g.offset[0], baseline_y + g.offset[1]];
                    if scaled_size <= 15.0 {
                        origin[0] = snap(origin[0]);
                        origin[1] = snap(origin[1]);
                    }
                    glyph_draws.push((origin, g.clone(), run.color, text_draw.z));
                }
            }
        }

        // Unified solids + text/images/SVGs pass
        self.pass.render_unified(
            &mut encoder,
            &mut self.allocator,
            &view,
            width,
            height,
            &unified_scene.gpu_scene,
            &glyph_draws,
            &svg_draws,
            &prepared_images,
            clear_wgpu,
            self.direct,
            &self.queue,
            self.preserve_surface,
        );

        // Call overlay callback last so overlays (e.g., devtools, debug UI)
        // are guaranteed to draw above all other content.
        if let Some(ref mut overlay_fn) = self.overlay {
            overlay_fn(
                &mut self.pass,
                &mut encoder,
                &view,
                &self.queue,
                width,
                height,
            );
        }

        // Submit and present
        let cb = encoder.finish();
        self.queue.submit(std::iter::once(cb));
        frame.present();
        Ok(())
    }
}
