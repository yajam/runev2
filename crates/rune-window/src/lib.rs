//! rune-window: minimal winit + wgpu window/event wrapper for Rune.
//!
//! Responsibilities:
//! - Create window + surface + device/queue.
//! - Manage surface configuration and resizing.
//! - Dispatch basic events (redraw, resize, mouse move/click).
//! - Expose helpers to acquire a frame for drawing and to request redraws.

use anyhow::Result;
use engine_core::{make_surface_config, wgpu};
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, Event, MouseButton, WindowEvent};
use winit::event_loop::{EventLoop, EventLoopWindowTarget};
use winit::window::{Window, WindowBuilder};

pub mod events;

pub struct RuneWindow {
    // Winit objects
    event_loop: EventLoop<()>,
    // We must leak the window to satisfy wgpu surface lifetime requirements.
    window: &'static Window,
    // Wgpu objects
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    _adapter: wgpu::Adapter,
    device: std::sync::Arc<wgpu::Device>,
    queue: std::sync::Arc<wgpu::Queue>,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    scale_factor: f64,
}

pub struct WindowCtx<'a> {
    window: &'a Window,
    device: &'a std::sync::Arc<wgpu::Device>,
    queue: &'a std::sync::Arc<wgpu::Queue>,
    surface: &'a wgpu::Surface<'static>,
    config: &'a mut wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    scale_factor: f64,
    last_cursor_pos: [f32; 2],
    elwt: &'a EventLoopWindowTarget<()>,
}

impl<'a> WindowCtx<'a> {
    pub fn window(&self) -> &Window { self.window }
    pub fn device(&self) -> &wgpu::Device { &*self.device }
    pub fn queue(&self) -> &wgpu::Queue { &*self.queue }
    pub fn device_arc(&self) -> std::sync::Arc<wgpu::Device> { self.device.clone() }
    pub fn queue_arc(&self) -> std::sync::Arc<wgpu::Queue> { self.queue.clone() }
    pub fn surface(&self) -> &wgpu::Surface<'static> { self.surface }
    pub fn surface_config(&self) -> &wgpu::SurfaceConfiguration { self.config }
    pub fn surface_config_mut(&mut self) -> &mut wgpu::SurfaceConfiguration { self.config }
    pub fn size(&self) -> PhysicalSize<u32> { self.size }
    pub fn scale_factor(&self) -> f64 { self.scale_factor }
    pub fn mouse_pos(&self) -> [f32; 2] { self.last_cursor_pos }
    pub fn request_redraw(&self) { self.window.request_redraw(); }
    pub fn acquire_current_frame(&self) -> Result<wgpu::SurfaceTexture> {
        Ok(self.surface.get_current_texture()?)
    }
    pub fn event_loop_target(&self) -> &EventLoopWindowTarget<()> { self.elwt }
}

pub trait EventHandler {
    fn init(&mut self, _ctx: &mut WindowCtx) -> Result<()> { Ok(()) }
    fn on_resize(&mut self, _ctx: &mut WindowCtx, _size: PhysicalSize<u32>) -> Result<()> { Ok(()) }
    fn on_mouse_move(&mut self, _ctx: &mut WindowCtx, _pos: [f32; 2]) -> Result<()> { Ok(()) }
    fn on_mouse_input(&mut self, _ctx: &mut WindowCtx, _state: ElementState, _button: MouseButton) -> Result<()> { Ok(()) }
    fn on_redraw(&mut self, _ctx: &mut WindowCtx) -> Result<()> { Ok(()) }
    fn on_event(&mut self, _ctx: &mut WindowCtx, _event: crate::events::RuneWindowEvent) -> Result<()> { Ok(()) }
}

impl RuneWindow {
    pub fn new(title: &str) -> Result<Self> {
        // Create event loop and window
        let event_loop = EventLoop::new()?;
        let window = WindowBuilder::new().with_title(title).build(&event_loop)?;
        let window: &'static Window = Box::leak(Box::new(window));

        // Create wgpu instance + surface
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window)?;

        // Request adapter/device
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .expect("No suitable GPU adapters found");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))?;

        // Configure surface
        let size = window.inner_size();
        let scale_factor = window.scale_factor();
        let config = make_surface_config(&adapter, &surface, size.width, size.height);
        surface.configure(&device, &config);

        Ok(Self { event_loop, window, _instance: instance, surface, _adapter: adapter, device: std::sync::Arc::new(device), queue: std::sync::Arc::new(queue), config, size, scale_factor })
    }

    pub fn run(mut self, mut handler: impl EventHandler + 'static) -> Result<()> {
        let mut last_cursor_pos: [f32; 2] = [0.0, 0.0];
        let mut needs_init = true;

        Ok(self.event_loop.run(move |event, elwt| {
            match event {
                Event::Resumed => {
                    if needs_init {
                        let mut ctx = WindowCtx {
                            window: self.window,
                            device: &self.device,
                            queue: &self.queue,
                            surface: &self.surface,
                            config: &mut self.config,
                            size: self.size,
                            scale_factor: self.scale_factor,
                            last_cursor_pos,
                            elwt,
                        };
                        let _ = handler.init(&mut ctx);
                        needs_init = false;
                    }
                }
                Event::WindowEvent { window_id, event } if window_id == self.window.id() => {
                    match event {
                        WindowEvent::CloseRequested => elwt.exit(),
                        WindowEvent::Resized(new_size) => {
                            self.size = new_size;
                            if new_size.width > 0 && new_size.height > 0 {
                                self.config.width = new_size.width;
                                self.config.height = new_size.height;
                                self.surface.configure(&self.device, &self.config);
                            }
                            // synthesized event
                            let mut ctx_for_event = WindowCtx { window: self.window, device: &self.device, queue: &self.queue, surface: &self.surface, config: &mut self.config, size: self.size, scale_factor: self.scale_factor, last_cursor_pos, elwt };
                            let _ = handler.on_event(&mut ctx_for_event, crate::events::RuneWindowEvent::Resized(new_size));
                            let mut ctx = WindowCtx {
                                window: self.window,
                                device: &self.device,
                                queue: &self.queue,
                                surface: &self.surface,
                                config: &mut self.config,
                                size: self.size,
                                scale_factor: self.scale_factor,
                                last_cursor_pos,
                                elwt,
                            };
                            let _ = handler.on_resize(&mut ctx, new_size);
                        }
                        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                            self.scale_factor = scale_factor;
                            // synthesized event
                            let mut ctx = WindowCtx { window: self.window, device: &self.device, queue: &self.queue, surface: &self.surface, config: &mut self.config, size: self.size, scale_factor: self.scale_factor, last_cursor_pos, elwt };
                            let _ = handler.on_event(&mut ctx, crate::events::RuneWindowEvent::ScaleFactorChanged(scale_factor));
                        }
                        WindowEvent::CursorMoved { position, .. } => {
                            last_cursor_pos = [position.x as f32, position.y as f32];
                            // synthesized event
                            let mut ctx_for_event = WindowCtx { window: self.window, device: &self.device, queue: &self.queue, surface: &self.surface, config: &mut self.config, size: self.size, scale_factor: self.scale_factor, last_cursor_pos, elwt };
                            let _ = handler.on_event(&mut ctx_for_event, crate::events::RuneWindowEvent::CursorMoved { position: last_cursor_pos });
                            let mut ctx = WindowCtx {
                                window: self.window,
                                device: &self.device,
                                queue: &self.queue,
                                surface: &self.surface,
                                config: &mut self.config,
                                size: self.size,
                                scale_factor: self.scale_factor,
                                last_cursor_pos,
                                elwt,
                            };
                            let _ = handler.on_mouse_move(&mut ctx, last_cursor_pos);
                        }
                        WindowEvent::MouseInput { state, button, .. } => {
                            // synthesized event
                            let mut ctx_for_event = WindowCtx { window: self.window, device: &self.device, queue: &self.queue, surface: &self.surface, config: &mut self.config, size: self.size, scale_factor: self.scale_factor, last_cursor_pos, elwt };
                            let _ = handler.on_event(&mut ctx_for_event,
                                match state { ElementState::Pressed => crate::events::RuneWindowEvent::MousePressed(button), ElementState::Released => crate::events::RuneWindowEvent::MouseReleased(button) }
                            );
                            let mut ctx = WindowCtx {
                                window: self.window,
                                device: &self.device,
                                queue: &self.queue,
                                surface: &self.surface,
                                config: &mut self.config,
                                size: self.size,
                                scale_factor: self.scale_factor,
                                last_cursor_pos,
                                elwt,
                            };
                            let _ = handler.on_mouse_input(&mut ctx, state, button);
                        }
                        _ => {}
                    }
                }
                Event::AboutToWait => {
                    // Ensure at least one redraw after init on platforms where
                    // request_redraw during init may be deferred.
                    self.window.request_redraw();
                }
                Event::WindowEvent { window_id, event: WindowEvent::RedrawRequested } if window_id == self.window.id() => {
                    // synthesized event
                    let mut ctx_for_event = WindowCtx { window: self.window, device: &self.device, queue: &self.queue, surface: &self.surface, config: &mut self.config, size: self.size, scale_factor: self.scale_factor, last_cursor_pos, elwt };
                    let _ = handler.on_event(&mut ctx_for_event, crate::events::RuneWindowEvent::RedrawRequested);
                    let mut ctx = WindowCtx {
                        window: self.window,
                        device: &self.device,
                        queue: &self.queue,
                        surface: &self.surface,
                        config: &mut self.config,
                        size: self.size,
                        scale_factor: self.scale_factor,
                        last_cursor_pos,
                        elwt,
                    };
                    let _ = handler.on_redraw(&mut ctx);
                }
                Event::WindowEvent { .. } => {}
                Event::DeviceEvent { .. } => {}
                Event::UserEvent(_) => {}
                Event::Suspended => {}
                Event::NewEvents(_) => {}
                Event::LoopExiting => {}
                _ => {}
            }
        })?)
    }

    pub fn window(&self) -> &Window { self.window }
}
