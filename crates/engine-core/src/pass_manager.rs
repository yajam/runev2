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
    vp_buffer: wgpu::Buffer,
    bg: BackgroundRenderer,
    bg_buffer: wgpu::Buffer,
}

impl PassManager {
    pub fn new(device: Arc<wgpu::Device>, target_format: wgpu::TextureFormat) -> Self {
        let offscreen_format = wgpu::TextureFormat::Rgba8Unorm; // linear offscreen
        let solid_offscreen = BasicSolidRenderer::new(device.clone(), offscreen_format);
        let solid_direct = BasicSolidRenderer::new(device.clone(), target_format);
        let compositor = Compositor::new(device.clone(), target_format);
        let bg = BackgroundRenderer::new(device.clone(), target_format);
        let vp_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viewport-uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bg_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("background-uniform"),
            size: 96, // plenty for struct packing
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { device, solid_offscreen, solid_direct, compositor, offscreen_format, vp_buffer, bg, bg_buffer }
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
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("solid-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &targets.color.view,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.solid_offscreen.record(&mut pass, vp_bg, scene);
    }

    pub fn composite_to_surface(&self, encoder: &mut wgpu::CommandEncoder, surface_view: &wgpu::TextureView, offscreen: &PassTargets, clear: wgpu::Color) {
        let bg = self.compositor.bind_group(&self.device, &offscreen.color.view);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("composite-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
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
        struct BgData {
            start: [f32; 2],
            end: [f32; 2],
            color_a: [f32; 4],
            color_b: [f32; 4],
            pos_a: f32,
            pos_b: f32,
            mode: u32,
            _pad: u32,
        }
        let c0 = stop0.1;
        let c1 = stop1.1;
        let data = BgData {
            start: start_uv,
            end: end_uv,
            color_a: [c0.r, c0.g, c0.b, c0.a],
            color_b: [c1.r, c1.g, c1.b, c1.a],
            pos_a: stop0.0,
            pos_b: stop1.0,
            mode: 1,
            _pad: 0,
        };
        queue.write_buffer(&self.bg_buffer, 0, bytemuck::bytes_of(&data));
        let bg_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg-bind"),
            layout: self.bg.bgl(),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: self.bg_buffer.as_entire_binding() }],
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
        struct BgData {
            start: [f32; 2],
            end: [f32; 2],
            color_a: [f32; 4],
            color_b: [f32; 4],
            pos_a: f32,
            pos_b: f32,
            mode: u32,
            _pad: u32,
        }
        let c0 = stop0.1;
        let c1 = stop1.1;
        let data = BgData {
            start: start_uv,
            end: end_uv,
            color_a: [c0.r, c0.g, c0.b, c0.a],
            color_b: [c1.r, c1.g, c1.b, c1.a],
            pos_a: stop0.0,
            pos_b: stop1.0,
            mode: 1,
            _pad: 0,
        };
        queue.write_buffer(&self.bg_buffer, 0, bytemuck::bytes_of(&data));
        let bg_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg-bind"),
            layout: self.bg.bgl(),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: self.bg_buffer.as_entire_binding() }],
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
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("direct-solid-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
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
        self.composite_to_surface(encoder, surface_view, &targets, clear);
        // Return textures to allocator pool to avoid repeated allocations.
        allocator.release_texture(targets.color);
    }
}
