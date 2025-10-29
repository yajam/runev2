use std::sync::Arc;

// use anyhow::Result;

use crate::allocator::{RenderAllocator, TexKey};
use crate::pipeline::{BackgroundRenderer, BasicSolidRenderer, Compositor};
use crate::upload::GpuScene;

pub struct PassTargets {
    pub color: crate::OwnedTexture,
}

pub enum Background {
    Solid(crate::scene::ColorLinPremul),
    LinearGradient {
        start_uv: [f32; 2],
        end_uv: [f32; 2],
        stop0: (f32, crate::scene::ColorLinPremul),
        stop1: (f32, crate::scene::ColorLinPremul),
    },
}

pub struct PassManager {
    device: Arc<wgpu::Device>,
    pub solid_offscreen: BasicSolidRenderer,
    pub solid_direct: BasicSolidRenderer,
    pub compositor: Compositor,
    offscreen_format: wgpu::TextureFormat,
    surface_format: wgpu::TextureFormat,
    vp_buffer: wgpu::Buffer,
    bg: BackgroundRenderer,
    bg_param_buffer: wgpu::Buffer,
    bg_stops_buffer: wgpu::Buffer,
    // Platform DPI scale factor (used for mac-specific radial centering fix)
    scale_factor: f32,
}

impl PassManager {
    /// Choose the best offscreen format: Rgba16Float if supported, otherwise Rgba8Unorm
    fn choose_offscreen_format(device: &wgpu::Device) -> wgpu::TextureFormat {
        // Rgba16Float is widely supported on modern GPUs and provides better gradient quality
        // It's part of WebGPU core and doesn't require special features for render targets
        let preferred = wgpu::TextureFormat::Rgba16Float;
        
        // Try to create a small test texture to verify support
        let test_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("format-test"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: preferred,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        }));
        
        match test_result {
            Ok(_) => {
                eprintln!("Using Rgba16Float for offscreen buffer (higher quality gradients)");
                preferred
            }
            Err(_) => {
                eprintln!("Rgba16Float not supported, falling back to Rgba8Unorm for offscreen buffer");
                wgpu::TextureFormat::Rgba8Unorm
            }
        }
    }

    pub fn new(device: Arc<wgpu::Device>, target_format: wgpu::TextureFormat) -> Self {
        // Try Rgba16Float for better gradient quality, fallback to Rgba8Unorm if not supported
        let offscreen_format = Self::choose_offscreen_format(&device);
        let msaa_count = 4;
        let solid_offscreen = BasicSolidRenderer::new(device.clone(), offscreen_format, msaa_count);
        let solid_direct = BasicSolidRenderer::new(device.clone(), target_format, msaa_count);
        let compositor = Compositor::new(device.clone(), target_format);
        let bg = BackgroundRenderer::new(device.clone(), target_format);
        let vp_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viewport-uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bg_param_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("background-params"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bg_stops_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("background-stops"),
            size: 256, // 8 stops x 32 bytes
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { device, solid_offscreen, solid_direct, compositor, offscreen_format, surface_format: target_format, vp_buffer, bg, bg_param_buffer, bg_stops_buffer, scale_factor: 1.0 }
    }

    /// Set the platform DPI scale factor. On macOS this is used to correct
    /// radial gradient centering when using normalized UVs for fullscreen fills.
    pub fn set_scale_factor(&mut self, sf: f32) {
        if sf.is_finite() && sf > 0.0 {
            self.scale_factor = sf;
        } else {
            self.scale_factor = 1.0;
        }
    }

    pub fn alloc_targets(&self, allocator: &mut RenderAllocator, width: u32, height: u32) -> PassTargets {
        let color = allocator.allocate_texture(TexKey {
            width,
            height,
            format: self.offscreen_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        });
        PassTargets { color }
    }

    pub fn render_solids_to_offscreen(&self, encoder: &mut wgpu::CommandEncoder, vp_bg: &wgpu::BindGroup, targets: &PassTargets, scene: &GpuScene) {
        // Multisampled color target with resolve to offscreen color
        let msaa_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("solid-msaa-offscreen"),
            size: wgpu::Extent3d { width: targets.color.key.width, height: targets.color.key.height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format: self.offscreen_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let msaa_view = msaa_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("solid-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &msaa_view,
                resolve_target: Some(&targets.color.view),
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.solid_offscreen.record(&mut pass, vp_bg, scene);
    }

    pub fn composite_to_surface(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        offscreen: &PassTargets,
        clear: Option<wgpu::Color>,
    ) {
        let bg = self.compositor.bind_group(&self.device, &offscreen.color.view);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("composite-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: match clear { Some(c) => wgpu::LoadOp::Clear(c), None => wgpu::LoadOp::Load },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.compositor.record(&mut pass, &bg);
    }

    pub fn paint_root(&self, encoder: &mut wgpu::CommandEncoder, surface_view: &wgpu::TextureView, bg: &Background, queue: &wgpu::Queue) {
        // If solid, do a minimal clear pass
        if let Background::Solid(c) = bg {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bg-solid-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: c.r as f64, g: c.g as f64, b: c.b as f64, a: c.a as f64 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            return;
        }

        // For gradient, draw fullscreen triangle
        let (start_uv, end_uv, stop0, stop1) = match bg {
            Background::LinearGradient { start_uv, end_uv, stop0, stop1 } => (*start_uv, *end_uv, *stop0, *stop1),
            _ => unreachable!(),
        };
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct BgParams {
            start: [f32; 2],
            end: [f32; 2],
            center: [f32; 2],
            radius: f32,
            stop_count: u32,
            mode: u32,
            _pad: u32,
        }
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct Stop { pos: f32, _pad0: [f32; 3], color: [f32; 4] }

        let params = BgParams { start: start_uv, end: end_uv, center: [0.5, 0.5], radius: 1.0, stop_count: 2, mode: 1, _pad: 0 };
        let c0 = stop0.1; let c1 = stop1.1;
        let stops = [
            Stop { pos: stop0.0, _pad0: [0.0; 3], color: [c0.r, c0.g, c0.b, c0.a] },
            Stop { pos: stop1.0, _pad0: [0.0; 3], color: [c1.r, c1.g, c1.b, c1.a] },
            Stop { pos: 0.0, _pad0: [0.0;3], color: [0.0;4] },
            Stop { pos: 0.0, _pad0: [0.0;3], color: [0.0;4] },
            Stop { pos: 0.0, _pad0: [0.0;3], color: [0.0;4] },
            Stop { pos: 0.0, _pad0: [0.0;3], color: [0.0;4] },
            Stop { pos: 0.0, _pad0: [0.0;3], color: [0.0;4] },
            Stop { pos: 0.0, _pad0: [0.0;3], color: [0.0;4] },
        ];

        queue.write_buffer(&self.bg_param_buffer, 0, bytemuck::bytes_of(&params));
        queue.write_buffer(&self.bg_stops_buffer, 0, bytemuck::cast_slice(&stops));
        let bg_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg-bind"),
            layout: self.bg.bgl(),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.bg_param_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.bg_stops_buffer.as_entire_binding() },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("bg-grad-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.bg.record(&mut pass, &bg_bind);
    }

    /// Multi-stop linear gradient background
    pub fn paint_root_linear_gradient_multi(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        start_uv: [f32; 2],
        end_uv: [f32; 2],
        stops_in: &[(f32, crate::scene::ColorLinPremul)],
        queue: &wgpu::Queue,
    ) {
        // Normalize and sort stops for deterministic evaluation
        let mut sorted: Vec<(f32, crate::scene::ColorLinPremul)> = stops_in
            .iter()
            .map(|(p, c)| (p.clamp(0.0, 1.0), *c))
            .collect();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let count = sorted.len().min(8).max(2) as u32;
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct BgParams { start_end: [f32;4], center_radius_stop: [f32;4], flags: [f32;4] }
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct Stop { pos: f32, _pad0: [f32;3], color: [f32;4] }
        let mut stops: [Stop; 8] = [Stop { pos: 0.0, _pad0: [0.0;3], color: [0.0;4] }; 8];
        for (i, (p, c)) in sorted.iter().take(8).enumerate() {
            stops[i] = Stop { pos: *p, _pad0: [0.0;3], color: [c.r, c.g, c.b, c.a] };
        }
        let debug_flag = std::env::var("DEBUG_RADIAL").ok().map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);
        let params = BgParams {
            start_end: [start_uv[0], start_uv[1], end_uv[0], end_uv[1]],
            center_radius_stop: [0.5, 0.5, 1.0, count as f32],
            flags: [1.0, if debug_flag { 1.0 } else { 0.0 }, 0.0, 0.0],
        };
        queue.write_buffer(&self.bg_param_buffer, 0, bytemuck::bytes_of(&params));
        queue.write_buffer(&self.bg_stops_buffer, 0, bytemuck::cast_slice(&stops));
        let bg_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg-bind-linear"),
            layout: self.bg.bgl(),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.bg_param_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.bg_stops_buffer.as_entire_binding() },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("bg-linear-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.bg.record(&mut pass, &bg_bind);
    }

    /// Multi-stop radial gradient background
    pub fn paint_root_radial_gradient_multi(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        center_uv: [f32; 2],
        radius: f32,
        stops_in: &[(f32, crate::scene::ColorLinPremul)],
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
    ) {
        // Normalize and sort stops for deterministic evaluation
        let mut sorted: Vec<(f32, crate::scene::ColorLinPremul)> = stops_in
            .iter()
            .map(|(p, c)| (p.clamp(0.0, 1.0), *c))
            .collect();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let count = sorted.len().min(8).max(2) as u32;
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct BgParams { start_end: [f32;4], center_radius_stop: [f32;4], flags: [f32;4] }
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct Stop { pos: f32, _pad0: [f32;3], color: [f32;4] }
        let mut stops: [Stop; 8] = [Stop { pos: 0.0, _pad0: [0.0;3], color: [0.0;4] }; 8];
        for (i, (p, c)) in sorted.iter().take(8).enumerate() {
            stops[i] = Stop { pos: *p, _pad0: [0.0;3], color: [c.r, c.g, c.b, c.a] };
        }
        let debug_flag = std::env::var("DEBUG_RADIAL").ok().map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);
        let aspect_ratio = (width.max(1) as f32) / (height.max(1) as f32);
        if debug_flag {
            eprintln!("=== RADIAL GRADIENT DEBUG ===");
            eprintln!("Window size: {}x{}", width, height);
            eprintln!("Center UV: {:?}", center_uv);
            eprintln!("Radius: {}", radius);
            eprintln!("Aspect ratio: {}", aspect_ratio);
            eprintln!("Stops count: {}", count);
            eprintln!("Input stops (sorted):");
            for (i, (p, c)) in sorted.iter().take(count as usize).enumerate() {
                eprintln!("  Input {}: pos={}, ColorLinPremul(r={}, g={}, b={}, a={})", i, p, c.r, c.g, c.b, c.a);
            }
            eprintln!("Buffer stops:");
            for (i, stop) in stops.iter().take(count as usize).enumerate() {
                eprintln!("  Stop {}: pos={}, color={:?}", i, stop.pos, stop.color);
            }
        }
        // macOS-specific DPI correction: Only adjust for centered fullscreen radials.
        // When center ~ [0.5,0.5], divide center and radius by scale factor to correct
        // for retina scaling differences in UV sampling. No-op elsewhere.
        let mut adj_center = center_uv;
        let mut adj_radius = radius;
        #[cfg(target_os = "macos")]
        {
            let sf = self.scale_factor.max(1.0);
            // Within ~1e-3 of exact center counts as centered
            if (adj_center[0] - 0.5).abs() < 1e-3 && (adj_center[1] - 0.5).abs() < 1e-3 {
                adj_center = [adj_center[0] / sf, adj_center[1] / sf];
                adj_radius = adj_radius / sf;
                if debug_flag {
                    eprintln!("macOS DPI correction applied: sf={}, adj_center={:?}, adj_radius={}", sf, adj_center, adj_radius);
                }
            }
        }
        let params = BgParams {
            start_end: [0.0, 0.0, 1.0, 1.0],
            center_radius_stop: [adj_center[0], adj_center[1], adj_radius, count as f32],
            flags: [2.0, if debug_flag { 1.0 } else { 0.0 }, aspect_ratio, 0.0],
        };
        queue.write_buffer(&self.bg_param_buffer, 0, bytemuck::bytes_of(&params));
        queue.write_buffer(&self.bg_stops_buffer, 0, bytemuck::cast_slice(&stops));
        let bg_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg-bind-radial"),
            layout: self.bg.bgl(),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.bg_param_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.bg_stops_buffer.as_entire_binding() },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("bg-radial-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.bg.record(&mut pass, &bg_bind);
    }

    /// Convenience: paint a solid background color directly to the surface.
    pub fn paint_root_color(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        color: crate::scene::ColorLinPremul,
    ) {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("bg-solid-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: color.r as f64, g: color.g as f64, b: color.b as f64, a: color.a as f64 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
    }

    /// Convenience: paint a simple 2-stop linear gradient to the surface.
    pub fn paint_root_gradient(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        start_uv: [f32; 2],
        end_uv: [f32; 2],
        stop0: (f32, crate::scene::ColorLinPremul),
        stop1: (f32, crate::scene::ColorLinPremul),
        queue: &wgpu::Queue,
    ) {
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct BgData { start_end: [f32;4], center_radius_stop: [f32;4], flags: [f32;4] }
        let c0 = stop0.1;
        let c1 = stop1.1;
        // Reuse the multi-stop layout by writing two stops into the stops buffer
        let debug_flag = std::env::var("DEBUG_RADIAL").ok().map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);
        let params = BgData {
            start_end: [start_uv[0], start_uv[1], end_uv[0], end_uv[1]],
            center_radius_stop: [0.5, 0.5, 1.0, 2.0],
            flags: [1.0, if debug_flag { 1.0 } else { 0.0 }, 0.0, 0.0],
        };
        // Populate first two stops in the stop buffer for the simple gradient helper
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct Stop { pos: f32, _pad0: [f32;3], color: [f32;4] }
        let stops: [Stop; 2] = [
            Stop { pos: stop0.0, _pad0: [0.0;3], color: [c0.r, c0.g, c0.b, c0.a] },
            Stop { pos: stop1.0, _pad0: [0.0;3], color: [c1.r, c1.g, c1.b, c1.a] },
        ];
        queue.write_buffer(&self.bg_param_buffer, 0, bytemuck::bytes_of(&params));
        queue.write_buffer(&self.bg_stops_buffer, 0, bytemuck::cast_slice(&stops));
        let bg_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg-bind"),
            layout: self.bg.bgl(),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.bg_param_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.bg_stops_buffer.as_entire_binding() },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("bg-grad-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.bg.record(&mut pass, &bg_bind);
    }

    /// Render the scene either via offscreen+composite, or directly to the surface when `direct` is true.
    pub fn render_frame(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        allocator: &mut RenderAllocator,
        surface_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        scene: &GpuScene,
        clear: wgpu::Color,
        direct: bool,
        queue: &wgpu::Queue,
        preserve_surface: bool,
    ) {
        // Update viewport uniform
        let scale = [2.0f32 / (width.max(1) as f32), -2.0f32 / (height.max(1) as f32)];
        let translate = [-1.0f32, 1.0f32];
        let vp_data = [scale[0], scale[1], translate[0], translate[1]];
        let data = bytemuck::bytes_of(&vp_data);
        queue.write_buffer(&self.vp_buffer, 0, data);
        let vp_bg_off = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vp-bg-offscreen"),
            layout: self.solid_offscreen.viewport_bgl(),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: self.vp_buffer.as_entire_binding() }],
        });
        let vp_bg_direct = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vp-bg-direct"),
            layout: self.solid_direct.viewport_bgl(),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: self.vp_buffer.as_entire_binding() }],
        });
        if direct {
            // MSAA render directly to surface with resolve
            let msaa_tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("solid-msaa-direct"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 4,
                dimension: wgpu::TextureDimension::D2,
                format: self.surface_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let msaa_view = msaa_tex.create_view(&wgpu::TextureViewDescriptor::default());
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("direct-solid-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &msaa_view,
                    resolve_target: Some(surface_view),
                    ops: wgpu::Operations {
                        load: if preserve_surface { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(clear) },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.solid_direct.record(&mut pass, &vp_bg_direct, scene);
            return;
        }

        let targets = self.alloc_targets(allocator, width.max(1), height.max(1));
        self.render_solids_to_offscreen(encoder, &vp_bg_off, &targets, scene);
        self.composite_to_surface(encoder, surface_view, &targets, if preserve_surface { None } else { Some(clear) });
        // Return textures to allocator pool to avoid repeated allocations.
        allocator.release_texture(targets.color);
    }
}
