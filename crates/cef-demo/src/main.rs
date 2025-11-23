use anyhow::Result;
use rune_cef::{HeadlessBuilder, HeadlessRenderer, WgpuTextureTarget};
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

struct Renderer {
    cef: Box<dyn HeadlessRenderer>,
    target: WgpuTextureTarget,
}

impl Renderer {
    fn new(device: &wgpu::Device, width: u32, height: u32, url: &str, scale: f32) -> Result<Self> {
        let mut cef = HeadlessBuilder::new()
            .with_size(width, height)
            .with_scale_factor(scale)
            .build_cef()?;
        cef.navigate(url)?;
        let _ = cef.wait_for_load(5_000);

        let target = WgpuTextureTarget::new(device, width, height, Some("cef-demo-texture"));
        Ok(Self {
            cef: Box::new(cef),
            target,
        })
    }

    fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let _ = self.cef.resize(width, height);
        self.target = WgpuTextureTarget::new(device, width, height, Some("cef-demo-texture"));
    }

    fn update(&mut self, queue: &wgpu::Queue) {
        if let Ok(frame) = self.cef.capture_frame() {
            if !frame.is_empty() {
                let _ = self.target.upload(queue, &frame);
            }
        }
    }
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

fn main() -> Result<()> {
    let url = std::env::args()
        .find_map(|a| a.strip_prefix("--cef-url=").map(|s| s.to_string()))
        .unwrap_or_else(|| "https://example.com".to_string());

    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("CEF Demo")
        .build(&event_loop)?;
    let window = std::sync::Arc::new(window);

    let size: PhysicalSize<u32> = window.inner_size();
    let scale_factor = window.scale_factor() as f32;

    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(&*window)?;
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }))
    .expect("No suitable adapter");

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
        },
        None,
    ))?;

    let caps = surface.get_capabilities(&adapter);
    let format = caps.formats[0];
    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width,
        height: size.height,
        present_mode: caps.present_modes[0],
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 1,
    };
    surface.configure(&device, &config);

    // Pipeline for textured quad.
    let texture_bgl = WgpuTextureTarget::create_bind_group_layout(&device);
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

    eprintln!("cef-demo: starting with URL: {}", url);
    let mut renderer = match Renderer::new(&device, size.width, size.height, &url, scale_factor) {
        Ok(r) => Some(r),
        Err(e) => {
            eprintln!("cef-demo: CEF init failed: {}", e);
            None
        }
    };
    let mut texture_bind_group = renderer
        .as_ref()
        .map(|r| r.target.create_bind_group(&device, &texture_bgl));

    let window_handle = window.clone();
    event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(new_size) => {
                    config.width = new_size.width.max(1);
                    config.height = new_size.height.max(1);
                    surface.configure(&device, &config);
                    if let Some(r) = renderer.as_mut() {
                        r.resize(&device, config.width, config.height);
                        texture_bind_group = Some(r.target.create_bind_group(&device, &texture_bgl));
                    }
                }
                _ => {}
            },
            Event::AboutToWait => {
                if let Some(r) = renderer.as_mut() {
                    r.update(&queue);
                }
                let frame = match surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(_) => {
                        surface.configure(&device, &config);
                        return;
                    }
                };
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("encoder"),
                });

                // If we have no bind group (CEF failed), skip drawing.
                if texture_bind_group.is_none() {
                    queue.submit(Some(encoder.finish()));
                    frame.present();
                    window_handle.request_redraw();
                    return;
                }

                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("render-pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
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
                    rpass.set_pipeline(&pipeline);
                    let bg = texture_bind_group.as_ref().unwrap();
                    rpass.set_bind_group(0, bg, &[]);
                    rpass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    rpass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                    rpass.draw_indexed(0..index_count, 0, 0..1);
                }

                queue.submit(Some(encoder.finish()));
                frame.present();
                window_handle.request_redraw();
            }
            _ => {}
        }
    })?;

    // Unreachable
    #[allow(unreachable_code)]
    Ok(())
}
