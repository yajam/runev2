//! CEF Demo library for embedding in native applications.
//!
//! This crate provides FFI exports for wgpu rendering of CEF content
//! from an Xcode/macOS application. CEF initialization and browser
//! management is handled by the Xcode side; this library just handles
//! the wgpu rendering.

pub mod ffi;

use anyhow::Result;
use parking_lot::Mutex;
use wgpu::util::DeviceExt;

/// Shared renderer state accessible from FFI
pub struct AppRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    texture_bgl: wgpu::BindGroupLayout,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    cef_texture: Option<wgpu::Texture>,
    cef_texture_view: Option<wgpu::TextureView>,
    texture_bind_group: Option<wgpu::BindGroup>,
    texture_width: u32,
    texture_height: u32,
    width: u32,
    height: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

fn vertex_data() -> (Vec<Vertex>, Vec<u16>) {
    let vertices = vec![
        Vertex { pos: [-1.0, -1.0], uv: [0.0, 1.0] },
        Vertex { pos: [1.0, -1.0], uv: [1.0, 1.0] },
        Vertex { pos: [1.0, 1.0], uv: [1.0, 0.0] },
        Vertex { pos: [-1.0, 1.0], uv: [0.0, 0.0] },
    ];
    let indices = vec![0, 1, 2, 0, 2, 3];
    (vertices, indices)
}

/// Create the bind group layout for texture sampling
fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("cef_texture_bind_group_layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

impl AppRenderer {
    /// Create a new renderer with a CAMetalLayer
    pub fn new(width: u32, height: u32, _scale: f32, metal_layer: *mut std::ffi::c_void) -> Result<Self> {
        log::info!("AppRenderer::new width={} height={}", width, height);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..Default::default()
        });

        // Create surface from CAMetalLayer
        // Safety: The metal_layer pointer must be valid for the lifetime of the surface
        let surface = unsafe {
            let target = wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(metal_layer);
            instance.create_surface_unsafe(target)?
        };

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .ok_or_else(|| anyhow::anyhow!("No suitable adapter"))?;

        log::info!("Using adapter: {:?}", adapter.get_info());

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("cef-demo-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        ))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.first()
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

        // Create pipeline
        let texture_bgl = create_bind_group_layout(&device);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("quad-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline-layout"),
            bind_group_layouts: &[&texture_bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quad-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let (vertices, indices) = vertex_data();
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vb"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ib"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let index_count = indices.len() as u32;

        Ok(Self {
            device,
            queue,
            surface,
            config,
            pipeline,
            texture_bgl,
            vertex_buffer,
            index_buffer,
            index_count,
            cef_texture: None,
            cef_texture_view: None,
            texture_bind_group: None,
            texture_width: 0,
            texture_height: 0,
            width,
            height,
        })
    }

    /// Upload pixel data from CEF OnPaint callback
    /// Pixels are in BGRA format (CEF native)
    pub fn upload_pixels(&mut self, pixels: &[u8], width: u32, height: u32) {
        if width == 0 || height == 0 || pixels.is_empty() {
            return;
        }

        // Recreate texture if size changed
        if self.texture_width != width || self.texture_height != height {
            log::debug!("Creating new texture {}x{}", width, height);

            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("cef-texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,  // Use sRGB for correct color
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

            let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("cef_texture_sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });

            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cef_texture_bind_group"),
                layout: &self.texture_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });

            self.cef_texture = Some(texture);
            self.cef_texture_view = Some(view);
            self.texture_bind_group = Some(bind_group);
            self.texture_width = width;
            self.texture_height = height;
        }

        // Convert BGRA to RGBA and upload
        if let Some(texture) = &self.cef_texture {
            // Swap B and R channels (BGRA -> RGBA)
            let mut rgba = pixels.to_vec();
            for chunk in rgba.chunks_exact_mut(4) {
                chunk.swap(0, 2);  // Swap B and R
            }

            let bytes_per_row = width * 4;
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &rgba,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    /// Resize the surface
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        log::debug!("AppRenderer::resize width={} height={}", width, height);

        self.width = width;
        self.height = height;
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Render a frame
    pub fn render(&mut self) {
        // Render to surface
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(e) => {
                log::warn!("Failed to get current texture: {:?}", e);
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };

        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("encoder"),
        });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.1, b: 0.1, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // Draw texture if available (may briefly stretch during resize until CEF re-renders)
            if let Some(bg) = self.texture_bind_group.as_ref() {
                rpass.set_pipeline(&self.pipeline);
                rpass.set_bind_group(0, bg, &[]);
                rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                rpass.draw_indexed(0..self.index_count, 0, 0..1);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}

/// Global renderer instance (for FFI)
pub static RENDERER: once_cell::sync::OnceCell<Mutex<Option<AppRenderer>>> = once_cell::sync::OnceCell::new();

fn get_renderer() -> &'static Mutex<Option<AppRenderer>> {
    RENDERER.get_or_init(|| Mutex::new(None))
}
