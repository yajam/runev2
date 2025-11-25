//! CEF/CDP headless rendering demo scene.
//!
//! Renders web content via CEF or CDP into a wgpu texture and displays it.
//!
//! Two backends are supported:
//! - `cef` feature: Native CEF - requires CEF binary distribution and complex setup
//! - `cdp` feature: Chrome DevTools Protocol - just needs Chrome installed (recommended)
//!
//! For CEF: Set CEF_PATH environment variable to point to your CEF distribution.
//! Run with: cargo run -p demo-app --features cef -- --scene=cef
//!
//! For CDP (easier): Just have Chrome installed.
//! Run with: cargo run -p demo-app --features cdp -- --scene=cef

use super::{Scene, SceneKind};
use engine_core::{DisplayList, PassManager, Viewport};
use rune_cef::{HeadlessBuilder, HeadlessRenderer, WgpuTextureTarget};
use std::cell::RefCell;

/// Demo scene that renders web content via headless CEF or CDP.
pub struct CefScene {
    /// Headless renderer (initialized lazily).
    renderer: RefCell<Option<Box<dyn HeadlessRenderer>>>,
    /// wgpu texture target for rendered frames.
    texture_target: RefCell<Option<WgpuTextureTarget>>,
    /// Current viewport dimensions.
    width: u32,
    height: u32,
    /// URL to render (can be changed via CEF_URL env var).
    url: String,
    /// Error message if initialization failed.
    error_msg: RefCell<Option<String>>,
}

impl Default for CefScene {
    fn default() -> Self {
        Self::new(None)
    }
}

impl CefScene {
    /// Build a new scene with an optional URL override.
    pub fn new(url: Option<String>) -> Self {
        let url = url
            .or_else(|| std::env::var("CEF_URL").ok())
            .unwrap_or_else(|| {
                "data:text/html,<html><body style='background:linear-gradient(135deg,%23667eea%200%25,%23764ba2%20100%25);display:flex;justify-content:center;align-items:center;height:100vh;margin:0;font-family:system-ui'><div style='text-align:center;color:white'><h1 style='font-size:48px;margin:0'>CEF Rendering</h1><p style='font-size:24px;opacity:0.8'>Headless Chromium on wgpu</p><p style='font-size:16px;opacity:0.6'>Set CEF_URL to render a different page</p></div></body></html>".to_string()
            });

        Self {
            renderer: RefCell::new(None),
            texture_target: RefCell::new(None),
            width: 1280,
            height: 720,
            url,
            error_msg: RefCell::new(None),
        }
    }

    /// Initialize renderer (called lazily on first paint).
    /// Tries CDP first (if available), then falls back to native CEF.
    fn ensure_initialized(&self, device: &wgpu::Device) {
        // Debug: write to file to trace execution
        {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/cef_scene_trace.log")
            {
                let _ = writeln!(f, "ensure_initialized called");
                let _ = f.flush();
            }
        }

        if self.renderer.borrow().is_some() || self.error_msg.borrow().is_some() {
            return;
        }

        // Try CDP first if available (much easier to set up)
        #[cfg(feature = "cdp")]
        {
            eprintln!("CDP: Attempting to connect to Chrome via DevTools Protocol...");
            // CDP requires async - use a runtime
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("Failed to create tokio runtime: {}", e);
                    // Fall through to CEF
                    return self.try_cef(device);
                }
            };

            let width = self.width;
            let height = self.height;
            let url = self.url.clone();

            match rt.block_on(async {
                HeadlessBuilder::new()
                    .with_size(width, height)
                    .with_scale_factor(1.0)
                    .build_cdp()
                    .await
            }) {
                Ok(mut renderer) => {
                    eprintln!("CDP: Connected successfully, navigating to URL...");
                    // Navigate to URL
                    if let Err(e) = renderer.navigate(&url) {
                        *self.error_msg.borrow_mut() = Some(format!("CDP Navigation failed: {}", e));
                        return;
                    }

                    // Wait for initial load (with timeout)
                    if let Err(e) = renderer.wait_for_load(5000) {
                        eprintln!("Warning: CDP page load timeout: {}", e);
                    }

                    *self.renderer.borrow_mut() = Some(Box::new(renderer));

                    // Create texture target
                    *self.texture_target.borrow_mut() = Some(WgpuTextureTarget::new(
                        device,
                        width,
                        height,
                        Some("cdp_scene_texture"),
                    ));
                    return;
                }
                Err(e) => {
                    eprintln!("CDP initialization failed: {}", e);
                    eprintln!("Make sure Chrome or Chromium is installed.");
                    #[cfg(feature = "cef")]
                    {
                        eprintln!("Falling back to native CEF...");
                    }
                }
            }
        }

        // Try native CEF
        #[cfg(feature = "cef")]
        self.try_cef(device);

        // If neither feature is properly enabled, show error
        #[cfg(not(any(feature = "cef", feature = "cdp")))]
        {
            *self.error_msg.borrow_mut() = Some(
                "No browser backend available. Enable 'cef' or 'cdp' feature.".to_string()
            );
        }
    }

    #[cfg(feature = "cef")]
    fn try_cef(&self, device: &wgpu::Device) {
        eprintln!("CEF: Attempting native CEF initialization...");
        match HeadlessBuilder::new()
            .with_size(self.width, self.height)
            .with_scale_factor(1.0)
            .build_cef()
        {
            Ok(mut renderer) => {
                // Navigate to URL
                if let Err(e) = renderer.navigate(&self.url) {
                    *self.error_msg.borrow_mut() = Some(format!("Navigation failed: {}", e));
                    return;
                }

                // Wait for initial load (with timeout)
                if let Err(e) = renderer.wait_for_load(5000) {
                    eprintln!("Warning: Page load timeout: {}", e);
                }

                *self.renderer.borrow_mut() = Some(Box::new(renderer));

                // Create texture target
                *self.texture_target.borrow_mut() = Some(WgpuTextureTarget::new(
                    device,
                    self.width,
                    self.height,
                    Some("cef_scene_texture"),
                ));
            }
            Err(e) => {
                let msg = format!(
                    "CEF initialization failed: {}\n\n\
                     Native CEF on macOS requires a proper app bundle structure.\n\
                     For easier setup, use the CDP backend instead:\n\
                     cargo run -p demo-app --features cdp -- --scene=cef",
                    e
                );
                eprintln!("{}", msg);
                *self.error_msg.borrow_mut() = Some(msg);
            }
        }
    }

    /// Capture frame from CEF and upload to texture.
    fn update_texture(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let mut renderer = self.renderer.borrow_mut();
        let mut target = self.texture_target.borrow_mut();

        if let Some(ref mut r) = renderer.as_mut() {
            match r.capture_frame() {
                Ok(frame) => {
                    if frame.is_empty() {
                        return;
                    }

                    let frame_dims = (frame.width, frame.height);
                    let needs_new = target.as_ref().map(|t| t.dimensions()) != Some(frame_dims);
                    if needs_new {
                        *target = Some(WgpuTextureTarget::new(
                            device,
                            frame.width,
                            frame.height,
                            Some("cef_scene_texture"),
                        ));
                    }

                    if let Some(ref t) = *target {
                        if let Err(e) = t.upload(queue, &frame) {
                            eprintln!("Frame upload failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Frame capture failed: {}", e);
                }
            }
        }
    }
}

impl Scene for CefScene {
    fn kind(&self) -> SceneKind {
        SceneKind::FullscreenBackground
    }

    fn init_display_list(&mut self, viewport: Viewport) -> Option<DisplayList> {
        self.width = viewport.width;
        self.height = viewport.height;
        None
    }

    fn on_resize(&mut self, viewport: Viewport) -> Option<DisplayList> {
        self.width = viewport.width;
        self.height = viewport.height;

        // Resize CEF renderer and texture if initialized
        if let Some(ref mut renderer) = *self.renderer.borrow_mut() {
            let _ = renderer.resize(viewport.width, viewport.height);
        }

        // Texture will be recreated on next paint if size changed
        *self.texture_target.borrow_mut() = None;

        None
    }

    fn paint_root_background(
        &self,
        passes: &mut PassManager,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
    ) {
        let device = passes.device();

        // Initialize CEF on first paint
        self.ensure_initialized(&device);

        // Clear background
        {
            let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cef-clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.15,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }

        // Check for error state
        if let Some(ref _msg) = *self.error_msg.borrow() {
            // Error occurred - just show dark background
            // In a full implementation, we'd render error text here
            return;
        }

        // Recreate texture target if needed (after resize)
        if self.texture_target.borrow().is_none() {
            if self.renderer.borrow().is_some() {
                *self.texture_target.borrow_mut() = Some(WgpuTextureTarget::new(
                    &device,
                    self.width,
                    self.height,
                    Some("cef_scene_texture"),
                ));
            }
        }

        // Update texture from CEF frame
        self.update_texture(&device, queue);

        // Draw the CEF texture to the surface
        if let Some(ref target) = *self.texture_target.borrow() {
            let (tex_w, tex_h) = target.dimensions();

            // Scale to fit while preserving aspect ratio
            let scale_x = width as f32 / tex_w as f32;
            let scale_y = height as f32 / tex_h as f32;
            let scale = scale_x.min(scale_y);

            let draw_w = tex_w as f32 * scale;
            let draw_h = tex_h as f32 * scale;

            // Center in viewport
            let ox = (width as f32 - draw_w) * 0.5;
            let oy = (height as f32 - draw_h) * 0.5;

            passes.draw_image_quad(
                encoder,
                surface_view,
                [ox, oy],
                [draw_w, draw_h],
                target.view(),
                queue,
                width,
                height,
            );
        }
    }
}
