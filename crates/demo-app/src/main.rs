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
    // Track DPI scale factor and push into engine-core (mac only)
    #[cfg(target_os = "macos")]
    let mut scale_factor: f32 = window.scale_factor() as f32;
    let mut config = make_surface_config(&adapter, &surface, size.width, size.height);

    // Initialize core engine skeleton
    let mut engine = GraphicsEngine::new(device, queue);
    surface.configure(&engine.device(), &config);

    // Scene selection: default, radial-only (fullscreen), radial-circle (centered circle), or linear (angled linear gradient)
    let scene_env = std::env::var("DEMO_SCENE").ok();
    let radial_only = scene_env.as_deref() == Some("radial")
        || std::env::args().any(|a| a == "--scene=radial" || a == "--radial");
    let radial_circle = scene_env.as_deref() == Some("circle")
        || std::env::args().any(|a| a == "--scene=circle" || a == "--circle");
    let linear_angled = scene_env.as_deref() == Some("linear")
        || std::env::args().any(|a| a == "--scene=linear" || a == "--linear");

    // Build a simple display list and upload once (unless radial-only)
    let mut painter = Painter::begin_frame(Viewport {
        width: size.width,
        height: size.height,
    });
    if radial_circle {
        // Centered circle with a radial gradient
        let center = [size.width as f32 * 0.5, size.height as f32 * 0.5];
        let radius = (size.width.min(size.height) as f32) * 0.30;
        painter.circle(
            center,
            radius,
            Brush::RadialGradient {
                center: [0.5, 0.5],
                radius: 1.0,
                stops: vec![
                    (0.0, ColorLinPremul::from_srgba_u8([0x3b, 0x82, 0xf6, 0xff])),
                    (0.5, ColorLinPremul::from_srgba_u8([0x1e, 0x3a, 0x8a, 0xff])),
                    (0.75, ColorLinPremul::from_srgba_u8([0x10, 0xb9, 0x81, 0xff])),
                    (1.0, ColorLinPremul::from_srgba_u8([0x10, 0xb9, 0x81, 0xff])),
                ],
            },
            0,
        );
    } else if !radial_only {
        // Solid rect
        painter.rect(
            Rect {
                x: 60.0,
                y: 60.0,
                w: 300.0,
                h: 200.0,
            },
            Brush::Solid(ColorLinPremul::from_srgba_u8([82, 167, 232, 255])),
            0,
        );
        // Linear gradient rect (left→right)
        // Move the gradient rect down to avoid overlap with the ellipse
        painter.rect(
            Rect {
                x: 380.0,
                y: 600.0,
                w: 380.0,
                h: 180.0,
            },
            Brush::LinearGradient {
                start: [0.0, 0.0],
                end: [1.0, 0.0],
                stops: vec![
                    (0.0, ColorLinPremul::from_srgba_u8([255, 140, 140, 255])),
                    (0.5, ColorLinPremul::from_srgba_u8([140, 180, 255, 255])),
                    (1.0, ColorLinPremul::from_srgba_u8([140, 255, 180, 255])),
                ],
            },
            0,
        );
        // Rounded-rect
        painter.rounded_rect(
            engine_core::RoundedRect {
                rect: Rect {
                    x: 420.0,
                    y: 80.0,
                    w: 300.0,
                    h: 180.0,
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
            [210.0, 360.0],
            100.0,
            Brush::Solid(ColorLinPremul::from_srgba_u8([133, 235, 190, 255])),
            0,
        );
        // Ellipse with radial gradient (center→edge)
        painter.ellipse(
            [600.0, 420.0],
            [180.0, 110.0],
            Brush::RadialGradient {
                center: [0.5, 0.5],
                radius: 1.0,
                stops: vec![
                    (0.0, ColorLinPremul::from_srgba_u8([255, 255, 190, 255])),
                    (1.0, ColorLinPremul::from_srgba_u8([220, 220, 80, 255])),
                ],
            },
            0,
        );
    }

    // Radial-gradient circle centered in the window (multi-stops)
    // radial_circle and linear_angled use full-screen backgrounds instead of geometry
    let mut dlist: DisplayList = painter.finish();
    let queue = engine.queue();
    let mut gpu_scene = if !radial_only && !linear_angled {
        engine_core::upload_display_list(engine.allocator_mut(), &queue, &dlist)
            .expect("upload failed")
    } else {
        // Dummy empty buffers for fullscreen gradient scenes
        engine_core::upload_display_list(
            engine.allocator_mut(),
            &queue,
            &DisplayList {
                viewport: dlist.viewport,
                commands: vec![],
            },
        )
        .expect("upload failed")
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
                config = make_surface_config(&adapter, &surface, size.width, size.height);
                surface.configure(&engine.device(), &config);
                // Re-upload geometry with updated viewport so pixel sizes remain stable.
                dlist.viewport = Viewport {
                    width: size.width,
                    height: size.height,
                };
                // Rebuild circle geometry on resize to keep it centered
                if radial_circle {
                    let mut p = Painter::begin_frame(dlist.viewport);
                    let center = [size.width as f32 * 0.5, size.height as f32 * 0.5];
                    let radius = (size.width.min(size.height) as f32) * 0.30;
                    p.circle(
                        center,
                        radius,
                        Brush::RadialGradient {
                            center: [0.5, 0.5],
                            radius: 1.0,
                            stops: vec![
                                (0.0, ColorLinPremul::from_srgba_u8([0x3b, 0x82, 0xf6, 0xff])),
                                (0.5, ColorLinPremul::from_srgba_u8([0x1e, 0x3a, 0x8a, 0xff])),
                                (0.75, ColorLinPremul::from_srgba_u8([0x10, 0xb9, 0x81, 0xff])),
                                (1.0, ColorLinPremul::from_srgba_u8([0x10, 0xb9, 0x81, 0xff])),
                            ],
                        },
                        0,
                    );
                    dlist = p.finish();
                }
                needs_reupload = true;
                // Immediately draw a full frame to avoid a white flash before the first redraw.
                if let Ok(frame) = surface.get_current_texture() {
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder =
                        engine
                            .device()
                            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("resize-clear"),
                            });
                if radial_only {
                    let q = engine.queue();
                    let stops_circle = vec![
                        // Multi-stop radial gradient test
                        (0.0, engine_core::ColorLinPremul::from_srgba_u8([0x3b, 0x82, 0xf6, 0xff])),  // blue
                        (0.5, engine_core::ColorLinPremul::from_srgba_u8([0x1e, 0x3a, 0x8a, 0xff])),  // dark blue
                        (0.75, engine_core::ColorLinPremul::from_srgba_u8([0x10, 0xb9, 0x81, 0xff])), // teal
                        (1.0, engine_core::ColorLinPremul::from_srgba_u8([0x10, 0xb9, 0x81, 0xff])),  // teal
                    ];
                    let stops_ref = &stops_circle;
                    // Centered fullscreen radial; engine-core will apply mac DPI fix if needed
                    let center_uv = [0.5f32, 0.5f32];
                    let radius = 0.5f32 / std::f32::consts::SQRT_2; // ~0.35355339
                    passes.paint_root_radial_gradient_multi(&mut encoder, &view, center_uv, radius, stops_ref, &q, size.width, size.height);
                } else if linear_angled {
                    let q = engine.queue();
                    // Multi-stop angled linear gradient (45 degrees, top-left to bottom-right)
                    let stops_linear = vec![
                        (0.0, engine_core::ColorLinPremul::from_srgba_u8([0xff, 0x6b, 0x6b, 0xff])),  // coral red
                        (0.25, engine_core::ColorLinPremul::from_srgba_u8([0xff, 0xd9, 0x3d, 0xff])), // golden yellow
                        (0.5, engine_core::ColorLinPremul::from_srgba_u8([0x6b, 0xcf, 0x63, 0xff])),  // green
                        (0.75, engine_core::ColorLinPremul::from_srgba_u8([0x4d, 0x96, 0xff, 0xff])), // blue
                        (1.0, engine_core::ColorLinPremul::from_srgba_u8([0xc7, 0x7d, 0xff, 0xff])),  // purple
                    ];
                    // 45-degree angle: from top-left [0,0] to bottom-right [1,1]
                    passes.paint_root_linear_gradient_multi(&mut encoder, &view, [0.0, 0.0], [1.0, 1.0], &stops_linear, &q);
                } else {
                        // Paint root background (solid) cross-platform
                        passes.paint_root_color(
                            &mut encoder,
                            &view,
                            engine_core::ColorLinPremul::from_srgba_u8([80, 85, 97, 255]),
                        );
                    }
                    // No reupload needed; viewport uniform will handle the size change.
                    if !radial_only && !linear_angled {
                        let queue = engine.queue();
                        passes.render_frame(
                            &mut encoder,
                            engine.allocator_mut(),
                            &view,
                            size.width,
                            size.height,
                            &gpu_scene,
                            wgpu::Color {
                                r: 0.08,
                                g: 0.09,
                                b: 0.12,
                                a: 1.0,
                            },
                            bypass,
                            &queue,
                            true, // preserve background painted above
                        );
                    }
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
                if radial_only {
                    let q = engine.queue();
                    let stops_circle = vec![
                        // Multi-stop radial gradient test
                        (0.0, engine_core::ColorLinPremul::from_srgba_u8([0x3b, 0x82, 0xf6, 0xff])),  // blue
                        (0.5, engine_core::ColorLinPremul::from_srgba_u8([0x1e, 0x3a, 0x8a, 0xff])),  // dark blue
                        (0.75, engine_core::ColorLinPremul::from_srgba_u8([0x10, 0xb9, 0x81, 0xff])), // teal
                        (1.0, engine_core::ColorLinPremul::from_srgba_u8([0x10, 0xb9, 0x81, 0xff])),  // teal
                    ];
                    let stops_ref = &stops_circle;
                    let center_uv = [0.5f32, 0.5f32];
                    let radius = 0.5f32 / std::f32::consts::SQRT_2;
                    passes.paint_root_radial_gradient_multi(&mut encoder, &view, center_uv, radius, stops_ref, &q, size.width, size.height);
                } else if linear_angled {
                    let q = engine.queue();
                    // Multi-stop angled linear gradient (45 degrees, top-left to bottom-right)
                    let stops_linear = vec![
                        (0.0, engine_core::ColorLinPremul::from_srgba_u8([0xff, 0x6b, 0x6b, 0xff])),  // coral red
                        (0.25, engine_core::ColorLinPremul::from_srgba_u8([0xff, 0xd9, 0x3d, 0xff])), // golden yellow
                        (0.5, engine_core::ColorLinPremul::from_srgba_u8([0x6b, 0xcf, 0x63, 0xff])),  // green
                        (0.75, engine_core::ColorLinPremul::from_srgba_u8([0x4d, 0x96, 0xff, 0xff])), // blue
                        (1.0, engine_core::ColorLinPremul::from_srgba_u8([0xc7, 0x7d, 0xff, 0xff])),  // purple
                    ];
                    // 45-degree angle: from top-left [0,0] to bottom-right [1,1]
                    passes.paint_root_linear_gradient_multi(&mut encoder, &view, [0.0, 0.0], [1.0, 1.0], &stops_linear, &q);
                } else {
                    // Root background: multi-stop linear then a subtle radial highlight
                    let queue0 = engine.queue();
                    passes.paint_root_linear_gradient_multi(
                        &mut encoder,
                        &view,
                        [0.0, 0.0],
                        [1.0, 1.0],
                        &[
                            (
                                0.0,
                                engine_core::ColorLinPremul::from_srgba_u8([72, 76, 88, 255]),
                            ),
                            (
                                0.5,
                                engine_core::ColorLinPremul::from_srgba_u8([80, 85, 97, 255]),
                            ),
                            (
                                1.0,
                                engine_core::ColorLinPremul::from_srgba_u8([60, 64, 74, 255]),
                            ),
                        ],
                        &queue0,
                    );
                    passes.paint_root_radial_gradient_multi(
                        &mut encoder,
                        &view,
                        [0.75, 0.3],
                        0.5,
                        &[
                            (
                                0.0,
                                engine_core::ColorLinPremul::from_srgba_u8([40, 40, 40, 0]),
                            ),
                            (
                                1.0,
                                engine_core::ColorLinPremul::from_srgba_u8([40, 40, 40, 64]),
                            ),
                        ],
                        &queue0,
                        size.width,
                        size.height,
                    );
                }
                if needs_reupload {
                    if !radial_only && !radial_circle && !linear_angled {
                        gpu_scene = engine_core::upload_display_list(
                            engine.allocator_mut(),
                            &queue,
                            &dlist,
                        )
                        .expect("upload failed on resize");
                    }
                    needs_reupload = false;
                }
                if !radial_only && !linear_angled {
                    let queue = engine.queue();
                    passes.render_frame(
                        &mut encoder,
                        engine.allocator_mut(),
                        &view,
                        size.width,
                        size.height,
                        &gpu_scene,
                        wgpu::Color {
                            r: 0.08,
                            g: 0.09,
                            b: 0.12,
                            a: 1.0,
                        },
                        bypass,
                        &queue,
                        true, // preserve background painted above
                    );
                }
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
        Event::WindowEvent { event: WindowEvent::ScaleFactorChanged { scale_factor: new_sf, .. }, window_id } if window_id == window.id() => {
            // Update scale factor dynamically (mac only) and notify engine-core
            #[cfg(target_os = "macos")]
            {
                scale_factor = new_sf as f32;
                passes.set_scale_factor(scale_factor);
                // Reconfigure surface to match new physical size
                size = window.inner_size();
                config = make_surface_config(&adapter, &surface, size.width, size.height);
                surface.configure(&engine.device(), &config);
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
