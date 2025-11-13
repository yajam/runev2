use std::sync::Arc;

use anyhow::Result;

use engine_core::{
    Viewport,
    PassManager,
    Painter,
    ColorLinPremul,
    upload_display_list,
    RenderAllocator,
    wgpu, // import wgpu from engine-core to keep type identity
};

use crate::canvas::Canvas;

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
}

impl RuneSurface {
    /// Create a new surface wrapper using an existing device/queue and the chosen surface format.
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>, surface_format: wgpu::TextureFormat) -> Self {
        let pass = PassManager::new(device.clone(), surface_format);
        let allocator = RenderAllocator::new(device.clone());
        Self {
            device,
            queue,
            pass,
            allocator,
            direct: false,
            preserve_surface: false,
        }
    }

    /// Convenience: construct from shared device/queue handles.
    pub fn from_device_queue(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>, surface_format: wgpu::TextureFormat) -> Self {
        Self::new(device, queue, surface_format)
    }

    pub fn device(&self) -> Arc<wgpu::Device> { self.device.clone() }
    pub fn queue(&self) -> Arc<wgpu::Queue> { self.queue.clone() }
    pub fn pass_manager(&mut self) -> &mut PassManager { &mut self.pass }
    pub fn allocator_mut(&mut self) -> &mut RenderAllocator { &mut self.allocator }

    /// Choose whether to render directly to the surface (bypass compositor).
    pub fn set_direct(&mut self, direct: bool) { self.direct = direct; }
    /// Control whether to preserve existing contents on the surface.
    pub fn set_preserve_surface(&mut self, preserve: bool) { self.preserve_surface = preserve; }

    /// Begin a canvas frame of the given size (in pixels).
    pub fn begin_frame(&self, width: u32, height: u32) -> Canvas {
        let vp = Viewport { width, height };
        Canvas {
            viewport: vp,
            painter: Painter::begin_frame(vp),
            clear_color: None,
            text_provider: None,
            glyph_draws: Vec::new(),
        }
    }

    /// Finish the frame by rendering accumulated commands to the provided surface texture.
    pub fn end_frame(&mut self, frame: wgpu::SurfaceTexture, canvas: Canvas) -> Result<()> {
        // Build final display list from painter
        let list = canvas.painter.finish();
        let width = canvas.viewport.width.max(1);
        let height = canvas.viewport.height.max(1);

        // Upload geometry to GPU
        let scene = upload_display_list(&mut self.allocator, &self.queue, &list)?;

        // Create target view
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("rune-surface-encoder") });

        // Clear color or transparent
        let clear = canvas
            .clear_color
            .unwrap_or(ColorLinPremul { r: 0.0, g: 0.0, b: 0.0, a: 0.0 });
        let clear_wgpu = wgpu::Color { r: clear.r as f64, g: clear.g as f64, b: clear.b as f64, a: clear.a as f64 };

        // Render solids (and optionally text runs)
        if let Some(provider) = &canvas.text_provider {
            self.pass.render_frame_and_text(
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
                &list,
                provider.as_ref(),
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

        // Draw any low-level glyph masks on top
        for (origin, glyph, color) in canvas.glyph_draws.iter() {
            let ox = origin[0] + glyph.offset[0];
            let oy = origin[1] + glyph.offset[1];
            self.pass.draw_text_mask(
                &mut encoder,
                &view,
                [ox, oy],
                &glyph.mask,
                *color,
                &self.queue,
            );
        }

        // Submit and present
        let cb = encoder.finish();
        self.queue.submit(std::iter::once(cb));
        frame.present();
        Ok(())
    }
}
