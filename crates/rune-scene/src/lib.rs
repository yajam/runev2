use anyhow::Result;
use winit::event::{Event, WindowEvent};
use winit::window::WindowBuilder;
use winit::event_loop::EventLoop;
use engine_core::{make_surface_config, SubpixelOrientation, Brush, Transform2D};

pub mod elements;
pub mod text;
pub mod zones;
pub mod sample_ui;

use zones::{ZoneManager, ZoneId, TOGGLE_BUTTON_REGION_ID};

/// Render zone backgrounds and borders (excluding devtools which renders last)
fn render_zones(canvas: &mut rune_surface::Canvas, zone_manager: &ZoneManager) {
    // Z-index values are ignored since we're using painter's algorithm (emission order)
    // Render in back-to-front order: viewport, toolbar, sidebar
    // DevTools is rendered separately at the end to appear on top
    const Z: i32 = 0; // Z-index doesn't matter, using emission order

    // Render viewport, toolbar, and sidebar (but NOT devtools)
    for zone_id in [ZoneId::Viewport, ZoneId::Toolbar, ZoneId::Sidebar] {
        let rect = zone_manager.layout.get_zone(zone_id);
        let style = zone_manager.get_style(zone_id);

        // Background
        canvas.fill_rect(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            Brush::Solid(style.bg_color),
            Z,
        );

        // Border (draw as four rectangles)
        let bw = style.border_width;
        let border_brush = Brush::Solid(style.border_color);
        
        // Top border
        canvas.fill_rect(rect.x, rect.y, rect.w, bw, border_brush.clone(), Z);
        // Bottom border
        canvas.fill_rect(rect.x, rect.y + rect.h - bw, rect.w, bw, border_brush.clone(), Z);
        // Left border
        canvas.fill_rect(rect.x, rect.y, bw, rect.h, border_brush.clone(), Z);
        // Right border
        canvas.fill_rect(rect.x + rect.w - bw, rect.y, bw, rect.h, border_brush, Z);
    }
}

/// Render devtools zone background and border (called last to appear on top)
fn render_devtools_zone(canvas: &mut rune_surface::Canvas, zone_manager: &ZoneManager) {
    const Z: i32 = 0; // Z-index doesn't matter, using emission order
    
    let rect = zone_manager.layout.get_zone(ZoneId::DevTools);
    let style = zone_manager.get_style(ZoneId::DevTools);

    // Background
    canvas.fill_rect(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        Brush::Solid(style.bg_color),
        Z,
    );

    // Border
    let bw = style.border_width;
    let border_brush = Brush::Solid(style.border_color);
    canvas.fill_rect(rect.x, rect.y, rect.w, bw, border_brush.clone(), Z);
    canvas.fill_rect(rect.x, rect.y + rect.h - bw, rect.w, bw, border_brush.clone(), Z);
    canvas.fill_rect(rect.x, rect.y, bw, rect.h, border_brush.clone(), Z);
    canvas.fill_rect(rect.x + rect.w - bw, rect.y, bw, rect.h, border_brush, Z);
}

/// Zone-based rendering architecture for rune-scene
/// 
/// Zones:
/// - Viewport: Main content area (will be replaced with IR-based rendering)
/// - Sidebar: Left panel for tools/navigation
/// - Toolbar: Top bar for actions
/// - DevTools: Bottom panel for debugging
pub fn run() -> Result<()> {
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Rune Scene — UI Elements")
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
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))?;

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

    // Zone manager for layout and styling (use logical pixels)
    let logical_width = (size.width as f32 / scale_factor) as u32;
    let logical_height = (size.height as f32 / scale_factor) as u32;
    let mut zone_manager = ZoneManager::new(logical_width, logical_height);
    
    // Sample UI elements (will be replaced with IR-based rendering)
    let sample_ui = sample_ui::create_sample_elements();
    
    // Dirty flag: only redraw when something changes
    let mut needs_redraw = true;
    // Track last resize time to enable frequent redraws during resize
    let mut last_resize_time: Option<std::time::Instant> = None;
    let mut current_text_width: Option<f32> = None;

    // Shared state for sidebar visibility (needed for overlay callback)
    let sidebar_visible = std::sync::Arc::new(std::sync::Mutex::new(true));
    let sidebar_visible_overlay = sidebar_visible.clone();
    
    // TEMP: Overlay callback disabled to avoid dangling checkmark during testing
    // Uncomment below to re-enable crisp SVG tick rendering for checkboxes
    /*
    let checkboxes_for_overlay = sample_ui.checkboxes.clone();
    surf.set_overlay(Box::new(move |passes, encoder, view, queue, width, height| {
        let sidebar_vis = *sidebar_visible_overlay.lock().unwrap();
        let layout = zones::ZoneLayout::calculate(width, height, sidebar_vis);
        let viewport = layout.get_zone(ZoneId::Viewport);
        let inset = 2.0f32;
        for cb in checkboxes_for_overlay.iter() {
            if cb.checked {
                let inner_x = (cb.rect.x + viewport.x + inset).round();
                let inner_y = (cb.rect.y + viewport.y + inset).round();
                let inner_w = (cb.rect.w - 2.0 * inset).max(0.0).round();
                let inner_h = (cb.rect.h - 2.0 * inset).max(0.0).round();
                if let Some((tick_view, _sw, _sh)) = passes.rasterize_svg_to_view(
                    std::path::Path::new("images/check_white.svg"), 1.0, None, queue,
                ) {
                    passes.draw_image_quad(encoder, view, [inner_x, inner_y], [inner_w, inner_h],
                        &tick_view, queue, width, height);
                }
            }
        }
    }));
    */

    // Track cursor position and hit index for interaction
    let mut cursor_position: Option<(f32, f32)> = None;
    let mut hit_index: Option<engine_core::HitIndex> = None;
    
    Ok(event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { window_id, event } if window_id == window.id() => {
                match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::CursorMoved { position, .. } => {
                        cursor_position = Some((position.x as f32, position.y as f32));
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        if button == winit::event::MouseButton::Left && state == winit::event::ElementState::Pressed {
                            if let Some((cursor_x, cursor_y)) = cursor_position {
                                let logical_x = cursor_x / scale_factor;
                                let logical_y = cursor_y / scale_factor;
                                
                                // Perform hit test using the stored hit index
                                if let Some(ref index) = hit_index {
                                    if let Some(hit) = index.topmost_at([logical_x, logical_y]) {
                                        if let Some(region_id) = hit.region_id {
                                            if region_id == TOGGLE_BUTTON_REGION_ID {
                                                let logical_width = (size.width as f32 / scale_factor) as u32;
                                                let logical_height = (size.height as f32 / scale_factor) as u32;
                                                zone_manager.toggle_sidebar(logical_width, logical_height);
                                                *sidebar_visible.lock().unwrap() = zone_manager.is_sidebar_visible();
                                                needs_redraw = true;
                                                window.request_redraw();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    WindowEvent::Resized(new_size) => {
                        size = new_size;
                        if size.width > 0 && size.height > 0 {
                            config.width = size.width;
                            config.height = size.height;
                            surface.configure(surf.device().as_ref(), &config);
                            let logical_width = (size.width as f32 / scale_factor) as u32;
                            let logical_height = (size.height as f32 / scale_factor) as u32;
                            zone_manager.resize(logical_width, logical_height);
                        }
                        last_resize_time = Some(std::time::Instant::now());
                        needs_redraw = true;
                        window.request_redraw();
                    }
                    WindowEvent::ScaleFactorChanged { scale_factor: sf, .. } => {
                        let new_scale = sf as f32;
                        surf.set_dpi_scale(new_scale);
                        needs_redraw = true;
                        window.request_redraw();
                    }
                    WindowEvent::RedrawRequested => {
                        if !needs_redraw || size.width == 0 || size.height == 0 { return; }
                        let frame = match surface.get_current_texture() {
                            Ok(f) => f,
                            Err(_) => { window.request_redraw(); return; }
                        };
                        let mut canvas = surf.begin_frame(size.width, size.height);
                        canvas.set_text_provider(provider.clone());

                        // Render zone backgrounds and borders first (lowest z-index)
                        render_zones(&mut canvas, &zone_manager);
                        
                        // Render sample UI elements in viewport zone with local coordinates
                        // This must come BEFORE devtools so devtools renders on top
                        let viewport_rect = zone_manager.layout.get_zone(ZoneId::Viewport);
                        canvas.push_transform(Transform2D::translate(viewport_rect.x, viewport_rect.y));
                        // TEMP: Disable clip to test if it's causing issues
                        // use engine_core::Rect;
                        // let local_clip = Rect { x: 0.0, y: 0.0, w: viewport_rect.w, h: viewport_rect.h };
                        // canvas.push_clip_rect(local_clip);
                        sample_ui.render(&mut canvas, scale_factor, size.width);
                        // canvas.pop_clip();
                        canvas.pop_transform();
                        
                        // Render toolbar content with hit regions (on top of viewport)
                        let toolbar_rect = zone_manager.layout.get_zone(ZoneId::Toolbar);
                        canvas.push_transform(Transform2D::translate(toolbar_rect.x, toolbar_rect.y));
                        zone_manager.toolbar.render(&mut canvas, toolbar_rect);
                        canvas.pop_transform();
                        
                        // Render devtools zone background and border (after viewport to ensure overlay)
                        render_devtools_zone(&mut canvas, &zone_manager);
                        
                        // Render devtools content
                        let devtools_rect = zone_manager.layout.get_zone(ZoneId::DevTools);
                        canvas.push_transform(Transform2D::translate(devtools_rect.x, devtools_rect.y));
                        zone_manager.devtools.render(&mut canvas, devtools_rect);
                        canvas.pop_transform();
                        
                        // Calculate text width for resize detection
                        let logical_width = size.width as f32 / scale_factor;
                        let right_margin = 40.0f32;
                        let container_width = (logical_width - 40.0 - right_margin).max(200.0).min(1200.0);
                        current_text_width = Some(container_width);
                        
                        // Build hit index from display list before ending frame
                        hit_index = Some(engine_core::HitIndex::build(canvas.display_list()));
                        
                        surf.end_frame(frame, canvas).ok();
                        needs_redraw = false;
                    }
                    _ => {}
                }
            }
            Event::AboutToWait => {
                // During active resize (within 100ms of last resize event), request continuous redraws
                if let Some(last_time) = last_resize_time {
                    if last_time.elapsed() < std::time::Duration::from_millis(100) {
                        needs_redraw = true;
                        window.request_redraw();
                    } else {
                        // Resize ended - check if text width changed
                        last_resize_time = None;
                        
                        // Calculate new text width
                        let logical_width = size.width as f32 / scale_factor;
                        let right_margin = 40.0f32;
                        let new_text_width = (logical_width - 40.0 - right_margin).max(200.0).min(1200.0);
                        
                        // Compare with current width (threshold of 10px to avoid minor changes)
                        let width_changed = current_text_width.map_or(true, |old_width| {
                            (new_text_width - old_width).abs() > 10.0
                        });
                        
                        if width_changed {
                            needs_redraw = true;
                            window.request_redraw();
                        }
                    }
                } else if needs_redraw {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    })?)
}
