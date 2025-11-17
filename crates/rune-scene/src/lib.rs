use anyhow::Result;
use winit::event::{Event, WindowEvent};
use winit::window::WindowBuilder;
use winit::event_loop::EventLoop;
use engine_core::{
    make_surface_config,
    SubpixelOrientation,
    Brush,
    Rect,
    RoundedRect,
    RoundedRadii,
    ColorLinPremul,
    TextRun,
    Transform2D,
};

pub mod elements;
pub mod text;
pub mod zones;
pub mod viewport_ir;

use zones::{ZoneManager, ZoneId, DevToolsTab, TOGGLE_BUTTON_REGION_ID, DEVTOOLS_BUTTON_REGION_ID, DEVTOOLS_CLOSE_BUTTON_REGION_ID, DEVTOOLS_ELEMENTS_TAB_REGION_ID, DEVTOOLS_CONSOLE_TAB_REGION_ID};

/// Render zone backgrounds and borders (viewport, toolbar, sidebar).
fn render_zones(canvas: &mut rune_surface::Canvas, zone_manager: &ZoneManager) {
    const Z: i32 = 0;

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

    // Provide a text provider.
    //
    // Default: RuneTextProvider (harfrust + swash) - the recommended approach.
    // - If `RUNE_TEXT_FONT=path`, load custom font from path.
    let provider: std::sync::Arc<dyn engine_core::TextProvider> = {
        // Check for custom font path
        if let Ok(path) = std::env::var("RUNE_TEXT_FONT") {
            if let Ok(bytes) = std::fs::read(&path) {
                if let Ok(p) = engine_core::RuneTextProvider::from_bytes(
                    &bytes,
                    SubpixelOrientation::RGB,
                ) {
                    std::sync::Arc::new(p)
                } else {
                    create_default_provider()
                }
            } else {
                create_default_provider()
            }
        } else {
            create_default_provider()
        }
    };

    fn create_default_provider() -> std::sync::Arc<dyn engine_core::TextProvider> {
        std::sync::Arc::new(
            engine_core::RuneTextProvider::from_system_fonts(SubpixelOrientation::RGB)
                .expect("Failed to load system fonts")
        )
    }

    // Zone manager for layout and styling (use logical pixels)
    let logical_width = (size.width as f32 / scale_factor) as u32;
    let logical_height = (size.height as f32 / scale_factor) as u32;
    let mut zone_manager = ZoneManager::new(logical_width, logical_height);
    
    // Viewport IR content (formerly sample_ui)
    let mut viewport_ir = viewport_ir::create_sample_elements();
    
    // Text layout cache for efficient resize performance
    let text_cache = std::sync::Arc::new(engine_core::TextLayoutCache::new(200));
    
    // Dirty flag: only redraw when something changes
    let mut needs_redraw = true;
    // Track last resize time to debounce redraws (only redraw after resize settles)
    let mut last_resize_time: Option<std::time::Instant> = None;
    // Track first resize time to enforce maximum debounce duration
    let mut first_resize_time: Option<std::time::Instant> = None;
    // Track if we need to redraw backgrounds immediately (no debounce)
    let mut needs_background_redraw = false;
    // Track previous size to detect actual size changes (vs just resize events)
    let mut prev_size = size;

    // Shared state for sidebar visibility
    let sidebar_visible = std::sync::Arc::new(std::sync::Mutex::new(true));
    let sidebar_visible_overlay = sidebar_visible.clone();

    // Devtools and toolbar overlay: drawn after all other content via RuneSurface overlay,
    // so they always appear above viewport content, including raster images.
    let devtools_style = zone_manager.devtools.style.clone();
    let toolbar_style = zone_manager.toolbar.style.clone();
    let overlay_provider = provider.clone();
    let overlay_scale = scale_factor;

    // Shared state for devtools visibility
    let devtools_visible = std::sync::Arc::new(std::sync::Mutex::new(false));
    let devtools_visible_overlay = devtools_visible.clone();
    
    // Shared state for devtools active tab
    let devtools_active_tab = std::sync::Arc::new(std::sync::Mutex::new(DevToolsTab::Elements));
    let devtools_active_tab_overlay = devtools_active_tab.clone();

    // Overlay callback for toolbar chrome (always) and devtools chrome (when visible)
    surf.set_overlay(Box::new(move |passes, encoder, view, queue, width, height| {
        // Recompute layout in logical coordinates for the current size (shared by toolbar/devtools).
        let sidebar_vis = *sidebar_visible_overlay.lock().unwrap();
        let logical_width = (width as f32 / overlay_scale).max(0.0) as u32;
        let logical_height = (height as f32 / overlay_scale).max(0.0) as u32;
        let layout = zones::ZoneLayout::calculate(logical_width, logical_height, sidebar_vis);

        // --- Toolbar overlay: always render above viewport content ---
        let toolbar_rect = layout.get_zone(ZoneId::Toolbar);

        // Toolbar background strip
        let toolbar_rrect = RoundedRect {
            rect: toolbar_rect,
            radii: RoundedRadii { tl: 0.0, tr: 0.0, br: 0.0, bl: 0.0 },
        };
        passes.draw_filled_rounded_rect(
            encoder,
            view,
            width,
            height,
            toolbar_rrect,
            toolbar_style.bg_color,
            queue,
        );

        // Optional toolbar border (bottom edge to visually separate viewport).
        if toolbar_style.border_width > 0.0 {
            let bw = toolbar_style.border_width;
            let border_rect = Rect {
                x: toolbar_rect.x,
                y: toolbar_rect.y + toolbar_rect.h - bw,
                w: toolbar_rect.w,
                h: bw,
            };
            let border_rrect = RoundedRect {
                rect: border_rect,
                radii: RoundedRadii { tl: 0.0, tr: 0.0, br: 0.0, bl: 0.0 },
            };
            passes.draw_filled_rounded_rect(
                encoder,
                view,
                width,
                height,
                border_rrect,
                toolbar_style.border_color,
                queue,
            );
        }

        // Toolbar icons (sidebar toggle and devtools toggle)
        let toggle_size = 24.0;
        let toggle_margin = 12.0;

        // Sidebar toggle on the left
        let toggle_x = toolbar_rect.x + toggle_margin;
        let toggle_y = toolbar_rect.y + (toolbar_rect.h - toggle_size) * 0.5;

        let white = ColorLinPremul::rgba(255, 255, 255, 255);
        let icon_style = engine_core::SvgStyle::new()
            .with_stroke(white)
            .with_stroke_width(1.5);

        draw_svg_icon(
            passes,
            encoder,
            view,
            queue,
            width,
            height,
            "images/panel-left.svg",
            [toggle_x, toggle_y],
            [toggle_size, toggle_size],
            icon_style,
        );

        // Devtools toggle on the right
        let devtools_x = toolbar_rect.x + toolbar_rect.w - toggle_size - toggle_margin;
        let devtools_y = toolbar_rect.y + (toolbar_rect.h - toggle_size) * 0.5;

        draw_svg_icon(
            passes,
            encoder,
            view,
            queue,
            width,
            height,
            "images/inspection-panel.svg",
            [devtools_x, devtools_y],
            [toggle_size, toggle_size],
            icon_style,
        );

        // --- Devtools overlay: only render when visible ---
        if !*devtools_visible_overlay.lock().unwrap() {
            return;
        }

        let devtools_rect = layout.get_zone(ZoneId::DevTools);

        // Background panel: solid rounded-rect (zero radii => plain rect).
        let rrect = RoundedRect {
            rect: devtools_rect,
            radii: RoundedRadii { tl: 0.0, tr: 0.0, br: 0.0, bl: 0.0 },
        };
        passes.draw_filled_rounded_rect(
            encoder,
            view,
            width,
            height,
            rrect,
            devtools_style.bg_color,
            queue,
        );

        // Optional border: draw a thin inset rectangle with border color.
        if devtools_style.border_width > 0.0 {
            let bw = devtools_style.border_width;
            let inset_rect = Rect {
                x: devtools_rect.x + bw * 0.5,
                y: devtools_rect.y + bw * 0.5,
                w: (devtools_rect.w - bw).max(0.0),
                h: (devtools_rect.h - bw).max(0.0),
            };
            let border_rrect = RoundedRect {
                rect: inset_rect,
                radii: RoundedRadii { tl: 0.0, tr: 0.0, br: 0.0, bl: 0.0 },
            };
            passes.draw_filled_rounded_rect(
                encoder,
                view,
                width,
                height,
                border_rrect,
                devtools_style.border_color,
                queue,
            );
        }

        // Header + tabs
        let button_size = 18.0;
        let tab_height = 24.0;
        let tab_padding = 10.0;
        let icon_text_gap = 6.0;
        let white = ColorLinPremul::rgba(255, 255, 255, 255);
        let inactive_color = ColorLinPremul::rgba(160, 170, 180, 255);
        let header_bg = ColorLinPremul::rgba(34, 41, 60, 255);
        let active_tab_bg = ColorLinPremul::rgba(54, 61, 80, 255);
        let inactive_tab_bg = ColorLinPremul::rgba(40, 47, 66, 255);
        let header_height = tab_height + 8.0;

        // Header strip behind tabs
        let header_rect = Rect {
            x: devtools_rect.x,
            y: devtools_rect.y,
            w: devtools_rect.w,
            h: header_height,
        };
        let header_rr = RoundedRect {
            rect: header_rect,
            radii: RoundedRadii { tl: 0.0, tr: 0.0, br: 0.0, bl: 0.0 },
        };
        passes.draw_filled_rounded_rect(
            encoder,
            view,
            width,
            height,
            header_rr,
            header_bg,
            queue,
        );

        // Active tab
        let active_tab = *devtools_active_tab_overlay.lock().unwrap();

        // Elements tab geometry
        let elements_x = devtools_rect.x + tab_padding;
        let elements_y = devtools_rect.y + (tab_height - button_size) * 0.5;
        let elements_tab_width = button_size + icon_text_gap + 8.0 + 54.0 + tab_padding * 3.0;
        let elements_rect = Rect {
            x: elements_x,
            y: elements_y,
            w: elements_tab_width,
            h: tab_height,
        };
        let is_elements_active = active_tab == DevToolsTab::Elements;
        let elements_bg = if is_elements_active { active_tab_bg } else { inactive_tab_bg };
        let elements_color = if is_elements_active { white } else { inactive_color };

        let elements_rr = RoundedRect {
            rect: elements_rect,
            radii: RoundedRadii { tl: 0.0, tr: 0.0, br: 0.0, bl: 0.0 },
        };
        passes.draw_filled_rounded_rect(
            encoder,
            view,
            width,
            height,
            elements_rr,
            elements_bg,
            queue,
        );

        // Console tab geometry
        let console_x = elements_x + elements_tab_width + 8.0;
        let console_y = devtools_rect.y + (tab_height - button_size) * 0.5;
        let console_tab_width = button_size + icon_text_gap + 8.0 + 50.0 + tab_padding * 3.0;
        let console_rect = Rect {
            x: console_x,
            y: console_y,
            w: console_tab_width,
            h: tab_height,
        };
        let is_console_active = active_tab == DevToolsTab::Console;
        let console_bg = if is_console_active { active_tab_bg } else { inactive_tab_bg };
        let console_color = if is_console_active { white } else { inactive_color };

        let console_rr = RoundedRect {
            rect: console_rect,
            radii: RoundedRadii { tl: 0.0, tr: 0.0, br: 0.0, bl: 0.0 },
        };
        passes.draw_filled_rounded_rect(
            encoder,
            view,
            width,
            height,
            console_rr,
            console_bg,
            queue,
        );

        // Labels and content text via text renderer
        let mut overlay_list = engine_core::DisplayList {
            viewport: engine_core::Viewport { width, height },
            commands: Vec::new(),
        };

        // Elements label
        let elements_label_run = TextRun {
            text: "Elements".to_string(),
            pos: [
                elements_x + button_size + icon_text_gap + 8.0,
                devtools_rect.y + tab_height - 6.0,
            ],
            size: 11.0,
            color: elements_color,
        };
        overlay_list.commands.push(engine_core::Command::DrawText {
            run: elements_label_run,
            z: 10100,
            transform: Transform2D::identity(),
            id: 1,
            dynamic: false,
        });

        // Console label
        let console_label_run = TextRun {
            text: "Console".to_string(),
            pos: [
                console_x + button_size + icon_text_gap + 8.0,
                devtools_rect.y + tab_height - 6.0,
            ],
            size: 11.0,
            color: console_color,
        };
        overlay_list.commands.push(engine_core::Command::DrawText {
            run: console_label_run,
            z: 10100,
            transform: Transform2D::identity(),
            id: 2,
            dynamic: false,
        });

        // Content label inside devtools body based on active tab
        let content_text = match active_tab {
            DevToolsTab::Console => "Console",
            DevToolsTab::Elements => "Elements",
        };
        let label_color: ColorLinPremul = ColorLinPremul::rgba(220, 230, 240, 255);
        let content_label_run = TextRun {
            text: content_text.to_string(),
            pos: [devtools_rect.x + tab_padding + 4.0, devtools_rect.y + header_height + 14.0],
            size: 12.0,
            color: label_color,
        };
        overlay_list.commands.push(engine_core::Command::DrawText {
            run: content_label_run,
            z: 10150,
            transform: Transform2D::identity(),
            id: 3,
            dynamic: false,
        });

        passes.render_text_for_list(encoder, view, &overlay_list, queue, overlay_provider.as_ref());

        // Icons and close button SVGs drawn on top
        let icon_style_elements = engine_core::SvgStyle::new()
            .with_stroke(elements_color)
            .with_stroke_width(2.0);
        let icon_style_console = engine_core::SvgStyle::new()
            .with_stroke(console_color)
            .with_stroke_width(2.0);
        let close_white = white;

        // Helper to draw a styled SVG at the given origin and max size (rasterized).
        fn draw_svg_icon(
            passes: &mut engine_core::PassManager,
            encoder: &mut wgpu::CommandEncoder,
            view: &wgpu::TextureView,
            queue: &wgpu::Queue,
            width: u32,
            height: u32,
            path_str: &str,
            origin: [f32; 2],
            max_size: [f32; 2],
            style: engine_core::SvgStyle,
        ) {
            let path = std::path::Path::new(path_str);
            if let Some((_view1x, w1, h1)) = passes.rasterize_svg_to_view(path, 1.0, Some(style), queue) {
                let base_w = w1.max(1) as f32;
                let base_h = h1.max(1) as f32;
                let scale = (max_size[0] / base_w).min(max_size[1] / base_h).max(0.0);
                if let Some((view_scaled, sw, sh)) = passes.rasterize_svg_to_view(path, scale, Some(style), queue) {
                    passes.draw_image_quad(
                        encoder,
                        view,
                        origin,
                        [sw as f32, sh as f32],
                        &view_scaled,
                        queue,
                        width,
                        height,
                    );
                }
            }
        }

        let elements_icon_origin = [elements_x, elements_y];
        let console_icon_origin = [console_x, console_y];
        let close_size = 20.0;
        let close_margin = 12.0;
        let close_origin = [
            devtools_rect.x + devtools_rect.w - close_size - close_margin,
            devtools_rect.y + 6.0,
        ];

        draw_svg_icon(
            passes,
            encoder,
            view,
            queue,
            width,
            height,
            "images/square-mouse-pointer.svg",
            elements_icon_origin,
            [button_size, button_size],
            icon_style_elements,
        );

        draw_svg_icon(
            passes,
            encoder,
            view,
            queue,
            width,
            height,
            "images/square-terminal.svg",
            console_icon_origin,
            [button_size, button_size],
            icon_style_console,
        );

        let close_icon_style = engine_core::SvgStyle::new()
            .with_stroke(close_white)
            .with_stroke_width(2.0);
        draw_svg_icon(
            passes,
            encoder,
            view,
            queue,
            width,
            height,
            "images/x.svg",
            close_origin,
            [close_size, close_size],
            close_icon_style,
        );
    }));

    // Track cursor position and hit index for interaction
    let mut cursor_position: Option<(f32, f32)> = None;
    let mut hit_index: Option<engine_core::HitIndex> = None;
    
    // Track time for cursor blink animation
    let mut last_frame_time = std::time::Instant::now();
    
    // Track modifier keys state
    let mut modifiers_state = winit::keyboard::ModifiersState::empty();
    
    // Phase 5: Track click timing for double-click and triple-click detection
    let mut last_click_time: Option<std::time::Instant> = None;
    let mut click_count: u32 = 0;
    let double_click_threshold = std::time::Duration::from_millis(500);
    
    Ok(event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { window_id, event } if window_id == window.id() => {
                match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::ModifiersChanged(new_state) => {
                        modifiers_state = new_state.state();
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        cursor_position = Some((position.x as f32, position.y as f32));
                        
                        // Phase 5: Handle mouse drag for selection extension
                        let logical_x = position.x as f32 / scale_factor;
                        let logical_y = position.y as f32 / scale_factor;
                        let viewport_rect = zone_manager.layout.get_zone(ZoneId::Viewport);
                        let viewport_local_x = logical_x - viewport_rect.x;
                        let viewport_local_y = logical_y - viewport_rect.y + zone_manager.viewport.scroll_offset;
                        
                        // Check if any input box is in mouse selection mode
                        for input_box in viewport_ir.input_boxes.iter_mut() {
                            if input_box.focused {
                                // Extend selection based on click count
                                if click_count == 3 {
                                    input_box.extend_line_selection(viewport_local_x, viewport_local_y);
                                    input_box.update_scroll();
                                    needs_redraw = true;
                                    window.request_redraw();
                                } else if click_count == 2 {
                                    input_box.extend_word_selection(viewport_local_x, viewport_local_y);
                                    input_box.update_scroll();
                                    needs_redraw = true;
                                    window.request_redraw();
                                } else {
                                    input_box.extend_mouse_selection(viewport_local_x, viewport_local_y);
                                    input_box.update_scroll();
                                    needs_redraw = true;
                                    window.request_redraw();
                                }
                                break;
                            }
                        }
                    }
                    WindowEvent::MouseWheel { delta, .. } => {
                        // Handle scrolling in viewport
                        use winit::event::MouseScrollDelta;
                        let scroll_delta = match delta {
                            MouseScrollDelta::LineDelta(_x, y) => y * 20.0, // 20 pixels per line
                            MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                        };
                        
                        let viewport_rect = zone_manager.layout.get_zone(ZoneId::Viewport);
                        zone_manager.viewport.scroll(-scroll_delta, viewport_rect.h);
                        needs_redraw = true;
                        window.request_redraw();
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        if button == winit::event::MouseButton::Left && state == winit::event::ElementState::Released {
                            // Phase 5: End mouse selection on button release
                            for input_box in viewport_ir.input_boxes.iter_mut() {
                                if input_box.focused {
                                    input_box.end_mouse_selection();
                                    break;
                                }
                            }
                        } else if button == winit::event::MouseButton::Left && state == winit::event::ElementState::Pressed {
                            if let Some((cursor_x, cursor_y)) = cursor_position {
                                let logical_x = cursor_x / scale_factor;
                                let logical_y = cursor_y / scale_factor;
                                
                                // Phase 5: Track click timing for double/triple-click detection
                                let now = std::time::Instant::now();
                                let is_quick_click = if let Some(last_time) = last_click_time {
                                    now.duration_since(last_time) < double_click_threshold
                                } else {
                                    false
                                };
                                
                                if is_quick_click {
                                    click_count += 1;
                                } else {
                                    click_count = 1;
                                }
                                last_click_time = Some(now);
                                
                                // Check if click is on an input box (adjust for viewport transform and scroll)
                                let viewport_rect = zone_manager.layout.get_zone(ZoneId::Viewport);
                                let viewport_local_x = logical_x - viewport_rect.x;
                                let viewport_local_y = logical_y - viewport_rect.y + zone_manager.viewport.scroll_offset;
                                
                                let mut clicked_input = false;
                                for (idx, input_box) in viewport_ir.input_boxes.iter_mut().enumerate() {
                                    let in_bounds = viewport_local_x >= input_box.rect.x 
                                        && viewport_local_x <= input_box.rect.x + input_box.rect.w
                                        && viewport_local_y >= input_box.rect.y
                                        && viewport_local_y <= input_box.rect.y + input_box.rect.h;
                                    
                                    if in_bounds {
                                        // Unfocus all, focus this one
                                        for other in viewport_ir.input_boxes.iter_mut() {
                                            other.focused = false;
                                        }
                                        let input = &mut viewport_ir.input_boxes[idx];
                                        input.focused = true;
                                        
                                        // Phase 5: Handle double-click and triple-click
                                        if click_count == 3 {
                                            // Triple-click: select entire line
                                            input.start_line_selection(viewport_local_x, viewport_local_y);
                                        } else if click_count == 2 {
                                            // Double-click: select word
                                            input.start_word_selection(viewport_local_x, viewport_local_y);
                                        } else {
                                            // Single click: start mouse selection (place cursor)
                                            input.start_mouse_selection(viewport_local_x, viewport_local_y);
                                        }
                                        input.update_scroll();
                                        
                                        clicked_input = true;
                                        needs_redraw = true;
                                        window.request_redraw();
                                        break;
                                    }
                                }
                                
                                // If clicked outside all input boxes, unfocus all
                                if !clicked_input {
                                    for input_box in viewport_ir.input_boxes.iter_mut() {
                                        if input_box.focused {
                                            input_box.focused = false;
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                }
                                
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
                                            } else if region_id == DEVTOOLS_BUTTON_REGION_ID {
                                                zone_manager.toggle_devtools();
                                                *devtools_visible.lock().unwrap() = zone_manager.is_devtools_visible();
                                                needs_redraw = true;
                                                window.request_redraw();
                                            } else if region_id == DEVTOOLS_CLOSE_BUTTON_REGION_ID {
                                                zone_manager.toggle_devtools();
                                                *devtools_visible.lock().unwrap() = zone_manager.is_devtools_visible();
                                                needs_redraw = true;
                                                window.request_redraw();
                                            } else if region_id == DEVTOOLS_ELEMENTS_TAB_REGION_ID {
                                                zone_manager.devtools.set_active_tab(DevToolsTab::Elements);
                                                *devtools_active_tab.lock().unwrap() = DevToolsTab::Elements;
                                                needs_redraw = true;
                                                window.request_redraw();
                                            } else if region_id == DEVTOOLS_CONSOLE_TAB_REGION_ID {
                                                zone_manager.devtools.set_active_tab(DevToolsTab::Console);
                                                *devtools_active_tab.lock().unwrap() = DevToolsTab::Console;
                                                needs_redraw = true;
                                                window.request_redraw();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
                        
                        // Baseline single-line InputBox editing path for viewport_ir:
                        // keyboard events are translated into InputBox editing methods
                        // (insert_char, delete_before_cursor, cursor movement, etc.).
                        // Phase 0 keeps this wiring as the source of truth while
                        // allowing InputBox to internally toggle its TextLayout backend.
                        // Find the focused input box
                        if let Some(focused_input) = viewport_ir.input_boxes.iter_mut().find(|ib| ib.focused) {
                            if event.state == winit::event::ElementState::Pressed {
                                let has_cmd = modifiers_state.contains(ModifiersState::SUPER);
                                let has_ctrl = modifiers_state.contains(ModifiersState::CONTROL);
                                let has_alt = modifiers_state.contains(ModifiersState::ALT);
                                let has_shift = modifiers_state.contains(ModifiersState::SHIFT);
                                
                                // On macOS: Cmd for line start/end, Option for word movement
                                // On Windows/Linux: Ctrl for line start/end, Ctrl for word movement (same as Cmd on macOS)
                                let line_modifier = has_cmd || has_ctrl;
                                let word_modifier = has_alt;

                                // Phase 5: Handle clipboard and undo/redo shortcuts
                                // Cmd/Ctrl+C: Copy, Cmd/Ctrl+X: Cut, Cmd/Ctrl+V: Paste
                                // Cmd/Ctrl+Z: Undo, Cmd/Ctrl+Shift+Z or Ctrl+Y: Redo
                                // Cmd/Ctrl+A: Select All
                                match event.physical_key {
                                    PhysicalKey::Code(KeyCode::KeyA) if line_modifier && !has_shift => {
                                        // Cmd/Ctrl+A: Select all text
                                        focused_input.select_all();
                                        focused_input.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::KeyC) if line_modifier && !has_shift => {
                                        // Cmd/Ctrl+C: Copy to clipboard
                                        if let Err(e) = focused_input.copy_to_clipboard() {
                                            eprintln!("Failed to copy: {}", e);
                                        }
                                        // No redraw needed for copy
                                    }
                                    PhysicalKey::Code(KeyCode::KeyX) if line_modifier && !has_shift => {
                                        // Cmd/Ctrl+X: Cut to clipboard
                                        if let Err(e) = focused_input.cut_to_clipboard() {
                                            eprintln!("Failed to cut: {}", e);
                                        } else {
                                            focused_input.update_scroll();
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                    PhysicalKey::Code(KeyCode::KeyV) if line_modifier && !has_shift => {
                                        // Cmd/Ctrl+V: Paste from clipboard
                                        if let Err(e) = focused_input.paste_from_clipboard() {
                                            eprintln!("Failed to paste: {}", e);
                                        } else {
                                            focused_input.update_scroll();
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                    PhysicalKey::Code(KeyCode::KeyZ) if line_modifier && has_shift => {
                                        // Cmd/Ctrl+Shift+Z: Redo
                                        if focused_input.redo() {
                                            focused_input.update_scroll();
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                    PhysicalKey::Code(KeyCode::KeyZ) if line_modifier && !has_shift => {
                                        // Cmd/Ctrl+Z: Undo
                                        if focused_input.undo() {
                                            focused_input.update_scroll();
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                    PhysicalKey::Code(KeyCode::KeyY) if has_ctrl && !has_shift => {
                                        // Ctrl+Y: Redo (Windows/Linux alternative)
                                        if focused_input.redo() {
                                            focused_input.update_scroll();
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                    PhysicalKey::Code(KeyCode::Backspace) => {
                                        focused_input.delete_before_cursor();
                                        focused_input.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::Delete) => {
                                        focused_input.delete_after_cursor();
                                        focused_input.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::ArrowLeft) => {
                                        if line_modifier && has_shift {
                                            // Cmd+Shift+Left (macOS) or Ctrl+Shift+Left (Windows): select to line start
                                            focused_input.extend_selection_to_line_start();
                                        } else if line_modifier {
                                            // Cmd+Left (macOS) or Ctrl+Left (Windows): move to line start
                                            focused_input.move_cursor_line_start();
                                        } else if word_modifier && has_shift {
                                            // Option+Shift+Left (macOS) or Alt+Shift+Left (Windows): extend selection left by word
                                            focused_input.extend_selection_left_word();
                                        } else if word_modifier {
                                            // Option+Left (macOS) or Alt+Left (Windows): move left by word
                                            focused_input.move_cursor_left_word();
                                        } else if has_shift {
                                            // Shift+Left: extend selection left by character
                                            focused_input.extend_selection_left();
                                        } else {
                                            // Left: move left by character
                                            focused_input.move_cursor_left();
                                        }
                                        focused_input.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::ArrowRight) => {
                                        if line_modifier && has_shift {
                                            // Cmd+Shift+Right (macOS) or Ctrl+Shift+Right (Windows): select to line end
                                            focused_input.extend_selection_to_line_end();
                                        } else if line_modifier {
                                            // Cmd+Right (macOS) or Ctrl+Right (Windows): move to line end
                                            focused_input.move_cursor_line_end();
                                        } else if word_modifier && has_shift {
                                            // Option+Shift+Right (macOS) or Alt+Shift+Right (Windows): extend selection right by word
                                            focused_input.extend_selection_right_word();
                                        } else if word_modifier {
                                            // Option+Right (macOS) or Alt+Right (Windows): move right by word
                                            focused_input.move_cursor_right_word();
                                        } else if has_shift {
                                            // Shift+Right: extend selection right by character
                                            focused_input.extend_selection_right();
                                        } else {
                                            // Right: move right by character
                                            focused_input.move_cursor_right();
                                        }
                                        focused_input.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::Home) => {
                                        if has_shift {
                                            // Shift+Home: extend selection to start
                                            focused_input.extend_selection_to_start();
                                        } else {
                                            // Home: move cursor to start
                                            focused_input.move_cursor_to_start();
                                        }
                                        focused_input.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::End) => {
                                        if has_shift {
                                            // Shift+End: extend selection to end
                                            focused_input.extend_selection_to_end();
                                        } else {
                                            // End: move cursor to end
                                            focused_input.move_cursor_to_end();
                                        }
                                        focused_input.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::Space) => {
                                        focused_input.insert_char(' ');
                                        focused_input.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    _ => {
                                        // Handle text input via the text field
                                        if let Some(text) = &event.text {
                                            for ch in text.chars() {
                                                // Skip control characters but allow space
                                                if !ch.is_control() || ch == ' ' {
                                                    focused_input.insert_char(ch);
                                                }
                                            }
                                            focused_input.update_scroll();
                                            needs_redraw = true;
                                            window.request_redraw();
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
                        let now = std::time::Instant::now();
                        last_resize_time = Some(now);
                        // Track first resize to enforce maximum debounce
                        if first_resize_time.is_none() {
                            first_resize_time = Some(now);
                        }
                        needs_redraw = true;
                        // Trigger immediate background redraw to prevent white edges
                        needs_background_redraw = true;
                        window.request_redraw();
                    }
                    WindowEvent::ScaleFactorChanged { scale_factor: sf, .. } => {
                        let new_scale = sf as f32;
                        surf.set_dpi_scale(new_scale);
                        needs_redraw = true;
                        window.request_redraw();
                    }
                    WindowEvent::RedrawRequested => {
                        // Check debounce timing HERE (not in AboutToWait which may not fire frequently)
                        if let Some(last_time) = last_resize_time {
                            let settled = last_time.elapsed() >= std::time::Duration::from_millis(200);
                            let max_exceeded = first_resize_time
                                .map(|t| t.elapsed() >= std::time::Duration::from_millis(300))
                                .unwrap_or(false);
                            
                            if settled || max_exceeded {
                                // Resize ended or max debounce exceeded - clear resize state
                                last_resize_time = None;
                                first_resize_time = None;
                            }
                        }
                        
                        // Check if we should render: either full redraw or just backgrounds
                        let should_render_backgrounds = needs_background_redraw;
                        // Detect if size actually changed (not just a resize event)
                        let size_changed = prev_size.width != size.width || prev_size.height != size.height;
                        // Force full redraw if: no resize in progress OR max debounce exceeded (300ms)
                        // Note: We no longer force full redraw on size change because intermediate texture
                        // is preserved during expansion (only reallocated when growing or shrinking >25%)
                        let max_debounce_exceeded = first_resize_time
                            .map(|t| t.elapsed() >= std::time::Duration::from_millis(300))
                            .unwrap_or(false);
                        let should_render_full = needs_redraw && (last_resize_time.is_none() || max_debounce_exceeded);
                        
                        if (!should_render_backgrounds && !should_render_full) || size.width == 0 || size.height == 0 { 
                            return; 
                        }
                        
                        // Preserve intermediate texture content during resize to avoid white edges
                        // The intermediate texture is kept across frames (only reallocated when needed)
                        // Backgrounds will render incrementally over preserved content
                        surf.set_preserve_surface(true);
                        
                        if size_changed {
                            prev_size = size;
                        }
                        
                        let frame = match surface.get_current_texture() {
                            Ok(f) => f,
                            Err(_) => { window.request_redraw(); return; }
                        };
                        // Canvas uses physical pixels, but zones are in logical pixels
                        // So we pass physical size to canvas
                        let mut canvas = surf.begin_frame(size.width, size.height);
                        canvas.set_text_provider(provider.clone());

                        // Always render zone backgrounds and borders first (lowest z-index)
                        // This happens immediately during resize without debounce
                        // Note: render_zones uses zone_manager which has logical coordinates,
                        // but the canvas will scale them to physical via logical_pixels mode
                        render_zones(&mut canvas, &zone_manager);
                        
                        // Only render foreground content if not in debounce period
                        // This prevents expensive text layout during rapid resize
                        if should_render_full {
                            // Update cursor blink animation
                            let now = std::time::Instant::now();
                            let delta_time = (now - last_frame_time).as_secs_f32();
                            last_frame_time = now;
                            
                            for input_box in viewport_ir.input_boxes.iter_mut() {
                                input_box.update_blink(delta_time);
                            }
                            
                            // Request continuous redraw for cursor blinking while any input is focused.
                            if viewport_ir.input_boxes.iter().any(|ib| ib.focused) {
                                needs_redraw = true;
                                window.request_redraw();
                            }
                            
                            // Render sample UI elements in viewport zone with local coordinates.
                            let viewport_rect = zone_manager.layout.get_zone(ZoneId::Viewport);
                            
                            // Apply transform first to position viewport content
                            canvas.push_transform(Transform2D::translate(viewport_rect.x, viewport_rect.y));
                            
                            // Push clip rect in viewport-local coordinates (0,0 origin)
                            let local_clip = Rect {
                                x: 0.0,
                                y: 0.0,
                                w: viewport_rect.w,
                                h: viewport_rect.h,
                            };
                            canvas.push_clip_rect(local_clip);
                            
                            // Apply scroll offset as a nested transform (negative to scroll down)
                            canvas.push_transform(Transform2D::translate(0.0, -zone_manager.viewport.scroll_offset));
                            
                            let content_height = viewport_ir.render(
                                &mut canvas,
                                scale_factor,
                                viewport_rect.w as u32,
                                provider.as_ref(),
                                text_cache.as_ref(),
                            );
                            
                            canvas.pop_transform(); // Pop scroll transform
                            canvas.pop_clip();      // Pop clip rect
                            canvas.pop_transform(); // Pop viewport position transform
                            
                            // Update viewport content height
                            zone_manager.viewport.set_content_height(content_height, viewport_rect.h);
                            
                            // Render toolbar content with hit regions (above viewport)
                            let toolbar_rect = zone_manager.layout.get_zone(ZoneId::Toolbar);
                            canvas.push_transform(Transform2D::translate(toolbar_rect.x, toolbar_rect.y));
                            zone_manager.toolbar.render(&mut canvas, toolbar_rect);
                            canvas.pop_transform();
                        }
                        
                        // Register devtools hit regions (tabs and close button) if devtools is visible.
                        // Visual chrome is rendered in the RuneSurface overlay callback so it
                        // appears above all content, including raster images.
                        // Only register hit regions during full render (not background-only)
                        if should_render_full && zone_manager.is_devtools_visible() {
                            let devtools_rect = zone_manager.layout.get_zone(ZoneId::DevTools);
                            canvas.push_transform(Transform2D::translate(devtools_rect.x, devtools_rect.y));
                            
                            let button_size = 18.0;
                            let tab_height = 24.0;
                            let tab_padding = 10.0;
                            let icon_text_gap = 6.0;

                            // Elements tab
                            let elements_x = tab_padding;
                            let elements_y = (tab_height - button_size) * 0.5;
                            // Calculate tab width based on text
                            let elements_tab_width = button_size + icon_text_gap + 8.0 + 54.0 + tab_padding * 3.0;
                            
                            let elements_rect = Rect {
                                x: elements_x,
                                y: elements_y,
                                w: elements_tab_width,
                                h: tab_height,
                            };
                            
                            canvas.hit_region_rect(DEVTOOLS_ELEMENTS_TAB_REGION_ID, elements_rect, 10300);
                            
                            // Console tab
                            let console_x = elements_x + elements_tab_width + 8.0;
                            let console_y = (tab_height - button_size) * 0.5;

                            let console_tab_width = button_size + icon_text_gap + 8.0 + 50.0 + tab_padding * 3.0;
                            
                            let console_rect = Rect {
                                x: console_x,
                                y: console_y,
                                w: console_tab_width,
                                h: tab_height,
                            };
                            
                            canvas.hit_region_rect(DEVTOOLS_CONSOLE_TAB_REGION_ID, console_rect, 10300);
                            
                            // Close button in top-right corner
                            let close_size = 20.0;
                            let close_margin = 12.0;
                            let close_x = devtools_rect.w - close_size - close_margin;
                            let close_y = 6.0;

                            let close_rect = Rect {
                                x: close_x,
                                y: close_y,
                                w: close_size,
                                h: close_size,
                            };
                            
                            canvas.hit_region_rect(DEVTOOLS_CLOSE_BUTTON_REGION_ID, close_rect, 10300);
                            
                            canvas.pop_transform();
                        }
                        
                        // Build hit index from display list before ending frame
                        // Only rebuild hit index during full render
                        if should_render_full {
                            hit_index = Some(engine_core::HitIndex::build(canvas.display_list()));
                        }
                        
                        surf.end_frame(frame, canvas).ok();
                        
                        // Clear flags after rendering.
                        // Keep needs_redraw true while an input box is focused so
                        // cursor blinking continues to drive redraws.
                        if should_render_full {
                            let any_focused_input = viewport_ir.input_boxes.iter().any(|ib| ib.focused);
                            if !any_focused_input {
                                needs_redraw = false;
                            }
                        }
                        needs_background_redraw = false;
                    }
                    _ => {}
                }
            }
            Event::AboutToWait => {
                // Check debounce timer and request redraws as needed
                if let Some(last_time) = last_resize_time {
                    let settled = last_time.elapsed() >= std::time::Duration::from_millis(200);
                    let max_exceeded = first_resize_time
                        .map(|t| t.elapsed() >= std::time::Duration::from_millis(300))
                        .unwrap_or(false);
                    
                    if settled || max_exceeded {
                        // Resize ended or max debounce exceeded - trigger full redraw
                        last_resize_time = None;
                        needs_redraw = true;
                        window.request_redraw();
                    }
                    // Note: We don't request continuous redraws here to avoid overwhelming the system
                    // The RedrawRequested handler will check the timer on each redraw anyway
                } else if needs_redraw {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    })?)
}
