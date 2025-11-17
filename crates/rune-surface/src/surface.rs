use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use anyhow::Result;

use engine_core::{
    ColorLinPremul,
    Painter,
    PassManager,
    RenderAllocator,
    Transform2D,
    Viewport,
    upload_display_list,
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

/// Message sent from background loader thread
struct LoadedImageData {
    path: PathBuf,
    data: image::RgbaImage,
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
    /// When true, use unified rendering (Phase 3) for optimal z-ordering across all draw types.
    /// This renders solids, text, images, and SVGs in a single pass sorted by z-index.
    use_unified_rendering: bool,
    /// When true, positions are interpreted as logical pixels and scaled by dpi_scale in PassManager.
    logical_pixels: bool,
    /// Current DPI scale factor (e.g., 2.0 on Retina).
    dpi_scale: f32,
    /// Additional UI scale multiplier
    ui_scale: f32,
    /// Optional overlay callback for post-render passes (e.g., SVG overlays)
    overlay: Option<OverlayCallback>,
    /// Channel for receiving loaded images from background thread
    image_loader_tx: Sender<PathBuf>,
    image_loader_rx: Receiver<LoadedImageData>,
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

        // Create channels for async image loading
        let (load_tx, load_rx) = channel();
        let (result_tx, result_rx) = channel();

        // Spawn background thread for image loading
        thread::spawn(move || {
            while let Ok(path) = load_rx.recv() {
                match image::open(&path) {
                    Ok(img) => {
                        let rgba = img.to_rgba8();
                        let _ = result_tx.send(LoadedImageData { path, data: rgba });
                    }
                    Err(_e) => {}
                }
            }
        });

        Self {
            device,
            queue,
            pass,
            allocator,
            direct: false,
            preserve_surface: false,
            use_intermediate: true,
            use_unified_rendering: true, // Enable Phase 3 unified rendering by default
            logical_pixels: true,
            dpi_scale: 1.0,
            ui_scale: 1.0,
            overlay: None,
            image_loader_tx: load_tx,
            image_loader_rx: result_rx,
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
    /// Enable or disable unified rendering (Phase 3) for optimal z-ordering.
    pub fn set_use_unified_rendering(&mut self, use_it: bool) {
        self.use_unified_rendering = use_it;
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

    /// Upload a loaded image from background thread to GPU
    fn upload_loaded_image(&mut self, loaded: LoadedImageData) {
        let (width, height) = loaded.data.dimensions();
        let device = self.pass.device();

        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("async-image:{}", loaded.path.display())),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &loaded.data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        // Store in cache
        self.pass
            .store_loaded_image(&loaded.path, Arc::new(tex), width, height);
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
        // Process any loaded images from background thread
        while let Ok(loaded) = self.image_loader_rx.try_recv() {
            self.upload_loaded_image(loaded);
        }

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

        // Upload geometry to GPU
        let scene = upload_display_list(&mut self.allocator, &self.queue, &list)?;

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

        // Choose rendering path based on use_unified_rendering flag
        if self.use_unified_rendering {
            // Phase 3: Unified rendering - all draw types in a single pass with optimal z-ordering
            
            // Sort SVG draws by z-index
            let mut svg_draws = canvas.svg_draws.clone();
            svg_draws.sort_by_key(|(_, _, _, _, z, _)| *z);
            
            // Sort image draws by z-index and prepare simplified data
            let mut image_draws = canvas.image_draws.clone();
            image_draws.sort_by_key(|(_, _, _, _, z, _)| *z);
            
            // Convert image draws to simplified format (path, origin, size, z)
            // Apply transforms and fit calculations here
            let mut prepared_images: Vec<(std::path::PathBuf, [f32; 2], [f32; 2], i32)> = Vec::new();
            for (path, origin, size, fit, z, transform) in image_draws.iter() {
                if let Some((tex_view, img_w, img_h)) =
                    self.pass.try_get_image_view(std::path::Path::new(path))
                {
                    drop(tex_view); // We just need to check if it's ready
                    let transformed_origin = apply_transform_to_point(*origin, *transform);
                    let (render_origin, render_size) = calculate_image_fit(
                        transformed_origin,
                        *size,
                        img_w as f32,
                        img_h as f32,
                        *fit,
                    );
                    prepared_images.push((path.clone(), render_origin, render_size, *z));
                } else {
                    // Request async load if not ready
                    if !self.pass.is_image_ready(std::path::Path::new(path)) {
                        self.pass.request_image_load(std::path::Path::new(path));
                        let _ = self.image_loader_tx.send(path.clone());
                    }
                }
            }
            
            // Call unified rendering method
            self.pass.render_unified(
                &mut encoder,
                &mut self.allocator,
                &view,
                width,
                height,
                &scene,
                &canvas.glyph_draws,
                &svg_draws,
                &prepared_images,
                clear_wgpu,
                self.direct,
                &self.queue,
                self.preserve_surface,
            );
        } else {
            // Legacy path: Separate passes for each draw type
            
            // Render solids (text is now handled separately via glyph_draws for simplicity)
            if self.use_intermediate {
                self.pass.render_frame_with_intermediate(
                    &mut encoder,
                    &mut self.allocator,
                    &view,
                    width,
                    height,
                    &scene,
                    clear_wgpu,
                    self.direct,
                    &self.queue,
                    self.preserve_surface, // Preserve intermediate texture if requested
                );
            } else {
                self.pass.render_frame(
                    &mut encoder,
                    &mut self.allocator,
                    &view,
                    width,
                    height,
                    &scene,
                    clear_wgpu,
                    self.direct,
                    &self.queue,
                    self.preserve_surface,
                );
            }

            // Draw all glyph masks in a single batched call (critical for performance!)
            if !canvas.glyph_draws.is_empty() {
                let batch: Vec<_> = canvas
                    .glyph_draws
                    .iter()
                    .map(|(origin, glyph, color)| {
                        // origin already includes glyph.offset from when it was added to glyph_draws
                        (glyph.mask.clone(), *origin, *color)
                    })
                    .collect();

                self.pass
                    .draw_text_mask(&mut encoder, &view, width, height, &batch, &self.queue);
            }

            // Sort SVG draws by z-index to respect layering
            let mut svg_draws = canvas.svg_draws.clone();
            svg_draws.sort_by_key(|(_, _, _, _, z, _)| *z);

            // Rasterize and draw any queued SVGs
            for (path, origin, max_size, style, _z, transform) in svg_draws.iter() {
                // Apply transform to origin
                let transformed_origin = apply_transform_to_point(*origin, *transform);

                // First get 1x size
                if let Some((_view1x, w1, h1)) = self.pass.rasterize_svg_to_view(
                    std::path::Path::new(path),
                    1.0,
                    *style,
                    &self.queue,
                ) {
                    let base_w = w1.max(1) as f32;
                    let base_h = h1.max(1) as f32;
                    let scale = (max_size[0] / base_w).min(max_size[1] / base_h).max(0.0);
                    let (view_scaled, sw, sh) = if let Some((v, w, h)) = self
                        .pass
                        .rasterize_svg_to_view(std::path::Path::new(path), scale, *style, &self.queue)
                    {
                        (v, w as f32, h as f32)
                    } else {
                        continue;
                    };
                    // Draw at transformed origin with scaled size
                    self.pass.draw_image_quad(
                        &mut encoder,
                        &view,
                        transformed_origin,
                        [sw, sh],
                        &view_scaled,
                        &self.queue,
                        width,
                        height,
                    );
                }
            }

            // Sort image draws by z-index to respect layering
            let mut image_draws = canvas.image_draws.clone();
            image_draws.sort_by_key(|(_, _, _, _, z, _)| *z);

            // Draw any queued raster images
            for (path, origin, size, fit, _z, transform) in image_draws.iter() {
                // Try to get the image from cache (non-blocking)
                if let Some((tex_view, img_w, img_h)) =
                    self.pass.try_get_image_view(std::path::Path::new(path))
                {
                    // Apply transform to origin
                    let transformed_origin = apply_transform_to_point(*origin, *transform);

                    // Image is ready - calculate actual render size and position based on fit mode
                    let (render_origin, render_size) = calculate_image_fit(
                        transformed_origin,
                        *size,
                        img_w as f32,
                        img_h as f32,
                        *fit,
                    );

                    self.pass.draw_image_quad(
                        &mut encoder,
                        &view,
                        render_origin,
                        render_size,
                        &tex_view,
                        &self.queue,
                        width,
                        height,
                    );
                } else {
                    // Image not ready - request async load if not already loading
                    if !self.pass.is_image_ready(std::path::Path::new(path)) {
                        self.pass.request_image_load(std::path::Path::new(path));
                        let _ = self.image_loader_tx.send(path.clone());
                    }
                    // Image will appear on next frame after background load completes
                }
            }
        }

        // Call overlay callback last so overlays (e.g., devtools, debug UI)
        // are guaranteed to draw above SVGs and raster images.
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
