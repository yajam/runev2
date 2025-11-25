use std::sync::{Arc, Mutex};

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

/// Storage for the last rendered raw image rect (used for hit testing WebViews).
static LAST_RAW_IMAGE_RECT: Mutex<Option<(f32, f32, f32, f32)>> = Mutex::new(None);

/// Set the last raw image rect (called during rendering).
fn set_last_raw_image_rect(x: f32, y: f32, w: f32, h: f32) {
    if let Ok(mut guard) = LAST_RAW_IMAGE_RECT.lock() {
        *guard = Some((x, y, w, h));
    }
}

/// Get the last raw image rect (for hit testing from FFI).
pub fn get_last_raw_image_rect() -> Option<(f32, f32, f32, f32)> {
    if let Ok(guard) = LAST_RAW_IMAGE_RECT.lock() {
        *guard
    } else {
        None
    }
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
    /// When true, run SMAA resolve; when false, favor a direct blit for crisper text.
    enable_smaa: bool,
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
            enable_smaa: false,
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
    /// Enable or disable SMAA. Disabling skips the post-process filter to keep small text crisp.
    pub fn set_enable_smaa(&mut self, enable: bool) {
        self.enable_smaa = enable;
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
            raw_image_draws: Vec::new(),
            dpi_scale: self.dpi_scale,
            clip_stack: vec![None],
            overlay_draws: Vec::new(),
            scrim_draws: Vec::new(),
        }
    }

    /// Finish the frame by rendering accumulated commands to the provided surface texture.
    pub fn end_frame(&mut self, frame: wgpu::SurfaceTexture, canvas: Canvas) -> Result<()> {
        // Keep passes in sync with DPI/logical settings
        self.pass.set_scale_factor(self.dpi_scale);
        self.pass.set_logical_pixels(self.logical_pixels);
        self.pass.set_ui_scale(self.ui_scale);

        // Determine the render target: prefer intermediate when SMAA or Vello-style resizing is on.
        let use_intermediate = self.enable_smaa || self.use_intermediate;

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
        let scene_view = if use_intermediate {
            self.pass
                .ensure_intermediate_texture(&mut self.allocator, width, height);
            let scene_target = self
                .pass
                .intermediate_texture
                .as_ref()
                .expect("intermediate render target not allocated");
            scene_target
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default())
        } else {
            frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default())
        };

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
        let unified_scene =
            engine_core::upload_display_list_unified(&mut self.allocator, &self.queue, &list)?;

        // Sort SVG draws by z-index and resolve paths for app bundle
        let mut svg_draws: Vec<_> = canvas
            .svg_draws
            .iter()
            .map(|(path, origin, max_size, style, z, transform)| {
                let resolved_path = crate::resolve_asset_path(path);
                (resolved_path, *origin, *max_size, *style, *z, *transform)
            })
            .collect();
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
        let mut prepared_images: Vec<(std::path::PathBuf, [f32; 2], [f32; 2], i32)> = Vec::new();
        for (path, origin, size, fit, z, transform) in image_draws.iter() {
            // Resolve path to check app bundle resources
            let resolved_path = crate::resolve_asset_path(path);

            // Synchronously load (or fetch from cache) to ensure the texture
            // is available for this frame. This mirrors the demo-app unified
            // path and avoids images only appearing after a later redraw.
            if let Some((tex_view, img_w, img_h)) = self
                .pass
                .load_image_to_view(&resolved_path, &self.queue)
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
                prepared_images.push((resolved_path.clone(), render_origin, render_size, *z));
            }
        }

        // Process raw image draws (e.g., WebView CEF pixels)
        // Optimizations:
        // 1. Always reuse textures - only recreate if size changes
        // 2. Use BGRA format to match CEF native output (no CPU conversion)
        // 3. Support dirty rect partial uploads
        for (i, raw_draw) in canvas.raw_image_draws.iter().enumerate() {
            if raw_draw.src_width == 0 || raw_draw.src_height == 0 {
                continue;
            }

            // Use a fixed path for webview texture - reused across frames
            let raw_path = std::path::PathBuf::from(format!("__webview_texture_{}__", i));

            // If pixels are empty, reuse cached texture from previous frame
            let has_new_pixels = !raw_draw.pixels.is_empty();

            // Check if we need a new texture (only if size changed)
            let need_new_texture = if let Some((_, cached_w, cached_h)) =
                self.pass.try_get_image_view(&raw_path)
            {
                // Only recreate if dimensions changed - always reuse otherwise
                cached_w != raw_draw.src_width || cached_h != raw_draw.src_height
            } else {
                true
            };

            // Create texture only when needed (first time or size change)
            if need_new_texture && has_new_pixels {
                // Create texture with BGRA format to match CEF's native output
                // This eliminates CPU-side BGRA->RGBA conversion
                let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("cef-webview-texture"),
                    size: wgpu::Extent3d {
                        width: raw_draw.src_width,
                        height: raw_draw.src_height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    // BGRA format matches CEF native output - no conversion needed
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });

                // Store in image cache for reuse
                self.pass.store_loaded_image(
                    &raw_path,
                    Arc::new(texture),
                    raw_draw.src_width,
                    raw_draw.src_height,
                );
            }

            // Upload pixels only when we have new data
            if has_new_pixels {
                if let Some((tex, _, _)) = self.pass.get_cached_texture(&raw_path) {
                    // Always upload full frame - CEF provides complete buffer even for partial updates.
                    // The dirty_rects are informational but the buffer is always complete.
                    // This ensures no flickering from partial/stale data.
                    self.queue.write_texture(
                        wgpu::ImageCopyTexture {
                            texture: &tex,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        &raw_draw.pixels,
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(raw_draw.src_width * 4),
                            rows_per_image: Some(raw_draw.src_height),
                        },
                        wgpu::Extent3d {
                            width: raw_draw.src_width,
                            height: raw_draw.src_height,
                            depth_or_array_layers: 1,
                        },
                    );
                }
            }

            // Skip rendering if no cached texture exists (no pixels uploaded yet)
            if self.pass.try_get_image_view(&raw_path).is_none() {
                continue;
            }

            // Apply the canvas transform to the origin - same as regular images.
            // The origin from draw_raw_image is in local (viewport) coordinates and
            // needs to be transformed to screen coordinates.
            let transformed_origin = apply_transform_to_point(raw_draw.origin, raw_draw.transform);

            // Store the transformed rect for hit testing (accessible via get_last_raw_image_rect)
            set_last_raw_image_rect(
                transformed_origin[0],
                transformed_origin[1],
                raw_draw.dst_size[0],
                raw_draw.dst_size[1],
            );

            prepared_images.push((raw_path, transformed_origin, raw_draw.dst_size, raw_draw.z));
        }

        // Merge glyphs supplied explicitly via Canvas (draw_text_run/draw_text_direct)
        // with text runs extracted from the display list (e.g., hyperlinks) for
        // unified text rendering.
        let mut glyph_draws = canvas.glyph_draws.clone();

        if let Some(ref provider) = canvas.text_provider {
            // Use the same snapping strategy as direct text paths so small
            // text (e.g., 13–15px) lands cleanly on device pixels.
            let sf = if self.dpi_scale.is_finite() && self.dpi_scale > 0.0 {
                self.dpi_scale
            } else {
                1.0
            };
            let snap = |v: f32| -> f32 { (v * sf).round() / sf };

            for text_draw in &unified_scene.text_draws {
                let run = &text_draw.run;
                let [a, b, c, d, e, f] = text_draw.transform.m;

                // Transform the run origin (baseline-left) into world coordinates.
                let origin_x = a * run.pos[0] + c * run.pos[1] + e;
                let origin_y = b * run.pos[0] + d * run.pos[1] + f;

                // Infer uniform scale from the linear part of the transform so
                // text respects any explicit scaling in the display list.
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

                // Rasterize at logical size scaled only by the display-list
                // transform. DPI scaling is applied later by PassManager.
                let scaled_size = (run.size * s).max(1.0);
                let run_for_provider = engine_core::TextRun {
                    text: run.text.clone(),
                    pos: [0.0, 0.0],
                    size: scaled_size,
                    color: run.color,
                };

                // Rasterize glyphs for this run and push into glyph_draws.
                // Origins are kept in logical coordinates; PassManager applies
                // DPI/UI scaling centrally so geometry and text stay aligned.
                let glyphs =
                    engine_core::rasterize_run_cached(provider.as_ref(), &run_for_provider);
                for g in glyphs.iter() {
                    let mut origin = [origin_x + g.offset[0], origin_y + g.offset[1]];
                    if scaled_size <= 15.0 {
                        origin[0] = snap(origin[0]);
                        origin[1] = snap(origin[1]);
                    }
                    glyph_draws.push((origin, g.clone(), run.color, text_draw.z));
                }
            }
        }

        // Unified solids + text/images/SVGs pass
        let preserve_surface = self.preserve_surface;
        let direct = self.direct || !use_intermediate;
        self.pass.render_unified(
            &mut encoder,
            &mut self.allocator,
            &scene_view,
            width,
            height,
            &unified_scene.gpu_scene,
            &glyph_draws,
            &svg_draws,
            &prepared_images,
            clear_wgpu,
            direct,
            &self.queue,
            preserve_surface,
        );

        // Render scrims; support both simple rects and stencil cutouts.
        for scrim in &canvas.scrim_draws {
            match scrim {
                crate::ScrimDraw::Rect(rect, color) => {
                    self.pass.draw_scrim_rect(
                        &mut encoder,
                        &scene_view,
                        width,
                        height,
                        *rect,
                        *color,
                        &self.queue,
                    );
                }
                crate::ScrimDraw::Cutout { hole, color } => {
                    self.pass.draw_scrim_with_cutout(
                        &mut encoder,
                        &mut self.allocator,
                        &scene_view,
                        width,
                        height,
                        *hole,
                        *color,
                        &self.queue,
                    );
                }
            }
        }

        // Render overlay rectangles (modal scrims) without depth testing.
        // These blend over the entire scene without blocking text.
        for (rect, color) in &canvas.overlay_draws {
            self.pass.draw_overlay_rect(
                &mut encoder,
                &scene_view,
                width,
                height,
                *rect,
                *color,
                &self.queue,
            );
        }

        // Call overlay callback last so overlays (e.g., devtools, debug UI)
        // are guaranteed to draw above all other content.
        if let Some(ref mut overlay_fn) = self.overlay {
            overlay_fn(
                &mut self.pass,
                &mut encoder,
                &scene_view,
                &self.queue,
                width,
                height,
            );
        }

        // Resolve to the swapchain: SMAA when enabled, otherwise a nearest-neighbor blit for sharper text.
        if use_intermediate {
            if self.enable_smaa {
                self.pass.apply_smaa(
                    &mut encoder,
                    &mut self.allocator,
                    &scene_view,
                    &view,
                    width,
                    height,
                    &self.queue,
                );
            } else {
                self.pass.blit_to_surface(&mut encoder, &view);
            }
        }

        // Submit and present
        let cb = encoder.finish();
        self.queue.submit(std::iter::once(cb));
        frame.present();
        Ok(())
    }
}
