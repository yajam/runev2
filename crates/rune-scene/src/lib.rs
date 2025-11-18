use anyhow::Result;
use engine_core::{
    Brush, ColorLinPremul, Rect, SubpixelOrientation,
    Transform2D, make_surface_config,
};
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

pub mod elements;
pub mod text;
pub mod viewport_ir;
pub mod zones;

use zones::{
    DEVTOOLS_BUTTON_REGION_ID, DEVTOOLS_CLOSE_BUTTON_REGION_ID, DEVTOOLS_CONSOLE_TAB_REGION_ID,
    DEVTOOLS_ELEMENTS_TAB_REGION_ID, DevToolsTab, TOGGLE_BUTTON_REGION_ID, ZoneId, ZoneManager,
};

/// Render zone backgrounds and borders (viewport, toolbar, sidebar).
///
/// Z-index strategy:
/// - Viewport + sidebar chrome use a low z so UI widgets rendered later
///   (checkboxes, text, etc.) appear above their own backgrounds.
/// - Toolbar background/border use a very high z so scrolling viewport
///   content never appears on top of the toolbar; it visually passes
///   “under” the toolbar as you scroll.
fn render_zones(canvas: &mut rune_surface::Canvas, zone_manager: &ZoneManager) {
    for zone_id in [ZoneId::Viewport, ZoneId::Toolbar, ZoneId::Sidebar] {
        let z = match zone_id {
            ZoneId::Toolbar => 9000,
            _ => 0,
        };

        let rect = zone_manager.layout.get_zone(zone_id);
        let style = zone_manager.get_style(zone_id);

        // Background
        canvas.fill_rect(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            Brush::Solid(style.bg_color),
            z,
        );

        // Border (draw as four rectangles)
        let bw = style.border_width;
        let border_brush = Brush::Solid(style.border_color);

        // Top border
        canvas.fill_rect(rect.x, rect.y, rect.w, bw, border_brush.clone(), z);
        // Bottom border
        canvas.fill_rect(
            rect.x,
            rect.y + rect.h - bw,
            rect.w,
            bw,
            border_brush.clone(),
            z,
        );
        // Left border
        canvas.fill_rect(rect.x, rect.y, bw, rect.h, border_brush.clone(), z);
        // Right border
        canvas.fill_rect(rect.x + rect.w - bw, rect.y, bw, rect.h, border_brush, z);
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
        .with_maximized(true)
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
    // Unified rendering (Phase 3) is now the only rendering path
    // Keep direct=false so PassManager uses the offscreen unified path.
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
                if let Ok(p) =
                    engine_core::RuneTextProvider::from_bytes(&bytes, SubpixelOrientation::RGB)
                {
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
                .expect("Failed to load system fonts"),
        )
    }

    // Zone manager for layout and styling (use logical pixels)
    let logical_width = (size.width as f32 / scale_factor) as u32;
    let logical_height = (size.height as f32 / scale_factor) as u32;
    let mut zone_manager = ZoneManager::new(logical_width, logical_height);

    // Viewport IR content - incremental implementation
    let viewport_ir =
        std::sync::Arc::new(std::sync::Mutex::new(viewport_ir::ViewportContent::new()));
    let _viewport_ir_overlay = viewport_ir.clone();

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

    // Devtools and toolbar were previously rendered via a RuneSurface overlay.
    // With unified rendering enabled, we render all chrome through the Canvas
    // so it participates in the same z-ordered scene. Keep the overlay hook
    // installed but make it a no-op to avoid unused warnings and preserve the
    // ability to reintroduce GPU overlays later if needed.
    let devtools_style = zone_manager.devtools.style.clone();
    let toolbar_style = zone_manager.toolbar.style.clone();
    let overlay_provider = provider.clone();
    let overlay_scale = scale_factor;

    // Shared state for devtools visibility (kept for future overlay-based debugging)
    let devtools_visible = std::sync::Arc::new(std::sync::Mutex::new(false));
    let devtools_visible_overlay = devtools_visible.clone();

    // Shared state for devtools active tab
    let devtools_active_tab = std::sync::Arc::new(std::sync::Mutex::new(DevToolsTab::Elements));
    let devtools_active_tab_overlay = devtools_active_tab.clone();

    // Shared state for viewport scroll offset
    let viewport_scroll_offset = std::sync::Arc::new(std::sync::Mutex::new(0.0f32));
    let viewport_scroll_offset_overlay = viewport_scroll_offset.clone();

    surf.set_overlay(Box::new(
        move |_passes, _encoder, _view, _queue, width, height| {
            let sidebar_vis = *sidebar_visible_overlay.lock().unwrap();
            let logical_width = (width as f32 / overlay_scale).max(0.0) as u32;
            let logical_height = (height as f32 / overlay_scale).max(0.0) as u32;
            let _layout = zones::ZoneLayout::calculate(logical_width, logical_height, sidebar_vis);

            // Mark captured vars as used so we can easily re-enable overlay
            // rendering later without changing the closure signature.
            let _ = (
                &devtools_style,
                &toolbar_style,
                &overlay_provider,
                &devtools_visible_overlay,
                &devtools_active_tab_overlay,
                &viewport_scroll_offset_overlay,
            );

            // All toolbar/devtools chrome is now rendered via Canvas in the
            // unified path; this overlay intentionally does nothing.
        },
    ));

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
                        let viewport_local_y =
                            logical_y - viewport_rect.y + zone_manager.viewport.scroll_offset;

                        let mut viewport_ir_lock = viewport_ir.lock().unwrap();

                        // Check if any input box is in mouse selection mode
                        for input_box in viewport_ir_lock.input_boxes.iter_mut() {
                            if input_box.focused {
                                // Extend selection based on click count
                                if click_count == 3 {
                                    input_box
                                        .extend_line_selection(viewport_local_x, viewport_local_y);
                                    input_box.update_scroll();
                                    needs_redraw = true;
                                    window.request_redraw();
                                } else if click_count == 2 {
                                    input_box
                                        .extend_word_selection(viewport_local_x, viewport_local_y);
                                    input_box.update_scroll();
                                    needs_redraw = true;
                                    window.request_redraw();
                                } else {
                                    input_box
                                        .extend_mouse_selection(viewport_local_x, viewport_local_y);
                                    input_box.update_scroll();
                                    needs_redraw = true;
                                    window.request_redraw();
                                }
                                break;
                            }
                        }

                        // Check if any text area is in mouse selection mode
                        for textarea in viewport_ir_lock.text_areas.iter_mut() {
                            if textarea.focused {
                                // Extend selection based on click count
                                if click_count == 3 {
                                    textarea
                                        .extend_line_selection(viewport_local_x, viewport_local_y);
                                    textarea.update_scroll();
                                    needs_redraw = true;
                                    window.request_redraw();
                                } else if click_count == 2 {
                                    textarea
                                        .extend_word_selection(viewport_local_x, viewport_local_y);
                                    textarea.update_scroll();
                                    needs_redraw = true;
                                    window.request_redraw();
                                } else {
                                    textarea
                                        .extend_mouse_selection(viewport_local_x, viewport_local_y);
                                    textarea.update_scroll();
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
                        *viewport_scroll_offset.lock().unwrap() =
                            zone_manager.viewport.scroll_offset;
                        needs_redraw = true;
                        window.request_redraw();
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        // Enable hit testing for toolbar and devtools buttons
                        // Track if toolbar button was clicked to prevent double-handling
                        let mut clicked_toolbar_button = false;

                        if button == winit::event::MouseButton::Left
                            && state == winit::event::ElementState::Pressed
                        {
                            if let Some((cursor_x, cursor_y)) = cursor_position {
                                let logical_x = cursor_x / scale_factor;
                                let logical_y = cursor_y / scale_factor;

                                // Perform hit test for toolbar and devtools buttons
                                // eprintln!("🖱️ Click at logical ({}, {})", logical_x, logical_y);
                                if let Some(ref index) = hit_index {
                                    // eprintln!("   Hit index exists");
                                    if let Some(hit) = index.topmost_at([logical_x, logical_y]) {
                                        // eprintln!("   Hit found: region_id={:?}, z={}", hit.region_id, hit.z);
                                        if let Some(region_id) = hit.region_id {
                                            if region_id == TOGGLE_BUTTON_REGION_ID {
                                                let logical_width =
                                                    (size.width as f32 / scale_factor) as u32;
                                                let logical_height =
                                                    (size.height as f32 / scale_factor) as u32;
                                                zone_manager.toggle_sidebar(
                                                    logical_width,
                                                    logical_height,
                                                );
                                                *sidebar_visible.lock().unwrap() =
                                                    zone_manager.is_sidebar_visible();
                                                clicked_toolbar_button = true;
                                                needs_redraw = true;
                                                window.request_redraw();
                                            } else if region_id == DEVTOOLS_BUTTON_REGION_ID {
                                                zone_manager.toggle_devtools();
                                                let visible = zone_manager.is_devtools_visible();
                                                // eprintln!("🔧 DevTools toggled: visible = {}", visible);
                                                *devtools_visible.lock().unwrap() = visible;
                                                clicked_toolbar_button = true;
                                                needs_redraw = true;
                                                window.request_redraw();
                                            } else if region_id == DEVTOOLS_CLOSE_BUTTON_REGION_ID {
                                                zone_manager.toggle_devtools();
                                                *devtools_visible.lock().unwrap() =
                                                    zone_manager.is_devtools_visible();
                                                clicked_toolbar_button = true;
                                                needs_redraw = true;
                                                window.request_redraw();
                                            } else if region_id == DEVTOOLS_ELEMENTS_TAB_REGION_ID {
                                                zone_manager.devtools.set_active_tab(DevToolsTab::Elements);
                                                clicked_toolbar_button = true;
                                                needs_redraw = true;
                                                window.request_redraw();
                                            } else if region_id == DEVTOOLS_CONSOLE_TAB_REGION_ID {
                                                zone_manager.devtools.set_active_tab(DevToolsTab::Console);
                                                clicked_toolbar_button = true;
                                                needs_redraw = true;
                                                window.request_redraw();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        let mut viewport_ir_lock = viewport_ir.lock().unwrap();

                        if button == winit::event::MouseButton::Left
                            && state == winit::event::ElementState::Released
                        {
                            // End mouse selection on button release
                            for input_box in viewport_ir_lock.input_boxes.iter_mut() {
                                if input_box.focused {
                                    input_box.end_mouse_selection();
                                    break;
                                }
                            }
                            for textarea in viewport_ir_lock.text_areas.iter_mut() {
                                if textarea.focused {
                                    textarea.end_mouse_selection();
                                    break;
                                }
                            }
                        } else if button == winit::event::MouseButton::Left
                            && state == winit::event::ElementState::Pressed
                        {
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
                                let viewport_local_y = logical_y - viewport_rect.y
                                    + zone_manager.viewport.scroll_offset;

                                // FIRST PRIORITY: Check open date picker/select popups to block clicks to elements below
                                let mut clicked_popup = false;
                                for datepicker in viewport_ir_lock.date_pickers.iter_mut() {
                                    if datepicker.open {
                                        use elements::date_picker::PickerMode;

                                        let popup_width = 280.0;
                                        let popup_height = match datepicker.picker_mode {
                                            PickerMode::Days => 334.0,
                                            PickerMode::Months => 280.0,
                                            PickerMode::Years => 240.0,
                                        };
                                        let header_height = 40.0;
                                        let popup_x = datepicker.rect.x;
                                        let popup_y = datepicker.rect.y - popup_height - 4.0;

                                        let in_popup = viewport_local_x >= popup_x
                                            && viewport_local_x <= popup_x + popup_width
                                            && viewport_local_y >= popup_y
                                            && viewport_local_y <= popup_y + popup_height;

                                        if in_popup {
                                            clicked_popup = true;
                                            let header_click = viewport_local_y >= popup_y && viewport_local_y <= popup_y + header_height;

                                            if header_click {
                                                let arrow_size = 16.0;
                                                let prev_arrow_x = popup_x + 12.0;
                                                let next_arrow_x = popup_x + popup_width - arrow_size - 12.0;
                                                let arrow_y = popup_y + (header_height - arrow_size) * 0.5;

                                                if viewport_local_x >= prev_arrow_x && viewport_local_x <= prev_arrow_x + arrow_size
                                                    && viewport_local_y >= arrow_y && viewport_local_y <= arrow_y + arrow_size {
                                                    match datepicker.picker_mode {
                                                        PickerMode::Days => {
                                                            if datepicker.current_view_month == 1 {
                                                                datepicker.current_view_month = 12;
                                                                datepicker.current_view_year -= 1;
                                                            } else { datepicker.current_view_month -= 1; }
                                                        }
                                                        PickerMode::Months => { datepicker.current_view_year -= 1; }
                                                        PickerMode::Years => { datepicker.current_view_year -= 9; }
                                                    }
                                                    needs_redraw = true;
                                                    window.request_redraw();
                                                    break;
                                                }

                                                if viewport_local_x >= next_arrow_x && viewport_local_x <= next_arrow_x + arrow_size
                                                    && viewport_local_y >= arrow_y && viewport_local_y <= arrow_y + arrow_size {
                                                    match datepicker.picker_mode {
                                                        PickerMode::Days => {
                                                            if datepicker.current_view_month == 12 {
                                                                datepicker.current_view_month = 1;
                                                                datepicker.current_view_year += 1;
                                                            } else { datepicker.current_view_month += 1; }
                                                        }
                                                        PickerMode::Months => { datepicker.current_view_year += 1; }
                                                        PickerMode::Years => { datepicker.current_view_year += 9; }
                                                    }
                                                    needs_redraw = true;
                                                    window.request_redraw();
                                                    break;
                                                }

                                                let left_arrow_end = popup_x + 40.0;
                                                let right_arrow_start = popup_x + popup_width - 40.0;
                                                if viewport_local_x > left_arrow_end && viewport_local_x < right_arrow_start {
                                                    match datepicker.picker_mode {
                                                        PickerMode::Days => { datepicker.picker_mode = PickerMode::Years; needs_redraw = true; window.request_redraw(); break; }
                                                        PickerMode::Months => { datepicker.picker_mode = PickerMode::Years; needs_redraw = true; window.request_redraw(); break; }
                                                        PickerMode::Years => {}
                                                    }
                                                }
                                            }

                                            match datepicker.picker_mode {
                                                PickerMode::Days => {
                                                    let day_cell_size = 36.0;
                                                    let button_height = 36.0;
                                                    let button_margin = 8.0;
                                                    let grid_start_x = 10.0;
                                                    let grid_start_y = header_height + 35.0;
                                                    let cell_local_x = viewport_local_x - popup_x - grid_start_x;
                                                    let cell_local_y = viewport_local_y - popup_y - grid_start_y;

                                                    if cell_local_x >= 0.0 && cell_local_y >= 0.0 {
                                                        let col = (cell_local_x / day_cell_size) as usize;
                                                        let row = (cell_local_y / day_cell_size) as usize;
                                                        if col < 7 && row < 6 {
                                                            let days_in_month = crate::viewport_ir::days_in_month(datepicker.current_view_year, datepicker.current_view_month);
                                                            let first_day = crate::viewport_ir::first_day_of_month(datepicker.current_view_year, datepicker.current_view_month);
                                                            let cell_index = row * 7 + col;
                                                            if cell_index >= first_day as usize {
                                                                let day = (cell_index - first_day as usize + 1) as u32;
                                                                if day <= days_in_month {
                                                                    datepicker.selected_date = Some((datepicker.current_view_year, datepicker.current_view_month, day));
                                                                    datepicker.open = false;
                                                                    datepicker.picker_mode = PickerMode::Days;
                                                                    needs_redraw = true;
                                                                    window.request_redraw();
                                                                    break;
                                                                }
                                                            }
                                                        }
                                                    }

                                                    let buttons_y = popup_y + popup_height - button_height - button_margin;
                                                    let button_width = (popup_width - button_margin * 3.0) * 0.5;
                                                    let today_button_x = popup_x + button_margin;
                                                    if viewport_local_x >= today_button_x && viewport_local_x <= today_button_x + button_width
                                                        && viewport_local_y >= buttons_y && viewport_local_y <= buttons_y + button_height {
                                                        datepicker.selected_date = Some((2025, 11, 18));
                                                        datepicker.current_view_month = 11;
                                                        datepicker.current_view_year = 2025;
                                                        datepicker.open = false;
                                                        datepicker.picker_mode = PickerMode::Days;
                                                        needs_redraw = true;
                                                        window.request_redraw();
                                                        break;
                                                    }

                                                    let clear_button_x = popup_x + button_margin * 2.0 + button_width;
                                                    if viewport_local_x >= clear_button_x && viewport_local_x <= clear_button_x + button_width
                                                        && viewport_local_y >= buttons_y && viewport_local_y <= buttons_y + button_height {
                                                        datepicker.selected_date = None;
                                                        datepicker.open = false;
                                                        datepicker.picker_mode = PickerMode::Days;
                                                        needs_redraw = true;
                                                        window.request_redraw();
                                                        break;
                                                    }
                                                }
                                                PickerMode::Months => {
                                                    let month_cell_width = (popup_width - 32.0) / 3.0;
                                                    let month_cell_height = 45.0;
                                                    let grid_padding = 16.0;
                                                    let grid_start_x = grid_padding;
                                                    let grid_start_y = header_height + grid_padding;
                                                    let cell_local_x = viewport_local_x - popup_x - grid_start_x;
                                                    let cell_local_y = viewport_local_y - popup_y - grid_start_y;

                                                    if cell_local_x >= 0.0 && cell_local_y >= 0.0 {
                                                        let col = (cell_local_x / month_cell_width) as usize;
                                                        let row = (cell_local_y / month_cell_height) as usize;
                                                        if col < 3 && row < 4 {
                                                            let month = (row * 3 + col + 1) as u32;
                                                            datepicker.current_view_month = month;
                                                            datepicker.picker_mode = PickerMode::Days;
                                                            needs_redraw = true;
                                                            window.request_redraw();
                                                            break;
                                                        }
                                                    }
                                                }
                                                PickerMode::Years => {
                                                    let year_cell_size = 70.0;
                                                    let grid_padding = 16.0;
                                                    let grid_start_x = grid_padding;
                                                    let grid_start_y = header_height + grid_padding;
                                                    let cell_local_x = viewport_local_x - popup_x - grid_start_x;
                                                    let cell_local_y = viewport_local_y - popup_y - grid_start_y;

                                                    if cell_local_x >= 0.0 && cell_local_y >= 0.0 {
                                                        let col = (cell_local_x / year_cell_size) as usize;
                                                        let row = (cell_local_y / year_cell_size) as usize;
                                                        if col < 3 && row < 3 {
                                                            let idx = row * 3 + col;
                                                            let start_year = datepicker.current_view_year - 4;
                                                            let year = start_year + idx as u32;
                                                            datepicker.current_view_year = year;
                                                            datepicker.picker_mode = PickerMode::Months;
                                                            needs_redraw = true;
                                                            window.request_redraw();
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                            break;
                                        }
                                    }
                                }

                                // Check if click is on an open select dropdown overlay
                                if !clicked_popup {
                                    for select in viewport_ir_lock.selects.iter_mut() {
                                        if select.open && !select.options.is_empty() {
                                            let option_height = 36.0;
                                            let overlay_padding = 4.0;
                                            let overlay_height = (select.options.len() as f32 * option_height) + (overlay_padding * 2.0);

                                            // Position overlay below the select box
                                            let overlay_x = select.rect.x;
                                            let overlay_y = select.rect.y + select.rect.h + 4.0;
                                            let overlay_width = select.rect.w;

                                            let in_overlay = viewport_local_x >= overlay_x
                                                && viewport_local_x <= overlay_x + overlay_width
                                                && viewport_local_y >= overlay_y
                                                && viewport_local_y <= overlay_y + overlay_height;

                                            if in_overlay {
                                                clicked_popup = true;

                                                // Calculate which option was clicked
                                                let option_local_x = viewport_local_x - overlay_x - overlay_padding;
                                                let option_local_y = viewport_local_y - overlay_y - overlay_padding;

                                                if option_local_x >= 0.0 && option_local_y >= 0.0 {
                                                    let option_idx = (option_local_y / option_height) as usize;
                                                    if option_idx < select.options.len() {
                                                        // Update selected index
                                                        select.selected_index = Some(option_idx);

                                                        // Update label to show selected option
                                                        select.label = select.options[option_idx].clone();

                                                        // Close the dropdown
                                                        select.open = false;

                                                        needs_redraw = true;
                                                        window.request_redraw();
                                                    }
                                                }
                                                break;
                                            }
                                        }
                                    }
                                }

                                let mut clicked_input = false;
                                if !clicked_popup {
                                    for (idx, input_box) in
                                        viewport_ir_lock.input_boxes.iter_mut().enumerate()
                                    {
                                    let in_bounds = viewport_local_x >= input_box.rect.x
                                        && viewport_local_x <= input_box.rect.x + input_box.rect.w
                                        && viewport_local_y >= input_box.rect.y
                                        && viewport_local_y <= input_box.rect.y + input_box.rect.h;

                                    if in_bounds {
                                        // Unfocus all, focus this one
                                        for other in viewport_ir_lock.input_boxes.iter_mut() {
                                            other.focused = false;
                                        }
                                        let input = &mut viewport_ir_lock.input_boxes[idx];
                                        input.focused = true;

                                        // Phase 5: Handle double-click and triple-click
                                        if click_count == 3 {
                                            // Triple-click: select entire line
                                            input.start_line_selection(
                                                viewport_local_x,
                                                viewport_local_y,
                                            );
                                        } else if click_count == 2 {
                                            // Double-click: select word
                                            input.start_word_selection(
                                                viewport_local_x,
                                                viewport_local_y,
                                            );
                                        } else {
                                            // Single click: start mouse selection (place cursor)
                                            input.start_mouse_selection(
                                                viewport_local_x,
                                                viewport_local_y,
                                            );
                                        }
                                        input.update_scroll();

                                        clicked_input = true;
                                        needs_redraw = true;
                                        window.request_redraw();
                                        break;
                                    }
                                    }
                                }

                                // Check if click is on a text area
                                let mut clicked_textarea = false;
                                if !clicked_input && !clicked_popup {
                                    for (idx, textarea) in
                                        viewport_ir_lock.text_areas.iter_mut().enumerate()
                                    {
                                        let in_bounds = viewport_local_x >= textarea.rect.x
                                            && viewport_local_x
                                                <= textarea.rect.x + textarea.rect.w
                                            && viewport_local_y >= textarea.rect.y
                                            && viewport_local_y
                                                <= textarea.rect.y + textarea.rect.h;

                                        if in_bounds {
                                            // Unfocus all input boxes and text areas
                                            for input in viewport_ir_lock.input_boxes.iter_mut() {
                                                input.focused = false;
                                            }
                                            for other in viewport_ir_lock.text_areas.iter_mut() {
                                                other.focused = false;
                                            }

                                            let textarea = &mut viewport_ir_lock.text_areas[idx];
                                            textarea.focused = true;

                                            // Handle double-click and triple-click
                                            if click_count == 3 {
                                                textarea.start_line_selection(
                                                    viewport_local_x,
                                                    viewport_local_y,
                                                );
                                            } else if click_count == 2 {
                                                textarea.start_word_selection(
                                                    viewport_local_x,
                                                    viewport_local_y,
                                                );
                                            } else {
                                                textarea.start_mouse_selection(
                                                    viewport_local_x,
                                                    viewport_local_y,
                                                );
                                            }
                                            textarea.update_scroll();

                                            clicked_textarea = true;
                                            needs_redraw = true;
                                            window.request_redraw();
                                            break;
                                        }
                                    }
                                }

                                // Check if click is on a checkbox or its label (adjust for viewport transform and scroll)
                                let mut clicked_checkbox = false;
                                if !clicked_input && !clicked_textarea {
                                    for (idx, checkbox) in
                                        viewport_ir_lock.checkboxes.iter_mut().enumerate()
                                    {
                                        // Calculate clickable area including label
                                        // Label starts at rect.x + rect.w + 8.0 (8px gap)
                                        // Estimate label width: ~7px per character at 16px font size (conservative estimate)
                                        let label_width = if let Some(label) = checkbox.label {
                                            let char_width = checkbox.label_size * 0.5; // rough estimate
                                            label.len() as f32 * char_width + 8.0 // text width + gap
                                        } else {
                                            0.0
                                        };

                                        // Clickable area includes checkbox + label
                                        let clickable_width = checkbox.rect.w + label_width;
                                        let clickable_height =
                                            checkbox.rect.h.max(checkbox.label_size * 1.2); // ensure label height is covered

                                        let in_bounds = viewport_local_x >= checkbox.rect.x
                                            && viewport_local_x
                                                <= checkbox.rect.x + clickable_width
                                            && viewport_local_y >= checkbox.rect.y
                                            && viewport_local_y
                                                <= checkbox.rect.y + clickable_height;

                                        if in_bounds {
                                            // Toggle the checkbox
                                            checkbox.checked = !checkbox.checked;

                                            // Clear focus from all checkboxes, input boxes, and text areas
                                            for cb in viewport_ir_lock.checkboxes.iter_mut() {
                                                cb.focused = false;
                                            }
                                            for input in viewport_ir_lock.input_boxes.iter_mut() {
                                                input.focused = false;
                                            }
                                            for ta in viewport_ir_lock.text_areas.iter_mut() {
                                                ta.focused = false;
                                            }

                                            // Set focus on clicked checkbox
                                            viewport_ir_lock.checkboxes[idx].focused = true;

                                            clicked_checkbox = true;
                                            needs_redraw = true;
                                            window.request_redraw();
                                            break;
                                        }
                                    }
                                }

                                // Check if click is on a radio button or its label (adjust for viewport transform and scroll)
                                let mut clicked_radio = false;
                                if !clicked_input && !clicked_textarea && !clicked_checkbox {
                                    for (idx, radio) in
                                        viewport_ir_lock.radios.iter_mut().enumerate()
                                    {
                                        // Calculate clickable area including label
                                        // Radio uses center and radius, so convert to bounds
                                        let radio_left = radio.center[0] - radio.radius;
                                        let radio_top = radio.center[1] - radio.radius;
                                        let radio_size = radio.radius * 2.0;

                                        // Label starts at center[0] + radius + 8.0 (8px gap)
                                        let label_width = if let Some(label) = radio.label {
                                            let char_width = radio.label_size * 0.5; // rough estimate
                                            label.len() as f32 * char_width + 8.0 // text width + gap
                                        } else {
                                            0.0
                                        };

                                        // Clickable area includes radio circle + label
                                        let clickable_width = radio_size + label_width;
                                        let clickable_height =
                                            radio_size.max(radio.label_size * 1.2);

                                        let in_bounds = viewport_local_x >= radio_left
                                            && viewport_local_x <= radio_left + clickable_width
                                            && viewport_local_y >= radio_top
                                            && viewport_local_y <= radio_top + clickable_height;

                                        if in_bounds {
                                            // Select this radio button and deselect all others in the group
                                            for (i, r) in
                                                viewport_ir_lock.radios.iter_mut().enumerate()
                                            {
                                                r.selected = i == idx;
                                                r.focused = false;
                                            }

                                            // Clear focus from all other UI elements
                                            for cb in viewport_ir_lock.checkboxes.iter_mut() {
                                                cb.focused = false;
                                            }
                                            for input in viewport_ir_lock.input_boxes.iter_mut() {
                                                input.focused = false;
                                            }
                                            for ta in viewport_ir_lock.text_areas.iter_mut() {
                                                ta.focused = false;
                                            }

                                            // Set focus on clicked radio button
                                            viewport_ir_lock.radios[idx].focused = true;

                                            clicked_radio = true;
                                            needs_redraw = true;
                                            window.request_redraw();
                                            break;
                                        }
                                    }
                                }

                                // Old duplicate date picker check removed - now handled at the beginning

                                // Check if click is on a select dropdown (adjust for viewport transform and scroll)
                                let mut clicked_select = false;
                                if !clicked_input
                                    && !clicked_textarea
                                    && !clicked_checkbox
                                    && !clicked_radio
                                    && !clicked_popup
                                {
                                    for (idx, select) in
                                        viewport_ir_lock.selects.iter_mut().enumerate()
                                    {
                                        let in_bounds = viewport_local_x >= select.rect.x
                                            && viewport_local_x <= select.rect.x + select.rect.w
                                            && viewport_local_y >= select.rect.y
                                            && viewport_local_y <= select.rect.y + select.rect.h;

                                        if in_bounds {
                                            // Toggle the select dropdown
                                            select.open = !select.open;

                                            // Clear focus from all other UI elements
                                            for cb in viewport_ir_lock.checkboxes.iter_mut() {
                                                cb.focused = false;
                                            }
                                            for input in viewport_ir_lock.input_boxes.iter_mut() {
                                                input.focused = false;
                                            }
                                            for ta in viewport_ir_lock.text_areas.iter_mut() {
                                                ta.focused = false;
                                            }
                                            for r in viewport_ir_lock.radios.iter_mut() {
                                                r.focused = false;
                                            }
                                            for s in viewport_ir_lock.selects.iter_mut() {
                                                s.focused = false;
                                            }

                                            // Set focus on clicked select
                                            viewport_ir_lock.selects[idx].focused = true;

                                            clicked_select = true;
                                            needs_redraw = true;
                                            window.request_redraw();
                                            break;
                                        }
                                    }
                                }

                                // Check if click is on a date picker input field (not popup - that's handled earlier)
                                if !clicked_input
                                    && !clicked_textarea
                                    && !clicked_checkbox
                                    && !clicked_radio
                                    && !clicked_select
                                    && !clicked_popup
                                {
                                    for (idx, datepicker) in
                                        viewport_ir_lock.date_pickers.iter_mut().enumerate()
                                    {
                                        // Skip popup interaction check - already handled earlier to prevent click-through
                                        // Only check if click is on the date picker input field itself
                                        if false { // Disabled popup check
                                            let popup_width = 280.0;
                                            let popup_height = 334.0; // Reduced: only need 5 rows max
                                            let header_height = 40.0;
                                            let day_cell_size = 36.0;
                                            let button_height = 36.0;
                                            let button_margin = 8.0;
                                            let grid_start_x = 10.0;
                                            let grid_start_y = header_height + 35.0;

                                            let popup_x = datepicker.rect.x;
                                            let popup_y = datepicker.rect.y - popup_height - 4.0; // Position above

                                            let in_popup = viewport_local_x >= popup_x
                                                && viewport_local_x <= popup_x + popup_width
                                                && viewport_local_y >= popup_y
                                                && viewport_local_y <= popup_y + popup_height;

                                            if in_popup {
                                                clicked_popup = true;

                                                // Check for navigation arrows
                                                let arrow_size = 16.0;
                                                let prev_arrow_x = popup_x + 12.0;
                                                let next_arrow_x = popup_x + popup_width - arrow_size - 12.0;
                                                let arrow_y = popup_y + (header_height - arrow_size) * 0.5;

                                                // Previous month arrow
                                                if viewport_local_x >= prev_arrow_x
                                                    && viewport_local_x <= prev_arrow_x + arrow_size
                                                    && viewport_local_y >= arrow_y
                                                    && viewport_local_y <= arrow_y + arrow_size
                                                {
                                                    // Go to previous month
                                                    if datepicker.current_view_month == 1 {
                                                        datepicker.current_view_month = 12;
                                                        datepicker.current_view_year -= 1;
                                                    } else {
                                                        datepicker.current_view_month -= 1;
                                                    }
                                                    needs_redraw = true;
                                                    window.request_redraw();
                                                    break;
                                                }

                                                // Next month arrow
                                                if viewport_local_x >= next_arrow_x
                                                    && viewport_local_x <= next_arrow_x + arrow_size
                                                    && viewport_local_y >= arrow_y
                                                    && viewport_local_y <= arrow_y + arrow_size
                                                {
                                                    // Go to next month
                                                    if datepicker.current_view_month == 12 {
                                                        datepicker.current_view_month = 1;
                                                        datepicker.current_view_year += 1;
                                                    } else {
                                                        datepicker.current_view_month += 1;
                                                    }
                                                    needs_redraw = true;
                                                    window.request_redraw();
                                                    break;
                                                }

                                                // Check for day cell clicks
                                                let cell_local_x = viewport_local_x - popup_x - grid_start_x;
                                                let cell_local_y = viewport_local_y - popup_y - grid_start_y;

                                                if cell_local_x >= 0.0 && cell_local_y >= 0.0 {
                                                    let col = (cell_local_x / day_cell_size) as usize;
                                                    let row = (cell_local_y / day_cell_size) as usize;

                                                    if col < 7 && row < 6 {
                                                        // Calculate day number
                                                        let days_in_month = crate::viewport_ir::days_in_month(
                                                            datepicker.current_view_year,
                                                            datepicker.current_view_month,
                                                        );
                                                        let first_day = crate::viewport_ir::first_day_of_month(
                                                            datepicker.current_view_year,
                                                            datepicker.current_view_month,
                                                        );

                                                        let cell_index = row * 7 + col;
                                                        if cell_index >= first_day as usize {
                                                            let day = (cell_index - first_day as usize + 1) as u32;
                                                            if day <= days_in_month {
                                                                // Select this date
                                                                datepicker.selected_date = Some((
                                                                    datepicker.current_view_year,
                                                                    datepicker.current_view_month,
                                                                    day,
                                                                ));
                                                                datepicker.open = false; // Close popup after selection
                                                                needs_redraw = true;
                                                                window.request_redraw();
                                                                break;
                                                            }
                                                        }
                                                    }
                                                }

                                                // Check for Today and Clear buttons
                                                let buttons_y = popup_y + popup_height - button_height - button_margin;
                                                let button_width = (popup_width - button_margin * 3.0) * 0.5;

                                                // Today button (left)
                                                let today_button_x = popup_x + button_margin;
                                                if viewport_local_x >= today_button_x
                                                    && viewport_local_x <= today_button_x + button_width
                                                    && viewport_local_y >= buttons_y
                                                    && viewport_local_y <= buttons_y + button_height
                                                {
                                                    // Set to today's date (using a fixed date for demo)
                                                    // In production, you'd use chrono or time crate
                                                    datepicker.selected_date = Some((2025, 11, 18)); // Example: Nov 18, 2025
                                                    datepicker.current_view_month = 11;
                                                    datepicker.current_view_year = 2025;
                                                    datepicker.open = false;
                                                    needs_redraw = true;
                                                    window.request_redraw();
                                                    break;
                                                }

                                                // Clear button (right)
                                                let clear_button_x = popup_x + button_margin * 2.0 + button_width;
                                                if viewport_local_x >= clear_button_x
                                                    && viewport_local_x <= clear_button_x + button_width
                                                    && viewport_local_y >= buttons_y
                                                    && viewport_local_y <= buttons_y + button_height
                                                {
                                                    // Clear the selected date
                                                    datepicker.selected_date = None;
                                                    datepicker.open = false;
                                                    needs_redraw = true;
                                                    window.request_redraw();
                                                    break;
                                                }

                                                // Click was in popup but not on a specific element
                                                break;
                                            }
                                        }

                                        // Check if click is on the date picker input field itself
                                        let in_bounds = viewport_local_x >= datepicker.rect.x
                                            && viewport_local_x <= datepicker.rect.x + datepicker.rect.w
                                            && viewport_local_y >= datepicker.rect.y
                                            && viewport_local_y <= datepicker.rect.y + datepicker.rect.h;

                                        if in_bounds {
                                            // Toggle the date picker popup
                                            datepicker.open = !datepicker.open;

                                            // If opening with no selected date, set current view to today (Nov 18, 2025 demo date)
                                            if datepicker.open && datepicker.selected_date.is_none() {
                                                datepicker.current_view_month = 11; // November
                                                datepicker.current_view_year = 2025;
                                            }

                                            // Reset to Days mode when opening
                                            if datepicker.open {
                                                datepicker.picker_mode = elements::date_picker::PickerMode::Days;
                                            }

                                            // Clear focus from all other UI elements
                                            for cb in viewport_ir_lock.checkboxes.iter_mut() {
                                                cb.focused = false;
                                            }
                                            for input in viewport_ir_lock.input_boxes.iter_mut() {
                                                input.focused = false;
                                            }
                                            for ta in viewport_ir_lock.text_areas.iter_mut() {
                                                ta.focused = false;
                                            }
                                            for r in viewport_ir_lock.radios.iter_mut() {
                                                r.focused = false;
                                            }
                                            for s in viewport_ir_lock.selects.iter_mut() {
                                                s.focused = false;
                                                s.open = false; // Close all select dropdowns
                                            }
                                            for (i, dp) in viewport_ir_lock.date_pickers.iter_mut().enumerate() {
                                                dp.focused = false;
                                                // Close all other date pickers to prevent z-index occlusion
                                                if i != idx {
                                                    dp.open = false;
                                                }
                                            }

                                            // Set focus on clicked date picker
                                            viewport_ir_lock.date_pickers[idx].focused = true;

                                            clicked_popup = true;
                                            needs_redraw = true;
                                            window.request_redraw();
                                            break;
                                        }
                                    }
                                }

                                // If clicked outside all input boxes, text areas, checkboxes, radios, selects, and date pickers, unfocus all
                                if !clicked_input
                                    && !clicked_textarea
                                    && !clicked_checkbox
                                    && !clicked_radio
                                    && !clicked_select
                                    && !clicked_popup
                                {
                                    for input_box in viewport_ir_lock.input_boxes.iter_mut() {
                                        if input_box.focused {
                                            input_box.focused = false;
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                    for textarea in viewport_ir_lock.text_areas.iter_mut() {
                                        if textarea.focused {
                                            textarea.focused = false;
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                    for checkbox in viewport_ir_lock.checkboxes.iter_mut() {
                                        if checkbox.focused {
                                            checkbox.focused = false;
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                    for radio in viewport_ir_lock.radios.iter_mut() {
                                        if radio.focused {
                                            radio.focused = false;
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                    for select in viewport_ir_lock.selects.iter_mut() {
                                        if select.focused || select.open {
                                            select.focused = false;
                                            select.open = false;
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                    for datepicker in viewport_ir_lock.date_pickers.iter_mut() {
                                        if datepicker.focused || datepicker.open {
                                            datepicker.focused = false;
                                            datepicker.open = false;
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                }

                                // Perform hit test using the stored hit index
                                // Only if we didn't click on an input box, text area, checkbox, radio, select, date picker, or toolbar button
                                if !clicked_input
                                    && !clicked_textarea
                                    && !clicked_checkbox
                                    && !clicked_radio
                                    && !clicked_select
                                    && !clicked_popup
                                    && !clicked_toolbar_button
                                {
                                    // Toolbar buttons are now handled in the first block above
                                    // This block is only for other hit regions (if any are added in the future)
                                }
                            }
                        }
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

                        let mut viewport_ir_lock = viewport_ir.lock().unwrap();

                        // Check if any date picker is open and handle arrow keys for navigation
                        let open_datepicker = viewport_ir_lock
                            .date_pickers
                            .iter_mut()
                            .find(|dp| dp.open);

                        if let Some(datepicker) = open_datepicker {
                            if event.state == winit::event::ElementState::Pressed {
                                use elements::date_picker::PickerMode;

                                match event.physical_key {
                                    PhysicalKey::Code(KeyCode::ArrowLeft) => {
                                        // Navigate backward (previous month/year/decade)
                                        match datepicker.picker_mode {
                                            PickerMode::Days => {
                                                if datepicker.current_view_month == 1 {
                                                    datepicker.current_view_month = 12;
                                                    datepicker.current_view_year -= 1;
                                                } else {
                                                    datepicker.current_view_month -= 1;
                                                }
                                            }
                                            PickerMode::Months => {
                                                datepicker.current_view_year -= 1;
                                            }
                                            PickerMode::Years => {
                                                datepicker.current_view_year -= 9;
                                            }
                                        }
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::ArrowRight) => {
                                        // Navigate forward (next month/year/decade)
                                        match datepicker.picker_mode {
                                            PickerMode::Days => {
                                                if datepicker.current_view_month == 12 {
                                                    datepicker.current_view_month = 1;
                                                    datepicker.current_view_year += 1;
                                                } else {
                                                    datepicker.current_view_month += 1;
                                                }
                                            }
                                            PickerMode::Months => {
                                                datepicker.current_view_year += 1;
                                            }
                                            PickerMode::Years => {
                                                datepicker.current_view_year += 9;
                                            }
                                        }
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::Escape) => {
                                        // Close the date picker on Escape
                                        datepicker.open = false;
                                        datepicker.picker_mode = PickerMode::Days;
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    _ => {
                                        // Other keys are not handled for date picker
                                    }
                                }
                            }
                        } else if let Some(focused_input) = viewport_ir_lock
                            .input_boxes
                            .iter_mut()
                            .find(|ib| ib.focused)
                        {
                            // Baseline single-line InputBox editing path for viewport_ir:
                            // keyboard events are translated into InputBox editing methods
                            // (insert_char, delete_before_cursor, cursor movement, etc.).
                            // Phase 0 keeps this wiring as the source of truth while
                            // allowing InputBox to internally toggle its TextLayout backend.
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
                                    PhysicalKey::Code(KeyCode::KeyA)
                                        if line_modifier && !has_shift =>
                                    {
                                        // Cmd/Ctrl+A: Select all text
                                        focused_input.select_all();
                                        focused_input.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::KeyC)
                                        if line_modifier && !has_shift =>
                                    {
                                        // Cmd/Ctrl+C: Copy to clipboard
                                        if let Err(_e) = focused_input.copy_to_clipboard() {
                                            // eprintln!("Failed to copy: {}", _e);
                                        }
                                        // No redraw needed for copy
                                    }
                                    PhysicalKey::Code(KeyCode::KeyX)
                                        if line_modifier && !has_shift =>
                                    {
                                        // Cmd/Ctrl+X: Cut to clipboard
                                        if let Err(_e) = focused_input.cut_to_clipboard() {
                                            // eprintln!("Failed to cut: {}", _e);
                                        } else {
                                            focused_input.update_scroll();
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                    PhysicalKey::Code(KeyCode::KeyV)
                                        if line_modifier && !has_shift =>
                                    {
                                        // Cmd/Ctrl+V: Paste from clipboard
                                        if let Err(_e) = focused_input.paste_from_clipboard() {
                                            // eprintln!("Failed to paste: {}", _e);
                                        } else {
                                            focused_input.update_scroll();
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                    PhysicalKey::Code(KeyCode::KeyZ)
                                        if line_modifier && has_shift =>
                                    {
                                        // Cmd/Ctrl+Shift+Z: Redo
                                        if focused_input.redo() {
                                            focused_input.update_scroll();
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                    PhysicalKey::Code(KeyCode::KeyZ)
                                        if line_modifier && !has_shift =>
                                    {
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
                        } else if let Some(focused_textarea) =
                            viewport_ir_lock.text_areas.iter_mut().find(|ta| ta.focused)
                        {
                            // TextArea keyboard handling (multi-line editing)
                            if event.state == winit::event::ElementState::Pressed {
                                let has_cmd = modifiers_state.contains(ModifiersState::SUPER);
                                let has_ctrl = modifiers_state.contains(ModifiersState::CONTROL);
                                let has_alt = modifiers_state.contains(ModifiersState::ALT);
                                let has_shift = modifiers_state.contains(ModifiersState::SHIFT);

                                let line_modifier = has_cmd || has_ctrl;
                                let word_modifier = has_alt;

                                match event.physical_key {
                                    PhysicalKey::Code(KeyCode::KeyA)
                                        if line_modifier && !has_shift =>
                                    {
                                        focused_textarea.select_all();
                                        focused_textarea.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::KeyC)
                                        if line_modifier && !has_shift =>
                                    {
                                        if let Err(_e) = focused_textarea.copy_to_clipboard() {
                                            // eprintln!("Failed to copy: {}", _e);
                                        }
                                    }
                                    PhysicalKey::Code(KeyCode::KeyX)
                                        if line_modifier && !has_shift =>
                                    {
                                        if let Err(_e) = focused_textarea.cut_to_clipboard() {
                                            // eprintln!("Failed to cut: {}", _e);
                                        } else {
                                            focused_textarea.update_scroll();
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                    PhysicalKey::Code(KeyCode::KeyV)
                                        if line_modifier && !has_shift =>
                                    {
                                        if let Err(_e) = focused_textarea.paste_from_clipboard() {
                                            // eprintln!("Failed to paste: {}", _e);
                                        } else {
                                            focused_textarea.update_scroll();
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                    PhysicalKey::Code(KeyCode::KeyZ)
                                        if line_modifier && has_shift =>
                                    {
                                        if focused_textarea.redo() {
                                            focused_textarea.update_scroll();
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                    PhysicalKey::Code(KeyCode::KeyZ)
                                        if line_modifier && !has_shift =>
                                    {
                                        if focused_textarea.undo() {
                                            focused_textarea.update_scroll();
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                    PhysicalKey::Code(KeyCode::KeyY) if has_ctrl && !has_shift => {
                                        if focused_textarea.redo() {
                                            focused_textarea.update_scroll();
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                    PhysicalKey::Code(KeyCode::Backspace) => {
                                        focused_textarea.delete_before_cursor();
                                        focused_textarea.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::Delete) => {
                                        focused_textarea.delete_after_cursor();
                                        focused_textarea.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::Enter) => {
                                        // Insert newline in TextArea
                                        focused_textarea.insert_char('\n');
                                        focused_textarea.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::ArrowUp) => {
                                        if line_modifier && has_shift {
                                            // Cmd+Shift+Up: select to document start
                                            focused_textarea.extend_selection_to_document_start();
                                        } else if line_modifier {
                                            // Cmd+Up: move to document start
                                            focused_textarea.move_cursor_to_document_start();
                                        } else if has_shift {
                                            // Shift+Up: extend selection up
                                            focused_textarea.extend_selection_up();
                                        } else {
                                            // Up: move cursor up
                                            focused_textarea.move_cursor_up();
                                        }
                                        focused_textarea.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::ArrowDown) => {
                                        if line_modifier && has_shift {
                                            // Cmd+Shift+Down: select to document end
                                            focused_textarea.extend_selection_to_document_end();
                                        } else if line_modifier {
                                            // Cmd+Down: move to document end
                                            focused_textarea.move_cursor_to_document_end();
                                        } else if has_shift {
                                            // Shift+Down: extend selection down
                                            focused_textarea.extend_selection_down();
                                        } else {
                                            // Down: move cursor down
                                            focused_textarea.move_cursor_down();
                                        }
                                        focused_textarea.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::ArrowLeft) => {
                                        if line_modifier && has_shift {
                                            focused_textarea.extend_selection_to_line_start();
                                        } else if line_modifier {
                                            focused_textarea.move_cursor_line_start();
                                        } else if word_modifier && has_shift {
                                            focused_textarea.extend_selection_left_word();
                                        } else if word_modifier {
                                            focused_textarea.move_cursor_left_word();
                                        } else if has_shift {
                                            focused_textarea.extend_selection_left();
                                        } else {
                                            focused_textarea.move_cursor_left();
                                        }
                                        focused_textarea.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::ArrowRight) => {
                                        if line_modifier && has_shift {
                                            focused_textarea.extend_selection_to_line_end();
                                        } else if line_modifier {
                                            focused_textarea.move_cursor_line_end();
                                        } else if word_modifier && has_shift {
                                            focused_textarea.extend_selection_right_word();
                                        } else if word_modifier {
                                            focused_textarea.move_cursor_right_word();
                                        } else if has_shift {
                                            focused_textarea.extend_selection_right();
                                        } else {
                                            focused_textarea.move_cursor_right();
                                        }
                                        focused_textarea.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::Home) => {
                                        if has_shift {
                                            focused_textarea.extend_selection_to_line_start();
                                        } else {
                                            focused_textarea.move_cursor_line_start();
                                        }
                                        focused_textarea.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::End) => {
                                        if has_shift {
                                            focused_textarea.extend_selection_to_line_end();
                                        } else {
                                            focused_textarea.move_cursor_line_end();
                                        }
                                        focused_textarea.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::Space) => {
                                        focused_textarea.insert_char(' ');
                                        focused_textarea.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    PhysicalKey::Code(KeyCode::Tab) => {
                                        // Insert tab as spaces (4 spaces)
                                        for _ in 0..4 {
                                            focused_textarea.insert_char(' ');
                                        }
                                        focused_textarea.update_scroll();
                                        needs_redraw = true;
                                        window.request_redraw();
                                    }
                                    _ => {
                                        if let Some(text) = &event.text {
                                            for ch in text.chars() {
                                                if !ch.is_control() || ch == ' ' || ch == '\n' {
                                                    focused_textarea.insert_char(ch);
                                                }
                                            }
                                            focused_textarea.update_scroll();
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
                    WindowEvent::ScaleFactorChanged {
                        scale_factor: sf, ..
                    } => {
                        let new_scale = sf as f32;
                        surf.set_dpi_scale(new_scale);
                        needs_redraw = true;
                        window.request_redraw();
                    }
                    WindowEvent::RedrawRequested => {
                        // Check debounce timing HERE (not in AboutToWait which may not fire frequently)
                        if let Some(last_time) = last_resize_time {
                            let settled =
                                last_time.elapsed() >= std::time::Duration::from_millis(200);
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
                        let size_changed =
                            prev_size.width != size.width || prev_size.height != size.height;
                        // Force full redraw if: no resize in progress OR max debounce exceeded (300ms)
                        // Note: We no longer force full redraw on size change because intermediate texture
                        // is preserved during expansion (only reallocated when growing or shrinking >25%)
                        let max_debounce_exceeded = first_resize_time
                            .map(|t| t.elapsed() >= std::time::Duration::from_millis(300))
                            .unwrap_or(false);
                        let should_render_full =
                            needs_redraw && (last_resize_time.is_none() || max_debounce_exceeded);

                        if (!should_render_backgrounds && !should_render_full)
                            || size.width == 0
                            || size.height == 0
                        {
                            return;
                        }

                        // For unified rendering we always clear the surface each frame
                        // and rely on depth/z-index for layering instead of preserving contents.
                        surf.set_preserve_surface(false);

                        if size_changed {
                            prev_size = size;
                        }

                        let frame = match surface.get_current_texture() {
                            Ok(f) => f,
                            Err(_) => {
                                window.request_redraw();
                                return;
                            }
                        };
                        // Canvas uses physical pixels, but zones are in logical pixels
                        // So we pass physical size to canvas
                        let mut canvas = surf.begin_frame(size.width, size.height);
                        canvas.set_text_provider(provider.clone());

                        // Set clear color to peach for visibility during debugging
                        canvas.clear(ColorLinPremul::from_srgba_u8([255, 229, 180, 255]));

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

                            // Update caret blink state for all editable controls and
                            // track whether any of them are currently focused. While
                            // a control is focused we keep `needs_redraw` true so
                            // that the event loop continues to request redraws and
                            // the caret can blink at a steady rate.
                            let any_focused_editable = {
                                let mut viewport_ir_lock = viewport_ir.lock().unwrap();
                                for input_box in viewport_ir_lock.input_boxes.iter_mut() {
                                    input_box.update_blink(delta_time);
                                }

                                for textarea in viewport_ir_lock.text_areas.iter_mut() {
                                    textarea.update_blink(delta_time);
                                }

                                viewport_ir_lock
                                    .input_boxes
                                    .iter()
                                    .any(|ib| ib.focused)
                                    || viewport_ir_lock
                                        .text_areas
                                        .iter()
                                        .any(|ta| ta.focused)
                            }; // Release viewport_ir lock

                            if any_focused_editable {
                                needs_redraw = true;
                            }

                            // Render sample UI elements in viewport zone with local coordinates.
                            let viewport_rect = zone_manager.layout.get_zone(ZoneId::Viewport);

                            // Apply transform first to position viewport content
                            canvas.push_transform(Transform2D::translate(
                                viewport_rect.x,
                                viewport_rect.y,
                            ));

                            // Push clip rect in viewport-local coordinates (0,0 origin)
                            let local_clip = Rect {
                                x: 0.0,
                                y: 0.0,
                                w: viewport_rect.w,
                                h: viewport_rect.h,
                            };
                            canvas.push_clip_rect(local_clip);

                            // Apply scroll offset as a nested transform (negative to scroll down)
                            canvas.push_transform(Transform2D::translate(
                                0.0,
                                -zone_manager.viewport.scroll_offset,
                            ));

                            let content_height = {
                                let mut viewport_ir_lock = viewport_ir.lock().unwrap();
                                viewport_ir_lock.render(
                                    &mut canvas,
                                    scale_factor,
                                    viewport_rect.w as u32,
                                    viewport_rect.h as u32,
                                    provider.as_ref(),
                                    text_cache.as_ref(),
                                )
                            };

                            canvas.pop_transform(); // Pop scroll transform
                            canvas.pop_clip(); // Pop clip rect
                            canvas.pop_transform(); // Pop viewport position transform

                            // Update viewport content height
                            zone_manager
                                .viewport
                                .set_content_height(content_height, viewport_rect.h);

                            // Render toolbar icons and hit regions
                            let toolbar_rect = zone_manager.layout.get_zone(ZoneId::Toolbar);
                            let toggle_size = 24.0;
                            let toggle_margin = 12.0;

                            // Use toolbar-local coordinates for both hit regions and icons
                            canvas.push_transform(Transform2D::translate(
                                toolbar_rect.x,
                                toolbar_rect.y,
                            ));

                            // Sidebar toggle (local coords)
                            let toggle_x_local = toggle_margin;
                            let toggle_y_local = (toolbar_rect.h - toggle_size) * 0.5;

                            let toggle_rect = Rect {
                                x: toggle_x_local,
                                y: toggle_y_local,
                                w: toggle_size,
                                h: toggle_size,
                            };

                            canvas.hit_region_rect(TOGGLE_BUTTON_REGION_ID, toggle_rect, 10100);

                            // Devtools toggle (local coords)
                            let devtools_x_local = toolbar_rect.w - toggle_size - toggle_margin;
                            let devtools_y_local = (toolbar_rect.h - toggle_size) * 0.5;

                            let devtools_rect_hit = Rect {
                                x: devtools_x_local,
                                y: devtools_y_local,
                                w: toggle_size,
                                h: toggle_size,
                            };

                            canvas.hit_region_rect(
                                DEVTOOLS_BUTTON_REGION_ID,
                                devtools_rect_hit,
                                10100,
                            );

                            let white = ColorLinPremul::rgba(255, 255, 255, 255);
                            let icon_style = engine_core::SvgStyle::new()
                                .with_stroke(white)
                                .with_stroke_width(1.5);

                            // Icons rendered in toolbar-local coordinates (transform applied)
                            canvas.draw_svg_styled(
                                "images/panel-left.svg",
                                [toggle_x_local, toggle_y_local],
                                [toggle_size, toggle_size],
                                icon_style,
                                10200,
                            );

                            canvas.draw_svg_styled(
                                "images/inspection-panel.svg",
                                [devtools_x_local, devtools_y_local],
                                [toggle_size, toggle_size],
                                icon_style,
                                10200,
                            );

                            canvas.pop_transform();
                        }

                        // Render devtools chrome and hit regions when devtools is visible.
                        // All visuals go through the unified Canvas path so they share the
                        // same coordinate system and z-ordering as the rest of the scene.
                        let devtools_visible = zone_manager.is_devtools_visible();
                        if should_render_full && devtools_visible {
                            let devtools_rect = zone_manager.layout.get_zone(ZoneId::DevTools);
                            canvas.push_transform(Transform2D::translate(
                                devtools_rect.x,
                                devtools_rect.y,
                            ));

                            let button_size = 18.0;
                            let tab_height = 24.0;
                            let tab_padding = 10.0;
                            let icon_text_gap = 6.0;

                            // Colors match the previous overlay implementation so visuals stay consistent.
                            let white = ColorLinPremul::rgba(255, 255, 255, 255);
                            let inactive_color = ColorLinPremul::rgba(160, 170, 180, 255);
                            let header_bg = ColorLinPremul::rgba(34, 41, 60, 255);
                            let active_tab_bg = ColorLinPremul::rgba(54, 61, 80, 255);
                            let inactive_tab_bg = ColorLinPremul::rgba(40, 47, 66, 255);
                            let header_height = tab_height + 8.0;

                            // Panel background in devtools-local coordinates.
                            let devtools_style = &zone_manager.devtools.style;
                            canvas.fill_rect(
                                0.0,
                                0.0,
                                devtools_rect.w,
                                devtools_rect.h,
                                Brush::Solid(devtools_style.bg_color),
                                10100,
                            );

                            // Optional border: draw as four rects around the panel.
                            if devtools_style.border_width > 0.0 {
                                let bw = devtools_style.border_width;
                                let border_brush = Brush::Solid(devtools_style.border_color);

                                // Top
                                canvas.fill_rect(0.0, 0.0, devtools_rect.w, bw, border_brush.clone(), 10100);
                                // Bottom
                                canvas.fill_rect(
                                    0.0,
                                    devtools_rect.h - bw,
                                    devtools_rect.w,
                                    bw,
                                    border_brush.clone(),
                                    10100,
                                );
                                // Left
                                canvas.fill_rect(0.0, 0.0, bw, devtools_rect.h, border_brush.clone(), 10100);
                                // Right
                                canvas.fill_rect(
                                    devtools_rect.w - bw,
                                    0.0,
                                    bw,
                                    devtools_rect.h,
                                    border_brush,
                                    10100,
                                );
                            }

                            // Header strip behind tabs
                            canvas.fill_rect(
                                0.0,
                                0.0,
                                devtools_rect.w,
                                header_height,
                                Brush::Solid(header_bg),
                                10110,
                            );

                            let active_tab = zone_manager.devtools.active_tab;

                            // Elements tab
                            let elements_x = tab_padding;
                            let elements_y = (tab_height - button_size) * 0.5;
                            let elements_tab_width =
                                button_size + icon_text_gap + 8.0 + 54.0 + tab_padding * 3.0;

                            let elements_rect = Rect {
                                x: elements_x,
                                y: elements_y,
                                w: elements_tab_width,
                                h: tab_height,
                            };
                            let is_elements_active = active_tab == DevToolsTab::Elements;
                            let elements_bg = if is_elements_active {
                                active_tab_bg
                            } else {
                                inactive_tab_bg
                            };
                            let elements_color = if is_elements_active {
                                white
                            } else {
                                inactive_color
                            };

                            // Elements tab background
                            canvas.fill_rect(
                                elements_rect.x,
                                elements_rect.y,
                                elements_rect.w,
                                elements_rect.h,
                                Brush::Solid(elements_bg),
                                10120,
                            );

                            // Register hit region in local coords
                            canvas.hit_region_rect(
                                DEVTOOLS_ELEMENTS_TAB_REGION_ID,
                                elements_rect,
                                10300,
                            );

                            // Console tab
                            let console_x = elements_x + elements_tab_width + 8.0;
                            let console_y = (tab_height - button_size) * 0.5;

                            let console_tab_width = button_size
                                + icon_text_gap
                                + 8.0
                                + 50.0
                                + tab_padding * 3.0;

                            let console_rect = Rect {
                                x: console_x,
                                y: console_y,
                                w: console_tab_width,
                                h: tab_height,
                            };
                            let is_console_active = active_tab == DevToolsTab::Console;
                            let console_bg = if is_console_active {
                                active_tab_bg
                            } else {
                                inactive_tab_bg
                            };
                            let console_color = if is_console_active {
                                white
                            } else {
                                inactive_color
                            };

                            // Console tab background
                            canvas.fill_rect(
                                console_rect.x,
                                console_rect.y,
                                console_rect.w,
                                console_rect.h,
                                Brush::Solid(console_bg),
                                10120,
                            );

                            // Register hit region in local coords
                            canvas.hit_region_rect(
                                DEVTOOLS_CONSOLE_TAB_REGION_ID,
                                console_rect,
                                10300,
                            );

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

                            canvas.hit_region_rect(
                                DEVTOOLS_CLOSE_BUTTON_REGION_ID,
                                close_rect,
                                10300,
                            );

                            // Elements tab icon (local coordinates within transform)
                            let icon_style_elements = engine_core::SvgStyle::new()
                                .with_stroke(elements_color)
                                .with_stroke_width(2.0);

                            canvas.draw_svg_styled(
                                "images/square-mouse-pointer.svg",
                                [elements_x, elements_y],
                                [button_size, button_size],
                                icon_style_elements,
                                10250,
                            );

                            // Console tab icon (local coordinates within transform)
                            let icon_style_console = engine_core::SvgStyle::new()
                                .with_stroke(console_color)
                                .with_stroke_width(2.0);

                            canvas.draw_svg_styled(
                                "images/square-terminal.svg",
                                [console_x, console_y],
                                [button_size, button_size],
                                icon_style_console,
                                10250,
                            );

                            // Close button icon (local coordinates within transform)
                            let close_icon_style = engine_core::SvgStyle::new()
                                .with_stroke(white)
                                .with_stroke_width(2.0);

                            canvas.draw_svg_styled(
                                "images/x.svg",
                                [close_x, close_y],
                                [close_size, close_size],
                                close_icon_style,
                                10250,
                            );

                            // Tab labels
                            canvas.draw_text_run(
                                [
                                    elements_x + button_size + icon_text_gap + 8.0,
                                    tab_height - 6.0,
                                ],
                                "Elements".to_string(),
                                11.0,
                                elements_color,
                                10260,
                            );

                            canvas.draw_text_run(
                                [
                                    console_x + button_size + icon_text_gap + 8.0,
                                    tab_height - 6.0,
                                ],
                                "Console".to_string(),
                                11.0,
                                console_color,
                                10260,
                            );

                            // Content label inside devtools body based on active tab
                            let content_text = match active_tab {
                                DevToolsTab::Console => "Console",
                                DevToolsTab::Elements => "Elements",
                            };
                            let label_color: ColorLinPremul =
                                ColorLinPremul::rgba(220, 230, 240, 255);
                            canvas.draw_text_run(
                                [tab_padding + 4.0, header_height + 14.0],
                                content_text.to_string(),
                                12.0,
                                label_color,
                                10260,
                            );

                            canvas.pop_transform();
                        }

                        // Build hit index from display list before ending frame
                        // Only rebuild hit index during full render
                        if should_render_full {
                            hit_index = Some(engine_core::HitIndex::build(canvas.display_list()));
                        }

                        surf.end_frame(frame, canvas).ok();

                        // Clear flags after rendering.
                        // Keep requesting redraws while any text input or text area
                        // has focus so the caret blink animation remains smooth.
                        if should_render_full {
                            let any_focused_editable = {
                                let viewport_ir_lock = viewport_ir.lock().unwrap();
                                viewport_ir_lock
                                    .input_boxes
                                    .iter()
                                    .any(|ib| ib.focused)
                                    || viewport_ir_lock
                                        .text_areas
                                        .iter()
                                        .any(|ta| ta.focused)
                            };

                            if !any_focused_editable {
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
