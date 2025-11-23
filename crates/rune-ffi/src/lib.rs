//! FFI exports for embedding rune-scene in native macOS applications.
//!
//! This crate provides C-compatible functions for initializing and running
//! the rune-scene IR renderer from an Xcode/macOS application.

pub mod ffi;

use anyhow::{Context, Result};
use rune_ir::{data::document::DataDocument, view::ViewDocument};
use std::cell::RefCell;
use std::sync::Arc;

/// Application renderer state
pub struct AppRenderer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    surf: rune_surface::RuneSurface,
    provider: Arc<dyn engine_core::TextProvider>,
    data_doc: DataDocument,
    view_doc: ViewDocument,
    width: u32,
    height: u32,
    logical_width: u32,
    logical_height: u32,
    scale_factor: f32,
    needs_redraw: bool,
}

impl AppRenderer {
    /// Create a new renderer with a CAMetalLayer
    pub fn new(
        width: u32,
        height: u32,
        scale: f32,
        metal_layer: *mut std::ffi::c_void,
        package_path: Option<&str>,
    ) -> Result<Self> {
        log::info!("AppRenderer::new width={} height={} scale={}", width, height, scale);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..Default::default()
        });

        // Create surface from CAMetalLayer
        let surface = unsafe {
            let target = wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(metal_layer);
            instance.create_surface_unsafe(target)?
        };

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .context("No suitable adapter")?;

        log::info!("Using adapter: {:?}", adapter.get_info());

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("rune-ffi-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        ))?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .first()
            .copied()
            .unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Set up RuneSurface
        let mut surf = rune_surface::RuneSurface::new(device.clone(), queue.clone(), format);
        surf.set_use_intermediate(true);
        surf.set_logical_pixels(true);
        surf.set_dpi_scale(scale);

        // Set up text provider
        let provider: Arc<dyn engine_core::TextProvider> = Arc::new(
            engine_core::RuneTextProvider::from_system_fonts(engine_core::SubpixelOrientation::RGB)
                .context("Failed to load system fonts")?,
        );

        // Load IR documents
        let (data_doc, view_doc) = load_ir_package(package_path)?;
        log::info!("Loaded IR package: data={}, view={}", data_doc.document_id, view_doc.view_id);

        let logical_width = (width as f32 / scale) as u32;
        let logical_height = (height as f32 / scale) as u32;

        Ok(Self {
            device,
            queue,
            surface,
            config,
            surf,
            provider,
            data_doc,
            view_doc,
            width,
            height,
            logical_width,
            logical_height,
            scale_factor: scale,
            needs_redraw: true,
        })
    }

    /// Resize the surface
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        log::debug!("AppRenderer::resize width={} height={}", width, height);

        self.width = width;
        self.height = height;
        self.logical_width = (width as f32 / self.scale_factor) as u32;
        self.logical_height = (height as f32 / self.scale_factor) as u32;
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.needs_redraw = true;
    }

    /// Render a frame
    pub fn render(&mut self) {
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(e) => {
                log::warn!("Failed to get current texture: {:?}", e);
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };

        // Create canvas for this frame
        let mut canvas = self.surf.begin_frame(self.width, self.height);
        canvas.set_text_provider(self.provider.clone());
        canvas.clear(engine_core::ColorLinPremul::from_srgba_u8([30, 41, 59, 255]));

        // Render IR documents using the public render_ir_document API
        rune_scene::ir_renderer::render_ir_document(
            &mut canvas,
            &self.data_doc,
            &self.view_doc,
            self.logical_width as f32,
            self.logical_height as f32,
            self.provider.as_ref(),
        );

        // End frame and present
        if let Err(e) = self.surf.end_frame(frame, canvas) {
            log::error!("End frame error: {:?}", e);
        }

        self.needs_redraw = false;
    }

    /// Handle mouse click event
    pub fn mouse_click(&mut self, x: f32, y: f32, pressed: bool) {
        let _logical_x = x / self.scale_factor;
        let _logical_y = y / self.scale_factor;

        if pressed {
            self.needs_redraw = true;
        }
    }

    /// Handle mouse move event
    pub fn mouse_move(&mut self, x: f32, y: f32) {
        let _logical_x = x / self.scale_factor;
        let _logical_y = y / self.scale_factor;
    }

    /// Handle key event
    pub fn key_event(&mut self, keycode: u32, pressed: bool) {
        log::debug!("Key event: keycode={} pressed={}", keycode, pressed);
        self.needs_redraw = true;
    }

    /// Check if redraw is needed
    pub fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    /// Request redraw
    pub fn request_redraw(&mut self) {
        self.needs_redraw = true;
    }
}

/// Load IR package from path or use default
fn load_ir_package(package_path: Option<&str>) -> Result<(DataDocument, ViewDocument)> {
    if let Some(path) = package_path {
        load_ir_package_from_path(path)
    } else {
        // Use default sample from rune-ir
        let package = rune_ir::package::RunePackage::sample()?;
        let (data, view) = package.entrypoint_documents()?;
        Ok((data.clone(), view.clone()))
    }
}

/// Load IR package from filesystem path
fn load_ir_package_from_path(package_path: &str) -> Result<(DataDocument, ViewDocument)> {
    use std::fs;
    use std::path::Path;

    let base = Path::new(package_path);

    // Read manifest
    let manifest_path = base.join("RUNE.MANIFEST.json");
    let manifest_str = fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read manifest: {:?}", manifest_path))?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_str)?;

    // Get entrypoint paths
    let entrypoint = manifest.get("entrypoint").context("No entrypoint in manifest")?;
    let data_path = entrypoint
        .get("data")
        .and_then(|v| v.as_str())
        .context("No data path in entrypoint")?;
    let view_path = entrypoint
        .get("view")
        .and_then(|v| v.as_str())
        .context("No view path in entrypoint")?;

    // Load documents
    let data_full_path = base.join(data_path);
    let view_full_path = base.join(view_path);

    let data_str = fs::read_to_string(&data_full_path)
        .with_context(|| format!("Failed to read data: {:?}", data_full_path))?;
    let view_str = fs::read_to_string(&view_full_path)
        .with_context(|| format!("Failed to read view: {:?}", view_full_path))?;

    let data_doc: DataDocument = serde_json::from_str(&data_str)?;
    let view_doc: ViewDocument = serde_json::from_str(&view_str)?;

    Ok((data_doc, view_doc))
}

// Use thread_local storage for the renderer since it contains non-Send types
thread_local! {
    static RENDERER: RefCell<Option<AppRenderer>> = const { RefCell::new(None) };
}

pub fn with_renderer<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut AppRenderer) -> R,
{
    RENDERER.with(|r| {
        let mut borrow = r.borrow_mut();
        borrow.as_mut().map(f)
    })
}

pub fn set_renderer(renderer: Option<AppRenderer>) {
    RENDERER.with(|r| {
        *r.borrow_mut() = renderer;
    });
}
