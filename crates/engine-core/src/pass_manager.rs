use std::collections::{HashMap, hash_map::Entry};
use std::hash::{Hash, Hasher};
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

/// Apply a 2D affine transform to a point
fn apply_transform_to_point(point: [f32; 2], transform: crate::Transform2D) -> [f32; 2] {
    let [a, b, c, d, e, f] = transform.m;
    let x = point[0];
    let y = point[1];
    [a * x + c * y + e, b * x + d * y + f]
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct TextCacheKey {
    id: u64,
    text_hash: u64,
    size_px: u32,
    color_rgba: [u8; 4],
    dpi_key: u32,
    provider_tag: u64,
    dynamic: bool,
    width: u32,
    origin_x: i32,
    baseline_y: i32,
}

struct TextCache {
    map: HashMap<TextCacheKey, crate::text::GlyphBatch>,
    max_entries: usize,
}

impl TextCache {
    fn new(max_entries: usize) -> Self {
        Self {
            map: HashMap::new(),
            max_entries: max_entries.max(1),
        }
    }

    fn get(&self, key: &TextCacheKey) -> Option<&crate::text::GlyphBatch> {
        self.map.get(key)
    }

    fn insert(&mut self, key: TextCacheKey, batch: crate::text::GlyphBatch) {
        if self.map.len() >= self.max_entries && !self.map.contains_key(&key) {
            if let Some(old_key) = self.map.keys().next().cloned() {
                self.map.remove(&old_key);
            }
        }
        match self.map.entry(key) {
            Entry::Occupied(_) => {
                // Keep existing entry; caller can decide whether to overwrite in future.
            }
            Entry::Vacant(v) => {
                v.insert(batch);
            }
        }
    }

    fn clear(&mut self) {
        self.map.clear();
    }
}

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
    pub text_offscreen: TextRenderer,
    pub image: crate::pipeline::ImageRenderer,
    pub image_offscreen: crate::pipeline::ImageRenderer,
    pub svg_cache: crate::svg::SvgRasterCache,
    pub image_cache: crate::image_cache::ImageCache,
    offscreen_format: wgpu::TextureFormat,
    surface_format: wgpu::TextureFormat,
    vp_buffer: wgpu::Buffer,
    vp_buffer_text: wgpu::Buffer,
    // Z-index uniform buffer for dynamic depth control (Phase 2)
    z_index_buffer: wgpu::Buffer,
    bg: BackgroundRenderer,
    bg_param_buffer: wgpu::Buffer,
    bg_stops_buffer: wgpu::Buffer,
    // Platform DPI scale factor (used for mac-specific radial centering fix)
    scale_factor: f32,
    // Additional UI scale multiplier for logical pixel mode
    ui_scale: f32,
    // When true, treat positions as logical pixels and scale by `scale_factor` centrally
    logical_pixels: bool,
    // Intermediate texture for Vello-style smooth resizing
    pub intermediate_texture: Option<crate::OwnedTexture>,
    // Depth texture for z-ordering across all element types
    depth_texture: Option<crate::OwnedTexture>,
    // Reusable GPU resources for text rendering to avoid per-glyph allocations.
    text_mask_atlas: wgpu::Texture,
    // Note: This view is not directly read but must be kept alive for the bind group reference
    #[allow(dead_code)]
    text_mask_atlas_view: wgpu::TextureView,
    text_vertex_buffer: wgpu::Buffer,
    text_index_buffer: wgpu::Buffer,
    text_bind_group: wgpu::BindGroup,
    text_cache: TextCache,
}

// Vertex structures for unified rendering
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TextQuadVtx {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ImageQuadVtx {
    pos: [f32; 2],
    uv: [f32; 2],
}

impl PassManager {
    // Fixed text vertex/index buffer sizes for batched glyph quads.
    // Kept in one place so capacity math stays in sync with buffer creation.
    const TEXT_VBUF_BYTES: u64 = 1024 * 1024; // 1MB vertex buffer
    const TEXT_IBUF_BYTES: u64 = 256 * 1024; // 256KB index buffer

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
            Ok(_) => preferred,
            Err(_) => wgpu::TextureFormat::Rgba8Unorm,
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
        let text_offscreen = TextRenderer::new(device.clone(), offscreen_format);
        let image = crate::pipeline::ImageRenderer::new(device.clone(), target_format);
        let image_offscreen = crate::pipeline::ImageRenderer::new(device.clone(), offscreen_format);
        let svg_cache = crate::svg::SvgRasterCache::new(device.clone());
        let image_cache = crate::image_cache::ImageCache::new(device.clone());
        let bg = BackgroundRenderer::new(device.clone(), target_format);
        let vp_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viewport-uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let vp_buffer_text = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viewport-uniform-text"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Z-index uniform buffer for dynamic depth control (Phase 2)
        let z_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("z-index-uniform"),
            size: 4, // Single f32 value
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
        // Text pipeline GPU resources
        let text_mask_atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("text-mask-atlas"),
            size: wgpu::Extent3d {
                width: 4096,
                height: 4096,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Use RGBA8 so we can store RGB subpixel coverage masks directly.
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let text_mask_atlas_view =
            text_mask_atlas.create_view(&wgpu::TextureViewDescriptor::default());
        let text_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text-vbuf"),
            size: Self::TEXT_VBUF_BYTES, // shared constant used for capacity checks
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Index buffer sized for many quads (1MB VBO / 32 bytes per vertex = ~32K vertices = ~8K quads = ~48K indices)
        let text_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text-ibuf"),
            size: Self::TEXT_IBUF_BYTES, // shared constant used for capacity checks
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let text_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text-mask-bgl"),
            layout: &text.tex_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&text_mask_atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&text.sampler),
                },
            ],
        });
        // Defaults: always interpret author coords as logical pixels and scale by DPI.
        let logical_default = true;
        let ui_scale = 1.0;
        let text_cache = TextCache::new(1024);
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
            text_offscreen,
            image,
            image_offscreen,
            svg_cache,
            image_cache,
            offscreen_format,
            surface_format: target_format,
            vp_buffer,
            vp_buffer_text,
            z_index_buffer,
            bg,
            bg_param_buffer,
            bg_stops_buffer,
            scale_factor: 1.0,
            ui_scale,
            logical_pixels: logical_default,
            intermediate_texture: None,
            depth_texture: None,
            text_mask_atlas,
            text_mask_atlas_view,
            text_vertex_buffer,
            text_index_buffer,
            text_bind_group,
            text_cache,
        }
    }

    /// Expose the device for scenes that need to create textures.
    pub fn device(&self) -> Arc<wgpu::Device> {
        self.device.clone()
    }

    /// Create a z-index bind group for the given z-index value.
    /// This is used for dynamic depth control in Phase 2.
    pub fn create_z_bind_group(&self, z_index: f32, queue: &wgpu::Queue) -> wgpu::BindGroup {
        queue.write_buffer(&self.z_index_buffer, 0, bytemuck::bytes_of(&z_index));
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("z-index-bg"),
            layout: self.solid_direct.z_index_bgl(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.z_index_buffer.as_entire_binding(),
            }],
        })
    }

    /// Create a z-index bind group backed by a dedicated uniform buffer for this draw group.
    /// This avoids sharing a single z-index uniform across multiple groups, which would cause
    /// all draws to use the last-written z value (breaking per-group z-ordering).
    fn create_group_z_bind_group(
        &self,
        z_index: f32,
        queue: &wgpu::Queue,
    ) -> (wgpu::BindGroup, wgpu::Buffer) {
        let z_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("z-index-group-buffer"),
            size: std::mem::size_of::<f32>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&z_buf, 0, bytemuck::bytes_of(&z_index));
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("z-index-bg-group"),
            layout: self.solid_direct.z_index_bgl(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: z_buf.as_entire_binding(),
            }],
        });
        (bg, z_buf)
    }

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
        // Update viewport uniform based on render target dimensions (+ logical pixel scale)
        let logical =
            crate::dpi::logical_multiplier(self.logical_pixels, self.scale_factor, self.ui_scale);
        let scale = [
            (2.0f32 / (width.max(1) as f32)) * logical,
            (-2.0f32 / (height.max(1) as f32)) * logical,
        ];
        let translate = [-1.0f32, 1.0f32];
        let vp_data = [scale[0], scale[1], translate[0], translate[1]];
        // debug log removed
        queue.write_buffer(&self.vp_buffer, 0, bytemuck::bytes_of(&vp_data));

        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct QuadVtx {
            pos: [f32; 2],
            uv: [f32; 2],
        }
        let x = origin[0];
        let y = origin[1];
        let w = size[0].max(0.0);
        let h = size[1].max(0.0);
        let verts = [
            QuadVtx {
                pos: [x, y],
                uv: [0.0, 0.0],
            },
            QuadVtx {
                pos: [x + w, y],
                uv: [1.0, 0.0],
            },
            QuadVtx {
                pos: [x + w, y + h],
                uv: [1.0, 1.0],
            },
            QuadVtx {
                pos: [x, y + h],
                uv: [0.0, 1.0],
            },
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
        if vsize > 0 {
            queue.write_buffer(&vbuf, 0, bytemuck::cast_slice(&verts));
        }
        if isize > 0 {
            queue.write_buffer(&ibuf, 0, bytemuck::cast_slice(&idx));
        }

        let vp_bg = self.image.vp_bind_group(&self.device, &self.vp_buffer);
        let z_bg = self.create_z_bind_group(0.0, queue);
        let tex_bg = self.image.tex_bind_group(&self.device, tex_view);

        // Create MSAA texture for image rendering
        let msaa_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("image-msaa"),
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
        
        // Create MSAA depth texture
        let msaa_depth_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("image-msaa-depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let msaa_depth_view = msaa_depth_tex.create_view(&wgpu::TextureViewDescriptor::default());

        let depth_attachment = Some(wgpu::RenderPassDepthStencilAttachment {
            view: &msaa_depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,  // Preserve existing depth values
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("image-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &msaa_view,
                resolve_target: Some(target_view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: depth_attachment,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.image
            .record(&mut pass, &vp_bg, &z_bg, &tex_bg, &vbuf, &ibuf, idx.len() as u32);
    }

    /// Rasterize an SVG file to a cached texture for the given scale.
    /// Returns a texture view and its pixel dimensions on success.
    /// Optional style parameter allows overriding fill, stroke, and stroke-width.
    pub fn rasterize_svg_to_view(
        &mut self,
        path: &std::path::Path,
        scale: f32,
        style: Option<crate::svg::SvgStyle>,
        queue: &wgpu::Queue,
    ) -> Option<(wgpu::TextureView, u32, u32)> {
        let svg_style = style.unwrap_or_default();
        let (tex, w, h) = self
            .svg_cache
            .get_or_rasterize(path, scale, svg_style, queue)?;
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        Some((view, w, h))
    }

    /// Load a raster image (PNG/JPEG/GIF/WebP) from disk to a cached GPU texture.
    /// Returns a texture view and its pixel dimensions on success.
    pub fn load_image_to_view(
        &mut self,
        path: &std::path::Path,
        queue: &wgpu::Queue,
    ) -> Option<(wgpu::TextureView, u32, u32)> {
        let (tex, w, h) = self.image_cache.get_or_load(path, queue)?;
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        Some((view, w, h))
    }

    /// Try to get an image from cache without blocking. Returns None if not ready.
    pub fn try_get_image_view(
        &mut self,
        path: &std::path::Path,
    ) -> Option<(wgpu::TextureView, u32, u32)> {
        let (tex, w, h) = self.image_cache.get(path)?;
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        Some((view, w, h))
    }

    /// Request an image to be loaded. Marks it as loading if not already in cache.
    pub fn request_image_load(&mut self, path: &std::path::Path) {
        self.image_cache.start_load(path);
    }

    /// Check if an image is ready in the cache.
    pub fn is_image_ready(&self, path: &std::path::Path) -> bool {
        self.image_cache.is_ready(path)
    }

    /// Store a pre-loaded image texture in the cache.
    pub fn store_loaded_image(
        &mut self,
        path: &std::path::Path,
        tex: Arc<wgpu::Texture>,
        width: u32,
        height: u32,
    ) {
        self.image_cache.store_ready(path, tex, width, height);
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
        glyphs: &[(
            crate::text::SubpixelMask,
            [f32; 2],
            crate::scene::ColorLinPremul,
        )],
        queue: &wgpu::Queue,
        z_index: f32,
    ) {
        // Prepare viewport transform
        let scale = [
            2.0f32 / (width.max(1) as f32),
            -2.0f32 / (height.max(1) as f32),
        ];
        let translate = [-1.0f32, 1.0f32];
        let vp_data = [scale[0], scale[1], translate[0], translate[1]];
        queue.write_buffer(&self.vp_buffer_text, 0, bytemuck::bytes_of(&vp_data));

        // ======= 1. UPLOAD ALL GLYPH MASKS INTO THE ATLAS ONCE =======
        let mut atlas_cursor_x = 0u32;
        let mut atlas_cursor_y = 0u32;
        let mut next_row_height = 0u32;

        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct QuadVtx {
            pos: [f32; 2],
            uv: [f32; 2],
            color: [f32; 4],
        }

        // Respect fixed VBO/IBO capacities: cap the number of quads we batch so
        // Queue::write_buffer never overruns the preallocated buffers.
        let vtx_size = std::mem::size_of::<QuadVtx>() as u64;
        let idx_size = std::mem::size_of::<u16>() as u64;
        let max_quads_v = (Self::TEXT_VBUF_BYTES / (vtx_size * 4)).max(1);
        let max_quads_i = (Self::TEXT_IBUF_BYTES / (idx_size * 6)).max(1);
        let max_quads = max_quads_v.min(max_quads_i) as usize;
        if max_quads == 0 || glyphs.is_empty() {
            return;
        }
        let mut vertices: Vec<QuadVtx> = Vec::with_capacity(glyphs.len().min(max_quads) * 4);

        for (mask, origin, color) in glyphs.iter().take(max_quads) {
            let c = [color.r, color.g, color.b, color.a];

            let w = mask.width;
            let h = mask.height;

            // Row wrap logic
            if atlas_cursor_x + w >= 4096 {
                atlas_cursor_x = 0;
                atlas_cursor_y += next_row_height;
                next_row_height = 0;
            }
            next_row_height = next_row_height.max(h);

            // Upload mask to atlas
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &self.text_mask_atlas,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: atlas_cursor_x,
                        y: atlas_cursor_y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &mask.data,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(mask.width * mask.bytes_per_pixel() as u32),
                    rows_per_image: Some(mask.height),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );

            // Compute UVs
            let u0 = atlas_cursor_x as f32 / 4096.0;
            let v0 = atlas_cursor_y as f32 / 4096.0;
            let u1 = (atlas_cursor_x + w) as f32 / 4096.0;
            let v1 = (atlas_cursor_y + h) as f32 / 4096.0;

            let x = origin[0];
            let y = origin[1];
            let wf = w as f32;
            let hf = h as f32;

            // Push quad vertices with per-vertex color
            vertices.extend_from_slice(&[
                QuadVtx {
                    pos: [x, y],
                    uv: [u0, v0],
                    color: c,
                },
                QuadVtx {
                    pos: [x + wf, y],
                    uv: [u1, v0],
                    color: c,
                },
                QuadVtx {
                    pos: [x + wf, y + hf],
                    uv: [u1, v1],
                    color: c,
                },
                QuadVtx {
                    pos: [x, y + hf],
                    uv: [u0, v1],
                    color: c,
                },
            ]);

            atlas_cursor_x += w;
        }

        // Upload all vertices at once
        if vertices.is_empty() {
            return;
        }
        let vbytes = (vertices.len() as u64) * vtx_size;
        if vbytes > Self::TEXT_VBUF_BYTES {
            // Extra safeguard: truncate to buffer capacity if our estimate drifts.
            let allowed_quads = (Self::TEXT_VBUF_BYTES / (vtx_size * 4)) as usize;
            let allowed_verts = allowed_quads.saturating_mul(4);
            vertices.truncate(allowed_verts);
        }
        queue.write_buffer(&self.text_vertex_buffer, 0, bytemuck::cast_slice(&vertices));

        // Generate indices for all quads (not using instancing, so we need indices for each quad)
        let quad_count = vertices.len() / 4;
        let mut indices: Vec<u16> = Vec::with_capacity(quad_count * 6);
        for i in 0..quad_count {
            let base = (i * 4) as u16;
            indices.extend_from_slice(&[
                base + 0,
                base + 1,
                base + 2,
                base + 0,
                base + 2,
                base + 3,
            ]);
        }

        // Upload indices
        let ibytes = (indices.len() as u64) * idx_size;
        if ibytes > Self::TEXT_IBUF_BYTES {
            let allowed_quads = (Self::TEXT_IBUF_BYTES / (idx_size * 6)) as usize;
            let allowed_indices = allowed_quads.saturating_mul(6);
            indices.truncate(allowed_indices);
        }
        if !indices.is_empty() {
            queue.write_buffer(&self.text_index_buffer, 0, bytemuck::cast_slice(&indices));
        } else {
            return;
        }

        // ======= 2. ONE RENDER PASS =======
        // View-projection bind group for text pipeline
        let vp_bg = self.text.vp_bind_group(&self.device, &self.vp_buffer_text);
        let z_bg = self.create_z_bind_group(z_index, queue);
        
        // Create MSAA texture for text rendering
        let msaa_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("text-msaa"),
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
        
        // Create MSAA depth texture
        let msaa_depth_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("text-msaa-depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let msaa_depth_view = msaa_depth_tex.create_view(&wgpu::TextureViewDescriptor::default());
        
        let depth_attachment = Some(wgpu::RenderPassDepthStencilAttachment {
            view: &msaa_depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,  // Preserve existing depth values
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("text-pass-batched"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &msaa_view,
                resolve_target: Some(target_view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: depth_attachment,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        pass.set_pipeline(&self.text.pipeline);
        pass.set_bind_group(0, &vp_bg, &[]);
        pass.set_bind_group(1, &z_bg, &[]);
        pass.set_bind_group(2, &self.text_bind_group, &[]);
        pass.set_vertex_buffer(0, self.text_vertex_buffer.slice(..));
        pass.set_index_buffer(self.text_index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        // Draw all quads
        let index_count = (quad_count * 6) as u32;
        pass.draw_indexed(0..index_count, 0, 0..1);
    }

    /// Set the platform DPI scale factor. On macOS this is used to correct
    /// radial gradient centering when using normalized UVs for fullscreen fills.
    pub fn set_scale_factor(&mut self, sf: f32) {
        if sf.is_finite() && sf > 0.0 {
            self.scale_factor = sf;
        } else {
            self.scale_factor = 1.0;
        }
        self.text_cache.clear();
    }

    /// Set author-controlled UI scale multiplier (applies in logical mode).
    pub fn set_ui_scale(&mut self, s: f32) {
        let s = if s.is_finite() { s } else { 1.0 };
        self.ui_scale = s.clamp(0.25, 4.0);
    }

    /// Toggle logical pixel mode.
    pub fn set_logical_pixels(&mut self, on: bool) {
        self.logical_pixels = on;
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
    ///
    /// Strategy: Always ensure texture matches exact size for MSAA resolve compatibility.
    /// We preserve content by using LoadOp::Load when rendering, not by keeping oversized textures.
    pub fn ensure_intermediate_texture(
        &mut self,
        allocator: &mut RenderAllocator,
        width: u32,
        height: u32,
    ) {
        let needs_realloc = match &self.intermediate_texture {
            Some(tex) => {
                // Reallocate if size doesn't match exactly
                // MSAA resolve requires exact size match between MSAA texture and resolve target
                tex.key.width != width || tex.key.height != height
            }
            None => true,
        };

        if needs_realloc {
            // Release old texture if it exists
            if let Some(old_tex) = self.intermediate_texture.take() {
                allocator.release_texture(old_tex);
            }

            // Allocate new intermediate texture with surface format at exact size
            let tex = allocator.allocate_texture(TexKey {
                width,
                height,
                format: self.surface_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST,
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
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        self.blitter.record(&mut pass, &bg);
    }

    /// Ensure depth texture is allocated and matches the given dimensions.
    /// Depth texture is used for z-ordering across all element types (solids, text, images, SVGs).
    pub fn ensure_depth_texture(
        &mut self,
        allocator: &mut RenderAllocator,
        width: u32,
        height: u32,
    ) {
        let needs_realloc = match &self.depth_texture {
            Some(tex) => tex.key.width != width || tex.key.height != height,
            None => true,
        };

        if needs_realloc {
            // Release old texture if it exists
            if let Some(old_tex) = self.depth_texture.take() {
                allocator.release_texture(old_tex);
            }

            // Allocate new depth texture at exact size
            let tex = allocator.allocate_texture(TexKey {
                width,
                height,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            });
            self.depth_texture = Some(tex);
        }
    }

    /// Get the depth texture view for use in render passes.
    /// Panics if depth texture hasn't been allocated via ensure_depth_texture.
    pub fn depth_view(&self) -> &wgpu::TextureView {
        &self
            .depth_texture
            .as_ref()
            .expect("depth texture must be allocated before use")
            .view
    }

    /// Iterate the display list and render all DrawText runs using the given provider to the target_view.
    pub fn render_text_for_list(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        list: &crate::display_list::DisplayList,
        queue: &wgpu::Queue,
        provider: &dyn crate::text::TextProvider,
    ) {
        // Update viewport uniform for text without applying logical/UI scale here.
        let scale = [
            2.0f32 / (list.viewport.width.max(1) as f32),
            -2.0f32 / (list.viewport.height.max(1) as f32),
        ];
        let translate = [-1.0f32, 1.0f32];
        let vp_data = [scale[0], scale[1], translate[0], translate[1]];
        // debug log removed
        queue.write_buffer(&self.vp_buffer_text, 0, bytemuck::bytes_of(&vp_data));

        // Optional: round run X position to whole pixels for diagnostics
        let snap_x = std::env::var("DEMO_SNAP_X")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        for cmd in &list.commands {
            if let crate::display_list::Command::DrawText {
                run,
                transform,
                id,
                dynamic,
                z,
                ..
            } = cmd
            {
                let [a, b, c, d, e, f] = transform.m;
                // Apply full affine transform (scale + translate) to the run origin
                let rx = a * run.pos[0] + c * run.pos[1] + e;
                let ry = b * run.pos[0] + d * run.pos[1] + f;

                // Infer uniform scale from the linear part of the transform (handles DPI scaling)
                let sx = (a * a + b * b).sqrt();
                let sy = (c * c + d * d).sqrt();
                let mut s = if sx.is_finite() && sy.is_finite() {
                    // When both are valid, take the average to avoid anisotropy surprises
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
                // Include logical pixel scale if enabled
                let logical_scale = if self.logical_pixels {
                    (self.scale_factor * self.ui_scale).max(0.0001)
                } else {
                    1.0
                };
                s *= logical_scale;

                // Prepare a scaled run for rasterization so glyph masks match the visual + DPI scale
                let scaled_size = (run.size * s).max(1.0);
                let run_for_provider = crate::scene::TextRun {
                    text: run.text.clone(),
                    pos: [0.0, 0.0],
                    size: scaled_size,
                    color: run.color,
                };

                let sf = if self.scale_factor.is_finite() && self.scale_factor > 0.0 {
                    self.scale_factor
                } else {
                    1.0
                };
                let snap = |v: f32| -> f32 { (v * sf).round() / sf };
                // Convert origin to physical pixels if in logical mode
                let rx_px = rx * logical_scale;
                let ry_px = ry * logical_scale;
                let run_origin_x = if snap_x { snap(rx_px) } else { rx_px };

                // Baseline snapping using metrics at the scaled size
                let baseline_y = if let Some(m) = provider.line_metrics(scaled_size) {
                    let asc = m.ascent;
                    snap(ry_px + asc) - asc
                } else {
                    snap(ry_px)
                };

                // Dynamic text nodes bypass the cache and always rasterize.
                if *dynamic {
                    let mut glyph_batch = crate::text::GlyphBatch::new();
                    for rg in provider.rasterize_run(&run_for_provider) {
                        let mut origin = [run_origin_x + rg.offset[0], baseline_y + rg.offset[1]];
                        if scaled_size <= 15.0 {
                            origin[0] = snap(origin[0]);
                            origin[1] = snap(origin[1]);
                        }
                        glyph_batch.glyphs.push((rg.mask, origin, run.color));
                    }
                    if !glyph_batch.glyphs.is_empty() {
                        self.draw_text_mask(
                            encoder,
                            target_view,
                            list.viewport.width,
                            list.viewport.height,
                            &glyph_batch.glyphs,
                            queue,
                            *z as f32,
                        );
                    }
                    continue;
                }

                // Build cache key for this text run (static text: same text/size/color/position/DPI).
                use std::collections::hash_map::DefaultHasher;
                let mut hasher = DefaultHasher::new();
                run.text.hash(&mut hasher);
                let text_hash = hasher.finish();
                let size_px = scaled_size.max(1.0).round() as u32;
                let color_rgba = run.color.to_srgba_u8();
                let dpi_key = if self.scale_factor.is_finite() && self.scale_factor > 0.0 {
                    (self.scale_factor * 100.0).round() as u32
                } else {
                    0
                };
                let provider_tag = provider.cache_tag();
                let origin_x_key = run_origin_x.round() as i32;
                let baseline_y_key = baseline_y.round() as i32;
                let key = TextCacheKey {
                    id: *id,
                    text_hash,
                    size_px,
                    color_rgba,
                    dpi_key,
                    provider_tag,
                    dynamic: false,
                    width: list.viewport.width,
                    origin_x: origin_x_key,
                    baseline_y: baseline_y_key,
                };

                if let Some(batch) = self.text_cache.get(&key) {
                    if !batch.glyphs.is_empty() {
                        self.draw_text_mask(
                            encoder,
                            target_view,
                            list.viewport.width,
                            list.viewport.height,
                            &batch.glyphs,
                            queue,
                            *z as f32,
                        );
                    }
                    continue;
                }

                // Cache miss: build glyph batch once, insert into cache, then draw.
                let mut glyph_batch = crate::text::GlyphBatch::new();
                for rg in provider.rasterize_run(&run_for_provider) {
                    let mut origin = [run_origin_x + rg.offset[0], baseline_y + rg.offset[1]];
                    // Pseudo-hinting for small pixel sizes
                    if scaled_size <= 15.0 {
                        origin[0] = snap(origin[0]);
                        origin[1] = snap(origin[1]);
                    }
                    glyph_batch.glyphs.push((rg.mask, origin, run.color));
                }
                if !glyph_batch.glyphs.is_empty() {
                    self.draw_text_mask(
                        encoder,
                        target_view,
                        list.viewport.width,
                        list.viewport.height,
                        &glyph_batch.glyphs,
                        queue,
                        *z as f32,
                    );
                    self.text_cache.insert(key, glyph_batch);
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
        let logical =
            crate::dpi::logical_multiplier(self.logical_pixels, self.scale_factor, self.ui_scale);
        let scale = [
            (2.0f32 / (width.max(1) as f32)) * logical,
            (-2.0f32 / (height.max(1) as f32)) * logical,
        ];
        let translate = [-1.0f32, 1.0f32];
        let vp_data = [scale[0], scale[1], translate[0], translate[1]];
        // debug log removed
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
        let z_bg = self.create_z_bind_group(0.0, queue);
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

            let z_bg_cutout = self.create_z_bind_group(0.0, queue);
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
        let logical =
            crate::dpi::logical_multiplier(self.logical_pixels, self.scale_factor, self.ui_scale);
        let scale = [
            (2.0f32 / (width.max(1) as f32)) * logical,
            (-2.0f32 / (height.max(1) as f32)) * logical,
        ];
        let translate = [-1.0f32, 1.0f32];
        let vp_data = [scale[0], scale[1], translate[0], translate[1]];
        // debug log removed
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
        let z_bg = self.create_z_bind_group(0.0, queue);
        
        // Add depth attachment (using 1x since this is non-MSAA rendering)
        let depth_attachment = self.depth_texture.as_ref().map(|tex| {
            wgpu::RenderPassDepthStencilAttachment {
                view: &tex.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load, // Preserve existing depth
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }
        });
        
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
            depth_stencil_attachment: depth_attachment,
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
        queue: &wgpu::Queue,
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
        
        // Create 4x MSAA depth texture to match color attachment sample count
        let msaa_depth_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("solid-msaa-depth-offscreen"),
            size: wgpu::Extent3d {
                width: targets.color.key.width,
                height: targets.color.key.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4, // Must match color attachment
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let msaa_depth_view = msaa_depth_tex.create_view(&wgpu::TextureViewDescriptor::default());
        
        let z_bg = self.create_z_bind_group(0.0, queue);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("solid-offscreen-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &msaa_view,
                resolve_target: Some(&targets.color.view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &msaa_depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
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
            // debug logging removed
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
                    // debug logging removed
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
    ///
    /// If `preserve_intermediate` is true, the existing intermediate texture content
    /// is preserved (LoadOp::Load), allowing incremental updates like background-only redraws.
    /// Note: When size changes, texture is reallocated so preservation doesn't apply.
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
        preserve_intermediate: bool,
    ) {
        // Check if we need to reallocate (size changed)
        let size_changed = match &self.intermediate_texture {
            Some(tex) => tex.key.width != width || tex.key.height != height,
            None => true,
        };

        // Ensure intermediate texture is allocated and matches surface size
        self.ensure_intermediate_texture(allocator, width, height);

        // Get intermediate texture view
        let intermediate_view = &self.intermediate_texture.as_ref().unwrap().view;

        // If size changed, we can't preserve content (texture was reallocated)
        // So we must clear even if preserve_intermediate is true
        let actually_preserve = preserve_intermediate && !size_changed;

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
            actually_preserve, // Only preserve if size didn't change
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

        // Get a raw pointer to the intermediate view to work around borrow checker.
        // This is safe because we know ensure_intermediate_texture() has just allocated it
        // and it won't be reallocated during this function.
        let intermediate_view_ptr: *const wgpu::TextureView = &self
            .intermediate_texture
            .as_ref()
            .expect("intermediate texture must be allocated before rendering text")
            .view;
        let intermediate_view = unsafe { &*intermediate_view_ptr };

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
        // Update viewport uniform (+ logical pixel scale)
        let logical =
            crate::dpi::logical_multiplier(self.logical_pixels, self.scale_factor, self.ui_scale);
        let scale = [
            (2.0f32 / (width.max(1) as f32)) * logical,
            (-2.0f32 / (height.max(1) as f32)) * logical,
        ];
        let translate = [-1.0f32, 1.0f32];
        let vp_data = [scale[0], scale[1], translate[0], translate[1]];
        let data = bytemuck::bytes_of(&vp_data);
        // debug log removed
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
            
            // Create 4x MSAA depth texture to match color attachment sample count
            let msaa_depth_tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("solid-msaa-depth-direct"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 4,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let msaa_depth_view = msaa_depth_tex.create_view(&wgpu::TextureViewDescriptor::default());
            
            let depth_attachment = Some(wgpu::RenderPassDepthStencilAttachment {
                view: &msaa_depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: if preserve_target {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(1.0)
                    },
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            });
            
            let z_bg = self.create_z_bind_group(0.0, queue);
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
                depth_stencil_attachment: depth_attachment,
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
            queue,
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
        // Update viewport uniform (+ logical pixel scale)
        let logical =
            crate::dpi::logical_multiplier(self.logical_pixels, self.scale_factor, self.ui_scale);
        let scale = [
            (2.0f32 / (width.max(1) as f32)) * logical,
            (-2.0f32 / (height.max(1) as f32)) * logical,
        ];
        let translate = [-1.0f32, 1.0f32];
        let vp_data = [scale[0], scale[1], translate[0], translate[1]];
        let data = bytemuck::bytes_of(&vp_data);
        // debug log removed
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
            
            // Create 4x MSAA depth texture to match color attachment sample count
            let msaa_depth_tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("solid-msaa-depth-direct"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 4,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let msaa_depth_view = msaa_depth_tex.create_view(&wgpu::TextureViewDescriptor::default());
            
            let depth_attachment = Some(wgpu::RenderPassDepthStencilAttachment {
                view: &msaa_depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: if preserve_surface {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(1.0)
                    },
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            });
            
            let z_bg = self.create_z_bind_group(0.0, queue);
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
                depth_stencil_attachment: depth_attachment,
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
            queue,
        );
        self.composite_to_surface(encoder, surface_view, &targets, None);
        // Return textures to allocator pool to avoid repeated allocations.
        allocator.release_texture(targets.color);
    }

    /// Render solids, then draw text over the same target.
    pub fn render_frame_and_text(
        &mut self,
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

    /// Unified rendering: Render all draw types (solids, text, images, SVGs) in a single pass
    /// with proper z-ordering. This is Phase 3 of the depth buffer implementation.
    /// 
    /// This method interleaves all draw calls based on z-index for optimal z-ordering performance.
    /// Draw calls are batched by material type when possible for efficiency.
    pub fn render_unified(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        allocator: &mut RenderAllocator,
        surface_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        scene: &GpuScene,
        glyph_draws: &[([f32; 2], crate::text::RasterizedGlyph, crate::ColorLinPremul, i32)], // (origin, glyph, color, z)
        svg_draws: &[(std::path::PathBuf, [f32; 2], [f32; 2], Option<crate::SvgStyle>, i32, crate::Transform2D)],
        image_draws: &[(std::path::PathBuf, [f32; 2], [f32; 2], i32)], // (path, origin, size, z)
        clear: wgpu::Color,
        direct: bool,
        queue: &wgpu::Queue,
        preserve_surface: bool,
    ) {
        // Update viewport uniform
        let logical =
            crate::dpi::logical_multiplier(self.logical_pixels, self.scale_factor, self.ui_scale);
        let scale = [
            (2.0f32 / (width.max(1) as f32)) * logical,
            (-2.0f32 / (height.max(1) as f32)) * logical,
        ];
        let translate = [-1.0f32, 1.0f32];
        let vp_data = [scale[0], scale[1], translate[0], translate[1]];
        let data = bytemuck::bytes_of(&vp_data);
        queue.write_buffer(&self.vp_buffer, 0, data);

        // Create viewport bind groups
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
                label: Some("unified-msaa-direct"),
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

            // Create 4x MSAA depth texture to match color attachment sample count
            let msaa_depth_tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("unified-msaa-depth-direct"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 4,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let msaa_depth_view =
                msaa_depth_tex.create_view(&wgpu::TextureViewDescriptor::default());

            let depth_attachment = Some(wgpu::RenderPassDepthStencilAttachment {
                view: &msaa_depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: if preserve_surface {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(1.0)
                    },
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            });

            // Create z-index bind group before render pass (must outlive the pass)
            let z_bg = self.create_z_bind_group(0.0, queue);

            // Pre-fetch (and lazily load) all image views before render pass (to avoid mutable borrow conflicts)
            let mut image_views: Vec<(wgpu::TextureView, [f32; 2], [f32; 2], f32)> = Vec::new();
            for (path, origin, size, z) in image_draws.iter() {
                let tex_opt = if let Some(view) =
                    self.try_get_image_view(std::path::Path::new(path))
                {
                    Some(view)
                } else {
                    self.load_image_to_view(std::path::Path::new(path), queue)
                };
                if let Some((tex_view, _w, _h)) = tex_opt {
                    image_views.push((tex_view, *origin, *size, *z as f32));
                }
            }
            
            // Pre-rasterize all SVGs before render pass (to avoid mutable borrow conflicts)
            let mut svg_views: Vec<(wgpu::TextureView, [f32; 2], [f32; 2], f32)> = Vec::new();
            for (path, origin, max_size, style, _z, transform) in svg_draws.iter() {
                if let Some((_view, w, h)) = self.rasterize_svg_to_view(
                    std::path::Path::new(path),
                    1.0,
                    *style,
                    queue,
                ) {
                    let base_w = w.max(1) as f32;
                    let base_h = h.max(1) as f32;
                    let scale = (max_size[0] / base_w).min(max_size[1] / base_h).max(0.0);

                    if let Some((view_scaled, sw, sh)) = self.rasterize_svg_to_view(
                        std::path::Path::new(path),
                        scale,
                        *style,
                        queue,
                    ) {
                        // Apply transform to origin for correct positioning
                        let transformed_origin = apply_transform_to_point(*origin, *transform);
                        svg_views.push((view_scaled, transformed_origin, [sw as f32, sh as f32], *_z as f32));
                    }
                }
            }
            
            // Group text by z-index for proper depth rendering
            eprintln!("🎨 render_unified received {} glyph_draws", glyph_draws.len());
            let mut text_by_z: std::collections::HashMap<i32, Vec<(usize, [f32; 2], &crate::text::RasterizedGlyph, &crate::ColorLinPremul)>> = std::collections::HashMap::new();
            for (idx, (origin, glyph, color, z)) in glyph_draws.iter().enumerate() {
                text_by_z.entry(*z).or_insert_with(Vec::new).push((idx, *origin, glyph, color));
            }
            eprintln!("🎨 Grouped text into {} z-index groups", text_by_z.len());

            // Prepare text rendering data before render pass
            let mut text_groups = if !glyph_draws.is_empty() {
                let mut atlas_cursor_x = 0u32;
                let mut atlas_cursor_y = 0u32;
                let mut next_row_height = 0u32;
                let mut all_text_groups: Vec<(i32, Vec<TextQuadVtx>)> = Vec::new();

                // Process each z-index group
                for (z_index, glyphs) in text_by_z.iter() {
                    let mut vertices: Vec<TextQuadVtx> = Vec::new();
                    eprintln!("      🔠 Processing z={} with {} glyphs", z_index, glyphs.len());

                    let mut local_idx = 0;
                    for (idx, origin, glyph, color) in glyphs.iter() {
                    let mask = &glyph.mask;
                    let w = mask.width;
                    let h = mask.height;
                    if local_idx == 0 {
                        eprintln!("        🔤 First glyph: origin=[{:.1}, {:.1}], size=[{}, {}], color=[{:.3}, {:.3}, {:.3}, {:.3}]",
                            origin[0], origin[1], w, h, color.r, color.g, color.b, color.a);
                    }
                    local_idx += 1;

                    if atlas_cursor_x + w >= 4096 {
                        atlas_cursor_x = 0;
                        atlas_cursor_y += next_row_height;
                        next_row_height = 0;
                    }
                    next_row_height = next_row_height.max(h);

                    queue.write_texture(
                        wgpu::ImageCopyTexture {
                            texture: &self.text_mask_atlas,
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: atlas_cursor_x,
                                y: atlas_cursor_y,
                                z: 0,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        &mask.data,
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(w * mask.bytes_per_pixel() as u32),
                            rows_per_image: Some(h),
                        },
                        wgpu::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                    );

                    let u0 = atlas_cursor_x as f32 / 4096.0;
                    let v0 = atlas_cursor_y as f32 / 4096.0;
                    let u1 = (atlas_cursor_x + w) as f32 / 4096.0;
                    let v1 = (atlas_cursor_y + h) as f32 / 4096.0;

                    if local_idx == 1 {
                        eprintln!("        📐 Atlas pos: cursor=({}, {}), uv=[{:.4}, {:.4}] to [{:.4}, {:.4}]",
                            atlas_cursor_x, atlas_cursor_y, u0, v0, u1, v1);
                    }

                    vertices.extend_from_slice(&[
                        TextQuadVtx {
                            pos: [origin[0], origin[1]],
                            uv: [u0, v0],
                            color: [color.r, color.g, color.b, color.a],
                        },
                        TextQuadVtx {
                            pos: [origin[0] + w as f32, origin[1]],
                            uv: [u1, v0],
                            color: [color.r, color.g, color.b, color.a],
                        },
                        TextQuadVtx {
                            pos: [origin[0] + w as f32, origin[1] + h as f32],
                            uv: [u1, v1],
                            color: [color.r, color.g, color.b, color.a],
                        },
                        TextQuadVtx {
                            pos: [origin[0], origin[1] + h as f32],
                            uv: [u0, v1],
                            color: [color.r, color.g, color.b, color.a],
                        },
                    ]);

                    atlas_cursor_x += w;
                }

                    // Store vertices for this z-index group
                    if !vertices.is_empty() {
                        all_text_groups.push((*z_index, vertices));
                    }
                }

                // Create buffers and bind groups for each text group
                eprintln!("🔧 all_text_groups.len() = {}", all_text_groups.len());
                let mut text_resources: Vec<(
                    i32,
                    wgpu::Buffer,
                    wgpu::Buffer,
                    u32,
                    wgpu::BindGroup,
                    wgpu::Buffer,
                )> = Vec::new();
                for (z_index, vertices) in all_text_groups {
                    eprintln!(
                        "  🛠️  Creating resources for z={}, vertices={}",
                        z_index,
                        vertices.len()
                    );
                    let quad_count = vertices.len() / 4;
                    let mut indices: Vec<u16> = Vec::with_capacity(quad_count * 6);
                    for i in 0..quad_count {
                        let base = (i * 4) as u16;
                        indices.extend_from_slice(&[
                            base,
                            base + 1,
                            base + 2,
                            base,
                            base + 2,
                            base + 3,
                        ]);
                    }

                    // Create vertex buffer for this group
                    let vbuf = self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("text-vertex-buffer-group"),
                        size: (vertices.len() * std::mem::size_of::<TextQuadVtx>()) as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });

                    // Create index buffer for this group
                    let ibuf = self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("text-index-buffer-group"),
                        size: (indices.len() * std::mem::size_of::<u16>()) as u64,
                        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });

                    queue.write_buffer(&vbuf, 0, bytemuck::cast_slice(&vertices));
                    queue.write_buffer(&ibuf, 0, bytemuck::cast_slice(&indices));

                    // Create z bind group for this text group
                    // Pass z_index as float directly - shader will convert to depth
                    eprintln!("    💎 z={} (passing as z-index to shader)", z_index);
                    let (z_bg, z_buf) = self.create_group_z_bind_group(z_index as f32, queue);

                    text_resources.push((z_index, vbuf, ibuf, indices.len() as u32, z_bg, z_buf));
                }

                text_resources
            } else {
                Vec::new()
            };

            // Sort text groups by z-index (back to front)
            text_groups.sort_by_key(|(z, _, _, _, _, _)| *z);

            // Create text bind groups before render pass so they live long enough
            let vp_bg_text = self.text.vp_bind_group(&self.device, &self.vp_buffer);

            // Prepare image resources (collect all buffers and bind groups so they live long enough)
            let mut image_resources: Vec<(
                wgpu::Buffer,
                wgpu::Buffer,
                wgpu::BindGroup,
                wgpu::BindGroup,
                wgpu::BindGroup,
                wgpu::Buffer,
            )> = Vec::new();
            for (tex_view, origin, size, z_val) in image_views.iter() {
                let verts = [
                    ImageQuadVtx {
                        pos: [origin[0], origin[1]],
                        uv: [0.0, 0.0],
                    },
                    ImageQuadVtx {
                        pos: [origin[0] + size[0], origin[1]],
                        uv: [1.0, 0.0],
                    },
                    ImageQuadVtx {
                        pos: [origin[0] + size[0], origin[1] + size[1]],
                        uv: [1.0, 1.0],
                    },
                    ImageQuadVtx {
                        pos: [origin[0], origin[1] + size[1]],
                        uv: [0.0, 1.0],
                    },
                ];
                let idx: [u16; 6] = [0, 1, 2, 0, 2, 3];

                let vbuf = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("image-vbuf-unified"),
                    size: (verts.len() * std::mem::size_of::<ImageQuadVtx>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let ibuf = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("image-ibuf-unified"),
                    size: (idx.len() * std::mem::size_of::<u16>()) as u64,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                queue.write_buffer(&vbuf, 0, bytemuck::cast_slice(&verts));
                queue.write_buffer(&ibuf, 0, bytemuck::cast_slice(&idx));

                let vp_bg_img = self.image.vp_bind_group(&self.device, &self.vp_buffer);
                // Pass z_index as float directly - shader will convert to depth
                let (z_bg_img, z_buf_img) = self.create_group_z_bind_group(*z_val as f32, queue);
                let tex_bg = self.image.tex_bind_group(&self.device, tex_view);
                
                image_resources.push((vbuf, ibuf, vp_bg_img, z_bg_img, tex_bg, z_buf_img));
            }
            
            // Prepare SVG resources
            let mut svg_resources: Vec<(
                wgpu::Buffer,
                wgpu::Buffer,
                wgpu::BindGroup,
                wgpu::BindGroup,
                wgpu::BindGroup,
                wgpu::Buffer,
            )> = Vec::new();
            for (view_scaled, origin, size, z_val) in svg_views.iter() {
                let verts = [
                    ImageQuadVtx {
                        pos: [origin[0], origin[1]],
                        uv: [0.0, 0.0],
                    },
                    ImageQuadVtx {
                        pos: [origin[0] + size[0], origin[1]],
                        uv: [1.0, 0.0],
                    },
                    ImageQuadVtx {
                        pos: [origin[0] + size[0], origin[1] + size[1]],
                        uv: [1.0, 1.0],
                    },
                    ImageQuadVtx {
                        pos: [origin[0], origin[1] + size[1]],
                        uv: [0.0, 1.0],
                    },
                ];
                let idx: [u16; 6] = [0, 1, 2, 0, 2, 3];

                let vbuf = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("svg-vbuf-unified"),
                    size: (verts.len() * std::mem::size_of::<ImageQuadVtx>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let ibuf = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("svg-ibuf-unified"),
                    size: (idx.len() * std::mem::size_of::<u16>()) as u64,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                queue.write_buffer(&vbuf, 0, bytemuck::cast_slice(&verts));
                queue.write_buffer(&ibuf, 0, bytemuck::cast_slice(&idx));

                let vp_bg_svg = self.image.vp_bind_group(&self.device, &self.vp_buffer);
                // Pass z_index as float directly - shader will convert to depth
                let (z_bg_svg, z_buf_svg) = self.create_group_z_bind_group(*z_val as f32, queue);
                let tex_bg = self.image.tex_bind_group(&self.device, view_scaled);
                
                svg_resources.push((vbuf, ibuf, vp_bg_svg, z_bg_svg, tex_bg, z_buf_svg));
            }

            // Begin unified render pass (after all resource preparation)
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("unified-render-pass"),
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
                depth_stencil_attachment: depth_attachment,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            
            // Render solids first (they're already sorted by z-index in the scene)
            eprintln!("🔵 DIRECT PATH: Rendering {} solid vertices", scene.vertices);
            self.solid_direct.record(&mut pass, &vp_bg_direct, scene);

            // Render text glyphs within the same pass (already sorted by z-index)
            eprintln!("📊 text_groups.len() = {}", text_groups.len());
            for (z_index, vbuf, ibuf, index_count, z_bg, _z_buf) in text_groups.iter() {
                eprintln!("  🎯 Rendering text group at z={} with {} indices", z_index, index_count);
                if *index_count > 0 {
                    pass.set_pipeline(&self.text.pipeline);
                    pass.set_bind_group(0, &vp_bg_text, &[]);
                    pass.set_bind_group(1, z_bg, &[]);
                    pass.set_bind_group(2, &self.text_bind_group, &[]);
                    pass.set_vertex_buffer(0, vbuf.slice(..));
                    pass.set_index_buffer(
                        ibuf.slice(..),
                        wgpu::IndexFormat::Uint16,
                    );
                    pass.draw_indexed(0..*index_count, 0, 0..1);
                    eprintln!("  ✅ Drew {} indices for z={}", index_count, z_index);
                }
            }

            // Render images within same pass
            for (vbuf, ibuf, vp_bg_img, z_bg_img, tex_bg, _z_buf_img) in image_resources.iter() {
                self.image
                    .record(&mut pass, vp_bg_img, z_bg_img, tex_bg, vbuf, ibuf, 6);
            }

            // Render SVGs within same pass
            for (vbuf, ibuf, vp_bg_svg, z_bg_svg, tex_bg, _z_buf_svg) in svg_resources.iter() {
                self.image
                    .record(&mut pass, vp_bg_svg, z_bg_svg, tex_bg, vbuf, ibuf, 6);
            }

            // NOW drop the pass - all rendering complete
            drop(pass);
            eprintln!("✨ Render pass completed successfully (DIRECT)");
            return;
        }

        // Offscreen path - unified rendering to offscreen target
        let targets = self.alloc_targets(allocator, width.max(1), height.max(1));
        
        // Pre-fetch (and lazily load) all image views before render pass (to avoid mutable borrow conflicts)
        let mut image_views_off: Vec<(wgpu::TextureView, [f32; 2], [f32; 2], f32)> = Vec::new();
        eprintln!("🔍 Pre-fetching {} images for unified offscreen render", image_draws.len());
        for (path, origin, size, z) in image_draws.iter() {
            eprintln!("  📦 Image at z={}: {:?}", z, path.file_name().unwrap_or_default());
            let tex_opt = if let Some(view) =
                self.try_get_image_view(std::path::Path::new(path))
            {
                Some(view)
            } else {
                self.load_image_to_view(std::path::Path::new(path), queue)
            };
            if let Some((tex_view, _w, _h)) = tex_opt {
                image_views_off.push((tex_view, *origin, *size, *z as f32));
            }
        }
        
        // Pre-rasterize all SVGs before creating render pass (to avoid mutable borrow conflicts)
        let mut svg_views_off: Vec<(wgpu::TextureView, [f32; 2], [f32; 2], f32)> = Vec::new();
        for (path, origin, max_size, style, _z, transform) in svg_draws.iter() {
            if let Some((_view, w, h)) = self.rasterize_svg_to_view(
                std::path::Path::new(path),
                1.0,
                *style,
                queue,
            ) {
                let base_w = w.max(1) as f32;
                let base_h = h.max(1) as f32;
                let scale = (max_size[0] / base_w).min(max_size[1] / base_h).max(0.0);

                if let Some((view_scaled, sw, sh)) = self.rasterize_svg_to_view(
                    std::path::Path::new(path),
                    scale,
                    *style,
                    queue,
                ) {
                    // Apply transform to origin for correct positioning (offscreen path)
                    let transformed_origin = apply_transform_to_point(*origin, *transform);
                    svg_views_off.push((
                        view_scaled,
                        transformed_origin,
                        [sw as f32, sh as f32],
                        *_z as f32,
                    ));
                }
            }
        }
        
        // Group text by z-index for proper depth rendering (offscreen path)
        let mut text_by_z_off: std::collections::HashMap<i32, Vec<(usize, [f32; 2], &crate::text::RasterizedGlyph, &crate::ColorLinPremul)>> = std::collections::HashMap::new();
        for (idx, (origin, glyph, color, z)) in glyph_draws.iter().enumerate() {
            text_by_z_off.entry(*z).or_insert_with(Vec::new).push((idx, *origin, glyph, color));
        }

        // Prepare text rendering data (same as direct path)
        let mut text_groups_off = if !glyph_draws.is_empty() {
            let mut atlas_cursor_x = 0u32;
            let mut atlas_cursor_y = 0u32;
            let mut next_row_height = 0u32;
            let mut all_text_groups: Vec<(i32, Vec<TextQuadVtx>)> = Vec::new();

            // Process each z-index group
            for (z_index, glyphs) in text_by_z_off.iter() {
                let mut vertices: Vec<TextQuadVtx> = Vec::new();

                for (_idx, origin, glyph, color) in glyphs.iter() {
                let mask = &glyph.mask;
                let w = mask.width;
                let h = mask.height;

                if atlas_cursor_x + w >= 4096 {
                    atlas_cursor_x = 0;
                    atlas_cursor_y += next_row_height;
                    next_row_height = 0;
                }
                next_row_height = next_row_height.max(h);

                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &self.text_mask_atlas,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: atlas_cursor_x,
                            y: atlas_cursor_y,
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &mask.data,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(w * mask.bytes_per_pixel() as u32),
                        rows_per_image: Some(h),
                    },
                    wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                );

                let u0 = atlas_cursor_x as f32 / 4096.0;
                let v0 = atlas_cursor_y as f32 / 4096.0;
                let u1 = (atlas_cursor_x + w) as f32 / 4096.0;
                let v1 = (atlas_cursor_y + h) as f32 / 4096.0;

                vertices.extend_from_slice(&[
                    TextQuadVtx {
                        pos: [origin[0], origin[1]],
                        uv: [u0, v0],
                        color: [color.r, color.g, color.b, color.a],
                    },
                    TextQuadVtx {
                        pos: [origin[0] + w as f32, origin[1]],
                        uv: [u1, v0],
                        color: [color.r, color.g, color.b, color.a],
                    },
                    TextQuadVtx {
                        pos: [origin[0] + w as f32, origin[1] + h as f32],
                        uv: [u1, v1],
                        color: [color.r, color.g, color.b, color.a],
                    },
                    TextQuadVtx {
                        pos: [origin[0], origin[1] + h as f32],
                        uv: [u0, v1],
                        color: [color.r, color.g, color.b, color.a],
                    },
                ]);

                atlas_cursor_x += w;
            }

                // Store vertices for this z-index group
                if !vertices.is_empty() {
                    all_text_groups.push((*z_index, vertices));
                }
            }

            // Create buffers and bind groups for each text group
            let mut text_resources: Vec<(
                i32,
                wgpu::Buffer,
                wgpu::Buffer,
                u32,
                wgpu::BindGroup,
                wgpu::Buffer,
            )> = Vec::new();
            for (z_index, vertices) in all_text_groups {
                let quad_count = vertices.len() / 4;
                let mut indices: Vec<u16> = Vec::with_capacity(quad_count * 6);
                for i in 0..quad_count {
                    let base = (i * 4) as u16;
                    indices.extend_from_slice(&[
                        base,
                        base + 1,
                        base + 2,
                        base,
                        base + 2,
                        base + 3,
                    ]);
                }

                // Create vertex buffer for this group
                let vbuf = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("text-vertex-buffer-group-off"),
                    size: (vertices.len() * std::mem::size_of::<TextQuadVtx>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });

                // Create index buffer for this group
                let ibuf = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("text-index-buffer-group-off"),
                    size: (indices.len() * std::mem::size_of::<u16>()) as u64,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });

                queue.write_buffer(&vbuf, 0, bytemuck::cast_slice(&vertices));
                queue.write_buffer(&ibuf, 0, bytemuck::cast_slice(&indices));

                // Create z bind group for this text group
                // Pass z_index as float directly - shader will convert to depth
                let (z_bg, z_buf) = self.create_group_z_bind_group(z_index as f32, queue);

                text_resources.push((z_index, vbuf, ibuf, indices.len() as u32, z_bg, z_buf));
            }

            text_resources
        } else {
            Vec::new()
        };

        // Sort text groups by z-index (back to front)
        text_groups_off.sort_by_key(|(z, _, _, _, _, _)| *z);

        // Create text bind groups (use offscreen text renderer for offscreen rendering)
        let vp_bg_text_off = self.text_offscreen.vp_bind_group(&self.device, &self.vp_buffer);
        
        // Prepare image resources (offscreen: use image_offscreen to match format)
        let mut image_resources_off: Vec<(
            wgpu::Buffer,
            wgpu::Buffer,
            wgpu::BindGroup,
            wgpu::BindGroup,
            wgpu::BindGroup,
            wgpu::Buffer,
        )> = Vec::new();
        for (tex_view, origin, size, z_val) in image_views_off.iter() {
            let verts = [
                ImageQuadVtx {
                    pos: [origin[0], origin[1]],
                    uv: [0.0, 0.0],
                },
                ImageQuadVtx {
                    pos: [origin[0] + size[0], origin[1]],
                    uv: [1.0, 0.0],
                },
                ImageQuadVtx {
                    pos: [origin[0] + size[0], origin[1] + size[1]],
                    uv: [1.0, 1.0],
                },
                ImageQuadVtx {
                    pos: [origin[0], origin[1] + size[1]],
                    uv: [0.0, 1.0],
                },
            ];
            let idx: [u16; 6] = [0, 1, 2, 0, 2, 3];

            let vbuf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("image-vbuf-unified-offscreen"),
                size: (verts.len() * std::mem::size_of::<ImageQuadVtx>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let ibuf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("image-ibuf-unified-offscreen"),
                size: (idx.len() * std::mem::size_of::<u16>()) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&vbuf, 0, bytemuck::cast_slice(&verts));
            queue.write_buffer(&ibuf, 0, bytemuck::cast_slice(&idx));

            let vp_bg_img = self.image_offscreen.vp_bind_group(&self.device, &self.vp_buffer);
            // Pass z_index as float directly - shader will convert to depth
            let (z_bg_img, z_buf_img) = self.create_group_z_bind_group(*z_val as f32, queue);
            let tex_bg = self.image_offscreen.tex_bind_group(&self.device, tex_view);

            image_resources_off.push((vbuf, ibuf, vp_bg_img, z_bg_img, tex_bg, z_buf_img));
        }
        
        // Prepare SVG resources (offscreen: use image_offscreen to match format)
        let mut svg_resources_off: Vec<(
            wgpu::Buffer,
            wgpu::Buffer,
            wgpu::BindGroup,
            wgpu::BindGroup,
            wgpu::BindGroup,
            wgpu::Buffer,
        )> = Vec::new();
        for (view_scaled, origin, size, z_val) in svg_views_off.iter() {
            let verts = [
                ImageQuadVtx {
                    pos: [origin[0], origin[1]],
                    uv: [0.0, 0.0],
                },
                ImageQuadVtx {
                    pos: [origin[0] + size[0], origin[1]],
                    uv: [1.0, 0.0],
                },
                ImageQuadVtx {
                    pos: [origin[0] + size[0], origin[1] + size[1]],
                    uv: [1.0, 1.0],
                },
                ImageQuadVtx {
                    pos: [origin[0], origin[1] + size[1]],
                    uv: [0.0, 1.0],
                },
            ];
            let idx: [u16; 6] = [0, 1, 2, 0, 2, 3];

            let vbuf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("svg-vbuf-unified-offscreen"),
                size: (verts.len() * std::mem::size_of::<ImageQuadVtx>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let ibuf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("svg-ibuf-unified-offscreen"),
                size: (idx.len() * std::mem::size_of::<u16>()) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&vbuf, 0, bytemuck::cast_slice(&verts));
            queue.write_buffer(&ibuf, 0, bytemuck::cast_slice(&idx));

            let vp_bg_svg = self.image_offscreen.vp_bind_group(&self.device, &self.vp_buffer);
            // Pass z_index as float directly - shader will convert to depth
            let (z_bg_svg, z_buf_svg) = self.create_group_z_bind_group(*z_val as f32, queue);
            let tex_bg = self.image_offscreen.tex_bind_group(&self.device, view_scaled);

            svg_resources_off.push((vbuf, ibuf, vp_bg_svg, z_bg_svg, tex_bg, z_buf_svg));
        }
        
        // Create MSAA color and depth textures for offscreen rendering
        let msaa_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("unified-msaa-offscreen"),
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
        
        let msaa_depth_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("unified-msaa-depth-offscreen"),
            size: wgpu::Extent3d {
                width: targets.color.key.width,
                height: targets.color.key.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let msaa_depth_view = msaa_depth_tex.create_view(&wgpu::TextureViewDescriptor::default());
        
        let depth_attachment = Some(wgpu::RenderPassDepthStencilAttachment {
            view: &msaa_depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        });
        
        let z_bg = self.create_z_bind_group(0.0, queue);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("unified-offscreen-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &msaa_view,
                resolve_target: Some(&targets.color.view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: depth_attachment,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        
        // Render solids first
        eprintln!("🟢 OFFSCREEN PATH: Rendering {} solid vertices", scene.vertices);
        self.solid_offscreen.record(&mut pass, &vp_bg_off, scene);

        // Render text glyphs within the same pass (already sorted by z-index)
        eprintln!("📊 text_groups_off.len() = {}", text_groups_off.len());
        for (z_index, vbuf, ibuf, index_count, z_bg, _z_buf) in text_groups_off.iter() {
            eprintln!("  🎯 Rendering text group at z={} with {} indices (OFFSCREEN)", z_index, index_count);
            if *index_count > 0 {
                pass.set_pipeline(&self.text_offscreen.pipeline);
                pass.set_bind_group(0, &vp_bg_text_off, &[]);
                pass.set_bind_group(1, z_bg, &[]);
                pass.set_bind_group(2, &self.text_bind_group, &[]);
                pass.set_vertex_buffer(0, vbuf.slice(..));
                pass.set_index_buffer(
                    ibuf.slice(..),
                    wgpu::IndexFormat::Uint16,
                );
                pass.draw_indexed(0..*index_count, 0, 0..1);
                eprintln!("  ✅ Drew {} indices for z={} (OFFSCREEN)", index_count, z_index);
            }
        }
        
        // Render images within same pass (offscreen image pipeline)
        eprintln!("📷 image_resources_off.len() = {}", image_resources_off.len());
        for (i, (vbuf, ibuf, vp_bg_img, z_bg_img, tex_bg, _z_buf_img)) in image_resources_off.iter().enumerate() {
            eprintln!("  🖼️ Rendering image {} (OFFSCREEN)", i);
            self.image_offscreen
                .record(&mut pass, vp_bg_img, z_bg_img, tex_bg, vbuf, ibuf, 6);
        }

        // Render SVGs within same pass (offscreen image pipeline)
        eprintln!("🎨 svg_resources_off.len() = {}", svg_resources_off.len());
        for (i, (vbuf, ibuf, vp_bg_svg, z_bg_svg, tex_bg, _z_buf_svg)) in svg_resources_off.iter().enumerate() {
            eprintln!("  🎨 Rendering SVG {} (OFFSCREEN)", i);
            self.image_offscreen
                .record(&mut pass, vp_bg_svg, z_bg_svg, tex_bg, vbuf, ibuf, 6);
        }
        
        // Drop the pass to complete offscreen rendering
        drop(pass);

        // Composite offscreen target to surface
        self.composite_to_surface(encoder, surface_view, &targets, Some(clear));
        allocator.release_texture(targets.color);
    }
}
