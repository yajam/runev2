//! FFI exports for embedding rune-scene in native macOS applications.
//!
//! This crate provides C-compatible functions for initializing and running
//! the rune-scene IR renderer from an Xcode/macOS application.
//!
//! This crate directly uses `render_frame_with_zones` from rune-scene to
//! render the full app UI with toolbar, sidebar, viewport, and devtools.
//! This ensures feature parity with the standalone rune-scene app including
//! hit testing, devtools, and all interactive features.
//!
//! ## WebView Integration
//!
//! CEF (Chromium Embedded Framework) is managed by the Xcode side. The pixel
//! data from CEF's OnPaint callback is uploaded via `rune_ffi_upload_webview_pixels`
//! to a texture that is then rendered as part of the WebView element.

pub mod ffi;

use anyhow::{Context, Result};
use engine_core::HitIndex;
use rune_ir::{data::document::DataDocument, view::ViewDocument};
use rune_scene::ir_renderer::{render_frame_with_zones, IrRenderer};
use rune_scene::zones::ZoneManager;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Application renderer state
pub struct AppRenderer {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    surf: rune_surface::RuneSurface,
    provider: Arc<dyn engine_core::TextProvider>,
    data_doc: DataDocument,
    view_doc: ViewDocument,
    ir_renderer: IrRenderer,
    zone_manager: ZoneManager,
    hit_index: Option<HitIndex>,
    width: u32,
    height: u32,
    logical_width: u32,
    logical_height: u32,
    scale_factor: f32,
    needs_redraw: bool,
    last_click_time: Option<Instant>,
    click_count: u32,
}

impl AppRenderer {
    /// Create a new renderer with a CAMetalLayer
    pub fn new(
        width: u32,
        height: u32,
        scale: f32,
        metal_layer: *mut std::ffi::c_void,
        package_path: Option<&str>,
    ) -> Result<Self> {
        log::info!(
            "AppRenderer::new width={} height={} scale={}",
            width,
            height,
            scale
        );

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..Default::default()
        });

        // Create surface from CAMetalLayer
        let surface = unsafe {
            let target = wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(metal_layer);
            instance.create_surface_unsafe(target)?
        };

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .context("No suitable adapter")?;

        log::info!("Using adapter: {:?}", adapter.get_info());

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("rune-ffi-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        ))?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .first()
            .copied()
            .unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Set up RuneSurface. We render IR content in logical pixels scaled by
        // the device DPI; CEF WebView pixels are composited via the unified
        // image pipeline using `Canvas::draw_raw_image` from rune-scene.
        let mut surf = rune_surface::RuneSurface::new(device, queue, format);
        surf.set_use_intermediate(true);
        surf.set_logical_pixels(true);
        surf.set_dpi_scale(scale);

        // Set up text provider
        let provider: Arc<dyn engine_core::TextProvider> = Arc::new(
            engine_core::RuneTextProvider::from_system_fonts(engine_core::SubpixelOrientation::RGB)
                .context("Failed to load system fonts")?,
        );

        // Load IR documents
        let (data_doc, view_doc) = load_ir_package(package_path)?;
        log::info!(
            "Loaded IR package: data={}, view={}",
            data_doc.document_id,
            view_doc.view_id
        );

        // Initialize navigation state to Home/IR mode since we start with the IR home page
        rune_scene::navigation::navigate_home();

        let logical_width = (width as f32 / scale) as u32;
        let logical_height = (height as f32 / scale) as u32;

        // Create IR renderer (from rune-scene)
        let ir_renderer = IrRenderer::new();

        // Create zone manager (from rune-scene) for full app layout
        let zone_manager = ZoneManager::new(logical_width, logical_height);
        log::info!(
            "Zone layout: viewport={:?}",
            zone_manager.layout.viewport
        );

        Ok(Self {
            surface,
            config,
            surf,
            provider,
            data_doc,
            view_doc,
            ir_renderer,
            zone_manager,
            hit_index: None,
            width,
            height,
            logical_width,
            logical_height,
            scale_factor: scale,
            needs_redraw: true,
            last_click_time: None,
            click_count: 0,
        })
    }

    /// Resize the surface
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        log::debug!("AppRenderer::resize width={} height={}", width, height);

        self.width = width;
        self.height = height;
        self.logical_width = (width as f32 / self.scale_factor) as u32;
        self.logical_height = (height as f32 / self.scale_factor) as u32;
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(self.surf.device().as_ref(), &self.config);

        // Pre-allocate the intermediate render target for the new size so
        // window resizing stays smooth and avoids repeated allocations.
        self.surf.prepare_for_resize(width, height);

        // Update zone manager layout
        self.zone_manager
            .resize(self.logical_width, self.logical_height);

        self.needs_redraw = true;
    }

    /// Try to navigate to a URL, loading IR package if applicable
    pub fn navigate_to_url(&mut self, url: &str) {
        use rune_ir::view::ViewNodeKind;
        use rune_scene::navigation;

        // Decide whether this URL should be handled by IR or CEF.
        let render_target = navigation::determine_render_target(url);

        match render_target {
            navigation::RenderTarget::Ir => {
                // IR URLs load IR packages directly (home_tab, sample apps, etc.).
                if let Some((new_data, new_view)) = try_load_ir_from_url(url) {
                    self.data_doc = new_data;
                    self.view_doc = new_view;
                    self.ir_renderer.element_state_mut().clear_all_focus();
                    log::info!("Loaded IR package for URL: {}", url);
                }
            }
            navigation::RenderTarget::Cef => {
                // CEF URLs should be shown in the browser host IR view (sample_webview)
                // so that the native CEF NSView has a WebView container/viewport.
                let has_webview = self
                    .view_doc
                    .nodes
                    .iter()
                    .any(|n| matches!(n.kind, ViewNodeKind::WebView(_)));

                if !has_webview {
                    let host_url = "rune://sample/webview";
                    if let Some((new_data, new_view)) = try_load_ir_from_url(host_url) {
                        self.data_doc = new_data;
                        self.view_doc = new_view;
                        self.ir_renderer.element_state_mut().clear_all_focus();
                        log::info!(
                            "Loaded browser host IR package '{}' for CEF URL: {}",
                            host_url,
                            url
                        );
                    } else {
                        log::warn!(
                            "Failed to load browser host IR package for CEF URL: {}",
                            url
                        );
                    }
                }
            }
        }

        // Update navigation state (mode + command queue for CEF).
        navigation::navigate_to(url);

        // Recompute zone layout for the new navigation mode (toolbar visibility, etc.).
        self.zone_manager
            .update_for_navigation_mode(self.logical_width, self.logical_height);

        self.needs_redraw = true;
    }

    /// Render a frame using the same function as rune-scene
    pub fn render(&mut self) {
        // Clear webview rect at start of frame - it will be set if a webview element is rendered
        rune_scene::elements::webview::clear_webview_rect();

        // Use the exact same render function as rune-scene
        match render_frame_with_zones(
            &mut self.surf,
            &mut self.ir_renderer,
            &mut self.zone_manager,
            &self.data_doc,
            &self.view_doc,
            &self.provider,
            &self.surface,
            self.width,
            self.height,
            self.logical_width,
            self.logical_height,
        ) {
            Ok(index) => {
                self.hit_index = Some(index);
            }
            Err(e) => {
                log::error!("Render error: {:?}", e);
            }
        }

        self.needs_redraw = false;
    }

    /// Handle mouse click event
    pub fn mouse_click(&mut self, x: f32, y: f32, pressed: bool) {
        use rune_scene::zones::{
            ADDRESS_BAR_REGION_ID, BACK_BUTTON_REGION_ID, DEVTOOLS_BUTTON_REGION_ID,
            DEVTOOLS_CLOSE_BUTTON_REGION_ID, DEVTOOLS_CONSOLE_TAB_REGION_ID,
            DEVTOOLS_ELEMENTS_TAB_REGION_ID, DevToolsTab, FORWARD_BUTTON_REGION_ID,
            HOME_BUTTON_REGION_ID, REFRESH_BUTTON_REGION_ID, TOGGLE_BUTTON_REGION_ID,
            SIDEBAR_BOOKMARK_REGION_BASE, SIDEBAR_TAB_REGION_BASE,
            SIDEBAR_ADD_BOOKMARK_REGION_ID, SIDEBAR_TAB_CLOSE_REGION_BASE,
            SIDEBAR_BOOKMARK_DELETE_REGION_BASE,
            DOCK_SCRIM_REGION_ID, DOCK_PANEL_REGION_ID, DOCK_PINNED_APP_REGION_BASE,
            CHAT_BUTTON_REGION_ID, CHAT_FAB_REGION_ID, CHAT_CLOSE_BUTTON_REGION_ID,
            CHAT_INPUT_REGION_ID, CHAT_SEND_BUTTON_REGION_ID,
        };
        use rune_scene::event_handler::MouseClickEvent;
        use winit::event::{ElementState, MouseButton};

        let logical_x = x / self.scale_factor;
        let logical_y = y / self.scale_factor;

        if pressed {
            // Multi-click tracking (single/double/triple click)
            let now = Instant::now();
            let double_click_threshold = Duration::from_millis(500);
            let is_quick_click = self
                .last_click_time
                .map(|t| now.duration_since(t) < double_click_threshold)
                .unwrap_or(false);
            if is_quick_click {
                self.click_count += 1;
            } else {
                self.click_count = 1;
            }
            self.last_click_time = Some(now);

            // Use hit index for hit testing
            if let Some(ref index) = self.hit_index {
                // Compute scene-local coordinates for IR elements (viewport space)
                let viewport = self.zone_manager.layout.viewport;
                let scene_x =
                    logical_x - viewport.x + self.zone_manager.viewport.scroll_offset_x;
                let scene_y =
                    logical_y - viewport.y + self.zone_manager.viewport.scroll_offset_y;

                // Close open select/date pickers unless click is on their overlay
                self.ir_renderer
                    .element_state_mut()
                    .close_selects_except_at_point(scene_x, scene_y);
                self.ir_renderer
                    .element_state_mut()
                    .close_date_pickers_except_at_point(scene_x, scene_y);

                if let Some(hit) = index.topmost_at([logical_x, logical_y]) {
                    log::debug!(
                        "Hit at ({}, {}): region_id={:?}, z={}",
                        logical_x,
                        logical_y,
                        hit.region_id,
                        hit.z
                    );

                    if let Some(region_id) = hit.region_id {
                        log::debug!(
                            "Dispatch: region_id={} TOGGLE={} DEVTOOLS={} ADDRESS={}",
                            region_id,
                            TOGGLE_BUTTON_REGION_ID,
                            DEVTOOLS_BUTTON_REGION_ID,
                            ADDRESS_BAR_REGION_ID
                        );

                        match region_id {
                            TOGGLE_BUTTON_REGION_ID => {
                                // Blur any focused fields when clicking toolbar chrome
                                if self.zone_manager.toolbar.address_bar.focused {
                                    self.zone_manager.toolbar.address_bar.focused = false;
                                    self.zone_manager
                                        .toolbar
                                        .address_bar
                                        .end_mouse_selection();
                                }
                                self.ir_renderer
                                    .element_state_mut()
                                    .clear_all_focus();

                                self.zone_manager
                                    .toggle_sidebar(self.logical_width, self.logical_height);
                                self.needs_redraw = true;
                            }
                            HOME_BUTTON_REGION_ID => {
                                // Home button opens the dock overlay
                                if self.zone_manager.toolbar.address_bar.focused {
                                    self.zone_manager.toolbar.address_bar.focused = false;
                                    self.zone_manager
                                        .toolbar
                                        .address_bar
                                        .end_mouse_selection();
                                }
                                self.ir_renderer
                                    .element_state_mut()
                                    .clear_all_focus();

                                log::info!("Home button clicked - opening dock");
                                self.zone_manager.show_dock();
                                self.needs_redraw = true;
                            }
                            DOCK_SCRIM_REGION_ID => {
                                // Clicking scrim dismisses the dock
                                self.zone_manager.hide_dock();
                                log::info!("Dock dismissed via scrim");
                                self.needs_redraw = true;
                            }
                            DOCK_PANEL_REGION_ID => {
                                // Clicking panel itself does nothing (prevents click-through)
                            }
                            // Dock pinned apps (region IDs 5100+)
                            id if id >= DOCK_PINNED_APP_REGION_BASE && id < DOCK_PINNED_APP_REGION_BASE + 100 => {
                                let app_index = (id - DOCK_PINNED_APP_REGION_BASE) as usize;
                                if let Some(app) = self.zone_manager.dock.pinned_apps.get(app_index) {
                                    let url = app.url.clone();
                                    let name = app.name.clone();
                                    log::info!("Navigate to pinned app: {} -> {}", name, url);
                                    self.zone_manager.hide_dock();
                                    // Use navigate_to_url which handles IR package loading
                                    self.navigate_to_url(&url);
                                }
                            }
                            // Chat button in toolbar
                            CHAT_BUTTON_REGION_ID => {
                                if self.zone_manager.toolbar.address_bar.focused {
                                    self.zone_manager.toolbar.address_bar.focused = false;
                                    self.zone_manager.toolbar.address_bar.end_mouse_selection();
                                }
                                self.ir_renderer.element_state_mut().clear_all_focus();

                                log::info!("Chat button clicked - toggling chat panel");
                                self.zone_manager.toggle_chat(self.logical_width, self.logical_height);
                                self.needs_redraw = true;
                            }
                            // Chat FAB (floating action button)
                            CHAT_FAB_REGION_ID => {
                                log::info!("Chat FAB clicked - opening chat panel");
                                self.zone_manager.show_chat(self.logical_width, self.logical_height);
                                self.needs_redraw = true;
                            }
                            // Chat close button
                            CHAT_CLOSE_BUTTON_REGION_ID => {
                                log::info!("Chat close button clicked");
                                self.zone_manager.hide_chat(self.logical_width, self.logical_height);
                                self.needs_redraw = true;
                            }
                            // Chat input region
                            CHAT_INPUT_REGION_ID => {
                                self.zone_manager.chat.input.focused = true;
                                log::info!("Chat input focused");
                                self.needs_redraw = true;
                            }
                            // Chat send button
                            CHAT_SEND_BUTTON_REGION_ID => {
                                let text = self.zone_manager.chat.input.text.trim().to_string();
                                if !text.is_empty() {
                                    self.zone_manager.chat.add_user_message(text);
                                    self.zone_manager.chat.input.text.clear();
                                    self.zone_manager.chat.input.cursor_position = 0;
                                    log::info!("Chat message sent");
                                }
                                self.needs_redraw = true;
                            }
                            DEVTOOLS_BUTTON_REGION_ID => {
                                if self.zone_manager.toolbar.address_bar.focused {
                                    self.zone_manager.toolbar.address_bar.focused = false;
                                    self.zone_manager
                                        .toolbar
                                        .address_bar
                                        .end_mouse_selection();
                                }
                                self.ir_renderer
                                    .element_state_mut()
                                    .clear_all_focus();
                                // Always toggle the Rune DevTools zone. When a native CEF view
                                // is active, the macOS side will shrink the CEF NSView so the
                                // IR-rendered DevTools panel is visible instead of opening the
                                // Chrome DevTools popup.
                                self.zone_manager.toggle_devtools();
                                self.needs_redraw = true;
                            }
                            BACK_BUTTON_REGION_ID => {
                                if self.zone_manager.toolbar.address_bar.focused {
                                    self.zone_manager.toolbar.address_bar.focused = false;
                                    self.zone_manager
                                        .toolbar
                                        .address_bar
                                        .end_mouse_selection();
                                }
                                self.ir_renderer
                                    .element_state_mut()
                                    .clear_all_focus();

                                log::info!("Back button clicked");
                                rune_scene::navigation::go_back();
                                self.needs_redraw = true;
                            }
                            FORWARD_BUTTON_REGION_ID => {
                                if self.zone_manager.toolbar.address_bar.focused {
                                    self.zone_manager.toolbar.address_bar.focused = false;
                                    self.zone_manager
                                        .toolbar
                                        .address_bar
                                        .end_mouse_selection();
                                }
                                self.ir_renderer
                                    .element_state_mut()
                                    .clear_all_focus();

                                log::info!("Forward button clicked");
                                rune_scene::navigation::go_forward();
                                self.needs_redraw = true;
                            }
                            REFRESH_BUTTON_REGION_ID => {
                                if self.zone_manager.toolbar.address_bar.focused {
                                    self.zone_manager.toolbar.address_bar.focused = false;
                                    self.zone_manager
                                        .toolbar
                                        .address_bar
                                        .end_mouse_selection();
                                }
                                self.ir_renderer
                                    .element_state_mut()
                                    .clear_all_focus();

                                // If loading, stop; otherwise refresh
                                if self.zone_manager.toolbar.is_loading {
                                    log::info!("Stop button clicked");
                                    rune_scene::navigation::stop();
                                } else {
                                    log::info!("Refresh button clicked");
                                    rune_scene::navigation::reload();
                                }
                                self.needs_redraw = true;
                            }
                            ADDRESS_BAR_REGION_ID => {
                                use rune_scene::zones::ZoneId;

                                let toolbar_rect =
                                    self.zone_manager.layout.get_zone(ZoneId::Toolbar);
                                let local_x = logical_x - toolbar_rect.x;
                                let local_y = logical_y - toolbar_rect.y;

                                // Blur any IR element focus when switching to address bar
                                self.ir_renderer
                                    .element_state_mut()
                                    .clear_all_focus();

                                self.zone_manager.toolbar.address_bar.focused = true;

                                if self.click_count == 3 {
                                    self.zone_manager
                                        .toolbar
                                        .address_bar
                                        .start_line_selection(local_x, local_y);
                                } else if self.click_count == 2 {
                                    self.zone_manager
                                        .toolbar
                                        .address_bar
                                        .start_word_selection(local_x, local_y);
                                } else {
                                    self.zone_manager
                                        .toolbar
                                        .address_bar
                                        .start_mouse_selection(local_x, local_y);
                                }
                                self.zone_manager.toolbar.address_bar.update_scroll();
                                self.needs_redraw = true;
                            }
                            DEVTOOLS_CLOSE_BUTTON_REGION_ID => {
                                if self.zone_manager.toolbar.address_bar.focused {
                                    self.zone_manager.toolbar.address_bar.focused = false;
                                    self.zone_manager
                                        .toolbar
                                        .address_bar
                                        .end_mouse_selection();
                                }
                                self.ir_renderer
                                    .element_state_mut()
                                    .clear_all_focus();

                                self.zone_manager.toggle_devtools();
                                self.needs_redraw = true;
                            }
                            DEVTOOLS_ELEMENTS_TAB_REGION_ID => {
                                if self.zone_manager.toolbar.address_bar.focused {
                                    self.zone_manager.toolbar.address_bar.focused = false;
                                    self.zone_manager
                                        .toolbar
                                        .address_bar
                                        .end_mouse_selection();
                                }
                                self.ir_renderer
                                    .element_state_mut()
                                    .clear_all_focus();

                                self.zone_manager
                                    .devtools
                                    .set_active_tab(DevToolsTab::Elements);
                                self.needs_redraw = true;
                            }
                            DEVTOOLS_CONSOLE_TAB_REGION_ID => {
                                if self.zone_manager.toolbar.address_bar.focused {
                                    self.zone_manager.toolbar.address_bar.focused = false;
                                    self.zone_manager
                                        .toolbar
                                        .address_bar
                                        .end_mouse_selection();
                                }
                                self.ir_renderer
                                    .element_state_mut()
                                    .clear_all_focus();

                                self.zone_manager
                                    .devtools
                                    .set_active_tab(DevToolsTab::Console);
                                self.needs_redraw = true;
                            }
                            // Sidebar: Add bookmark button
                            SIDEBAR_ADD_BOOKMARK_REGION_ID => {
                                if self.zone_manager.toolbar.address_bar.focused {
                                    self.zone_manager.toolbar.address_bar.focused = false;
                                    self.zone_manager
                                        .toolbar
                                        .address_bar
                                        .end_mouse_selection();
                                }
                                self.ir_renderer.element_state_mut().clear_all_focus();

                                // Add bookmark for current page using navigation state
                                let current_url = rune_scene::navigation::get_current_url()
                                    .unwrap_or_else(|| self.zone_manager.toolbar.address_bar.text.clone());
                                if !current_url.trim().is_empty() {
                                    let title = rune_scene::navigation::get_current_title()
                                        .filter(|t| !t.trim().is_empty())
                                        .unwrap_or_else(|| {
                                            current_url
                                                .split('/')
                                                .last()
                                                .filter(|s| !s.is_empty())
                                                .unwrap_or("Bookmark")
                                                .to_string()
                                        });
                                    self.zone_manager.sidebar.add_bookmark(title.clone(), current_url.clone());
                                    log::info!("Bookmark added: {} -> {}", title, current_url);
                                }
                                self.needs_redraw = true;
                            }
                            // Sidebar: Tab close buttons (region IDs 2300+)
                            id if id >= SIDEBAR_TAB_CLOSE_REGION_BASE && id < SIDEBAR_TAB_CLOSE_REGION_BASE + 100 => {
                                if self.zone_manager.toolbar.address_bar.focused {
                                    self.zone_manager.toolbar.address_bar.focused = false;
                                    self.zone_manager.toolbar.address_bar.end_mouse_selection();
                                }
                                self.ir_renderer.element_state_mut().clear_all_focus();

                                let tab_index = (id - SIDEBAR_TAB_CLOSE_REGION_BASE) as usize;
                                if self.zone_manager.sidebar.remove_tab(tab_index) {
                                    log::info!("Closed tab at index {}", tab_index);
                                }
                                self.needs_redraw = true;
                            }
                            // Sidebar: Bookmark delete buttons (region IDs 2400+)
                            id if id >= SIDEBAR_BOOKMARK_DELETE_REGION_BASE && id < SIDEBAR_BOOKMARK_DELETE_REGION_BASE + 100 => {
                                if self.zone_manager.toolbar.address_bar.focused {
                                    self.zone_manager.toolbar.address_bar.focused = false;
                                    self.zone_manager.toolbar.address_bar.end_mouse_selection();
                                }
                                self.ir_renderer.element_state_mut().clear_all_focus();

                                let bookmark_index = (id - SIDEBAR_BOOKMARK_DELETE_REGION_BASE) as usize;
                                if self.zone_manager.sidebar.remove_bookmark(bookmark_index) {
                                    log::info!("Deleted bookmark at index {}", bookmark_index);
                                }
                                self.needs_redraw = true;
                            }
                            // Sidebar: Tab items (region IDs 2100-2199)
                            id if id >= SIDEBAR_TAB_REGION_BASE && id < SIDEBAR_TAB_REGION_BASE + 100 => {
                                if self.zone_manager.toolbar.address_bar.focused {
                                    self.zone_manager.toolbar.address_bar.focused = false;
                                    self.zone_manager.toolbar.address_bar.end_mouse_selection();
                                }
                                self.ir_renderer.element_state_mut().clear_all_focus();

                                let tab_index = (id - SIDEBAR_TAB_REGION_BASE) as usize;
                                if let Some(tab) = self.zone_manager.sidebar.get_tab(tab_index) {
                                    let url = tab.url.clone();
                                    log::info!("Navigate to tab: {}", url);
                                    // Set this tab as active before navigating
                                    self.zone_manager.sidebar.set_active_tab(Some(tab_index));
                                    // Use unified navigation so IR URLs load packages and
                                    // CEF URLs are routed to the browser.
                                    self.navigate_to_url(&url);
                                }
                                self.needs_redraw = true;
                            }
                            // Sidebar: Bookmark items (region IDs 2000-2099)
                            id if id >= SIDEBAR_BOOKMARK_REGION_BASE && id < SIDEBAR_BOOKMARK_REGION_BASE + 100 => {
                                if self.zone_manager.toolbar.address_bar.focused {
                                    self.zone_manager.toolbar.address_bar.focused = false;
                                    self.zone_manager.toolbar.address_bar.end_mouse_selection();
                                }
                                self.ir_renderer.element_state_mut().clear_all_focus();

                                let bookmark_index = (id - SIDEBAR_BOOKMARK_REGION_BASE) as usize;
                                if let Some(bookmark) = self.zone_manager.sidebar.get_bookmark(bookmark_index) {
                                    let url = bookmark.url.clone();
                                    log::info!("Navigate to bookmark: {}", url);
                                    // Route through unified navigation so IR packages and
                                    // CEF navigation stay in sync.
                                    self.navigate_to_url(&url);
                                }
                                self.needs_redraw = true;
                            }
                            _ => {
                                // IR element region: dispatch click into IR element state
                                if let Some(view_node_id) = self
                                    .ir_renderer
                                    .hit_registry()
                                    .lookup(region_id)
                                    .cloned()
                                {
                                    log::debug!("IR element clicked: {}", view_node_id);

                                    let event = MouseClickEvent {
                                        button: MouseButton::Left,
                                        state: ElementState::Pressed,
                                        x: scene_x,
                                        y: scene_y,
                                        click_count: self.click_count,
                                    };

                                    let result = self
                                        .ir_renderer
                                        .element_state_mut()
                                        .handle_mouse_click(&view_node_id, event);

                                    if result.is_handled() {
                                        self.needs_redraw = true;
                                    }
                                } else {
                                    // Clicked non-interactive surface: blur all focus
                                    if self.zone_manager.toolbar.address_bar.focused {
                                        self.zone_manager.toolbar.address_bar.focused = false;
                                        self.zone_manager
                                            .toolbar
                                            .address_bar
                                            .end_mouse_selection();
                                    }
                                    self.ir_renderer
                                        .element_state_mut()
                                        .clear_all_focus();
                                    self.needs_redraw = true;
                                }
                            }
                        }
                    } else {
                        // No region id: clear focus when clicking empty surface
                        if self.zone_manager.toolbar.address_bar.focused {
                            self.zone_manager.toolbar.address_bar.focused = false;
                            self.zone_manager
                                .toolbar
                                .address_bar
                                .end_mouse_selection();
                        }
                        self.ir_renderer
                            .element_state_mut()
                            .clear_all_focus();
                        self.needs_redraw = true;
                    }
                }
            }
        } else {
            // Mouse release: end address bar selection drag
            self.zone_manager.toolbar.address_bar.end_mouse_selection();

            // Dispatch release to currently focused IR element so selections/drags terminate
            if let Some((view_node_id, _)) =
                self.ir_renderer.element_state().get_focused_element()
            {
                let viewport = self.zone_manager.layout.viewport;
                let scene_x =
                    logical_x - viewport.x + self.zone_manager.viewport.scroll_offset_x;
                let scene_y =
                    logical_y - viewport.y + self.zone_manager.viewport.scroll_offset_y;

                let release_event = MouseClickEvent {
                    button: MouseButton::Left,
                    state: ElementState::Released,
                    x: scene_x,
                    y: scene_y,
                    click_count: self.click_count,
                };

                let result = self
                    .ir_renderer
                    .element_state_mut()
                    .handle_mouse_click(&view_node_id, release_event);

                if result.is_handled() {
                    self.needs_redraw = true;
                }
            }

            // If IR element state is dirty after release, request redraw
            if self.ir_renderer.element_state().is_dirty() {
                self.needs_redraw = true;
            }
        }
    }

    /// Handle mouse move event
    pub fn mouse_move(&mut self, x: f32, y: f32) {
        use rune_scene::event_handler::MouseMoveEvent;

        let logical_x = x / self.scale_factor;
        let logical_y = y / self.scale_factor;

        let viewport = self.zone_manager.layout.viewport;
        let scene_x = logical_x - viewport.x + self.zone_manager.viewport.scroll_offset_x;
        let scene_y = logical_y - viewport.y + self.zone_manager.viewport.scroll_offset_y;

        let move_event = MouseMoveEvent {
            x: scene_x,
            y: scene_y,
        };

        let result = self
            .ir_renderer
            .element_state_mut()
            .handle_mouse_move(move_event);
        if result.is_handled() {
            self.needs_redraw = true;
        }
    }

    /// Handle scroll (mouse wheel / trackpad) event.
    ///
    /// `delta_x` and `delta_y` are in logical pixel units, matching the
    /// winit runner's use of `MouseScrollDelta` where positive values mean
    /// scroll right / down. The internal viewport scroll helper expects
    /// positive deltas to scroll the content down/right, so we negate here
    /// to keep user-facing behavior consistent with winit.
    pub fn scroll(&mut self, delta_x: f32, delta_y: f32) {
        use rune_scene::zones::ZoneId;

        let viewport_rect = self.zone_manager.layout.get_zone(ZoneId::Viewport);
        self.zone_manager.viewport.scroll(
            -delta_x,
            -delta_y,
            viewport_rect.w,
            viewport_rect.h,
        );
        self.needs_redraw = true;
    }

    /// Handle key event
    pub fn key_event(
        &mut self,
        keycode: u32,
        pressed: bool,
        modifiers: winit::keyboard::ModifiersState,
    ) {
        use winit::event::ElementState;

        log::debug!("Key event: keycode={} pressed={}", keycode, pressed);

        // When the address bar is focused, handle typical editing/navigation keys locally.
        if self.zone_manager.toolbar.address_bar.focused && pressed {
            let mut handled = false;
            let has_cmd = modifiers.contains(winit::keyboard::ModifiersState::SUPER);
            let has_ctrl = modifiers.contains(winit::keyboard::ModifiersState::CONTROL);
            let has_alt = modifiers.contains(winit::keyboard::ModifiersState::ALT);
            let has_shift = modifiers.contains(winit::keyboard::ModifiersState::SHIFT);

            let line_modifier = has_cmd || has_ctrl;
            let word_modifier = has_alt;

            if let Some(key_code) = map_macos_keycode_to_winit(keycode) {
                use winit::keyboard::KeyCode;

                match key_code {
                    KeyCode::KeyA if line_modifier && !has_shift => {
                        self.zone_manager.toolbar.address_bar.select_all();
                        handled = true;
                    }
                    KeyCode::Backspace => {
                        self.zone_manager.toolbar.address_bar.delete_before_cursor();
                        handled = true;
                    }
                    KeyCode::Delete => {
                        self.zone_manager.toolbar.address_bar.delete_after_cursor();
                        handled = true;
                    }
                    KeyCode::ArrowLeft => {
                        if word_modifier {
                            self.zone_manager
                                .toolbar
                                .address_bar
                                .move_cursor_left_word();
                        } else {
                            self.zone_manager.toolbar.address_bar.move_cursor_left();
                        }
                        handled = true;
                    }
                    KeyCode::ArrowRight => {
                        if word_modifier {
                            self.zone_manager
                                .toolbar
                                .address_bar
                                .move_cursor_right_word();
                        } else {
                            self.zone_manager.toolbar.address_bar.move_cursor_right();
                        }
                        handled = true;
                    }
                    KeyCode::Home => {
                        self.zone_manager
                            .toolbar
                            .address_bar
                            .move_cursor_to_start();
                        handled = true;
                    }
                    KeyCode::End => {
                        self.zone_manager
                            .toolbar
                            .address_bar
                            .move_cursor_to_end();
                        handled = true;
                    }
                    KeyCode::Escape => {
                        self.zone_manager.toolbar.address_bar.focused = false;
                        handled = true;
                    }
                    KeyCode::Enter | KeyCode::NumpadEnter => {
                        let url = self.zone_manager.toolbar.address_bar.text.clone();
                        log::info!("Address bar: navigating to '{}'", url);
                        self.navigate_to_url(&url);
                        self.zone_manager.toolbar.address_bar.focused = false;
                        handled = true;
                    }
                    _ => {}
                }
            }

            if handled {
                self.needs_redraw = true;
                return;
            }
        }

        // Forward key presses to IR elements for their own keyboard handling.
        if pressed {
            if let Some(key_code) = map_macos_keycode_to_winit(keycode) {
                use rune_scene::event_handler::KeyboardEvent;
                use rune_scene::ir_renderer::IrElementType;
                use winit::keyboard::KeyCode;

                let keyboard_event = KeyboardEvent {
                    key: key_code,
                    state: ElementState::Pressed,
                    modifiers,
                };

                let result = self
                    .ir_renderer
                    .element_state_mut()
                    .handle_keyboard(keyboard_event);

                if result.is_handled() {
                    self.needs_redraw = true;
                } else if matches!(key_code, KeyCode::Enter | KeyCode::NumpadEnter) {
                    // Home Tab: treat Enter in the home input box as a query submit.
                    // This mirrors the Peco chat UX: first query dismisses the hero
                    // banner and opens the chat panel.
                    let focused = self.ir_renderer.element_state().get_focused_element();
                    if let Some((view_node_id, IrElementType::InputBox)) = focused {
                        if view_node_id == "chat_input" && self.view_doc.view_id == "home" {
                            // Extract and clear the input text.
                            if let Some(input) = self
                                .ir_renderer
                                .element_state_mut()
                                .get_input_box_mut(&view_node_id)
                            {
                                let query = input.text.trim().to_string();
                                if !query.is_empty() {
                                    // Clear the input for the next message.
                                    input.text.clear();
                                    input.cursor_position = 0;

                                    // Mark home chat as started so the hero/greeting hide.
                                    self.ir_renderer
                                        .element_state_mut()
                                        .mark_home_chat_started();

                                    // Route the message into the Peco chat panel.
                                    self.zone_manager
                                        .chat
                                        .add_user_message(query.clone());
                                    self.zone_manager.chat.add_assistant_message(
                                        "Thanks for your question — Peco isn't yet wired to the \
                                        backend here, but this is where the AI response will appear."
                                            .to_string(),
                                    );

                                    // Show the chat panel and recompute layout.
                                    self.zone_manager
                                        .show_chat(self.logical_width, self.logical_height);

                                    self.needs_redraw = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Handle committed text input (UTF-8, already composed).
    ///
    /// This mirrors the `Ime::Commit` and fallback text paths in rune-scene's
    /// winit runner: when the toolbar address bar is focused, text goes there;
    /// otherwise it is dispatched to IR elements via `handle_text_input`.
    pub fn text_input(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        // When address bar is focused, insert plain characters.
        if self.zone_manager.toolbar.address_bar.focused {
            let mut inserted = false;
            for ch in text.chars() {
                if !ch.is_control() || ch == ' ' {
                    self.zone_manager.toolbar.address_bar.insert_char(ch);
                    inserted = true;
                }
            }
            if inserted {
                self.zone_manager.toolbar.address_bar.update_scroll();
                self.needs_redraw = true;
            }
            return;
        }

        // Otherwise, dispatch to IR elements.
        let result = self
            .ir_renderer
            .element_state_mut()
            .handle_text_input(text);
        if result.is_handled() {
            self.needs_redraw = true;
        }
    }

    // NOTE: upload_webview_pixels removed - now using native NSView-based CEF rendering
    // instead of OSR pixel uploads. See docs/NSview.md for architecture details.

    /// Check if redraw is needed
    pub fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    /// Request redraw
    pub fn request_redraw(&mut self) {
        self.needs_redraw = true;
    }

    /// Set the address bar text (e.g., when CEF navigates to a new URL)
    pub fn set_address_bar_text(&mut self, url: &str) {
        // Log the address bar update so we can debug cases where the URL
        // appears stuck or out of sync with CEF.
        let prev = self.zone_manager.toolbar.address_bar.text.clone();
        eprintln!(
            "[AppRenderer] set_address_bar_text: prev='{}' new='{}'",
            prev, url
        );

        self.zone_manager.toolbar.address_bar.set_text(url);
        self.needs_redraw = true;
    }

    /// Update toolbar loading state from navigation state.
    /// Call this each frame to animate the loading indicator.
    pub fn update_toolbar_loading(&mut self) {
        let nav_state = rune_scene::navigation::get_state();
        let was_loading = self.zone_manager.toolbar.is_loading;

        self.zone_manager.toolbar.set_loading(nav_state.is_loading);

        if nav_state.is_loading {
            // Update blink animation
            self.zone_manager.toolbar.update_loading_blink();
            // Keep redrawing while loading to animate
            self.needs_redraw = true;
        } else if was_loading {
            // Just stopped loading, need one more redraw
            self.needs_redraw = true;
        }
    }

    /// Get the URL of the first WebView element in the loaded IR package.
    pub fn get_webview_url(&self) -> Option<String> {
        use rune_ir::view::ViewNodeKind;

        // Search for WebView nodes in the view document
        for node in self.view_doc.nodes.iter() {
            if let ViewNodeKind::WebView(spec) = &node.kind {
                if let Some(ref url) = spec.url {
                    return Some(url.clone());
                }
            }
        }
        None
    }

    /// Get the size of the first WebView element in the loaded IR package.
    /// Returns the layout-computed size if available, otherwise falls back to spec size.
    pub fn get_webview_size(&self) -> Option<(u32, u32)> {
        use rune_ir::view::ViewNodeKind;

        // First try rune_surface (has size when pixels are being rendered)
        if let Some((_x, _y, w, h)) = rune_surface::get_last_raw_image_rect() {
            if w > 0.0 && h > 0.0 {
                return Some((w as u32, h as u32));
            }
        }

        // Then try rune_scene (has size from layout, even before pixels)
        if let Some((_x, _y, w, h)) = rune_scene::elements::webview::get_webview_rect() {
            if w > 0.0 && h > 0.0 {
                return Some((w as u32, h as u32));
            }
        }

        // Fall back to spec size if layout hasn't run yet
        for node in self.view_doc.nodes.iter() {
            if let ViewNodeKind::WebView(spec) = &node.kind {
                // Get width and height from style
                let width = spec.style.width.unwrap_or(800.0) as u32;
                let height = spec.style.height.unwrap_or(600.0) as u32;
                return Some((width, height));
            }
        }
        None
    }

    /// Check if the dock overlay is currently visible.
    pub fn is_dock_visible(&self) -> bool {
        self.zone_manager.is_dock_visible()
    }

    /// Get the height of the DevTools zone in logical pixels.
    /// Returns 0.0 if DevTools is not visible.
    pub fn get_devtools_height(&self) -> f32 {
        if self.zone_manager.is_devtools_visible() {
            use rune_scene::zones::ZoneId;
            let rect = self.zone_manager.layout.get_zone(ZoneId::DevTools);
            rect.h
        } else {
            0.0
        }
    }

    /// Append a message to the DevTools console.
    pub fn devtools_console_log(
        &mut self,
        level: rune_scene::zones::ConsoleLevel,
        message: impl Into<String>,
    ) {
        self.zone_manager
            .devtools
            .log_console(level, message.into());
        self.needs_redraw = true;
    }

    /// Clear all DevTools console entries.
    pub fn devtools_console_clear(&mut self) {
        self.zone_manager.devtools.clear_console();
        self.needs_redraw = true;
    }
}

/// Best-effort mapping from macOS virtual key codes to winit KeyCode.
///
/// This is intentionally minimal – it covers common navigation/editing keys
/// so IR text fields behave reasonably when embedded via rune-ffi.
fn map_macos_keycode_to_winit(code: u32) -> Option<winit::keyboard::KeyCode> {
    use winit::keyboard::KeyCode::*;

    Some(match code {
        // Arrows
        123 => ArrowLeft,
        124 => ArrowRight,
        125 => ArrowDown,
        126 => ArrowUp,
        // Editing
        51 => Backspace,
        117 => Delete,
        36 => Enter,
        76 => NumpadEnter,
        53 => Escape,
        115 => Home,
        119 => End,
        // Digits (top row)
        18 => Digit1,
        19 => Digit2,
        20 => Digit3,
        21 => Digit4,
        23 => Digit5,
        22 => Digit6,
        26 => Digit7,
        28 => Digit8,
        25 => Digit9,
        29 => Digit0,
        // Letters
        0 => KeyA,
        11 => KeyB,
        8 => KeyC,
        2 => KeyD,
        14 => KeyE,
        3 => KeyF,
        5 => KeyG,
        4 => KeyH,
        34 => KeyI,
        38 => KeyJ,
        40 => KeyK,
        37 => KeyL,
        46 => KeyM,
        45 => KeyN,
        31 => KeyO,
        35 => KeyP,
        12 => KeyQ,
        15 => KeyR,
        1 => KeyS,
        17 => KeyT,
        32 => KeyU,
        9 => KeyV,
        13 => KeyW,
        7 => KeyX,
        16 => KeyY,
        6 => KeyZ,
        _ => return None,
    })
}

/// Load IR package from explicit path or configuration, falling back to the built-in sample.
///
/// Priority:
/// 1. Explicit `package_path` argument (from FFI caller)
/// 2. `rune.toml` / env via `rune-config` (IR `package_path`)
/// 3. Built-in `home_tab` sample
fn load_ir_package(package_path: Option<&str>) -> Result<(DataDocument, ViewDocument)> {
    if let Some(path) = package_path {
        return load_ir_package_from_path(path);
    }

    // Priority 2: Try loading from rune.toml / env via rune-config
    let config = rune_config::RuneConfig::load();
    if let Some(package_path) = &config.ir.package_path {
        if let Some(path_str) = package_path.to_str() {
            if !path_str.is_empty() {
                match load_ir_package_from_path(path_str) {
                    Ok(result) => return Ok(result),
                    Err(err) => {
                        log::error!(
                            "Failed to load IR package from config path {:?}: {:?}",
                            package_path,
                            err
                        );
                    }
                }
            }
        }
    }

    // Priority 3: Use default built-in sample from rune-ir
    let package = rune_ir::package::RunePackage::sample()?;
    let (data, view) = package.entrypoint_documents()?;
    Ok((data.clone(), view.clone()))
}

/// Try to load IR package from a URL scheme.
/// Supports rune://, ir://, and sample shortcuts.
fn try_load_ir_from_url(url: &str) -> Option<(DataDocument, ViewDocument)> {
    let url_lower = url.to_lowercase();

    // Handle rune:// scheme
    if url_lower.starts_with("rune://") {
        let path = &url[7..]; // Strip "rune://"

        // Home/Peco - load default sample
        if path == "home" || path == "peco" || path.is_empty() {
            let package = rune_ir::package::RunePackage::sample().ok()?;
            let (data, view) = package.entrypoint_documents().ok()?;
            return Some((data.clone(), view.clone()));
        }

        // Sample packages
        if path.starts_with("sample/") || path.starts_with("samples/") {
            let sample_name = path.split('/').nth(1).unwrap_or("");
            return load_sample_package(sample_name);
        }

        // Direct sample name shortcuts
        match path {
            "first-node" | "firstnode" | "first_node" => return load_sample_package("first-node"),
            "webview" => return load_sample_package("webview"),
            "form" => return load_sample_package("form"),
            _ => {}
        }
    }

    // Handle ir:// scheme - treat as filesystem path
    if url_lower.starts_with("ir://") {
        let path = &url[5..]; // Strip "ir://"
        return load_ir_package_from_path(path).ok();
    }

    None
}

/// Load a sample package by name
fn load_sample_package(name: &str) -> Option<(DataDocument, ViewDocument)> {
    // Sample packages live in the workspace `examples/` directory. When
    // embedded via rune-ffi (rune-app), the process working directory is
    // the app bundle, so we resolve these paths relative to the workspace
    // root derived from `CARGO_MANIFEST_DIR` instead of relying on CWD.
    let rel_dir = match name {
        "first-node" | "firstnode" | "first_node" => "examples/sample_first_node",
        "webview" | "web-view" => "examples/sample_webview",
        "form" => "examples/sample_form",
        _ => {
            log::error!("Unknown sample package: {}", name);
            return None;
        }
    };

    // `CARGO_MANIFEST_DIR` for this crate is `<workspace>/crates/rune-ffi`.
    // Walk up two levels to reach the workspace root, then join the examples path.
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")));

    let sample_path = workspace_root.join(rel_dir);
    let sample_str = sample_path.to_str().unwrap_or(rel_dir);

    match load_ir_package_from_path(sample_str) {
        Ok((data, view)) => {
            log::info!("Loaded sample package: {} from {}", name, sample_str);
            Some((data, view))
        }
        Err(e) => {
            log::error!("Failed to load sample package '{}' from {:?}: {:?}", name, sample_path, e);
            None
        }
    }
}

/// Load IR package from filesystem path
fn load_ir_package_from_path(package_path: &str) -> Result<(DataDocument, ViewDocument)> {
    use std::fs;
    use std::path::Path;

    let base = Path::new(package_path);

    // Read manifest
    let manifest_path = base.join("RUNE.MANIFEST.json");
    let manifest_str = fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read manifest: {:?}", manifest_path))?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_str)?;

    // Get entrypoint paths
    let entrypoint = manifest
        .get("entrypoint")
        .context("No entrypoint in manifest")?;
    let data_path = entrypoint
        .get("data")
        .and_then(|v| v.as_str())
        .context("No data path in entrypoint")?;
    let view_path = entrypoint
        .get("view")
        .and_then(|v| v.as_str())
        .context("No view path in entrypoint")?;

    // Load documents
    let data_full_path = base.join(data_path);
    let view_full_path = base.join(view_path);

    let data_str = fs::read_to_string(&data_full_path)
        .with_context(|| format!("Failed to read data: {:?}", data_full_path))?;
    let view_str = fs::read_to_string(&view_full_path)
        .with_context(|| format!("Failed to read view: {:?}", view_full_path))?;

    let data_doc: DataDocument = serde_json::from_str(&data_str)?;
    let view_doc: ViewDocument = serde_json::from_str(&view_str)?;

    Ok((data_doc, view_doc))
}

// Use thread_local storage - all FFI calls must be on the main thread
use std::cell::RefCell;

thread_local! {
    static RENDERER: RefCell<Option<AppRenderer>> = const { RefCell::new(None) };
}

pub fn with_renderer<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut AppRenderer) -> R,
{
    RENDERER.with(|r| {
        let mut borrow = r.borrow_mut();
        borrow.as_mut().map(f)
    })
}

pub fn set_renderer(renderer: Option<AppRenderer>) {
    RENDERER.with(|r| {
        *r.borrow_mut() = renderer;
    });
}
