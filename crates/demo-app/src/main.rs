use anyhow::Result;
use engine_core::{
    GraphicsEngine, PassManager, Viewport, make_surface_config,
};
use pollster::FutureExt;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

mod scenes;
use scenes::{Scene, SceneKind};

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
    // Track DPI scale factor and push into engine-core (mac only)
    #[cfg(target_os = "macos")]
    let mut scale_factor: f32 = window.scale_factor() as f32;
    let config = make_surface_config(&adapter, &surface, size.width, size.height);

    // Initialize core engine skeleton
    let mut engine = GraphicsEngine::new(device, queue);
    surface.configure(&engine.device(), &config);

    // Scene selection and initialization
    let scene_env = std::env::var("DEMO_SCENE").ok();
    let mut scene: Box<dyn Scene> = if scene_env.as_deref() == Some("radial")
        || std::env::args().any(|a| a == "--scene=radial" || a == "--radial")
    {
        Box::new(scenes::radial::RadialBgScene::default())
    } else if scene_env.as_deref() == Some("circle")
        || std::env::args().any(|a| a == "--scene=circle" || a == "--circle")
    {
        Box::new(scenes::circle::CircleScene::default())
    } else if scene_env.as_deref() == Some("linear")
        || std::env::args().any(|a| a == "--scene=linear" || a == "--linear")
    {
        Box::new(scenes::linear::LinearBgScene::default())
    } else if scene_env.as_deref() == Some("centered")
        || std::env::args().any(|a| a == "--scene=centered" || a == "--centered")
    {
        Box::new(scenes::centered_rect::CenteredRectScene::default())
    } else {
        Box::new(scenes::default::DefaultScene::default())
    };

    let mut dlist_opt = scene.init_display_list(Viewport { width: size.width, height: size.height });
    let queue = engine.queue();
    let mut gpu_scene_opt = match scene.kind() {
        SceneKind::Geometry => {
            let dl = dlist_opt.as_ref().expect("geometry scene should provide DisplayList");
            Some(engine_core::upload_display_list(engine.allocator_mut(), &queue, dl).expect("upload failed"))
        }
        SceneKind::FullscreenBackground => None,
    };
    let mut passes = PassManager::new(engine.device(), config.format);
    #[cfg(target_os = "macos")]
    {
        passes.set_scale_factor(scale_factor);
    }
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
                let new_config = make_surface_config(&adapter, &surface, size.width, size.height);
                surface.configure(&engine.device(), &new_config);
                // Update or rebuild scene geometry on resize
                let vp = Viewport { width: size.width, height: size.height };
                match scene.kind() {
                    SceneKind::Geometry => {
                        if let Some(new_dl) = scene.on_resize(vp) {
                            dlist_opt = Some(new_dl);
                        } else if let Some(dl) = &mut dlist_opt {
                            dl.viewport = vp;
                        }
                    }
                    SceneKind::FullscreenBackground => {}
                }
                needs_reupload = true;
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
                let mut encoder = engine
                    .device()
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("clear-encoder"), });
                // Background will be rendered as clear color in offscreen buffer
                if needs_reupload {
                    if let (SceneKind::Geometry, Some(dl)) = (scene.kind(), dlist_opt.as_ref()) {
                        let q2 = engine.queue();
                        gpu_scene_opt = Some(engine_core::upload_display_list(engine.allocator_mut(), &q2, dl).expect("upload failed on resize"));
                    }
                    needs_reupload = false;
                }
                if let (SceneKind::Geometry, Some(gpu_scene)) = (scene.kind(), gpu_scene_opt.as_ref()) {
                    let queue = engine.queue();
                    passes.render_frame(
                        &mut encoder,
                        engine.allocator_mut(),
                        &view,
                        size.width,
                        size.height,
                        gpu_scene,
                        wgpu::Color { r: 0x0b as f64 / 255.0, g: 0x12 as f64 / 255.0, b: 0x20 as f64 / 255.0, a: 1.0 },
                        bypass,
                        &queue,
                        true,
                    );
                }
                engine.queue().submit(std::iter::once(encoder.finish()));
                frame.present();
            }
            Err(_) => {
                let new_config = make_surface_config(&adapter, &surface, size.width, size.height);
                surface.configure(&engine.device(), &new_config);
                needs_reupload = true;
                // No need to rebuild passes; surface format usually unchanged.
            }
        },
        Event::WindowEvent { event: WindowEvent::ScaleFactorChanged { scale_factor: new_sf, .. }, window_id } if window_id == window.id() => {
            // Update scale factor dynamically (mac only) and notify engine-core
            #[cfg(target_os = "macos")]
            {
                let _ = scale_factor;
                scale_factor = new_sf as f32;
                passes.set_scale_factor(scale_factor);
                // Reconfigure surface to match new physical size
                size = window.inner_size();
                let new_config = make_surface_config(&adapter, &surface, size.width, size.height);
                surface.configure(&engine.device(), &new_config);
                needs_reupload = true;
                window.request_redraw();
            }
        }
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
