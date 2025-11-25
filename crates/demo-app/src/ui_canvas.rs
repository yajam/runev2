use anyhow::Result;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

use engine_core::{ColorLinPremul, SubpixelOrientation, make_surface_config};

// Canvas-backed UI runner so we can verify using Canvas within demo-app
pub fn run() -> Result<()> {
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Rune Draw Demo — UI (Canvas)")
        .build(&event_loop)?;
    let window: &'static winit::window::Window = Box::leak(Box::new(window));

    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(window)?;
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }))
    .expect("No suitable GPU adapters found");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))?;

    let mut size = window.inner_size();
    let scale_factor = window.scale_factor() as f32;
    let mut config = make_surface_config(&adapter, &surface, size.width, size.height);
    surface.configure(&device, &config);

    // Canvas wrapper
    let mut surf = rune_surface::RuneSurface::new(
        std::sync::Arc::new(device),
        std::sync::Arc::new(queue),
        config.format,
    );
    surf.set_use_intermediate(true);
    surf.set_direct(true);
    surf.set_logical_pixels(true);
    surf.set_dpi_scale(scale_factor);

    // Provide a text provider (system fonts)
    let provider = engine_core::CosmicTextProvider::from_system_fonts(SubpixelOrientation::RGB);
    let provider = std::sync::Arc::new(provider);

    Ok(event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { window_id, event } if window_id == window.id() => {
                match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::Resized(new_size) => {
                        size = new_size;
                        if size.width > 0 && size.height > 0 {
                            config.width = size.width;
                            config.height = size.height;
                            surface.configure(surf.device().as_ref(), &config);
                        }
                        window.request_redraw();
                    }
                    WindowEvent::ScaleFactorChanged {
                        scale_factor: _sf, ..
                    } => {
                        window.request_redraw();
                    }
                    WindowEvent::RedrawRequested => {
                        if size.width == 0 || size.height == 0 {
                            return;
                        }
                        let frame = match surface.get_current_texture() {
                            Ok(f) => f,
                            Err(_) => {
                                window.request_redraw();
                                return;
                            }
                        };
                        let mut canvas = surf.begin_frame(size.width, size.height);
                        // Background
                        let bg = Color::rgba(26, 31, 51, 255);
                        canvas.clear(bg);
                        canvas.set_text_provider(provider.clone());

                        // Render the same UI as rune-scene using its elements on Canvas
                        use engine_core::{Brush, Color, Rect};
                        let col1_x = 40.0f32;
                        let w = size.width as f32;
                        let mut y = 40.0f32;
                        let row_h = 44.0f32;
                        canvas.fill_rect(0.0, 0.0, w, size.height as f32, Brush::Solid(bg), 0);
                        // Use pure white for crisp text on dark background
                        canvas.draw_text_run(
                            [col1_x, y],
                            "Rune Scene — UI Elements".to_string(),
                            22.0,
                            Color::rgba(255, 255, 255, 255),
                            10,
                        );
                        y += 36.0;
                        // Buttons
                        {
                            let btn1 = rune_scene::elements::button::Button {
                                rect: Rect {
                                    x: col1_x,
                                    y,
                                    w: 160.0,
                                    h: 36.0,
                                },
                                radius: 8.0,
                                bg: Color::rgba(63, 130, 246, 255),
                                fg: Color::rgba(255, 255, 255, 255),
                                label: "Primary".to_string(),
                                label_size: 16.0,
                                focused: false,
                                on_click_intent: None,
                            };
                            let btn2 = rune_scene::elements::button::Button {
                                rect: Rect {
                                    x: col1_x + 176.0,
                                    y,
                                    w: 180.0,
                                    h: 36.0,
                                },
                                radius: 8.0,
                                bg: Color::rgba(99, 104, 118, 255),
                                fg: Color::rgba(255, 255, 255, 255),
                                label: "Secondary (focused)".to_string(),
                                label_size: 16.0,
                                focused: true,
                                on_click_intent: None,
                            };
                            btn1.render(&mut canvas, 5);
                            btn2.render(&mut canvas, 5);
                            y += row_h;
                        }
                        // Checkbox examples
                        {
                            let cb1 = rune_scene::elements::checkbox::Checkbox {
                                rect: Rect {
                                    x: col1_x,
                                    y,
                                    w: 18.0,
                                    h: 18.0,
                                },
                                checked: false,
                                focused: false,
                                label: Some("Checkbox".to_string()),
                                label_size: 16.0,
                                label_color: Color::rgba(240, 240, 240, 255),
                                box_fill: Color::rgba(50, 50, 50, 255),
                                border_color: Color::rgba(100, 100, 100, 255),
                                border_width: 1.0,
                                check_color: Color::rgba(100, 200, 100, 255),
                            };
                            let cb2 = rune_scene::elements::checkbox::Checkbox {
                                rect: Rect {
                                    x: col1_x + 160.0,
                                    y,
                                    w: 18.0,
                                    h: 18.0,
                                },
                                checked: true,
                                focused: true,
                                label: Some("Checked + Focus".to_string()),
                                label_size: 16.0,
                                label_color: Color::rgba(240, 240, 240, 255),
                                box_fill: Color::rgba(50, 50, 50, 255),
                                border_color: Color::rgba(100, 180, 255, 255),
                                border_width: 2.0,
                                check_color: Color::rgba(100, 200, 100, 255),
                            };
                            cb1.render(&mut canvas, 5);
                            cb2.render(&mut canvas, 5);
                            y += row_h;
                        }
                        // Input box
                        {
                            let mut ib = rune_scene::elements::input_box::InputBox::new(
                                Rect {
                                    x: col1_x,
                                    y,
                                    w: 300.0,
                                    h: 34.0,
                                },
                                "Type here...".to_string(),
                                16.0,
                                ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
                                Some("Enter text...".to_string()),
                                true,
                            );
                            ib.render(&mut canvas, 5, provider.as_ref());
                        }
                        surf.end_frame(frame, canvas).ok();
                    }
                    _ => {}
                }
            }
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => {}
        }
    })?)
}
