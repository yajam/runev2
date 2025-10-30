use std::sync::Arc;

// use anyhow::Result;

use crate::allocator::{RenderAllocator, TexKey};
// use crate::display_list::{Command, DisplayList, Viewport};
use crate::pipeline::{
    BackgroundRenderer, BasicSolidRenderer, Blitter, BlurRenderer, Compositor,
    ShadowCompositeRenderer, TextRenderer,
};
use crate::scene::{BoxShadowSpec, RoundedRadii, RoundedRect};
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
    pub solid_direct_no_msaa: BasicSolidRenderer,
    pub compositor: Compositor,
    pub blitter: Blitter,
    // Shadow/blur pipelines and helpers
    pub mask_renderer: BasicSolidRenderer,
    pub blur_r8: BlurRenderer,
    pub shadow_comp: ShadowCompositeRenderer,
    pub text: TextRenderer,
    pub image: crate::pipeline::ImageRenderer,
    offscreen_format: wgpu::TextureFormat,
    surface_format: wgpu::TextureFormat,
    vp_buffer: wgpu::Buffer,
    bg: BackgroundRenderer,
    bg_param_buffer: wgpu::Buffer,
    bg_stops_buffer: wgpu::Buffer,
    // Platform DPI scale factor (used for mac-specific radial centering fix)
    scale_factor: f32,
    // Intermediate texture for Vello-style smooth resizing
    pub intermediate_texture: Option<crate::OwnedTexture>,
}

impl PassManager {
    /// Choose the best offscreen format: Rgba16Float if supported, otherwise Rgba8Unorm
    fn choose_offscreen_format(device: &wgpu::Device) -> wgpu::TextureFormat {
        // WORKAROUND: Use Rgba8UnormSrgb instead of Rgba16Float due to Metal blending bug
        let preferred = wgpu::TextureFormat::Rgba8UnormSrgb;

        // Try to create a small test texture to verify support
        let test_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("format-test"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
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
                eprintln!("Using {:?} for offscreen buffer", preferred);
                preferred
            }
            Err(_) => {
                eprintln!(
                    "{:?} not supported, falling back to Rgba8Unorm for offscreen buffer",
                    preferred
                );
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
        let solid_direct_no_msaa = BasicSolidRenderer::new(device.clone(), target_format, 1);
        let compositor = Compositor::new(device.clone(), target_format);
        let blitter = Blitter::new(device.clone(), target_format);
        // Shadow/blur pipelines
        let mask_renderer =
            BasicSolidRenderer::new(device.clone(), wgpu::TextureFormat::R8Unorm, 1);
        let blur_r8 = BlurRenderer::new(device.clone(), wgpu::TextureFormat::R8Unorm);
        let shadow_comp = ShadowCompositeRenderer::new(device.clone(), target_format);
        let text = TextRenderer::new(device.clone(), target_format);
        let image = crate::pipeline::ImageRenderer::new(device.clone(), target_format);
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
        Self {
            device,
            solid_offscreen,
            solid_direct,
            solid_direct_no_msaa,
            compositor,
            blitter,
            mask_renderer,
            blur_r8,
            shadow_comp,
            text,
            image,
            offscreen_format,
            surface_format: target_format,
            vp_buffer,
            bg,
            bg_param_buffer,
            bg_stops_buffer,
            scale_factor: 1.0,
            intermediate_texture: None,
        }
    }

    /// Expose the device for scenes that need to create textures.
    pub fn device(&self) -> Arc<wgpu::Device> { self.device.clone() }

    /// Render an image texture to the target at origin with size (in pixels, y-down).
    /// Expects `tex_view` to be created from an `Rgba8UnormSrgb` texture for proper sRGB sampling.
    pub fn draw_image_quad(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        origin: [f32; 2],
        size: [f32; 2],
        tex_view: &wgpu::TextureView,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
    ) {
        // Update viewport uniform based on render target dimensions
        let scale = [
            2.0f32 / (width.max(1) as f32),
            -2.0f32 / (height.max(1) as f32),
        ];
        let translate = [-1.0f32, 1.0f32];
        let vp_data = [scale[0], scale[1], translate[0], translate[1]];
        queue.write_buffer(&self.vp_buffer, 0, bytemuck::bytes_of(&vp_data));

        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct QuadVtx { pos: [f32; 2], uv: [f32; 2] }
        let x = origin[0];
        let y = origin[1];
        let w = size[0].max(0.0);
        let h = size[1].max(0.0);
        let verts = [
            QuadVtx { pos: [x,     y    ], uv: [0.0, 0.0] },
            QuadVtx { pos: [x + w, y    ], uv: [1.0, 0.0] },
            QuadVtx { pos: [x + w, y + h], uv: [1.0, 1.0] },
            QuadVtx { pos: [x,     y + h], uv: [0.0, 1.0] },
        ];
        let idx: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let vsize = (verts.len() * std::mem::size_of::<QuadVtx>()) as u64;
        let isize = (idx.len() * std::mem::size_of::<u16>()) as u64;
        let vbuf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("image-vbuf"),
            size: vsize.max(4),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ibuf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("image-ibuf"),
            size: isize.max(4),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if vsize > 0 { queue.write_buffer(&vbuf, 0, bytemuck::cast_slice(&verts)); }
        if isize > 0 { queue.write_buffer(&ibuf, 0, bytemuck::cast_slice(&idx)); }

        let vp_bg = self.image.vp_bind_group(&self.device, &self.vp_buffer);
        let tex_bg = self.image.tex_bind_group(&self.device, tex_view);

        // Preserve existing target content; draw over it
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("image-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.image.record(&mut pass, &vp_bg, &tex_bg, &vbuf, &ibuf, idx.len() as u32);
    }

    /// Draw an RGB subpixel coverage text mask tinted with the given premultiplied color
    /// onto the specified target view at a pixel-space rectangle (y-down).
    /// Supports RGBA8 and RGBA16 mask textures.
    pub fn draw_text_mask(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        origin: [f32; 2],
        mask: &crate::text::SubpixelMask,
        color: crate::scene::ColorLinPremul,
        queue: &wgpu::Queue,
    ) {
        // Upload color
        let c = [color.r, color.g, color.b, color.a];
        queue.write_buffer(&self.text.color_buffer, 0, bytemuck::bytes_of(&c));

        // Create mask texture and upload
        let format = match mask.format { crate::text::MaskFormat::Rgba8 => wgpu::TextureFormat::Rgba8Unorm, crate::text::MaskFormat::Rgba16 => wgpu::TextureFormat::Rgba16Unorm };
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("text-mask"),
            size: wgpu::Extent3d { width: mask.width.max(1), height: mask.height.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &mask.data,
            wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some((mask.width * (mask.bytes_per_pixel() as u32)) as u32), rows_per_image: Some(mask.height as u32) },
            wgpu::Extent3d { width: mask.width, height: mask.height, depth_or_array_layers: 1 },
        );
        let tex_view = tex.create_view(&wgpu::TextureViewDescriptor::default());

        // Build quad vertices in pixel space with UVs
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct QuadVtx { pos: [f32; 2], uv: [f32; 2] }
        let x = origin[0];
        let y = origin[1];
        let w = mask.width as f32;
        let h = mask.height as f32;
        let verts = [
            QuadVtx { pos: [x, y],         uv: [0.0, 0.0] },
            QuadVtx { pos: [x + w, y],     uv: [1.0, 0.0] },
            QuadVtx { pos: [x + w, y + h], uv: [1.0, 1.0] },
            QuadVtx { pos: [x, y + h],     uv: [0.0, 1.0] },
        ];
        let idx: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let vsize = (verts.len() * std::mem::size_of::<QuadVtx>()) as u64;
        let isize = (idx.len() * std::mem::size_of::<u16>()) as u64;
        let vbuf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text-vbuf"),
            size: vsize.max(4),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ibuf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text-ibuf"),
            size: isize.max(4),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if vsize > 0 { queue.write_buffer(&vbuf, 0, bytemuck::cast_slice(&verts)); }
        if isize > 0 { queue.write_buffer(&ibuf, 0, bytemuck::cast_slice(&idx)); }

        // Viewport bind group
        let vp_bg = self.text.vp_bind_group(&self.device, &self.vp_buffer);
        let tex_bg = self.text.tex_bind_group(&self.device, &tex_view);

        // Render pass that preserves existing content
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("text-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.text.record(&mut pass, &vp_bg, &tex_bg, &vbuf, &ibuf, idx.len() as u32);
        // temp buffers and texture are dropped at end of scope
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

    pub fn alloc_targets(
        &self,
        allocator: &mut RenderAllocator,
        width: u32,
        height: u32,
    ) -> PassTargets {
        let color = allocator.allocate_texture(TexKey {
            width,
            height,
            format: self.offscreen_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        });
        PassTargets { color }
    }

    /// Allocate or reuse intermediate texture matching the surface size.
    /// This texture is used for Vello-style smooth resizing.
    pub fn ensure_intermediate_texture(
        &mut self,
        allocator: &mut RenderAllocator,
        width: u32,
        height: u32,
    ) {
        let needs_realloc = match &self.intermediate_texture {
            Some(tex) => tex.key.width != width || tex.key.height != height,
            None => true,
        };

        if needs_realloc {
            // Release old texture if it exists
            if let Some(old_tex) = self.intermediate_texture.take() {
                allocator.release_texture(old_tex);
            }

            // Allocate new intermediate texture with surface format
            let tex = allocator.allocate_texture(TexKey {
                width,
                height,
                format: self.surface_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
            });
            self.intermediate_texture = Some(tex);
        }
    }

    /// Clear the intermediate texture with the specified color.
    /// This should be called before rendering to the intermediate texture.
    pub fn clear_intermediate_texture(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        clear_color: wgpu::Color,
    ) {
        let intermediate = self
            .intermediate_texture
            .as_ref()
            .expect("intermediate texture must be allocated before clearing");

        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear-intermediate"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &intermediate.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
    }

    /// Blit the intermediate texture to the surface. This is a very fast operation
    /// that enables smooth window resizing (Vello-style).
    pub fn blit_to_surface(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
    ) {
        let intermediate = self
            .intermediate_texture
            .as_ref()
            .expect("intermediate texture must be allocated before blitting");

        let bg = self.blitter.bind_group(&self.device, &intermediate.view);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("blit-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.blitter.record(&mut pass, &bg);
    }

    /// Iterate the display list and render all DrawText runs using the given provider to the target_view.
    pub fn render_text_for_list(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        list: &crate::display_list::DisplayList,
        queue: &wgpu::Queue,
        provider: &dyn crate::text::TextProvider,
    ) {
        // Update viewport uniform based on list viewport
        let scale = [
            2.0f32 / (list.viewport.width.max(1) as f32),
            -2.0f32 / (list.viewport.height.max(1) as f32),
        ];
        let translate = [-1.0f32, 1.0f32];
        let vp_data = [scale[0], scale[1], translate[0], translate[1]];
        queue.write_buffer(&self.vp_buffer, 0, bytemuck::bytes_of(&vp_data));

        // Optional: round run X position to whole pixels for diagnostics
        let snap_x = std::env::var("DEMO_SNAP_X")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        for cmd in &list.commands {
            if let crate::display_list::Command::DrawText { run, transform, .. } = cmd {
                let [_a, _b, _c, _d, e, f] = transform.m;
                let rx = run.pos[0] + e;
                let sf = if self.scale_factor.is_finite() && self.scale_factor > 0.0 { self.scale_factor } else { 1.0 };
                let snap = |v: f32| -> f32 { (v * sf).round() / sf };
                let run_origin_x = if snap_x { snap(rx) } else { rx };
                // Snap baseline using line metrics ascent so descenders remain stable
                let baseline_y = if let Some(m) = provider.line_metrics(run.size) {
                    let asc = m.ascent;
                    snap(run.pos[1] + f + asc) - asc
                } else {
                    snap(run.pos[1] + f)
                };
                for rg in provider.rasterize_run(run) {
                    let mut origin = [run_origin_x + rg.offset[0], baseline_y + rg.offset[1]];
                    // Pseudo-hinting for small sizes
                    if run.size <= 15.0 {
                        origin[0] = snap(origin[0]);
                        origin[1] = snap(origin[1]);
                    }
                    self.draw_text_mask(
                        encoder,
                        target_view,
                        list.viewport.width,
                        list.viewport.height,
                        origin,
                        &rg.mask,
                        run.color,
                        queue,
                    );
                }
            }
        }
    }

    /// Draw a box shadow for a rounded rect using an R8 mask + separable Gaussian blur pipeline.
    /// This composes the tinted shadow beneath current content on the target view.
    pub fn draw_box_shadow(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        rrect: RoundedRect,
        spec: BoxShadowSpec,
        queue: &wgpu::Queue,
    ) {
        // --- 1) Calibrate parameters ---
        // Soften falloff: browsers feel closer to sigma ≈ blur_radius
        // Larger sigma reduces the "band" look and increases penumbra.
        let blur = spec.blur_radius.max(0.0);
        let sigma = if blur > 0.0 { blur } else { 0.5 };
        let spread = spec.spread.max(0.0);
        let create_tex = |label: &str| -> wgpu::Texture {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: width.max(1),
                    height: height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };
        let mask_tex = create_tex("shadow-mask");
        let ping_tex = create_tex("shadow-ping");
        let mask_view = mask_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let ping_view = ping_tex.create_view(&wgpu::TextureViewDescriptor::default());

        // Viewport for full target size (y-down)
        let scale = [
            2.0f32 / (width.max(1) as f32),
            -2.0f32 / (height.max(1) as f32),
        ];
        let translate = [-1.0f32, 1.0f32];
        let vp_data = [scale[0], scale[1], translate[0], translate[1]];
        queue.write_buffer(&self.vp_buffer, 0, bytemuck::bytes_of(&vp_data));

        let shadow_radii = RoundedRadii {
            tl: (rrect.radii.tl + spread).max(0.0),
            tr: (rrect.radii.tr + spread).max(0.0),
            br: (rrect.radii.br + spread).max(0.0),
            bl: (rrect.radii.bl + spread).max(0.0),
        };
        // Expand source to give blur room so the outer halo is broad enough.
        // Slightly higher multiplier works better with the wider blur support above.
        let expand = spread + 1.8 * sigma + 1.0;
        let mut rect = rrect.rect;
        rect.x = rect.x + spec.offset[0] - expand;
        rect.y = rect.y + spec.offset[1] - expand;
        rect.w = (rect.w + 2.0 * expand).max(0.0);
        rect.h = (rect.h + 2.0 * expand).max(0.0);
        let expanded = RoundedRect {
            rect,
            radii: shadow_radii,
        };
        // Render with white for the shadow shape
        // Build vertices/indices for expanded rounded rect (fill)
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct Vtx {
            pos: [f32; 2],
            color: [f32; 4],
        }
        let mut vertices: Vec<Vtx> = Vec::new();
        let mut indices: Vec<u16> = Vec::new();
        let rect = expanded.rect;
        let tl = expanded.radii.tl.min(rect.w * 0.5).min(rect.h * 0.5);
        let tr = expanded.radii.tr.min(rect.w * 0.5).min(rect.h * 0.5);
        let br = expanded.radii.br.min(rect.w * 0.5).min(rect.h * 0.5);
        let bl = expanded.radii.bl.min(rect.w * 0.5).min(rect.h * 0.5);
        // Higher tessellation for smoother rounded corners (reduces polygonal artifacts before blur)
        let segs = 64u32;
        let mut ring: Vec<[f32; 2]> = Vec::new();
        fn arc_append(
            ring: &mut Vec<[f32; 2]>,
            c: [f32; 2],
            r: f32,
            start: f32,
            end: f32,
            segs: u32,
            include_start: bool,
        ) {
            if r <= 0.0 {
                return;
            }
            for i in 0..=segs {
                if i == 0 && !include_start {
                    continue;
                }
                let t = (i as f32) / (segs as f32);
                let ang = start + t * (end - start);
                let p = [c[0] + r * ang.cos(), c[1] - r * ang.sin()];
                ring.push(p);
            }
        }
        if tl > 0.0 {
            arc_append(
                &mut ring,
                [rect.x + tl, rect.y + tl],
                tl,
                std::f32::consts::FRAC_PI_2,
                std::f32::consts::PI,
                segs,
                true,
            );
        } else {
            ring.push([rect.x + 0.0, rect.y + 0.0]);
        }
        if bl > 0.0 {
            arc_append(
                &mut ring,
                [rect.x + bl, rect.y + rect.h - bl],
                bl,
                std::f32::consts::PI,
                std::f32::consts::FRAC_PI_2 * 3.0,
                segs,
                true,
            );
        } else {
            ring.push([rect.x + 0.0, rect.y + rect.h]);
        }
        if br > 0.0 {
            arc_append(
                &mut ring,
                [rect.x + rect.w - br, rect.y + rect.h - br],
                br,
                std::f32::consts::FRAC_PI_2 * 3.0,
                std::f32::consts::TAU,
                segs,
                true,
            );
        } else {
            ring.push([rect.x + rect.w, rect.y + rect.h]);
        }
        if tr > 0.0 {
            arc_append(
                &mut ring,
                [rect.x + rect.w - tr, rect.y + tr],
                tr,
                0.0,
                std::f32::consts::FRAC_PI_2,
                segs,
                true,
            );
        } else {
            ring.push([rect.x + rect.w, rect.y + 0.0]);
        }
        let center = [rect.x + rect.w * 0.5, rect.y + rect.h * 0.5];
        let white = [1.0, 1.0, 1.0, 1.0];
        let base = vertices.len() as u16;
        vertices.push(Vtx {
            pos: center,
            color: white,
        });
        for p in ring.iter() {
            vertices.push(Vtx {
                pos: *p,
                color: white,
            });
        }
        let ring_len = (vertices.len() as u16) - base - 1;
        for i in 0..ring_len {
            let i0 = base;
            let i1 = base + 1 + i;
            let i2 = base + 1 + ((i + 1) % ring_len);
            indices.extend_from_slice(&[i0, i1, i2]);
        }
        // Create GPU buffers directly
        let vsize = (vertices.len() * std::mem::size_of::<Vtx>()) as u64;
        let isize = (indices.len() * std::mem::size_of::<u16>()) as u64;
        let vbuf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow-mask-vbuf"),
            size: vsize.max(4),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ibuf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow-mask-ibuf"),
            size: isize.max(4),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if vsize > 0 {
            queue.write_buffer(&vbuf, 0, bytemuck::cast_slice(&vertices));
        }
        if isize > 0 {
            queue.write_buffer(&ibuf, 0, bytemuck::cast_slice(&indices));
        }
        let gpu = crate::upload::GpuScene {
            vertex: crate::allocator::OwnedBuffer {
                buffer: vbuf,
                key: crate::allocator::BufKey {
                    size: vsize.max(4),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                },
            },
            index: crate::allocator::OwnedBuffer {
                buffer: ibuf,
                key: crate::allocator::BufKey {
                    size: isize.max(4),
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                },
            },
            vertices: vertices.len() as u32,
            indices: indices.len() as u32,
        };

        // Bind groups for viewport
        let vp_bg_mask = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vp-bg-mask"),
            layout: self.mask_renderer.viewport_bgl(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.vp_buffer.as_entire_binding(),
            }],
        });
        // Render mask shape to R8 texture
        // Clear to BLACK, render WHITE for shadow shape
        // After blur: soft white blob. After cutout: white ring (shadow area)
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow-mask-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &mask_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.mask_renderer.record(&mut pass, &vp_bg_mask, &gpu);
        }

        // Horizontal blur (mask -> ping)
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct BlurParams {
            dir: [f32; 2],
            texel: [f32; 2],
            sigma: f32,
            _pad: f32,
        }
        let texel = [
            1.0f32 / (width.max(1) as f32),
            1.0f32 / (height.max(1) as f32),
        ];
        let bp_h = BlurParams {
            dir: [1.0, 0.0],
            texel,
            sigma,
            _pad: 0.0,
        };
        queue.write_buffer(&self.blur_r8.param_buffer, 0, bytemuck::bytes_of(&bp_h));
        let bg_h = self.blur_r8.bind_group(&self.device, &mask_view);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow-blur-h"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &ping_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.blur_r8.record(&mut pass, &bg_h);
        }

        // Vertical blur (ping -> mask)
        let bp_v = BlurParams {
            dir: [0.0, 1.0],
            texel,
            sigma,
            _pad: 0.0,
        };
        queue.write_buffer(&self.blur_r8.param_buffer, 0, bytemuck::bytes_of(&bp_v));
        let bg_v = self.blur_r8.bind_group(&self.device, &ping_view);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow-blur-v"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &mask_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.blur_r8.record(&mut pass, &bg_v);
        }

        // Step 5: Cut out the ORIGINAL shape (at original position, no offset)
        // This prevents the shadow from showing through semi-transparent elements
        {
            let mut cutout_vertices: Vec<Vtx> = Vec::new();
            let mut cutout_indices: Vec<u16> = Vec::new();
            // Use ORIGINAL rect (no spread/offset) in full target space
            let rect = rrect.rect;
            let tl = rrect.radii.tl.min(rect.w * 0.5).min(rect.h * 0.5);
            let tr = rrect.radii.tr.min(rect.w * 0.5).min(rect.h * 0.5);
            let br = rrect.radii.br.min(rect.w * 0.5).min(rect.h * 0.5);
            let bl = rrect.radii.bl.min(rect.w * 0.5).min(rect.h * 0.5);
            let mut ring: Vec<[f32; 2]> = Vec::new();
            if tl > 0.0 {
                arc_append(
                    &mut ring,
                    [rect.x + tl, rect.y + tl],
                    tl,
                    std::f32::consts::FRAC_PI_2,
                    std::f32::consts::PI,
                    segs,
                    true,
                );
            } else {
                ring.push([rect.x, rect.y]);
            }
            if bl > 0.0 {
                arc_append(
                    &mut ring,
                    [rect.x + bl, rect.y + rect.h - bl],
                    bl,
                    std::f32::consts::PI,
                    std::f32::consts::FRAC_PI_2 * 3.0,
                    segs,
                    true,
                );
            } else {
                ring.push([rect.x, rect.y + rect.h]);
            }
            if br > 0.0 {
                arc_append(
                    &mut ring,
                    [rect.x + rect.w - br, rect.y + rect.h - br],
                    br,
                    std::f32::consts::FRAC_PI_2 * 3.0,
                    std::f32::consts::TAU,
                    segs,
                    true,
                );
            } else {
                ring.push([rect.x + rect.w, rect.y + rect.h]);
            }
            if tr > 0.0 {
                arc_append(
                    &mut ring,
                    [rect.x + rect.w - tr, rect.y + tr],
                    tr,
                    0.0,
                    std::f32::consts::FRAC_PI_2,
                    segs,
                    true,
                );
            } else {
                ring.push([rect.x + rect.w, rect.y]);
            }
            let center = [rect.x + rect.w * 0.5, rect.y + rect.h * 0.5];
            // Use transparent (alpha=0) to clear the mask area
            // With premultiplied alpha: result = src * src.a + dst * (1 - src.a) = 0 * 0 + dst * 1 = dst
            // That won't work! We need alpha=1 to replace: result = src * 1 + dst * 0 = src
            // For R8, we want to write 0.0, so use black with alpha=1
            let clear_color = [0.0, 0.0, 0.0, 1.0];
            let base = cutout_vertices.len() as u16;
            cutout_vertices.push(Vtx {
                pos: center,
                color: clear_color,
            });
            for p in ring.iter() {
                cutout_vertices.push(Vtx {
                    pos: *p,
                    color: clear_color,
                });
            }
            let ring_len = (cutout_vertices.len() as u16) - base - 1;
            for i in 0..ring_len {
                let i0 = base;
                let i1 = base + 1 + i;
                let i2 = base + 1 + ((i + 1) % ring_len);
                cutout_indices.extend_from_slice(&[i0, i1, i2]);
            }

            let vsize = (cutout_vertices.len() * std::mem::size_of::<Vtx>()) as u64;
            let isize = (cutout_indices.len() * std::mem::size_of::<u16>()) as u64;
            let vbuf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("shadow-cutout-vbuf"),
                size: vsize.max(4),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let ibuf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("shadow-cutout-ibuf"),
                size: isize.max(4),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            if vsize > 0 {
                queue.write_buffer(&vbuf, 0, bytemuck::cast_slice(&cutout_vertices));
            }
            if isize > 0 {
                queue.write_buffer(&ibuf, 0, bytemuck::cast_slice(&cutout_indices));
            }
            let cutout_gpu = crate::upload::GpuScene {
                vertex: crate::allocator::OwnedBuffer {
                    buffer: vbuf,
                    key: crate::allocator::BufKey {
                        size: vsize.max(4),
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    },
                },
                index: crate::allocator::OwnedBuffer {
                    buffer: ibuf,
                    key: crate::allocator::BufKey {
                        size: isize.max(4),
                        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    },
                },
                vertices: cutout_vertices.len() as u32,
                indices: cutout_indices.len() as u32,
            };

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow-cutout"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &mask_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.mask_renderer
                .record(&mut pass, &vp_bg_mask, &cutout_gpu);
        }

        // Composite tinted shadow to target using premultiplied color
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct ShadowColor {
            color: [f32; 4],
        }
        let c = spec.color;
        let scol = ShadowColor {
            color: [c.r, c.g, c.b, c.a],
        };
        queue.write_buffer(&self.shadow_comp.color_buffer, 0, bytemuck::bytes_of(&scol));
        let bg = self.shadow_comp.bind_group(&self.device, &mask_view);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow-composite"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.shadow_comp.record(&mut pass, &bg);
        }

        // Temp textures are dropped at end of scope
    }

    /// Draw a filled rounded rectangle directly onto the target using the solid_direct pipeline.
    /// Uses premultiplied linear color.
    pub fn draw_filled_rounded_rect(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        rrect: RoundedRect,
        color: crate::scene::ColorLinPremul,
        queue: &wgpu::Queue,
    ) {
        // Update viewport uniform
        let scale = [
            2.0f32 / (width.max(1) as f32),
            -2.0f32 / (height.max(1) as f32),
        ];
        let translate = [-1.0f32, 1.0f32];
        let vp_data = [scale[0], scale[1], translate[0], translate[1]];
        queue.write_buffer(&self.vp_buffer, 0, bytemuck::bytes_of(&vp_data));

        // Tessellate rounded rect fill
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct Vtx {
            pos: [f32; 2],
            color: [f32; 4],
        }
        let mut vertices: Vec<Vtx> = Vec::new();
        let mut indices: Vec<u16> = Vec::new();
        let rect = rrect.rect;
        let tl = rrect.radii.tl.min(rect.w * 0.5).min(rect.h * 0.5);
        let tr = rrect.radii.tr.min(rect.w * 0.5).min(rect.h * 0.5);
        let br = rrect.radii.br.min(rect.w * 0.5).min(rect.h * 0.5);
        let bl = rrect.radii.bl.min(rect.w * 0.5).min(rect.h * 0.5);
        let segs = 64u32;
        let mut ring: Vec<[f32; 2]> = Vec::new();
        fn arc_append(
            ring: &mut Vec<[f32; 2]>,
            c: [f32; 2],
            r: f32,
            start: f32,
            end: f32,
            segs: u32,
            include_start: bool,
        ) {
            if r <= 0.0 {
                return;
            }
            for i in 0..=segs {
                if i == 0 && !include_start {
                    continue;
                }
                let t = (i as f32) / (segs as f32);
                let ang = start + t * (end - start);
                let p = [c[0] + r * ang.cos(), c[1] - r * ang.sin()];
                ring.push(p);
            }
        }
        if tl > 0.0 {
            arc_append(
                &mut ring,
                [rect.x + tl, rect.y + tl],
                tl,
                std::f32::consts::FRAC_PI_2,
                std::f32::consts::PI,
                segs,
                true,
            );
        } else {
            ring.push([rect.x + 0.0, rect.y + 0.0]);
        }
        if bl > 0.0 {
            arc_append(
                &mut ring,
                [rect.x + bl, rect.y + rect.h - bl],
                bl,
                std::f32::consts::PI,
                std::f32::consts::FRAC_PI_2 * 3.0,
                segs,
                true,
            );
        } else {
            ring.push([rect.x + 0.0, rect.y + rect.h]);
        }
        if br > 0.0 {
            arc_append(
                &mut ring,
                [rect.x + rect.w - br, rect.y + rect.h - br],
                br,
                std::f32::consts::FRAC_PI_2 * 3.0,
                std::f32::consts::TAU,
                segs,
                true,
            );
        } else {
            ring.push([rect.x + rect.w, rect.y + rect.h]);
        }
        if tr > 0.0 {
            arc_append(
                &mut ring,
                [rect.x + rect.w - tr, rect.y + tr],
                tr,
                0.0,
                std::f32::consts::FRAC_PI_2,
                segs,
                true,
            );
        } else {
            ring.push([rect.x + rect.w, rect.y + 0.0]);
        }
        let center = [rect.x + rect.w * 0.5, rect.y + rect.h * 0.5];
        let col = [color.r, color.g, color.b, color.a];
        let base = vertices.len() as u16;
        vertices.push(Vtx {
            pos: center,
            color: col,
        });
        for p in ring.iter() {
            vertices.push(Vtx {
                pos: *p,
                color: col,
            });
        }
        let ring_len = (vertices.len() as u16) - base - 1;
        for i in 0..ring_len {
            let i0 = base;
            let i1 = base + 1 + i;
            let i2 = base + 1 + ((i + 1) % ring_len);
            indices.extend_from_slice(&[i0, i1, i2]);
        }

        // Create GPU buffers
        let vsize = (vertices.len() * std::mem::size_of::<Vtx>()) as u64;
        let isize = (indices.len() * std::mem::size_of::<u16>()) as u64;
        let vbuf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rounded-rect-fill-vbuf"),
            size: vsize.max(4),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ibuf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rounded-rect-fill-ibuf"),
            size: isize.max(4),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if vsize > 0 {
            queue.write_buffer(&vbuf, 0, bytemuck::cast_slice(&vertices));
        }
        if isize > 0 {
            queue.write_buffer(&ibuf, 0, bytemuck::cast_slice(&indices));
        }
        let gpu = crate::upload::GpuScene {
            vertex: crate::allocator::OwnedBuffer {
                buffer: vbuf,
                key: crate::allocator::BufKey {
                    size: vsize.max(4),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                },
            },
            index: crate::allocator::OwnedBuffer {
                buffer: ibuf,
                key: crate::allocator::BufKey {
                    size: isize.max(4),
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                },
            },
            vertices: vertices.len() as u32,
            indices: indices.len() as u32,
        };

        // Bind viewport
        let vp_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vp-bg-direct-no-msaa"),
            layout: self.solid_direct_no_msaa.viewport_bgl(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.vp_buffer.as_entire_binding(),
            }],
        });

        // Render directly to target without MSAA to preserve existing content through blending
        // MSAA+resolve doesn't apply blend state correctly for layered rendering
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("rounded-rect-fill-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.solid_direct_no_msaa.record(&mut pass, &vp_bg, &gpu);
    }

    pub fn render_solids_to_offscreen(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        vp_bg: &wgpu::BindGroup,
        targets: &PassTargets,
        scene: &GpuScene,
        clear_color: wgpu::Color,
    ) {
        // Multisampled color target with resolve to offscreen color
        let msaa_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("solid-msaa-offscreen"),
            size: wgpu::Extent3d {
                width: targets.color.key.width,
                height: targets.color.key.height,
                depth_or_array_layers: 1,
            },
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
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
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
        let bg = self
            .compositor
            .bind_group(&self.device, &offscreen.color.view);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("composite-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: match clear {
                        Some(c) => wgpu::LoadOp::Clear(c),
                        None => wgpu::LoadOp::Load,
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.compositor.record(&mut pass, &bg);
    }

    /// Paint background to intermediate texture instead of directly to surface.
    /// This enables smooth resizing when combined with blit_to_surface.
    pub fn paint_root_to_intermediate(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        bg: &Background,
        queue: &wgpu::Queue,
    ) {
        let intermediate = self
            .intermediate_texture
            .as_ref()
            .expect("intermediate texture must be allocated before painting");
        self.paint_root(encoder, &intermediate.view, bg, queue);
    }

    pub fn paint_root(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        bg: &Background,
        queue: &wgpu::Queue,
    ) {
        // If solid, do a minimal clear pass
        if let Background::Solid(c) = bg {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bg-solid-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: c.r as f64,
                            g: c.g as f64,
                            b: c.b as f64,
                            a: c.a as f64,
                        }),
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
            Background::LinearGradient {
                start_uv,
                end_uv,
                stop0,
                stop1,
            } => (*start_uv, *end_uv, *stop0, *stop1),
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
        struct Stop {
            pos: f32,
            _pad0: [f32; 3],
            color: [f32; 4],
        }

        let params = BgParams {
            start: start_uv,
            end: end_uv,
            center: [0.5, 0.5],
            radius: 1.0,
            stop_count: 2,
            mode: 1,
            _pad: 0,
        };
        let c0 = stop0.1;
        let c1 = stop1.1;
        let stops = [
            Stop {
                pos: stop0.0,
                _pad0: [0.0; 3],
                color: [c0.r, c0.g, c0.b, c0.a],
            },
            Stop {
                pos: stop1.0,
                _pad0: [0.0; 3],
                color: [c1.r, c1.g, c1.b, c1.a],
            },
            Stop {
                pos: 0.0,
                _pad0: [0.0; 3],
                color: [0.0; 4],
            },
            Stop {
                pos: 0.0,
                _pad0: [0.0; 3],
                color: [0.0; 4],
            },
            Stop {
                pos: 0.0,
                _pad0: [0.0; 3],
                color: [0.0; 4],
            },
            Stop {
                pos: 0.0,
                _pad0: [0.0; 3],
                color: [0.0; 4],
            },
            Stop {
                pos: 0.0,
                _pad0: [0.0; 3],
                color: [0.0; 4],
            },
            Stop {
                pos: 0.0,
                _pad0: [0.0; 3],
                color: [0.0; 4],
            },
        ];

        queue.write_buffer(&self.bg_param_buffer, 0, bytemuck::bytes_of(&params));
        queue.write_buffer(&self.bg_stops_buffer, 0, bytemuck::cast_slice(&stops));
        let bg_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg-bind"),
            layout: self.bg.bgl(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.bg_param_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.bg_stops_buffer.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("bg-grad-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.bg.record(&mut pass, &bg_bind);
    }

    /// Paint linear gradient to intermediate texture.
    pub fn paint_root_linear_gradient_multi_to_intermediate(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        start_uv: [f32; 2],
        end_uv: [f32; 2],
        stops_in: &[(f32, crate::scene::ColorLinPremul)],
        queue: &wgpu::Queue,
    ) {
        let intermediate = self
            .intermediate_texture
            .as_ref()
            .expect("intermediate texture must be allocated before painting");
        self.paint_root_linear_gradient_multi(
            encoder,
            &intermediate.view,
            start_uv,
            end_uv,
            stops_in,
            queue,
        );
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
        struct BgParams {
            start_end: [f32; 4],
            center_radius_stop: [f32; 4],
            flags: [f32; 4],
        }
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct Stop {
            pos: f32,
            _pad0: [f32; 3],
            color: [f32; 4],
        }
        let mut stops: [Stop; 8] = [Stop {
            pos: 0.0,
            _pad0: [0.0; 3],
            color: [0.0; 4],
        }; 8];
        for (i, (p, c)) in sorted.iter().take(8).enumerate() {
            stops[i] = Stop {
                pos: *p,
                _pad0: [0.0; 3],
                color: [c.r, c.g, c.b, c.a],
            };
        }
        let debug_flag = std::env::var("DEBUG_RADIAL")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
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
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.bg_param_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.bg_stops_buffer.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("bg-linear-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.bg.record(&mut pass, &bg_bind);
    }

    /// Paint radial gradient to intermediate texture.
    pub fn paint_root_radial_gradient_multi_to_intermediate(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        center_uv: [f32; 2],
        radius: f32,
        stops_in: &[(f32, crate::scene::ColorLinPremul)],
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
    ) {
        let intermediate = self
            .intermediate_texture
            .as_ref()
            .expect("intermediate texture must be allocated before painting");
        self.paint_root_radial_gradient_multi(
            encoder,
            &intermediate.view,
            center_uv,
            radius,
            stops_in,
            queue,
            width,
            height,
        );
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
        struct BgParams {
            start_end: [f32; 4],
            center_radius_stop: [f32; 4],
            flags: [f32; 4],
        }
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct Stop {
            pos: f32,
            _pad0: [f32; 3],
            color: [f32; 4],
        }
        let mut stops: [Stop; 8] = [Stop {
            pos: 0.0,
            _pad0: [0.0; 3],
            color: [0.0; 4],
        }; 8];
        for (i, (p, c)) in sorted.iter().take(8).enumerate() {
            stops[i] = Stop {
                pos: *p,
                _pad0: [0.0; 3],
                color: [c.r, c.g, c.b, c.a],
            };
        }
        let debug_flag = std::env::var("DEBUG_RADIAL")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
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
                eprintln!(
                    "  Input {}: pos={}, ColorLinPremul(r={}, g={}, b={}, a={})",
                    i, p, c.r, c.g, c.b, c.a
                );
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
                    eprintln!(
                        "macOS DPI correction applied: sf={}, adj_center={:?}, adj_radius={}",
                        sf, adj_center, adj_radius
                    );
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
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.bg_param_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.bg_stops_buffer.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("bg-radial-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
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
        queue: &wgpu::Queue,
    ) {
        // Draw solid via the background fullscreen shader to avoid sRGB clear vs blit inconsistencies.
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct BgParams {
            start_end: [f32; 4],
            center_radius_stop: [f32; 4],
            flags: [f32; 4],
        }
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct Stop {
            pos: f32,
            _pad0: [f32; 3],
            color: [f32; 4],
        }
        let params = BgParams {
            start_end: [0.0, 0.0, 1.0, 1.0],
            center_radius_stop: [0.5, 0.5, 1.0, 1.0],
            flags: [0.0, 0.0, 0.0, 0.0], // mode = 0 => solid
        };
        let stops: [Stop; 1] = [Stop {
            pos: 0.0,
            _pad0: [0.0; 3],
            color: [color.r, color.g, color.b, color.a],
        }];
        // Write uniforms (only first stop used for solid mode)
        queue.write_buffer(&self.bg_param_buffer, 0, bytemuck::bytes_of(&params));
        queue.write_buffer(&self.bg_stops_buffer, 0, bytemuck::cast_slice(&stops));
        let bg_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg-bind-solid"),
            layout: self.bg.bgl(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.bg_param_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.bg_stops_buffer.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("bg-solid-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.bg.record(&mut pass, &bg_bind);
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
            start_end: [f32; 4],
            center_radius_stop: [f32; 4],
            flags: [f32; 4],
        }
        let c0 = stop0.1;
        let c1 = stop1.1;
        // Reuse the multi-stop layout by writing two stops into the stops buffer
        let debug_flag = std::env::var("DEBUG_RADIAL")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let params = BgData {
            start_end: [start_uv[0], start_uv[1], end_uv[0], end_uv[1]],
            center_radius_stop: [0.5, 0.5, 1.0, 2.0],
            flags: [1.0, if debug_flag { 1.0 } else { 0.0 }, 0.0, 0.0],
        };
        // Populate first two stops in the stop buffer for the simple gradient helper
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct Stop {
            pos: f32,
            _pad0: [f32; 3],
            color: [f32; 4],
        }
        let stops: [Stop; 2] = [
            Stop {
                pos: stop0.0,
                _pad0: [0.0; 3],
                color: [c0.r, c0.g, c0.b, c0.a],
            },
            Stop {
                pos: stop1.0,
                _pad0: [0.0; 3],
                color: [c1.r, c1.g, c1.b, c1.a],
            },
        ];
        queue.write_buffer(&self.bg_param_buffer, 0, bytemuck::bytes_of(&params));
        queue.write_buffer(&self.bg_stops_buffer, 0, bytemuck::cast_slice(&stops));
        let bg_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg-bind"),
            layout: self.bg.bgl(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.bg_param_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.bg_stops_buffer.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("bg-grad-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.bg.record(&mut pass, &bg_bind);
    }

    /// Render the scene to intermediate texture, then blit to surface.
    /// This is the Vello-style approach that enables smooth window resizing.
    pub fn render_frame_with_intermediate(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        allocator: &mut RenderAllocator,
        surface_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        scene: &GpuScene,
        clear: wgpu::Color,
        _direct: bool,
        queue: &wgpu::Queue,
    ) {
        // Ensure intermediate texture is allocated and matches surface size
        self.ensure_intermediate_texture(allocator, width, height);

        // Get intermediate texture view
        let intermediate_view = &self.intermediate_texture.as_ref().unwrap().view;

        // IMPORTANT: Always render directly to intermediate texture (no compositor step)
        // The blit shader will handle the final flip when copying to surface
        self.render_frame_internal(
            encoder,
            allocator,
            intermediate_view,
            width,
            height,
            scene,
            clear,
            true, // Force direct rendering to avoid double-flip from compositor
            queue,
            false, // Don't preserve intermediate texture
        );

        // Blit intermediate texture to surface (very fast operation)
        self.blit_to_surface(encoder, surface_view);
    }

    /// Render solids to intermediate, then draw text, then blit to surface.
    pub fn render_frame_with_intermediate_and_text(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        allocator: &mut RenderAllocator,
        surface_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        scene: &GpuScene,
        clear: wgpu::Color,
        queue: &wgpu::Queue,
        list: &crate::display_list::DisplayList,
        provider: &dyn crate::text::TextProvider,
    ) {
        // Ensure intermediate texture is allocated and matches surface size
        self.ensure_intermediate_texture(allocator, width, height);
        let intermediate_view = &self.intermediate_texture.as_ref().unwrap().view;

        // Render solids to intermediate
        self.render_frame_internal(
            encoder,
            allocator,
            intermediate_view,
            width,
            height,
            scene,
            clear,
            true, // direct to intermediate
            queue,
            false,
        );
        // Render text to intermediate
        self.render_text_for_list(encoder, intermediate_view, list, queue, provider);
        // Blit to surface
        self.blit_to_surface(encoder, surface_view);
    }

    /// Internal render method that can target any texture view.
    fn render_frame_internal(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        allocator: &mut RenderAllocator,
        target_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        scene: &GpuScene,
        clear: wgpu::Color,
        direct: bool,
        queue: &wgpu::Queue,
        preserve_target: bool,
    ) {
        // Update viewport uniform
        let scale = [
            2.0f32 / (width.max(1) as f32),
            -2.0f32 / (height.max(1) as f32),
        ];
        let translate = [-1.0f32, 1.0f32];
        let vp_data = [scale[0], scale[1], translate[0], translate[1]];
        let data = bytemuck::bytes_of(&vp_data);
        queue.write_buffer(&self.vp_buffer, 0, data);
        let vp_bg_off = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vp-bg-offscreen"),
            layout: self.solid_offscreen.viewport_bgl(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.vp_buffer.as_entire_binding(),
            }],
        });
        let vp_bg_direct = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vp-bg-direct"),
            layout: self.solid_direct.viewport_bgl(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.vp_buffer.as_entire_binding(),
            }],
        });
        if direct {
            // MSAA render directly to target with resolve
            let msaa_tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("solid-msaa-direct"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
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
                    resolve_target: Some(target_view),
                    ops: wgpu::Operations {
                        load: if preserve_target {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(clear)
                        },
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
        self.render_solids_to_offscreen(
            encoder,
            &vp_bg_off,
            &targets,
            scene,
            wgpu::Color::TRANSPARENT,
        );
        self.composite_to_surface(encoder, target_view, &targets, None);
        // Return textures to allocator pool to avoid repeated allocations.
        allocator.release_texture(targets.color);
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
        let scale = [
            2.0f32 / (width.max(1) as f32),
            -2.0f32 / (height.max(1) as f32),
        ];
        let translate = [-1.0f32, 1.0f32];
        let vp_data = [scale[0], scale[1], translate[0], translate[1]];
        let data = bytemuck::bytes_of(&vp_data);
        queue.write_buffer(&self.vp_buffer, 0, data);
        let vp_bg_off = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vp-bg-offscreen"),
            layout: self.solid_offscreen.viewport_bgl(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.vp_buffer.as_entire_binding(),
            }],
        });
        let vp_bg_direct = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vp-bg-direct"),
            layout: self.solid_direct.viewport_bgl(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.vp_buffer.as_entire_binding(),
            }],
        });
        if direct {
            // MSAA render directly to surface with resolve
            let msaa_tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("solid-msaa-direct"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
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
                        load: if preserve_surface {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(clear)
                        },
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
        self.render_solids_to_offscreen(
            encoder,
            &vp_bg_off,
            &targets,
            scene,
            wgpu::Color::TRANSPARENT,
        );
        self.composite_to_surface(encoder, surface_view, &targets, None);
        // Return textures to allocator pool to avoid repeated allocations.
        allocator.release_texture(targets.color);
    }

    /// Render solids, then draw text over the same target.
    pub fn render_frame_and_text(
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
        list: &crate::display_list::DisplayList,
        provider: &dyn crate::text::TextProvider,
    ) {
        // Render solids first
        self.render_frame(
            encoder,
            allocator,
            surface_view,
            width,
            height,
            scene,
            clear,
            direct,
            queue,
            preserve_surface,
        );
        // Then draw text
        self.render_text_for_list(encoder, surface_view, list, queue, provider);
    }
}
