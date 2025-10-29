use anyhow::Result;
use engine_core::{
    Brush, ColorLinPremul, DisplayList, GraphicsEngine, Painter, PassManager, Rect, Viewport,
    make_surface_config,
};
use pollster::FutureExt;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

fn main() -> Result<()> {
    // Create window and event loop
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Rune Draw Demo")
        .build(&event_loop)?;
    // Leak the window to satisfy wgpu surface lifetime; event loop never returns.
    let window: &'static winit::window::Window = Box::leak(Box::new(window));

    // Create wgpu instance and surface
    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(window)?;

    // Request adapter/device
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })
        .block_on()
        .expect("No suitable GPU adapters found");

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default(), None)
        .block_on()?;

    // Configure surface
    let mut size = window.inner_size();
    let mut config = make_surface_config(&adapter, &surface, size.width, size.height);

    // Initialize core engine skeleton
    let mut engine = GraphicsEngine::new(device, queue);
    surface.configure(&engine.device(), &config);

    // Build a simple display list and upload once (solid rect)
    let mut painter = Painter::begin_frame(Viewport {
        width: size.width,
        height: size.height,
    });
    // Solid rect
    painter.rect(
        Rect {
            x: 80.0,
            y: 80.0,
            w: 220.0,
            h: 140.0,
        },
        Brush::Solid(ColorLinPremul::from_srgba_u8([82, 167, 232, 255])),
        0,
    );
    // Rounded-rect
    painter.rounded_rect(
        engine_core::RoundedRect {
            rect: Rect {
                x: 340.0,
                y: 80.0,
                w: 220.0,
                h: 140.0,
            },
            radii: engine_core::RoundedRadii {
                tl: 24.0,
                tr: 32.0,
                br: 24.0,
                bl: 24.0,
            },
        },
        Brush::Solid(ColorLinPremul::from_srgba_u8([238, 154, 106, 255])),
        0,
    );
    // Circle
    painter.circle(
        [200.0, 300.0],
        70.0,
        Brush::Solid(ColorLinPremul::from_srgba_u8([133, 235, 190, 255])),
        0,
    );
    // Ellipse
    painter.ellipse(
        [450.0, 300.0],
        [120.0, 70.0],
        Brush::Solid(ColorLinPremul::from_srgba_u8([232, 232, 106, 255])),
        0,
    );
    let mut dlist: DisplayList = painter.finish();
    let queue = engine.queue();
    let mut gpu_scene = engine_core::upload_display_list(engine.allocator_mut(), &queue, &dlist)
        .expect("upload failed");
    let mut passes = PassManager::new(engine.device(), config.format);
    let mut bypass = std::env::var("BYPASS_COMPOSITOR")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let mut needs_reupload = false;

    // Basic event loop that clears the frame
    event_loop.run(move |event, target| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            window_id,
        } if window_id == window.id() => {
            target.exit();
        }
        Event::WindowEvent {
            event: WindowEvent::Resized(new_size),
            window_id,
        } if window_id == window.id() => {
            size = new_size;
            if size.width > 0 && size.height > 0 {
                config = make_surface_config(&adapter, &surface, size.width, size.height);
                surface.configure(&engine.device(), &config);
                // Re-upload geometry with updated viewport so pixel sizes remain stable.
                dlist.viewport = Viewport { width: size.width, height: size.height };
                needs_reupload = true;
                // Immediately draw a full frame to avoid a white flash before the first redraw.
                if let Ok(frame) = surface.get_current_texture() {
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder = engine
                        .device()
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("resize-clear"),
                        });
                    // Paint root background (solid) cross-platform
                    passes.paint_root_color(
                        &mut encoder,
                        &view,
                        engine_core::ColorLinPremul::from_srgba_u8([80, 85, 97, 255]),
                    );
                    // No reupload needed; viewport uniform will handle the size change.
                    let queue = engine.queue();
                    passes.render_frame(
                        &mut encoder,
                        engine.allocator_mut(),
                        &view,
                        size.width,
                        size.height,
                        &gpu_scene,
                        wgpu::Color { r: 0.08, g: 0.09, b: 0.12, a: 1.0 },
                        bypass,
                        &queue,
                    );
                    engine.queue().submit(std::iter::once(encoder.finish()));
                    frame.present();
                }
                window.request_redraw();
            }
        }
        Event::WindowEvent {
            event: WindowEvent::RedrawRequested,
            window_id,
        } if window_id == window.id() => match surface.get_current_texture() {
            Ok(frame) => {
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder =
                    engine
                        .device()
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("clear-encoder"),
                        });
                // Root background gradient
                let queue0 = engine.queue();
                passes.paint_root_gradient(
                    &mut encoder,
                    &view,
                    [0.0, 0.0],
                    [1.0, 1.0],
                    (0.0, engine_core::ColorLinPremul::from_srgba_u8([80, 85, 97, 255])),
                    (1.0, engine_core::ColorLinPremul::from_srgba_u8([60, 64, 74, 255])),
                    &queue0,
                );
                if needs_reupload {
                    gpu_scene = engine_core::upload_display_list(engine.allocator_mut(), &queue, &dlist)
                        .expect("upload failed on resize");
                    needs_reupload = false;
                }
                let queue = engine.queue();
                passes.render_frame(
                    &mut encoder,
                    engine.allocator_mut(),
                    &view,
                    size.width,
                    size.height,
                    &gpu_scene,
                    wgpu::Color { r: 0.08, g: 0.09, b: 0.12, a: 1.0 },
                    bypass,
                    &queue,
                );
                engine.queue().submit(std::iter::once(encoder.finish()));
                frame.present();
            }
            Err(_) => {
                config = make_surface_config(&adapter, &surface, size.width, size.height);
                surface.configure(&engine.device(), &config);
                needs_reupload = true;
                // No need to rebuild passes; surface format usually unchanged.
            }
        },
        Event::AboutToWait => {
            window.request_redraw();
            if let Ok(v) = std::env::var("BYPASS_COMPOSITOR") {
                bypass = v == "1" || v.eq_ignore_ascii_case("true");
            }
        }
        _ => {}
    })?;

    Ok(())
}
