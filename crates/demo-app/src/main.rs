use anyhow::Result;
use engine_core::{GraphicsEngine, PassManager, Viewport, make_surface_config};
use pollster::FutureExt;
use winit::event::{ElementState, Event, MouseButton, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

mod scenes;
use scenes::{Scene, SceneKind};
mod ui_canvas;

fn main() -> Result<()> {
    // Optional Canvas-based UI path to verify Canvas rendering in demo-app
    if std::env::var("DEMO_SCENE").as_deref().ok() == Some("ui-canvas")
        || std::env::args().any(|a| a == "--scene=ui-canvas" || a == "--ui-canvas")
    {
        return ui_canvas::run();
    }
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
    // Track DPI scale factor and push into engine-core
    let scale_factor: f32 = window.scale_factor() as f32;
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
    } else if scene_env.as_deref() == Some("shadow")
        || std::env::args().any(|a| a == "--scene=shadow" || a == "--shadow")
    {
        Box::new(scenes::shadow::ShadowScene::default())
    } else if scene_env.as_deref() == Some("overlay")
        || std::env::args().any(|a| a == "--scene=overlay" || a == "--overlay")
    {
        Box::new(scenes::overlay::OverlayScene::default())
    } else if scene_env.as_deref() == Some("zones")
        || std::env::args().any(|a| a == "--scene=zones" || a == "--zones")
    {
        Box::new(scenes::zones::ZonesScene::default())
    } else if scene_env.as_deref() == Some("images")
        || std::env::args().any(|a| a == "--scene=images" || a == "--images")
    {
        Box::new(scenes::images::ImagesScene::default())
    } else if scene_env.as_deref() == Some("hyperlinks")
        || std::env::args().any(|a| a == "--scene=hyperlinks" || a == "--hyperlinks")
    {
        Box::new(scenes::hyperlinks::HyperlinksScene::default())
    } else if scene_env.as_deref() == Some("svg")
        || std::env::args().any(|a| a == "--scene=svg" || a == "--svg" || a == "--scene=svg-geom")
    {
        Box::new(scenes::svg_geom::SvgGeomScene::default())
    } else if scene_env.as_deref() == Some("path")
        || std::env::args()
            .any(|a| a == "--scene=path" || a == "--path" || a == "--scene=path-demo")
    {
        Box::new(scenes::path_demo::PathDemoScene::default())
    } else if scene_env.as_deref() == Some("ui") || std::env::args().any(|a| a == "--scene=ui") {
        Box::new(scenes::ui::UiElementsScene::default())
    } else if scene_env.as_deref() == Some("unified-test")
        || std::env::args().any(|a| a == "--scene=unified-test" || a == "--unified-test")
    {
        Box::new(scenes::unified_test::UnifiedTestScene::default())
    } else {
        Box::new(scenes::default::DefaultScene::default())
    };

    // Pass initial DPI scale to scenes that care
    scene.set_scale_factor(scale_factor);
    let mut dlist_opt = scene.init_display_list(Viewport {
        width: size.width,
        height: size.height,
    });
    // Build initial hit-test index if the scene is geometry-based
    let mut hit_index_opt: Option<engine_core::HitIndex> = match (&scene.kind(), &dlist_opt) {
        (SceneKind::Geometry, Some(dl)) => Some(engine_core::HitIndex::build(dl)),
        _ => None,
    };
    let mut passes = PassManager::new(engine.device(), config.format);
    // Always render in logical pixels scaled by device DPI.
    passes.set_logical_pixels(true);
    passes.set_ui_scale(1.0);
    passes.set_scale_factor(scale_factor);
    // Unified rendering uses intermediate texture by default for smooth resizing
    let use_intermediate = std::env::var("USE_INTERMEDIATE")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(true); // Default to true for smooth resizing

    // Initialize intermediate texture for fullscreen backgrounds
    if use_intermediate && matches!(scene.kind(), SceneKind::FullscreenBackground) {
        passes.ensure_intermediate_texture(engine.allocator_mut(), size.width, size.height);
    }

    let mut hovered_id: Option<usize> = None;
    let mut pressed_id: Option<usize> = None;
    let mut last_cursor_pos: [f32; 2] = [0.0, 0.0];

    // Optional: font providers for text rendering (set DEMO_FONT to a .ttf path)
    // Build RGB, BGR subpixel providers and a grayscale provider for comparisons.
    let text_providers: Option<(
        Box<dyn engine_core::TextProvider>,
        Box<dyn engine_core::TextProvider>,
        Box<dyn engine_core::TextProvider>,
    )> = {
        // Prefer cosmic-text by default; fall back to fontdue if feature is off
        #[cfg(feature = "cosmic_text_shaper")]
        {
            #[cfg(feature = "freetype_ffi")]
            let use_freetype = std::env::var("DEMO_FREETYPE")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            if let Ok(path) = std::env::var("DEMO_FONT") {
                if let Ok(bytes) = std::fs::read(path) {
                    #[cfg(feature = "freetype_ffi")]
                    let (rgb_res, bgr_res) = if use_freetype {
                        (
                            engine_core::FreeTypeProvider::from_bytes(
                                &bytes,
                                engine_core::SubpixelOrientation::RGB,
                            )
                            .map(|p| -> Box<dyn engine_core::TextProvider> { Box::new(p) }),
                            engine_core::FreeTypeProvider::from_bytes(
                                &bytes,
                                engine_core::SubpixelOrientation::BGR,
                            )
                            .map(|p| -> Box<dyn engine_core::TextProvider> { Box::new(p) }),
                        )
                    } else {
                        (
                            engine_core::CosmicTextProvider::from_bytes(
                                &bytes,
                                engine_core::SubpixelOrientation::RGB,
                            )
                            .map(|p| -> Box<dyn engine_core::TextProvider> { Box::new(p) }),
                            engine_core::CosmicTextProvider::from_bytes(
                                &bytes,
                                engine_core::SubpixelOrientation::BGR,
                            )
                            .map(|p| -> Box<dyn engine_core::TextProvider> { Box::new(p) }),
                        )
                    };
                    #[cfg(not(feature = "freetype_ffi"))]
                    let (rgb_res, bgr_res) = (
                        engine_core::CosmicTextProvider::from_bytes(
                            &bytes,
                            engine_core::SubpixelOrientation::RGB,
                        )
                        .map(|p| -> Box<dyn engine_core::TextProvider> { Box::new(p) }),
                        engine_core::CosmicTextProvider::from_bytes(
                            &bytes,
                            engine_core::SubpixelOrientation::BGR,
                        )
                        .map(|p| -> Box<dyn engine_core::TextProvider> { Box::new(p) }),
                    );
                    let gray_res = engine_core::GrayscaleFontdueProvider::from_bytes(&bytes)
                        .map(|p| -> Box<dyn engine_core::TextProvider> { Box::new(p) });
                    if let (Ok(rgb), Ok(bgr), Ok(gray)) = (rgb_res, bgr_res, gray_res) {
                        Some((rgb, bgr, gray))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                // Use system fonts for cosmic provider when DEMO_FONT is not set
                let rgb = engine_core::CosmicTextProvider::from_system_fonts(
                    engine_core::SubpixelOrientation::RGB,
                );
                let bgr = engine_core::CosmicTextProvider::from_system_fonts(
                    engine_core::SubpixelOrientation::BGR,
                );
                // For grayscale comparison, reuse RGB provider (subpixel), if no DEMO_FONT available
                Some((
                    Box::new(rgb),
                    Box::new(bgr),
                    Box::new(engine_core::CosmicTextProvider::from_system_fonts(
                        engine_core::SubpixelOrientation::RGB,
                    )),
                ))
            }
        }
        #[cfg(not(feature = "cosmic_text_shaper"))]
        {
            std::env::var("DEMO_FONT")
                .ok()
                .and_then(|path| std::fs::read(path).ok())
                .and_then(|bytes| {
                    let rgb = engine_core::SimpleFontdueProvider::from_bytes(
                        &bytes,
                        engine_core::SubpixelOrientation::RGB,
                    )
                    .ok();
                    let bgr = engine_core::SimpleFontdueProvider::from_bytes(
                        &bytes,
                        engine_core::SubpixelOrientation::BGR,
                    )
                    .ok();
                    let gray = engine_core::GrayscaleFontdueProvider::from_bytes(&bytes).ok();
                    match (rgb, bgr, gray) {
                        (Some(rgb), Some(bgr), Some(gray)) => {
                            Some((Box::new(rgb), Box::new(bgr), Box::new(gray)))
                        }
                        _ => None,
                    }
                })
        }
    };

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
                let vp = Viewport {
                    width: size.width,
                    height: size.height,
                };
                match scene.kind() {
                    SceneKind::Geometry => {
                        if let Some(new_dl) = scene.on_resize(vp) {
                            dlist_opt = Some(new_dl);
                        } else if let Some(dl) = &mut dlist_opt {
                            dl.viewport = vp;
                        }
                        // Rebuild hit-test index for updated display list
                        hit_index_opt = match (&scene.kind(), &dlist_opt) {
                            (SceneKind::Geometry, Some(dl)) => {
                                Some(engine_core::HitIndex::build(dl))
                            }
                            _ => None,
                        };
                    }
                    SceneKind::FullscreenBackground => {
                        // Intermediate texture will be reallocated in RedrawRequested if size changed
                    }
                }
            }
        }
        // Map mouse moves to hit-test queries
        Event::WindowEvent {
            event: WindowEvent::CursorMoved { position, .. },
            window_id,
        } if window_id == window.id() => {
            if let Some(index) = hit_index_opt.as_ref() {
                let pos = [position.x as f32, position.y as f32];
                last_cursor_pos = pos;
                let res = index.topmost_at(pos);
                let new_hover = res.as_ref().map(|r| r.id);
                if new_hover != hovered_id {
                    hovered_id = new_hover;
                    if let Some(ref r) = res {
                        let title = format!(
                            "Rune Draw Demo — Hover: {:?} (id={}, z={})",
                            r.kind, r.id, r.z
                        );
                        window.set_title(&title);
                    } else {
                        window.set_title("Rune Draw Demo");
                    }
                }
                // Scene event: pointer move (may update DL for hover highlight)
                if let SceneKind::Geometry = scene.kind() {
                    if let Some(new_dl) = scene.on_pointer_move(pos, res.as_ref()) {
                        dlist_opt = Some(new_dl);
                        hit_index_opt = match (&scene.kind(), &dlist_opt) {
                            (SceneKind::Geometry, Some(dl)) => {
                                Some(engine_core::HitIndex::build(dl))
                            }
                            _ => None,
                        };
                        window.request_redraw();
                    }
                }
                if pressed_id.is_some() {
                    // Scene drag update
                    if let SceneKind::Geometry = scene.kind() {
                        if let Some(new_dl) = scene.on_drag(pos, res.as_ref()) {
                            dlist_opt = Some(new_dl);
                            hit_index_opt = match (&scene.kind(), &dlist_opt) {
                                (SceneKind::Geometry, Some(dl)) => {
                                    Some(engine_core::HitIndex::build(dl))
                                }
                                _ => None,
                            };
                            window.request_redraw();
                        }
                    }
                }
            }
        }
        // Mouse press/release -> click/drag bookkeeping
        Event::WindowEvent {
            event: WindowEvent::MouseInput { state, button, .. },
            window_id,
        } if window_id == window.id() => {
            if let (Some(index), MouseButton::Left) = (hit_index_opt.as_ref(), button) {
                if state == ElementState::Pressed {
                    let res = index.topmost_at(last_cursor_pos);
                    pressed_id = res.as_ref().map(|r| r.id);
                    // Scene pointer down
                    if let SceneKind::Geometry = scene.kind() {
                        if let Some(new_dl) = scene.on_pointer_down(last_cursor_pos, res.as_ref()) {
                            dlist_opt = Some(new_dl);
                            hit_index_opt = match (&scene.kind(), &dlist_opt) {
                                (SceneKind::Geometry, Some(dl)) => {
                                    Some(engine_core::HitIndex::build(dl))
                                }
                                _ => None,
                            };
                            window.request_redraw();
                        }
                    }
                } else {
                    let res = index.topmost_at(last_cursor_pos);
                    if let (Some(pid), Some(r)) = (pressed_id.take(), res.clone()) {
                        if pid == r.id {
                            // Scene click
                            if let SceneKind::Geometry = scene.kind() {
                                if let Some(new_dl) = scene.on_click(last_cursor_pos, res.as_ref())
                                {
                                    dlist_opt = Some(new_dl);
                                    hit_index_opt = match (&scene.kind(), &dlist_opt) {
                                        (SceneKind::Geometry, Some(dl)) => {
                                            Some(engine_core::HitIndex::build(dl))
                                        }
                                        _ => None,
                                    };
                                    window.request_redraw();
                                }
                            }
                        } else {
                        }
                    } else {
                        pressed_id = None;
                    }
                    // Scene pointer up
                    if let SceneKind::Geometry = scene.kind() {
                        if let Some(new_dl) = scene.on_pointer_up(last_cursor_pos, res.as_ref()) {
                            dlist_opt = Some(new_dl);
                            hit_index_opt = match (&scene.kind(), &dlist_opt) {
                                (SceneKind::Geometry, Some(dl)) => {
                                    Some(engine_core::HitIndex::build(dl))
                                }
                                _ => None,
                            };
                            window.request_redraw();
                        }
                    }
                }
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

                // Always use unified rendering - upload the current display list
                let unified_scene_opt = if let (SceneKind::Geometry, Some(dl)) = (scene.kind(), dlist_opt.as_ref()) {
                    let q2 = engine.queue();
                    // Use unified upload to extract all element types
                    Some(
                        engine_core::upload_display_list_unified(engine.allocator_mut(), &q2, dl)
                            .expect("unified upload failed"),
                    )
                } else {
                    None
                };

                match scene.kind() {
                    SceneKind::Geometry => {
                        // Always use unified rendering path
                        if let Some(unified_scene) = unified_scene_opt.as_ref() {
                            let queue = engine.queue();

                            // Rasterize text runs from the unified scene
                            let mut glyph_draws = Vec::new();
                            // eprintln!("🔍 Text draws in unified_scene: {}", unified_scene.text_draws.len());
                            if let Some(provider) = text_providers.as_ref().map(|(rgb, _, _)| &**rgb) {
                                for text_draw in &unified_scene.text_draws {
                                    // eprintln!("  📝 Processing text: '{}' at z={}", text_draw.run.text, text_draw.z);
                                    // Apply transform to text position
                                    let transformed_pos = [
                                        text_draw.run.pos[0] + text_draw.transform.m[4],
                                        text_draw.run.pos[1] + text_draw.transform.m[5],
                                    ];

                                    // Rasterize the text run
                                    let glyphs = provider.rasterize_run(&text_draw.run);
                                    // eprintln!("    ✏️  Rasterized {} glyphs", glyphs.len());

                                    // Add each glyph with its position, color, and z-index
                                    for glyph in glyphs {
                                        glyph_draws.push((
                                            [transformed_pos[0] + glyph.offset[0], transformed_pos[1] + glyph.offset[1]],
                                            glyph,
                                            text_draw.run.color,
                                            text_draw.z,
                                        ));
                                    }
                                }
                            } else {
                                // eprintln!("⚠️  No text provider available!");
                            }
                            // eprintln!("🔍 Total glyph_draws: {}", glyph_draws.len());

                            // Convert image and SVG draws to the format expected by render_unified
                            let image_draws: Vec<(std::path::PathBuf, [f32; 2], [f32; 2], i32)> =
                                unified_scene.image_draws.iter()
                                    .map(|d| (d.path.clone(), d.origin, d.size, d.z))
                                    .collect();

                            let svg_draws: Vec<(std::path::PathBuf, [f32; 2], [f32; 2], Option<engine_core::SvgStyle>, i32, engine_core::Transform2D)> =
                                unified_scene.svg_draws.iter()
                                    .map(|d| (d.path.clone(), d.origin, d.size, None, d.z, d.transform))
                                    .collect();

                            // Use unified rendering
                            passes.render_unified(
                                &mut encoder,
                                engine.allocator_mut(),
                                &view,
                                size.width,
                                size.height,
                                &unified_scene.gpu_scene,
                                &glyph_draws,
                                &svg_draws,
                                &image_draws,
                                wgpu::Color {
                                    r: 30.0 / 255.0,
                                    g: 30.0 / 255.0,
                                    b: 40.0 / 255.0,
                                    a: 1.0,
                                },
                                !use_intermediate,  // direct rendering if not using intermediate
                                &queue,
                                false,  // preserve_surface
                            );
                        }
                    }
                    SceneKind::FullscreenBackground => {
                        // Render fullscreen background (gradients, etc.)
                        let queue = engine.queue();
                        if use_intermediate {
                            // Ensure intermediate texture exists and matches current size
                            passes.ensure_intermediate_texture(
                                engine.allocator_mut(),
                                size.width,
                                size.height,
                            );
                            // Clear intermediate texture before rendering
                            passes
                                .clear_intermediate_texture(&mut encoder, wgpu::Color::TRANSPARENT);
                            // Create a temporary texture view to avoid borrow issues
                            let intermediate_tex = passes.intermediate_texture.as_ref().unwrap();
                            let intermediate_view = intermediate_tex
                                .texture
                                .create_view(&wgpu::TextureViewDescriptor::default());
                            scene.paint_root_background(
                                &mut passes,
                                &mut encoder,
                                &intermediate_view,
                                &queue,
                                size.width,
                                size.height,
                            );
                            passes.blit_to_surface(&mut encoder, &view);
                        } else {
                            // Direct rendering to surface
                            scene.paint_root_background(
                                &mut passes,
                                &mut encoder,
                                &view,
                                &queue,
                                size.width,
                                size.height,
                            );
                        }
                    }
                }
                engine.queue().submit(std::iter::once(encoder.finish()));
                frame.present();
            }
            Err(_) => {
                let new_config = make_surface_config(&adapter, &surface, size.width, size.height);
                surface.configure(&engine.device(), &new_config);
                // No need to rebuild passes; surface format usually unchanged.
            }
        },
        Event::WindowEvent {
            event:
                WindowEvent::ScaleFactorChanged {
                    scale_factor: new_sf,
                    ..
                },
            window_id,
        } if window_id == window.id() => {
            // Update scale factor dynamically and notify engine-core
            let new_scale_factor = new_sf as f32;
            passes.set_scale_factor(new_scale_factor);
            scene.set_scale_factor(new_scale_factor);
            // Reconfigure surface to match new physical size
            size = window.inner_size();
            let new_config = make_surface_config(&adapter, &surface, size.width, size.height);
            surface.configure(&engine.device(), &new_config);
            window.request_redraw();
        }
        Event::AboutToWait => {
            window.request_redraw();
        }
        _ => {}
    })?;

    Ok(())
}
