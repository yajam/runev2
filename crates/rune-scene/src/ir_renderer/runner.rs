//! Window, event-loop, and zone orchestration for the IR renderer.

use anyhow::{Context as AnyhowContext, Result};
use engine_core::{HitIndex, Transform2D};
use rune_ir::{data::document::DataDocument, view::ViewDocument};
use std::time::{Duration, Instant};
use tracing::warn;
use winit::dpi::{LogicalPosition, LogicalSize};

use super::IrRenderer;
use crate::persistence::WindowStateStore;
use crate::navigation;

/// Main entry point for IR-based rendering flow.
///
/// This function sets up the window, event loop, and rendering pipeline
/// using the IR renderer with Taffy layout and the new scene architecture.
///
/// Package loading priority:
/// 1. CLI argument: `cargo run -p rune-scene -- /path/to/package`
/// 2. Config file: `package_path` in `rune.toml` under `[ir]`
/// 3. Default: Built-in `home_tab` sample from rune-ir
///
/// # Examples
///
/// ```bash
/// # Load default home_tab sample
/// USE_IR=1 cargo run -p rune-scene
///
/// # Load from rune.toml config
/// # (set package_path = "examples/sample_first_node" in rune.toml)
/// USE_IR=1 cargo run -p rune-scene
///
/// # Load sample_first_node via CLI (overrides config)
/// USE_IR=1 cargo run -p rune-scene -- examples/sample_first_node
///
/// # Load custom package
/// USE_IR=1 cargo run -p rune-scene -- /path/to/my/package
/// ```
///
/// TODO: Add address bar navigation support
pub fn run() -> Result<()> {
    eprintln!("IR rendering mode enabled (USE_IR=1)");

    // Load IR package from CLI path or use default home_tab sample
    // These are mutable to support dynamic package switching via navigation
    let (mut data_doc, mut view_doc) = load_ir_package()?;

    eprintln!("Loaded IR package:");
    eprintln!("  - Data document ID: {}", data_doc.document_id);
    eprintln!("  - View document ID: {}", view_doc.view_id);
    eprintln!("  - Root node: {}", view_doc.root);

    eprintln!("\nInitializing rendering pipeline...");

    // Set up winit window and event loop
    use winit::event::{Event, WindowEvent};
    use winit::event_loop::EventLoop;
    use winit::window::WindowBuilder;
    let mut window_state = WindowStateStore::load()?;

    eprintln!("Creating event loop...");
    let event_loop = EventLoop::new()?;
    eprintln!("✓ Event loop created");
    let mut window_builder = WindowBuilder::new().with_title("Rune Scene — IR Renderer");
    if let Some(size) = window_state.window_size().filter(|s| s.is_valid()) {
        window_builder = window_builder.with_inner_size(LogicalSize::new(size.width, size.height));
    } else {
        window_builder = window_builder.with_inner_size(LogicalSize::new(1280.0, 720.0));
    }
    if let Some(position) = window_state.window_position() {
        window_builder = window_builder.with_position(LogicalPosition::new(position.x, position.y));
    }
    if let Some(maximized) = window_state.window_maximized() {
        window_builder = window_builder.with_maximized(maximized);
    }

    let window = window_builder.build(&event_loop)?;
    let window: &'static winit::window::Window = Box::leak(Box::new(window));

    // Set up wgpu
    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(window)?;
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }))
    .context("No suitable GPU adapters found")?;

    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))?;

    let mut size = window.inner_size();
    let scale_factor = window.scale_factor() as f32;
    let config = engine_core::make_surface_config(&adapter, &surface, size.width, size.height);
    surface.configure(&device, &config);

    // Set up RuneSurface wrapper
    let mut surf = rune_surface::RuneSurface::new(
        std::sync::Arc::new(device),
        std::sync::Arc::new(queue),
        config.format,
    );
    surf.set_use_intermediate(true);
    surf.set_logical_pixels(true);
    surf.set_dpi_scale(scale_factor);

    // Set up text provider
    let provider: std::sync::Arc<dyn engine_core::TextProvider> =
        std::sync::Arc::new(create_text_provider()?);

    // Create IR renderer
    let mut ir_renderer = IrRenderer::new();

    // Create zone manager for full app layout (toolbar, sidebar, viewport, devtools)
    let logical_width = (size.width as f32 / scale_factor) as u32;
    let logical_height = (size.height as f32 / scale_factor) as u32;
    let mut zone_manager = crate::zones::ZoneManager::new(logical_width, logical_height);

    // State
    let mut cursor_position: Option<(f32, f32)> = None;
    let mut hit_index: Option<HitIndex> = None;
    let mut last_frame_time = Instant::now();
    let mut modifiers_state = winit::keyboard::ModifiersState::empty();
    let mut last_click_time: Option<Instant> = None;
    let mut click_count: u32 = 0;
    let double_click_threshold = Duration::from_millis(500);
    let mut needs_redraw = true;
    // Resize debounce removed: redraw immediately when needed for smoother resizing.

    eprintln!("✓ Rendering pipeline initialized");
    eprintln!("✓ Zone layout: viewport={:?}", zone_manager.layout.viewport);
    eprintln!("Starting event loop...\n");

    // Event loop
    event_loop.run(move |event, target| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    if let Err(err) = window_state.save() {
                        warn!(?err, "failed to persist window state on close");
                    }
                    target.exit();
                }
                WindowEvent::Moved(new_position) => {
                    let logical = new_position.to_logical::<f64>(window.scale_factor());
                    window_state.update_position(logical.x.round() as i32, logical.y.round() as i32);
                }
                WindowEvent::Resized(new_size) => {
                    size = new_size;
                    let new_config = engine_core::make_surface_config(
                        &adapter,
                        &surface,
                        size.width,
                        size.height,
                    );
                    surface.configure(surf.device().as_ref(), &new_config);

                    let logical_width = (size.width as f32 / scale_factor) as u32;
                    let logical_height = (size.height as f32 / scale_factor) as u32;
                    zone_manager.resize(logical_width, logical_height);

                    let logical = new_size.to_logical::<f64>(window.scale_factor());
                    window_state.update_size(logical.width, logical.height);
                    window_state.update_maximized(window.is_maximized());

                    // Mark that we need a redraw for the new size immediately.
                    needs_redraw = true;
                }
                WindowEvent::CursorMoved { position, .. } => {
                    cursor_position = Some((position.x as f32, position.y as f32));

                    // Handle mouse move for IR element drag selection using scene-local coords
                    let logical_x = position.x as f32 / scale_factor;
                    let logical_y = position.y as f32 / scale_factor;
                    let viewport = zone_manager.layout.viewport;
                    // NOTE: Keep event coords in the same viewport-local space used for rendering
                    // (apply viewport origin + scroll here, not inside elements), or hit testing breaks.
                    let scene_x = logical_x - viewport.x + zone_manager.viewport.scroll_offset_x;
                    let scene_y = logical_y - viewport.y + zone_manager.viewport.scroll_offset_y;

                    use crate::event_handler::MouseMoveEvent;
                    let move_event = MouseMoveEvent {
                        x: scene_x,
                        y: scene_y,
                    };

                    let result = ir_renderer
                        .element_state_mut()
                        .handle_mouse_move(move_event);
                    if result.is_handled() {
                        needs_redraw = true;
                        window.request_redraw();
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    use winit::event::MouseScrollDelta;

                    let (scroll_x, scroll_y) = match delta {
                        MouseScrollDelta::LineDelta(x, y) => (x * 20.0, y * 20.0),
                        MouseScrollDelta::PixelDelta(pos) => (pos.x as f32, pos.y as f32),
                    };

                    let viewport_rect = zone_manager.layout.get_zone(crate::zones::ZoneId::Viewport);
                    zone_manager
                        .viewport
                        .scroll(-scroll_x, -scroll_y, viewport_rect.w, viewport_rect.h);
                    needs_redraw = true;
                    window.request_redraw();
                }
                WindowEvent::ModifiersChanged(new_state) => {
                    modifiers_state = new_state.state();
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if button == winit::event::MouseButton::Left
                        && state == winit::event::ElementState::Pressed
                    {
                        // Update multi-click tracking
                        let now = Instant::now();
                        let is_quick_click = last_click_time
                            .map(|t| now.duration_since(t) < double_click_threshold)
                            .unwrap_or(false);
                        if is_quick_click {
                            click_count += 1;
                        } else {
                            click_count = 1;
                        }
                        last_click_time = Some(now);

                        if let (Some((cursor_x, cursor_y)), Some(index)) =
                            (cursor_position, hit_index.as_ref())
                        {
                            let logical_x = cursor_x / scale_factor;
                            let logical_y = cursor_y / scale_factor;

                            // Calculate scene coordinates for select overlay check
                            let viewport = zone_manager.layout.viewport;
                            let scene_x =
                                logical_x - viewport.x + zone_manager.viewport.scroll_offset_x;
                            let scene_y =
                                logical_y - viewport.y + zone_manager.viewport.scroll_offset_y;

                            // Close open select dropdowns unless clicking on their overlay
                            ir_renderer
                                .element_state_mut()
                                .close_selects_except_at_point(scene_x, scene_y);

                            // Close open date picker popups unless clicking on their popup
                            ir_renderer
                                .element_state_mut()
                                .close_date_pickers_except_at_point(scene_x, scene_y);

                            if let Some(hit) = index.topmost_at([logical_x, logical_y]) {
                                if let Some(region_id) = hit.region_id {
                                    use crate::zones::ZoneId;
                                    use crate::zones::{
                                        ADDRESS_BAR_REGION_ID, BACK_BUTTON_REGION_ID,
                                        DEVTOOLS_BUTTON_REGION_ID, DEVTOOLS_CLOSE_BUTTON_REGION_ID,
                                        DEVTOOLS_CONSOLE_TAB_REGION_ID,
                                        DEVTOOLS_ELEMENTS_TAB_REGION_ID, FORWARD_BUTTON_REGION_ID,
                                        HOME_BUTTON_REGION_ID, REFRESH_BUTTON_REGION_ID,
                                        TOGGLE_BUTTON_REGION_ID, SIDEBAR_BOOKMARK_REGION_BASE,
                                        SIDEBAR_TAB_REGION_BASE, SIDEBAR_ADD_BOOKMARK_REGION_ID,
                                        SIDEBAR_TAB_CLOSE_REGION_BASE, SIDEBAR_BOOKMARK_DELETE_REGION_BASE,
                                        CHAT_BUTTON_REGION_ID, CHAT_FAB_REGION_ID,
                                        CHAT_CLOSE_BUTTON_REGION_ID, CHAT_INPUT_REGION_ID,
                                        CHAT_SEND_BUTTON_REGION_ID,
                                    };

                                    // Blur chat input for any click that is not on the chat
                                    // input region itself so keyboard events don't keep routing
                                    // to the sidebar once focus moves elsewhere.
                                    if region_id != CHAT_INPUT_REGION_ID {
                                        zone_manager.chat.input.focused = false;
                                    }

                                    match region_id {
                                        TOGGLE_BUTTON_REGION_ID => {
                                            // Clicking toolbar chrome should blur any focused fields.
                                            if zone_manager.toolbar.address_bar.focused {
                                                zone_manager.toolbar.address_bar.focused = false;
                                                zone_manager.toolbar.address_bar.end_mouse_selection();
                                            }
                                            ir_renderer.element_state_mut().clear_all_focus();

                                            let logical_width =
                                                (size.width as f32 / scale_factor) as u32;
                                            let logical_height =
                                                (size.height as f32 / scale_factor) as u32;
                                            zone_manager
                                                .toggle_sidebar(logical_width, logical_height);
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        DEVTOOLS_BUTTON_REGION_ID => {
                                            if zone_manager.toolbar.address_bar.focused {
                                                zone_manager.toolbar.address_bar.focused = false;
                                                zone_manager.toolbar.address_bar.end_mouse_selection();
                                            }
                                            ir_renderer.element_state_mut().clear_all_focus();

                                            zone_manager.toggle_devtools();
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        BACK_BUTTON_REGION_ID => {
                                            if zone_manager.toolbar.address_bar.focused {
                                                zone_manager.toolbar.address_bar.focused = false;
                                                zone_manager.toolbar.address_bar.end_mouse_selection();
                                            }
                                            ir_renderer.element_state_mut().clear_all_focus();

                                            println!("Back button clicked");
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        FORWARD_BUTTON_REGION_ID => {
                                            if zone_manager.toolbar.address_bar.focused {
                                                zone_manager.toolbar.address_bar.focused = false;
                                                zone_manager.toolbar.address_bar.end_mouse_selection();
                                            }
                                            ir_renderer.element_state_mut().clear_all_focus();

                                            println!("Forward button clicked");
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        REFRESH_BUTTON_REGION_ID => {
                                            if zone_manager.toolbar.address_bar.focused {
                                                zone_manager.toolbar.address_bar.focused = false;
                                                zone_manager.toolbar.address_bar.end_mouse_selection();
                                            }
                                            ir_renderer.element_state_mut().clear_all_focus();

                                            println!("Refresh button clicked");
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        HOME_BUTTON_REGION_ID => {
                                            if zone_manager.toolbar.address_bar.focused {
                                                zone_manager.toolbar.address_bar.focused = false;
                                                zone_manager.toolbar.address_bar.end_mouse_selection();
                                            }
                                            ir_renderer.element_state_mut().clear_all_focus();

                                            // Single click opens Dock overlay
                                            zone_manager.show_dock();
                                            println!("Home button clicked - opening Dock overlay");
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        ADDRESS_BAR_REGION_ID => {
                                            eprintln!("[ADDRESS BAR] Clicked!");
                                            let toolbar_rect =
                                                zone_manager.layout.get_zone(ZoneId::Toolbar);
                                            let local_x = logical_x - toolbar_rect.x;
                                            let local_y = logical_y - toolbar_rect.y;

                                            // Blur any IR element focus when switching to the address bar.
                                            ir_renderer.element_state_mut().clear_all_focus();

                                            eprintln!(
                                                "[ADDRESS BAR] Global click: ({:.1}, {:.1}), Local: ({:.1}, {:.1})",
                                                logical_x, logical_y, local_x, local_y
                                            );

                                            zone_manager.toolbar.address_bar.focused = true;

                                            if click_count == 3 {
                                                zone_manager
                                                    .toolbar
                                                    .address_bar
                                                    .start_line_selection(local_x, local_y);
                                            } else if click_count == 2 {
                                                zone_manager
                                                    .toolbar
                                                    .address_bar
                                                    .start_word_selection(local_x, local_y);
                                            } else {
                                                zone_manager
                                                    .toolbar
                                                    .address_bar
                                                    .start_mouse_selection(local_x, local_y);
                                            }
                                            zone_manager.toolbar.address_bar.update_scroll();
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        DEVTOOLS_CLOSE_BUTTON_REGION_ID => {
                                            if zone_manager.toolbar.address_bar.focused {
                                                zone_manager.toolbar.address_bar.focused = false;
                                                zone_manager.toolbar.address_bar.end_mouse_selection();
                                            }
                                            ir_renderer.element_state_mut().clear_all_focus();

                                            zone_manager.toggle_devtools();
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        DEVTOOLS_ELEMENTS_TAB_REGION_ID => {
                                            if zone_manager.toolbar.address_bar.focused {
                                                zone_manager.toolbar.address_bar.focused = false;
                                                zone_manager.toolbar.address_bar.end_mouse_selection();
                                            }
                                            ir_renderer.element_state_mut().clear_all_focus();

                                            zone_manager.devtools.set_active_tab(
                                                crate::zones::DevToolsTab::Elements,
                                            );
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        DEVTOOLS_CONSOLE_TAB_REGION_ID => {
                                            if zone_manager.toolbar.address_bar.focused {
                                                zone_manager.toolbar.address_bar.focused = false;
                                                zone_manager.toolbar.address_bar.end_mouse_selection();
                                            }
                                            ir_renderer.element_state_mut().clear_all_focus();

                                            zone_manager
                                                .devtools
                                                .set_active_tab(crate::zones::DevToolsTab::Console);
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        // Sidebar: Add bookmark button
                                        SIDEBAR_ADD_BOOKMARK_REGION_ID => {
                                            if zone_manager.toolbar.address_bar.focused {
                                                zone_manager.toolbar.address_bar.focused = false;
                                                zone_manager.toolbar.address_bar.end_mouse_selection();
                                            }
                                            ir_renderer.element_state_mut().clear_all_focus();

                                            // Add bookmark for current page using navigation state,
                                            // which reflects the actual CEF/IR URL and page title.
                                            let current_url = navigation::get_current_url()
                                                .unwrap_or_else(|| zone_manager.toolbar.address_bar.text.clone());
                                            if !current_url.trim().is_empty() {
                                                // Use the CEF page title if available, otherwise derive from URL
                                                let title = navigation::get_current_title()
                                                    .filter(|t| !t.trim().is_empty())
                                                    .unwrap_or_else(|| {
                                                        current_url
                                                            .split('/')
                                                            .last()
                                                            .filter(|s| !s.is_empty())
                                                            .unwrap_or("Bookmark")
                                                            .to_string()
                                                    });
                                                zone_manager.sidebar.add_bookmark(title.clone(), current_url.clone());
                                                println!("Bookmark added: {} -> {}", title, current_url);
                                            }
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        // Sidebar: Tab close buttons (region IDs 2300+)
                                        id if id >= SIDEBAR_TAB_CLOSE_REGION_BASE && id < SIDEBAR_TAB_CLOSE_REGION_BASE + 100 => {
                                            if zone_manager.toolbar.address_bar.focused {
                                                zone_manager.toolbar.address_bar.focused = false;
                                                zone_manager.toolbar.address_bar.end_mouse_selection();
                                            }
                                            ir_renderer.element_state_mut().clear_all_focus();

                                            let tab_index = (id - SIDEBAR_TAB_CLOSE_REGION_BASE) as usize;
                                            if zone_manager.sidebar.remove_tab(tab_index) {
                                                println!("Closed tab at index {}", tab_index);
                                            }
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        // Sidebar: Bookmark delete buttons (region IDs 2400+)
                                        id if id >= SIDEBAR_BOOKMARK_DELETE_REGION_BASE && id < SIDEBAR_BOOKMARK_DELETE_REGION_BASE + 100 => {
                                            if zone_manager.toolbar.address_bar.focused {
                                                zone_manager.toolbar.address_bar.focused = false;
                                                zone_manager.toolbar.address_bar.end_mouse_selection();
                                            }
                                            ir_renderer.element_state_mut().clear_all_focus();

                                            let bookmark_index = (id - SIDEBAR_BOOKMARK_DELETE_REGION_BASE) as usize;
                                            if zone_manager.sidebar.remove_bookmark(bookmark_index) {
                                                println!("Deleted bookmark at index {}", bookmark_index);
                                            }
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        // Sidebar: Tab items (region IDs 2100-2199)
                                        id if id >= SIDEBAR_TAB_REGION_BASE && id < SIDEBAR_TAB_REGION_BASE + 100 => {
                                            if zone_manager.toolbar.address_bar.focused {
                                                zone_manager.toolbar.address_bar.focused = false;
                                                zone_manager.toolbar.address_bar.end_mouse_selection();
                                            }
                                            ir_renderer.element_state_mut().clear_all_focus();

                                            let tab_index = (id - SIDEBAR_TAB_REGION_BASE) as usize;
                                            if let Some(tab) = zone_manager.sidebar.get_tab(tab_index) {
                                                let url = tab.url.clone();
                                                println!("Navigate to tab: {}", url);
                                                // Set this tab as active before navigating
                                                zone_manager.sidebar.set_active_tab(Some(tab_index));
                                                navigation::navigate_to(&url);
                                            }
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        // Sidebar: Bookmark items (region IDs 2000-2099)
                                        id if id >= SIDEBAR_BOOKMARK_REGION_BASE && id < SIDEBAR_BOOKMARK_REGION_BASE + 100 => {
                                            if zone_manager.toolbar.address_bar.focused {
                                                zone_manager.toolbar.address_bar.focused = false;
                                                zone_manager.toolbar.address_bar.end_mouse_selection();
                                            }
                                            ir_renderer.element_state_mut().clear_all_focus();

                                            let bookmark_index = (id - SIDEBAR_BOOKMARK_REGION_BASE) as usize;
                                            if let Some(bookmark) = zone_manager.sidebar.get_bookmark(bookmark_index) {
                                                let url = bookmark.url.clone();
                                                println!("Navigate to bookmark: {}", url);
                                                navigation::navigate_to(&url);
                                            }
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        // Chat button in toolbar
                                        CHAT_BUTTON_REGION_ID => {
                                            let logical_width = (size.width as f32 / scale_factor) as u32;
                                            let logical_height = (size.height as f32 / scale_factor) as u32;
                                            zone_manager.toggle_chat(logical_width, logical_height);
                                            println!("Chat button clicked - toggling chat panel");
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        // Chat FAB (floating action button)
                                        CHAT_FAB_REGION_ID => {
                                            let logical_width = (size.width as f32 / scale_factor) as u32;
                                            let logical_height = (size.height as f32 / scale_factor) as u32;
                                            zone_manager.show_chat(logical_width, logical_height);
                                            println!("Chat FAB clicked - opening chat panel");
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        // Chat close button
                                        CHAT_CLOSE_BUTTON_REGION_ID => {
                                            let logical_width = (size.width as f32 / scale_factor) as u32;
                                            let logical_height = (size.height as f32 / scale_factor) as u32;
                                            zone_manager.hide_chat(logical_width, logical_height);
                                            println!("Chat close button clicked");
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        // Chat input region
                                        CHAT_INPUT_REGION_ID => {
                                            zone_manager.chat.input.focused = true;
                                            println!("Chat input focused");
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        // Chat send button
                                        CHAT_SEND_BUTTON_REGION_ID => {
                                            let text = zone_manager.chat.input.text.trim().to_string();
                                            if !text.is_empty() {
                                                zone_manager.chat.add_user_message(text);
                                                zone_manager.chat.input.text.clear();
                                                zone_manager.chat.input.cursor_position = 0;
                                                println!("Chat message sent");
                                            }
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        // Dock: Scrim click (dismiss dock)
                                        crate::zones::DOCK_SCRIM_REGION_ID => {
                                            zone_manager.hide_dock();
                                            println!("Dock dismissed via scrim click");
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        // Dock: Panel region (prevents click-through, do nothing)
                                        crate::zones::DOCK_PANEL_REGION_ID => {
                                            // Clicking the panel itself does nothing
                                        }
                                        // Dock: Pinned apps (region IDs 5100-5199)
                                        id if id >= crate::zones::DOCK_PINNED_APP_REGION_BASE && id < crate::zones::DOCK_PINNED_APP_REGION_BASE + 100 => {
                                            let app_index = (id - crate::zones::DOCK_PINNED_APP_REGION_BASE) as usize;
                                            if let Some(app) = zone_manager.dock.pinned_apps.get(app_index) {
                                                let url = app.url.clone();
                                                let name = app.name.clone();
                                                println!("Navigate to pinned app: {} -> {}", name, url);
                                                zone_manager.hide_dock();

                                                // Try to load IR package if it's an IR URL
                                                let render_target = navigation::determine_render_target(&url);
                                                if render_target == navigation::RenderTarget::Ir {
                                                    if let Some((new_data, new_view)) = try_load_ir_from_url(&url) {
                                                        data_doc = new_data;
                                                        view_doc = new_view;
                                                        ir_renderer.element_state_mut().clear_all_focus();
                                                        println!("Loaded IR package: {}", url);
                                                    }
                                                }

                                                navigation::navigate_to(&url);
                                                // Update layout for new navigation mode
                                                let logical_width = (size.width as f32 / scale_factor) as u32;
                                                let logical_height = (size.height as f32 / scale_factor) as u32;
                                                zone_manager.update_for_navigation_mode(logical_width, logical_height);
                                            }
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        // Dock: Recent items (region IDs 5200-5299)
                                        id if id >= crate::zones::DOCK_RECENT_ITEM_REGION_BASE && id < crate::zones::DOCK_RECENT_ITEM_REGION_BASE + 100 => {
                                            let item_index = (id - crate::zones::DOCK_RECENT_ITEM_REGION_BASE) as usize;
                                            if let Some(item) = zone_manager.dock.recent_items.get(item_index) {
                                                let url = item.url.clone();
                                                let name = item.name.clone();
                                                println!("Navigate to recent item: {} -> {}", name, url);
                                                zone_manager.hide_dock();

                                                // Try to load IR package if it's an IR URL
                                                let render_target = navigation::determine_render_target(&url);
                                                if render_target == navigation::RenderTarget::Ir {
                                                    if let Some((new_data, new_view)) = try_load_ir_from_url(&url) {
                                                        data_doc = new_data;
                                                        view_doc = new_view;
                                                        ir_renderer.element_state_mut().clear_all_focus();
                                                        println!("Loaded IR package: {}", url);
                                                    }
                                                }

                                                navigation::navigate_to(&url);
                                                // Update layout for new navigation mode
                                                let logical_width = (size.width as f32 / scale_factor) as u32;
                                                let logical_height = (size.height as f32 / scale_factor) as u32;
                                                zone_manager.update_for_navigation_mode(logical_width, logical_height);
                                            }
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        // Root viewport region (empty surface/text): blur everything.
                                        std::u32::MAX => {
                                            if zone_manager.toolbar.address_bar.focused {
                                                zone_manager.toolbar.address_bar.focused = false;
                                                zone_manager.toolbar.address_bar.end_mouse_selection();
                                            }
                                            ir_renderer.element_state_mut().clear_all_focus();
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                        _ => {
                                            // Check if this is an IR element
                                            // Clone the view_node_id to avoid borrow checker issues
                                            if let Some(view_node_id) = ir_renderer
                                                .hit_registry()
                                                .lookup(region_id)
                                                .cloned()
                                            {
                                                // Blur toolbar focus when interacting with IR content.
                                                if zone_manager.toolbar.address_bar.focused {
                                                    zone_manager.toolbar.address_bar.focused = false;
                                                    zone_manager.toolbar.address_bar.end_mouse_selection();
                                                    needs_redraw = true;
                                                    window.request_redraw();
                                                }

                                                // Handle overlay-related clicks
                                                if view_node_id.starts_with("__scrim__") {
                                                    // Scrim clicked - dismiss the overlay
                                                    let overlay_id = view_node_id.strip_prefix("__scrim__").unwrap().to_string();
                                                    ir_renderer.element_state_mut().hide_overlay(&overlay_id);
                                                    needs_redraw = true;
                                                    window.request_redraw();
                                                } else if view_node_id.starts_with("__close__") {
                                                    // Close button clicked - dismiss the overlay
                                                    let overlay_id = view_node_id.strip_prefix("__close__").unwrap().to_string();
                                                    ir_renderer.element_state_mut().hide_overlay(&overlay_id);
                                                    needs_redraw = true;
                                                    window.request_redraw();
                                                } else if view_node_id.starts_with("__ok__") {
                                                    // OK button clicked - dismiss the overlay
                                                    let overlay_id = view_node_id.strip_prefix("__ok__").unwrap().to_string();
                                                    ir_renderer.element_state_mut().hide_overlay(&overlay_id);
                                                    needs_redraw = true;
                                                    window.request_redraw();
                                                } else if view_node_id.starts_with("__cancel__") {
                                                    // Cancel button clicked - dismiss the overlay
                                                    let overlay_id = view_node_id.strip_prefix("__cancel__").unwrap().to_string();
                                                    ir_renderer.element_state_mut().hide_overlay(&overlay_id);
                                                    needs_redraw = true;
                                                    window.request_redraw();
                                                } else if let Some(rest) =
                                                    view_node_id.strip_prefix("__modalbtn")
                                                {
                                                    // Any modal action button closes the modal for now
                                                    if let Some((_idx, overlay_id)) =
                                                        rest.split_once("__")
                                                    {
                                                        ir_renderer
                                                            .element_state_mut()
                                                            .hide_overlay(&overlay_id.to_string());
                                                        needs_redraw = true;
                                                        window.request_redraw();
                                                    }
                                                } else {
                                                    // Dispatch to IR element
                                                    use crate::event_handler::MouseClickEvent;

                                                    let viewport = zone_manager.layout.viewport;
                                                    let scene_x =
                                                        logical_x - viewport.x
                                                            + zone_manager.viewport.scroll_offset_x;
                                                    let scene_y = logical_y
                                                        - viewport.y
                                                        + zone_manager.viewport.scroll_offset_y;

                                                    let event = MouseClickEvent {
                                                        button: winit::event::MouseButton::Left,
                                                        state: winit::event::ElementState::Pressed,
                                                        x: scene_x,
                                                        y: scene_y,
                                                        click_count,
                                                    };

                                                    let result = ir_renderer
                                                        .element_state_mut()
                                                        .handle_mouse_click(&view_node_id, event);

                                                    if result.is_handled() {
                                                        needs_redraw = true;
                                                        window.request_redraw();
                                                    }
                                                }
                                            } else {
                                                if zone_manager.toolbar.address_bar.focused {
                                                    zone_manager.toolbar.address_bar.focused = false;
                                                    zone_manager.toolbar.address_bar.end_mouse_selection();
                                                }
                                                ir_renderer.element_state_mut().clear_all_focus();
                                                needs_redraw = true;
                                                window.request_redraw();
                                            }
                                        }
                                    }
                                } else {
                                    // Clicked a non-interactive surface (no region). Blur all focus.
                                    if zone_manager.toolbar.address_bar.focused {
                                        zone_manager.toolbar.address_bar.focused = false;
                                        zone_manager.toolbar.address_bar.end_mouse_selection();
                                    }
                                    ir_renderer.element_state_mut().clear_all_focus();
                                    needs_redraw = true;
                                    window.request_redraw();
                                }
                            }
                        }
                    } else if button == winit::event::MouseButton::Left
                        && state == winit::event::ElementState::Released
                    {
                        // End any in-progress toolbar selection drag
                        zone_manager.toolbar.address_bar.end_mouse_selection();

                        // Dispatch release to the currently focused IR element so text
                        // selections/caret drags can terminate even when not clicking again.
                        if let Some((cursor_x, cursor_y)) = cursor_position {
                            if let Some((view_node_id, _)) =
                                ir_renderer.element_state().get_focused_element()
                            {
                                let logical_x = cursor_x / scale_factor;
                                let logical_y = cursor_y / scale_factor;
                                let viewport = zone_manager.layout.viewport;
                                let scene_x =
                                    logical_x - viewport.x + zone_manager.viewport.scroll_offset_x;
                                let scene_y =
                                    logical_y - viewport.y + zone_manager.viewport.scroll_offset_y;

                                use crate::event_handler::MouseClickEvent;
                                let release_event = MouseClickEvent {
                                    button: winit::event::MouseButton::Left,
                                    state: winit::event::ElementState::Released,
                                    x: scene_x,
                                    y: scene_y,
                                    click_count,
                                };

                                let result = ir_renderer
                                    .element_state_mut()
                                    .handle_mouse_click(&view_node_id, release_event);

                                if result.is_handled() {
                                    needs_redraw = true;
                                    window.request_redraw();
                                }
                            }
                        }

                        // End any in-progress IR element selection drag
                        // This ensures proper cleanup when mouse is released
                        if ir_renderer.element_state().is_dirty() {
                            needs_redraw = true;
                            window.request_redraw();
                        }
                    }
                }
                WindowEvent::Ime(ime_event) => {
                    // Text input arrives via IME commit events in winit 0.29+.
                    if let winit::event::Ime::Commit(text) = ime_event {
                        // Ignore text input when a command-like modifier is held to avoid inserting
                        // characters for shortcuts.
                        let has_cmd =
                            modifiers_state.contains(winit::keyboard::ModifiersState::SUPER);
                        let has_ctrl =
                            modifiers_state.contains(winit::keyboard::ModifiersState::CONTROL);
                        let has_alt =
                            modifiers_state.contains(winit::keyboard::ModifiersState::ALT);

                        if zone_manager.toolbar.address_bar.focused {
                            let mut inserted = false;
                            for ch in text.chars() {
                                if !ch.is_control() || ch == ' ' {
                                    zone_manager.toolbar.address_bar.insert_char(ch);
                                    inserted = true;
                                }
                            }
                            if inserted {
                                zone_manager.toolbar.address_bar.update_scroll();
                                needs_redraw = true;
                                window.request_redraw();
                            }
                        } else if zone_manager.chat.input.focused && zone_manager.chat.is_visible()
                        {
                            let mut inserted = false;
                            for ch in text.chars() {
                                if !ch.is_control() || ch == ' ' {
                                    zone_manager.chat.input.insert_char(ch);
                                    inserted = true;
                                }
                            }
                            if inserted {
                                zone_manager.chat.input.update_scroll();
                                needs_redraw = true;
                                window.request_redraw();
                            }
                        } else if !has_cmd && !has_ctrl && !has_alt && !text.is_empty() {
                            let result = ir_renderer.element_state_mut().handle_text_input(&text);
                            if result.is_handled() {
                                needs_redraw = true;
                                window.request_redraw();
                            }
                        }
                    }
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

                    if zone_manager.toolbar.address_bar.focused
                        && event.state == winit::event::ElementState::Pressed
                    {
                        let has_cmd = modifiers_state.contains(ModifiersState::SUPER);
                        let has_ctrl = modifiers_state.contains(ModifiersState::CONTROL);
                        let has_alt = modifiers_state.contains(ModifiersState::ALT);

                        // First, delegate navigation/editing shortcuts (selection, clipboard, undo/redo, etc.)
                        // to the shared InputBox keyboard handler so the toolbar address bar matches
                        // the behavior of in-viewport input boxes.
                        if let PhysicalKey::Code(key_code) = event.physical_key {
                            let keyboard_event = crate::event_handler::KeyboardEvent {
                                key: key_code,
                                state: event.state,
                                modifiers: modifiers_state,
                            };

                            let result = crate::event_handler::EventHandler::handle_keyboard(
                                &mut zone_manager.toolbar.address_bar,
                                keyboard_event,
                            );
                            if result.is_handled() {
                                needs_redraw = true;
                                window.request_redraw();
                            }
                        }

                        // Then handle toolbar-specific keys that InputBox doesn't know about.
                        match event.physical_key {
                            PhysicalKey::Code(KeyCode::Escape) => {
                                zone_manager.toolbar.address_bar.focused = false;
                                // Also dismiss dock if visible
                                if zone_manager.is_dock_visible() {
                                    zone_manager.hide_dock();
                                }
                                needs_redraw = true;
                                window.request_redraw();
                            }
                            PhysicalKey::Code(KeyCode::Enter) => {
                                let url = zone_manager.toolbar.address_bar.text.trim().to_string();
                                if !url.is_empty() {
                                    println!("Navigate to: {}", url);

                                    // Try to load as IR package if it's an IR URL
                                    let render_target = navigation::determine_render_target(&url);
                                    if render_target == navigation::RenderTarget::Ir {
                                        // Try to load IR package
                                        if let Some((new_data, new_view)) = try_load_ir_from_url(&url) {
                                            data_doc = new_data;
                                            view_doc = new_view;
                                            ir_renderer.element_state_mut().clear_all_focus();
                                            println!("Loaded IR package: {}", url);
                                        }
                                    }

                                    // Navigate (updates mode and queues command for CEF if needed)
                                    navigation::navigate_to(&url);

                                    // Update layout for new navigation mode
                                    let logical_width = (size.width as f32 / scale_factor) as u32;
                                    let logical_height = (size.height as f32 / scale_factor) as u32;
                                    zone_manager
                                        .update_for_navigation_mode(logical_width, logical_height);

                                    // Blur address bar after navigation
                                    zone_manager.toolbar.address_bar.focused = false;
                                }
                                needs_redraw = true;
                                window.request_redraw();
                            }
                            _ => {
                                // Fallback path for printable keys when IME commit isn't delivered.
                                if let Some(text) = &event.text {
                                    if !text.is_empty() && !has_cmd && !has_ctrl && !has_alt {
                                        let mut inserted = false;
                                        for ch in text.chars() {
                                            if !ch.is_control() || ch == ' ' {
                                                zone_manager.toolbar.address_bar.insert_char(ch);
                                                inserted = true;
                                            }
                                        }
                                        if inserted {
                                            zone_manager.toolbar.address_bar.update_scroll();
                                            needs_redraw = true;
                                            window.request_redraw();
                                        }
                                    }
                                }
                            }
                        }
                    } else if event.state == winit::event::ElementState::Pressed {
                        // Chat input keyboard handling (navigation/editing shortcuts + Enter to send)
                        if zone_manager.chat.input.focused && zone_manager.chat.is_visible() {
                            use crate::event_handler::{EventHandler, KeyboardEvent};

                            if let PhysicalKey::Code(key_code) = event.physical_key {
                                // Treat Enter as "send" for the chat panel.
                                if let KeyCode::Enter | KeyCode::NumpadEnter = key_code {
                                    let text =
                                        zone_manager.chat.input.text.trim().to_string();
                                    if !text.is_empty() {
                                        zone_manager.chat.add_user_message(text);
                                        zone_manager.chat.input.text.clear();
                                        zone_manager.chat.input.cursor_position = 0;
                                        println!("Chat message sent via Enter key");
                                    }
                                    needs_redraw = true;
                                    window.request_redraw();
                                    return;
                                }

                                let keyboard_event = KeyboardEvent {
                                    key: key_code,
                                    state: winit::event::ElementState::Pressed,
                                    modifiers: modifiers_state,
                                };

                                if zone_manager
                                    .chat
                                    .input
                                    .handle_keyboard(keyboard_event)
                                    .is_handled()
                                {
                                    needs_redraw = true;
                                    window.request_redraw();
                                    return;
                                }
                            }
                        }

                        // Global keyboard shortcuts (regardless of focus)
                        if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                            // Escape dismisses dock if visible
                            if zone_manager.is_dock_visible() {
                                zone_manager.hide_dock();
                                println!("Dock dismissed via Escape key");
                                needs_redraw = true;
                                window.request_redraw();
                            }
                        }

                        // Keyboard events for IR elements (when toolbar is not focused)
                        use crate::event_handler::KeyboardEvent;

                        // Convert winit keyboard event to our KeyboardEvent format
                        if let PhysicalKey::Code(key_code) = event.physical_key {
                            let keyboard_event = KeyboardEvent {
                                key: key_code,
                                state: winit::event::ElementState::Pressed,
                                modifiers: modifiers_state,
                            };

                            let result = ir_renderer
                                .element_state_mut()
                                .handle_keyboard(keyboard_event);
                            if result.is_handled() {
                                needs_redraw = true;
                                window.request_redraw();
                            }
                        }

                        // Fallback text input path for non-IME printable keys.
                        // Skip when command modifiers are held so shortcuts don't insert text.
                        if let Some(text) = &event.text {
                            let has_cmd =
                                modifiers_state.contains(winit::keyboard::ModifiersState::SUPER);
                            let has_ctrl =
                                modifiers_state.contains(winit::keyboard::ModifiersState::CONTROL);
                            let has_alt =
                                modifiers_state.contains(winit::keyboard::ModifiersState::ALT);
                            if !text.is_empty() && !has_cmd && !has_ctrl && !has_alt {
                                let result =
                                    ir_renderer.element_state_mut().handle_text_input(text);
                                if result.is_handled() {
                                    needs_redraw = true;
                                    window.request_redraw();
                                }
                            }
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    // Render immediately when a redraw is needed instead of debouncing
                    // window resize events. This keeps IR layout updates smooth during
                    // interactive window resizing (matching native CEF behavior).
                    let should_render = needs_redraw;

                    if should_render {
                        needs_redraw = false;
                        // no-op: no resize debounce state to reset

                        // Get current logical size
                        let logical_width = (size.width as f32 / scale_factor) as u32;
                        let logical_height = (size.height as f32 / scale_factor) as u32;

                        // Update toolbar and IR element caret blink
                        let now = Instant::now();
                        let delta_time = (now - last_frame_time).as_secs_f32();
                        let delta_ms = delta_time * 1000.0;
                        last_frame_time = now;
                        zone_manager.toolbar.address_bar.update_blink(delta_time);
                        ir_renderer
                            .element_state_mut()
                            .update_blink_animation(delta_time);

                        // Update CSS-like animations (transitions and keyframes)
                        let has_active_animations = ir_renderer.update_animations(delta_ms);

                        // Render frame with zones
                        match render_frame_with_zones(
                            &mut surf,
                            &mut ir_renderer,
                            &mut zone_manager,
                            &data_doc,
                            &view_doc,
                            &provider,
                            &surface,
                            size.width,
                            size.height,
                            logical_width,
                            logical_height,
                        ) {
                            Ok(index) => {
                                hit_index = Some(index);
                                // Keep redraws flowing while:
                                // - Address bar or IR element is focused (caret blink)
                                // - CSS-like animations are active
                                if zone_manager.toolbar.address_bar.focused
                                    || ir_renderer.element_state().get_focused_element().is_some()
                                    || has_active_animations
                                {
                                    needs_redraw = true;
                                }
                            }
                            Err(e) => {
                                eprintln!("Render error: {}", e);
                            }
                        }

                        // No resize debounce state to reset.
                    }
                }
                _ => {}
            },
            Event::AboutToWait => {
                if needs_redraw {
                    window.request_redraw();
                }
            }
            Event::LoopExiting => {
                if let Err(err) = window_state.save() {
                    warn!(?err, "failed to persist window state on exit");
                }
            }
            _ => {}
        }
    })?;

    Ok(())
}

/// Load IR package from CLI path, config, or default to home_tab sample.
///
/// Priority:
/// 1. CLI argument (directory path)
/// 2. Config file `package_path` from rune.toml
/// 3. Built-in home_tab sample
fn load_ir_package() -> Result<(DataDocument, ViewDocument)> {
    // Priority 1: Try CLI argument first
    let mut args = std::env::args();
    let _bin = args.next(); // Skip binary name

    if let Some(path) = args.next() {
        eprintln!("Loading IR package from CLI path: {}", path);
        match load_package_from_path(&path) {
            Ok((data, view)) => {
                eprintln!("✓ Successfully loaded package from: {}", path);
                return Ok((data, view));
            }
            Err(e) => {
                eprintln!("✗ Failed to load package from '{}': {}", path, e);
                eprintln!("Falling back to config or default...");
            }
        }
    }

    // Priority 2: Try loading from rune.toml config
    let config = rune_config::RuneConfig::load();
    if let Some(package_path) = &config.ir.package_path {
        eprintln!("Loading IR package from config: {:?}", package_path);
        match load_package_from_path(package_path.to_str().unwrap_or("")) {
            Ok((data, view)) => {
                eprintln!(
                    "✓ Successfully loaded package from config: {:?}",
                    package_path
                );
                return Ok((data, view));
            }
            Err(e) => {
                eprintln!(
                    "✗ Failed to load package from config '{:?}': {}",
                    package_path, e
                );
                eprintln!("Falling back to default home_tab sample...");
            }
        }
    }

    // Priority 3: Load default home_tab sample
    eprintln!("Loading default home_tab sample...");
    load_default_package()
}

/// Load IR package from a directory path.
fn load_package_from_path(path: &str) -> Result<(DataDocument, ViewDocument)> {
    let dir = std::path::Path::new(path);
    let package = rune_ir::package::RunePackage::from_directory(dir)?;
    let (data, view) = package.entrypoint_documents()?;
    Ok((data.clone(), view.clone()))
}

/// Load the default home_tab sample package.
fn load_default_package() -> Result<(DataDocument, ViewDocument)> {
    let package = rune_ir::package::RunePackage::sample()?;
    let (data, view) = package.entrypoint_documents()?;
    eprintln!("✓ Successfully loaded home_tab sample");
    Ok((data.clone(), view.clone()))
}

/// Try to load an IR package from a URL.
///
/// Supported URL patterns:
/// - `rune://home` or `rune://peco` → home_tab sample
/// - `rune://sample/first-node` → examples/sample_first_node
/// - `rune://sample/webview` → examples/sample_webview
/// - `rune://sample/form` → examples/sample_form
/// - `ir://path/to/package` → load from filesystem path
/// - `file:///path/to/package` → load from filesystem path
///
/// Returns None if the URL doesn't match a known IR package or loading fails.
fn try_load_ir_from_url(url: &str) -> Option<(DataDocument, ViewDocument)> {
    let url_lower = url.to_lowercase();

    // Handle rune:// scheme
    if url_lower.starts_with("rune://") {
        let path = &url[7..]; // Strip "rune://"

        // Home/Peco - load default sample
        if path == "home" || path == "peco" || path.is_empty() {
            return load_default_package().ok();
        }

        // Sample packages
        if path.starts_with("sample/") || path.starts_with("samples/") {
            let sample_name = path.split('/').nth(1).unwrap_or("");
            return load_sample_package(sample_name);
        }

        // Direct sample name shortcuts
        match path {
            "first-node" | "firstnode" => return load_sample_package("first-node"),
            "webview" => return load_sample_package("webview"),
            "form" => return load_sample_package("form"),
            _ => {}
        }
    }

    // Handle ir:// scheme - treat as filesystem path
    if url_lower.starts_with("ir://") {
        let path = &url[5..]; // Strip "ir://"
        return load_package_from_path(path).ok();
    }

    // Handle file:// scheme
    if url_lower.starts_with("file://") {
        let path = &url[7..]; // Strip "file://"
        return load_package_from_path(path).ok();
    }

    // Handle direct filesystem paths (for development)
    if url.starts_with('/') || url.starts_with("examples/") || url.contains("/sample_") {
        return load_package_from_path(url).ok();
    }

    None
}

/// Load a sample package by name.
fn load_sample_package(name: &str) -> Option<(DataDocument, ViewDocument)> {
    let sample_dir = match name {
        "first-node" | "firstnode" | "first_node" => "examples/sample_first_node",
        "webview" | "web-view" => "examples/sample_webview",
        "form" => "examples/sample_form",
        _ => {
            eprintln!("Unknown sample package: {}", name);
            return None;
        }
    };

    match load_package_from_path(sample_dir) {
        Ok((data, view)) => {
            eprintln!("✓ Loaded sample package: {} from {}", name, sample_dir);
            Some((data, view))
        }
        Err(e) => {
            eprintln!("✗ Failed to load sample package '{}': {}", name, e);
            None
        }
    }
}

/// Create text provider (uses system fonts with RGB subpixel rendering).
fn create_text_provider() -> Result<impl engine_core::TextProvider> {
    engine_core::RuneTextProvider::from_system_fonts(engine_core::SubpixelOrientation::RGB)
        .context("Failed to load system fonts")
}

// Use the shared render_zones function from the zones module
use crate::zones::render_zones;

/// Render a single frame with zones (toolbar, sidebar, viewport, devtools).
///
/// This is the main rendering function for the full application UI.
/// IR content is rendered within the viewport zone bounds.
///
/// Returns a HitIndex for hit testing interactive elements.
pub fn render_frame_with_zones(
    surf: &mut rune_surface::RuneSurface,
    ir_renderer: &mut IrRenderer,
    zone_manager: &mut crate::zones::ZoneManager,
    data_doc: &DataDocument,
    view_doc: &ViewDocument,
    provider: &std::sync::Arc<dyn engine_core::TextProvider>,
    surface: &wgpu::Surface,
    phys_width: u32,
    phys_height: u32,
    _logical_width: u32,
    _logical_height: u32,
) -> Result<HitIndex> {
    use engine_core::Rect;

    // Get current frame
    let frame = surface.get_current_texture()?;

    // Create canvas for this frame (uses physical pixels)
    let mut canvas = surf.begin_frame(phys_width, phys_height);
    canvas.set_text_provider(provider.clone());

    // Clear with background color
    canvas.clear(engine_core::ColorLinPremul::from_srgba_u8([
        32, 32, 36, 255,
    ]));

    // Render zones
    render_zones(&mut canvas, zone_manager, provider.as_ref());

    // Render IR content within viewport zone bounds
    let viewport_rect = zone_manager.layout.viewport;

    // Translate into viewport-local space, clip locally, and apply scroll via transform
    // so geometry and hit regions share the same transform stack (mirrors lib_old).
    canvas.push_transform(Transform2D::translate(viewport_rect.x, viewport_rect.y));
    canvas.push_clip_rect(Rect {
        x: 0.0,
        y: 0.0,
        w: viewport_rect.w,
        h: viewport_rect.h,
    });
    canvas.push_transform(Transform2D::translate(
        -zone_manager.viewport.scroll_offset_x,
        -zone_manager.viewport.scroll_offset_y,
    ));

    // Compute layout using the visible viewport height so coordinates stay anchored
    // to the viewport instead of feeding back the previous frame's content height.
    let layout_height = viewport_rect.h;
    let (content_height, content_width) = ir_renderer.render_canvas_at_offset(
        &mut canvas,
        data_doc,
        view_doc,
        0.0,
        0.0,
        viewport_rect.w,
        layout_height,
        viewport_rect.h,
        zone_manager.viewport.scroll_offset_x,
        zone_manager.viewport.scroll_offset_y,
        provider.as_ref(),
    )?;

    canvas.pop_transform(); // scroll
    canvas.pop_clip();
    canvas.pop_transform(); // viewport

    zone_manager
        .viewport
        .set_content_size(content_width, content_height, viewport_rect.w, viewport_rect.h);

    // Render toolbar with navigation controls and address bar,
    // matching the legacy implementation's layout behavior.
    //
    // Use toolbar-local coordinates for hit regions and icons by
    // translating the canvas to the toolbar origin before rendering.
    //
    // Toolbar is only visible in Browser mode.
    if crate::navigation::should_show_toolbar() {
        use crate::zones::ZoneId;
        use engine_core::Transform2D;

        let toolbar_rect = zone_manager.layout.get_zone(ZoneId::Toolbar);

        // Get navigation state for back/forward button styling
        let nav_state = crate::navigation::get_state();

        // Render toolbar with transform for visual positioning
        // but pass the GLOBAL toolbar_rect so hit regions use global coordinates
        canvas.push_transform(Transform2D::translate(toolbar_rect.x, toolbar_rect.y));

        zone_manager.toolbar.render(
            &mut canvas,
            toolbar_rect,
            provider.as_ref(),
            nav_state.can_go_back,
            nav_state.can_go_forward,
        );

        canvas.pop_transform();
    }

    // Render sidebar with tabs and bookmarks when visible
    if zone_manager.sidebar.is_visible() {
        use crate::zones::ZoneId;
        use engine_core::Transform2D;

        let sidebar_rect = zone_manager.layout.get_zone(ZoneId::Sidebar);

        // Render sidebar content in sidebar-local coordinates
        canvas.push_transform(Transform2D::translate(sidebar_rect.x, sidebar_rect.y));

        zone_manager
            .sidebar
            .render(&mut canvas, sidebar_rect, provider.as_ref());

        canvas.pop_transform();
    }

    // Render devtools overlay when visible (mirrors legacy visuals and hit regions).
    if zone_manager.is_devtools_visible() {
        use crate::zones::{
            ConsoleLevel, DEVTOOLS_CLOSE_BUTTON_REGION_ID, DEVTOOLS_CONSOLE_TAB_REGION_ID,
            DEVTOOLS_ELEMENTS_TAB_REGION_ID, DevToolsTab, ZoneId,
        };
        use engine_core::{Brush, Color, ColorLinPremul, SvgStyle, Transform2D};

        let devtools_rect = zone_manager.layout.get_zone(ZoneId::DevTools);
        canvas.push_transform(Transform2D::translate(devtools_rect.x, devtools_rect.y));

        let devtools_style = &zone_manager.devtools.style;
        let white = ColorLinPremul::rgba(255, 255, 255, 255);
        let inactive_color = ColorLinPremul::rgba(160, 170, 180, 255);
        let header_bg = ColorLinPremul::rgba(34, 41, 60, 255);
        let active_tab_bg = ColorLinPremul::rgba(54, 61, 80, 255);
        let inactive_tab_bg = ColorLinPremul::rgba(40, 47, 66, 255);
        let white_stroke = Color::rgba(255, 255, 255, 255);
        let inactive_stroke = Color::rgba(160, 170, 180, 255);

        let button_size = 18.0;
        let tab_height = 24.0;
        let tab_padding = 10.0;
        let icon_text_gap = 6.0;
        let header_height = tab_height + 8.0;

        // Panel background
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

            canvas.fill_rect(0.0, 0.0, devtools_rect.w, bw, border_brush.clone(), 10100);
            canvas.fill_rect(
                0.0,
                devtools_rect.h - bw,
                devtools_rect.w,
                bw,
                border_brush.clone(),
                10100,
            );
            canvas.fill_rect(0.0, 0.0, bw, devtools_rect.h, border_brush.clone(), 10100);
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
        let elements_tab_width = button_size + icon_text_gap + 8.0 + 54.0 + tab_padding * 3.0;

        let elements_rect = engine_core::Rect {
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

        canvas.fill_rect(
            elements_rect.x,
            elements_rect.y,
            elements_rect.w,
            elements_rect.h,
            Brush::Solid(elements_bg),
            10120,
        );
        canvas.hit_region_rect(DEVTOOLS_ELEMENTS_TAB_REGION_ID, elements_rect, 10300);

        // Elements tab icon (matches legacy overlay)
        let elements_icon_style = SvgStyle::new()
            .with_stroke(if is_elements_active {
                white_stroke
            } else {
                inactive_stroke
            })
            .with_stroke_width(2.0);
        let elements_icon_x = elements_x + 8.0;
        let elements_icon_y = elements_y + (tab_height - button_size) * 0.5;
        canvas.draw_svg_styled(
            "images/square-mouse-pointer.svg",
            [elements_icon_x, elements_icon_y],
            [button_size, button_size],
            elements_icon_style,
            10250,
        );

        // Console tab
        let console_x = elements_x + elements_tab_width + 8.0;
        let console_y = (tab_height - button_size) * 0.5;
        let console_tab_width = button_size + icon_text_gap + 8.0 + 50.0 + tab_padding * 3.0;

        let console_rect = engine_core::Rect {
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

        canvas.fill_rect(
            console_rect.x,
            console_rect.y,
            console_rect.w,
            console_rect.h,
            Brush::Solid(console_bg),
            10120,
        );
        canvas.hit_region_rect(DEVTOOLS_CONSOLE_TAB_REGION_ID, console_rect, 10300);

        // Console tab icon
        let console_icon_style = SvgStyle::new()
            .with_stroke(if is_console_active {
                white_stroke
            } else {
                inactive_stroke
            })
            .with_stroke_width(2.0);
        let console_icon_x = console_x + 8.0;
        let console_icon_y = console_y + (tab_height - button_size) * 0.5;
        canvas.draw_svg_styled(
            "images/square-terminal.svg",
            [console_icon_x, console_icon_y],
            [button_size, button_size],
            console_icon_style,
            10250,
        );

        // Close button
        let close_size = 20.0;
        let close_margin = 12.0;
        let close_x = devtools_rect.w - close_size - close_margin;
        let close_y = 6.0;
        let close_rect = engine_core::Rect {
            x: close_x,
            y: close_y,
            w: close_size,
            h: close_size,
        };
        canvas.hit_region_rect(DEVTOOLS_CLOSE_BUTTON_REGION_ID, close_rect, 10300);
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

        // Simple labels for tabs and content
        let text_y = elements_y + tab_height * 0.5 + 4.0;
        canvas.draw_text_run(
            [elements_x + button_size + icon_text_gap + 8.0, text_y],
            "Elements".to_string(),
            11.0,
            elements_color,
            10260,
        );
        canvas.draw_text_run(
            [console_x + button_size + icon_text_gap + 8.0, text_y],
            "Console".to_string(),
            11.0,
            console_color,
            10260,
        );

        let label_color: ColorLinPremul = ColorLinPremul::rgba(220, 230, 240, 255);

        // Content area background (below header)
        let content_y = header_height;
        let content_h = (devtools_rect.h - header_height).max(0.0);
        canvas.fill_rect(
            0.0,
            content_y,
            devtools_rect.w,
            content_h,
            Brush::Solid(ColorLinPremul::rgba(28, 34, 48, 255)),
            10105,
        );

        match active_tab {
            DevToolsTab::Elements => {
                canvas.draw_text_run(
                    [tab_padding + 4.0, header_height + 14.0],
                    "Elements (stub)".to_string(),
                    12.0,
                    label_color,
                    10260,
                );
            }
            DevToolsTab::Console => {
                // Simple console log view: render each entry on its own line.
                let line_height = 14.0;
                let mut y = header_height + 4.0;
                let left_margin = tab_padding + 4.0;

                for entry in zone_manager.devtools.console_entries.iter() {
                    let (icon, color) = match entry.level {
                        ConsoleLevel::Warn => ("⚠", ColorLinPremul::rgba(255, 200, 120, 255)),
                        ConsoleLevel::Error => ("⨯", ColorLinPremul::rgba(255, 140, 140, 255)),
                        ConsoleLevel::Log => ("•", label_color),
                    };

                    canvas.draw_text_run(
                        [left_margin, y],
                        icon.to_string(),
                        11.0,
                        color,
                        10260,
                    );
                    canvas.draw_text_run(
                        [left_margin + 16.0, y],
                        entry.message.clone(),
                        11.0,
                        label_color,
                        10260,
                    );

                    y += line_height;
                    if y > devtools_rect.h - 4.0 {
                        break;
                    }
                }
            }
        }

        canvas.pop_transform();
    }

    // Render chat panel when visible
    if zone_manager.is_chat_visible() {
        let chat_rect = zone_manager.layout.chat;
        canvas.push_transform(Transform2D::translate(chat_rect.x, chat_rect.y));
        zone_manager.chat.render(&mut canvas, chat_rect, provider.as_ref());
        canvas.pop_transform();
    }

    // Render FAB when chat is hidden and not in Home mode
    if !zone_manager.is_chat_visible() && crate::navigation::get_navigation_mode() != crate::navigation::NavigationMode::Home {
        let viewport_rect = zone_manager.layout.viewport;
        canvas.push_transform(Transform2D::translate(viewport_rect.x, viewport_rect.y));
        crate::zones::render_chat_fab(&mut canvas, viewport_rect, zone_manager.chat.has_unread);
        canvas.pop_transform();
    }

    // Render dock overlay when visible (rendered last to be on top of everything)
    if zone_manager.is_dock_visible() {
        let logical_width = _logical_width as f32;
        let logical_height = _logical_height as f32;
        zone_manager.dock.render(&mut canvas, logical_width, logical_height, provider.as_ref());
    }

    // Build hit index for interactive regions (toolbar buttons, address bar, devtools, dock, chat)
    let hit_index = HitIndex::build(canvas.display_list());

    // End frame and present
    surf.end_frame(frame, canvas)?;

    Ok(hit_index)
}

/// Render a single frame using Canvas and IR documents (standalone, no zones).
///
/// This function delegates rendering to self-contained elements that handle
/// their own drawing and will handle their own events (via event_router).
#[allow(dead_code)]
fn render_frame(
    surf: &mut rune_surface::RuneSurface,
    ir_renderer: &mut IrRenderer,
    data_doc: &DataDocument,
    view_doc: &ViewDocument,
    provider: &std::sync::Arc<dyn engine_core::TextProvider>,
    surface: &wgpu::Surface,
    phys_width: u32,
    phys_height: u32,
    logical_width: u32,
    logical_height: u32,
) -> Result<()> {
    // Get current frame
    let frame = surface.get_current_texture()?;

    // Create canvas for this frame (uses physical pixels)
    let mut canvas = surf.begin_frame(phys_width, phys_height);
    canvas.set_text_provider(provider.clone());
    canvas.clear(engine_core::ColorLinPremul::from_srgba_u8([
        245, 245, 250, 255,
    ]));

    // Render IR documents using element-based rendering
    // Each element is self-contained and handles its own rendering + events
    let _ = ir_renderer.render_canvas(
        &mut canvas,
        data_doc,
        view_doc,
        logical_width as f32,
        logical_height as f32,
        provider.as_ref(),
    )?;

    // End frame and present
    surf.end_frame(frame, canvas)?;

    Ok(())
}
