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
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Once;
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

    /// Render a frame using the same function as rune-scene
    pub fn render(&mut self) {
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
            REFRESH_BUTTON_REGION_ID, TOGGLE_BUTTON_REGION_ID,
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

                                log::info!("Refresh button clicked");
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

    /// Handle key event
    pub fn key_event(&mut self, keycode: u32, pressed: bool) {
        use winit::event::ElementState;
        use winit::keyboard::{KeyCode, ModifiersState};

        // macOS virtual key codes for navigation/editing keys we care about.
        const VK_BACKSPACE: u32 = 51;
        const VK_DELETE_FORWARD: u32 = 117;
        const VK_LEFT: u32 = 123;
        const VK_RIGHT: u32 = 124;
        const VK_DOWN: u32 = 125;
        const VK_UP: u32 = 126;
        const VK_HOME: u32 = 115;
        const VK_END: u32 = 119;
        const VK_ESCAPE: u32 = 53;

        log::debug!("Key event: keycode={} pressed={}", keycode, pressed);

        // When the address bar is focused, handle typical editing/navigation keys locally.
        if self.zone_manager.toolbar.address_bar.focused && pressed {
            let mut handled = false;
            let word_modifier = false; // TODO: wire real modifiers from macOS events.

            match keycode {
                VK_BACKSPACE => {
                    self.zone_manager.toolbar.address_bar.delete_before_cursor();
                    handled = true;
                }
                VK_DELETE_FORWARD => {
                    self.zone_manager.toolbar.address_bar.delete_after_cursor();
                    handled = true;
                }
                VK_LEFT => {
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
                VK_RIGHT => {
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
                VK_HOME => {
                    self.zone_manager
                        .toolbar
                        .address_bar
                        .move_cursor_to_start();
                    handled = true;
                }
                VK_END => {
                    self.zone_manager
                        .toolbar
                        .address_bar
                        .move_cursor_to_end();
                    handled = true;
                }
                VK_ESCAPE => {
                    self.zone_manager.toolbar.address_bar.focused = false;
                    handled = true;
                }
                _ => {}
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

                let keyboard_event = KeyboardEvent {
                    key: key_code,
                    state: ElementState::Pressed,
                    modifiers: ModifiersState::empty(),
                };

                let result = self
                    .ir_renderer
                    .element_state_mut()
                    .handle_keyboard(keyboard_event);
                if result.is_handled() {
                    self.needs_redraw = true;
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

    /// Upload WebView pixels from CEF
    pub fn upload_webview_pixels(&mut self, pixels: &[u8], width: u32, height: u32) {
        log::debug!(
            "upload_webview_pixels: {}x{}, {} bytes",
            width,
            height,
            pixels.len()
        );

        // Optional debug path: allow freezing the WebView texture after
        // a few frames while the rest of the scene continues to redraw.
        // Enable via RUNE_FREEZE_WEBVIEW_PIXELS=1 to diagnose whether
        // later CEF frames are causing the disappearance.
        static FREEZE_PIXELS_ENABLED: AtomicBool = AtomicBool::new(false);
        static INIT_FREEZE_PIXELS: Once = Once::new();
        static WEBVIEW_FRAME_COUNT: AtomicU32 = AtomicU32::new(0);

        INIT_FREEZE_PIXELS.call_once(|| {
            if let Ok(val) = std::env::var("RUNE_FREEZE_WEBVIEW_PIXELS") {
                let enabled = val == "1" || val.eq_ignore_ascii_case("true");
                FREEZE_PIXELS_ENABLED.store(enabled, Ordering::Relaxed);
                if enabled {
                    log::info!("upload_webview_pixels: RUNE_FREEZE_WEBVIEW_PIXELS enabled");
                }
            }
        });

        if FREEZE_PIXELS_ENABLED.load(Ordering::Relaxed) {
            let frame = WEBVIEW_FRAME_COUNT.fetch_add(1, Ordering::Relaxed);
            // After 120 frames, ignore further CEF uploads so the last
            // good WebView texture stays static.
            if frame > 120 {
                return;
            }
        }

        // Pass BGRA pixels directly - the GPU texture is now BGRA format
        // to match CEF's native output and avoid CPU-side conversion.
        // Force opaque alpha to prevent disappearing frames.
        let mut bgra_pixels = pixels.to_vec();
        for chunk in bgra_pixels.chunks_exact_mut(4) {
            chunk[3] = 255; // Force opaque alpha
        }

        // Store in the rune-scene external pixels storage (BGRA format)
        rune_scene::elements::webview::set_external_pixels(bgra_pixels, width, height);
        self.needs_redraw = true;
    }

    /// Check if redraw is needed
    pub fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    /// Request redraw
    pub fn request_redraw(&mut self) {
        self.needs_redraw = true;
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
