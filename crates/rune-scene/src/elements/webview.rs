//! WebView element for rendering embedded browser content.
//!
//! This module provides the `WebView` element which renders web content (HTML/CSS/JS)
//! to a texture within the IR scene graph using CEF (Chromium Embedded Framework).
//!
//! # Features
//!
//! - URL navigation and HTML content rendering
//! - Mouse and keyboard event forwarding to the browser
//! - Texture-based rendering for integration with wgpu pipeline
//! - Resize handling with automatic texture recreation
//! - External texture mode for FFI integration (CEF managed by host app)
//!
//! # Usage
//!
//! ```ignore
//! let mut webview = WebView::new(rect, "https://example.com", None)?;
//! webview.render(canvas, z, queue, device)?;
//! ```

use engine_core::{Brush, ColorLinPremul, Rect};
use rune_surface::Canvas;

#[cfg(feature = "webview-cef")]
use rune_cef::{
    HeadlessBuilder, HeadlessRenderer, KeyEvent, KeyEventKind, Modifiers, MouseButton,
    MouseEvent, MouseEventKind, WgpuTextureTarget,
};

/// Global storage for external WebView pixel data and layout info.
/// This is used when CEF is managed externally (e.g., from Xcode/FFI).
mod external_pixels {
    use std::sync::Mutex;

    /// External pixel data for WebView rendering.
    pub struct ExternalWebViewPixels {
        pub pixels: Vec<u8>,
        pub width: u32,
        pub height: u32,
        pub dirty: bool,
    }

    /// WebView position in scene coordinates (set during layout).
    pub struct WebViewRect {
        pub x: f32,
        pub y: f32,
        pub w: f32,
        pub h: f32,
    }

    static EXTERNAL_PIXELS: Mutex<Option<ExternalWebViewPixels>> = Mutex::new(None);
    static WEBVIEW_RECT: Mutex<Option<WebViewRect>> = Mutex::new(None);

    /// Set external pixel data for WebView rendering.
    /// The pixels should be in RGBA format.
    pub fn set_pixels(pixels: Vec<u8>, width: u32, height: u32) {
        if let Ok(mut guard) = EXTERNAL_PIXELS.lock() {
            *guard = Some(ExternalWebViewPixels {
                pixels,
                width,
                height,
                dirty: true,
            });
        }
    }

    /// Get the external pixel data if available.
    pub fn get_pixels() -> Option<(Vec<u8>, u32, u32)> {
        if let Ok(guard) = EXTERNAL_PIXELS.lock() {
            guard.as_ref().map(|p| (p.pixels.clone(), p.width, p.height))
        } else {
            None
        }
    }

    /// Check if external pixels are available.
    pub fn has_pixels() -> bool {
        if let Ok(guard) = EXTERNAL_PIXELS.lock() {
            guard.is_some()
        } else {
            false
        }
    }

    /// Clear the external pixel data.
    pub fn clear() {
        if let Ok(mut guard) = EXTERNAL_PIXELS.lock() {
            *guard = None;
        }
    }

    /// Set the WebView rect (called during layout).
    pub fn set_rect(x: f32, y: f32, w: f32, h: f32) {
        if let Ok(mut guard) = WEBVIEW_RECT.lock() {
            *guard = Some(WebViewRect { x, y, w, h });
        }
    }

    /// Get the WebView rect if available.
    pub fn get_rect() -> Option<(f32, f32, f32, f32)> {
        if let Ok(guard) = WEBVIEW_RECT.lock() {
            guard.as_ref().map(|r| (r.x, r.y, r.w, r.h))
        } else {
            None
        }
    }
}

/// Set external pixel data for WebView rendering (RGBA format).
pub fn set_external_pixels(pixels: Vec<u8>, width: u32, height: u32) {
    external_pixels::set_pixels(pixels, width, height);
}

/// Set the WebView rect in scene coordinates.
pub fn set_webview_rect(x: f32, y: f32, w: f32, h: f32) {
    external_pixels::set_rect(x, y, w, h);
}

/// Get the WebView rect in scene coordinates.
pub fn get_webview_rect() -> Option<(f32, f32, f32, f32)> {
    external_pixels::get_rect()
}

/// Get the external pixel data if available.
pub fn get_external_pixels() -> Option<(Vec<u8>, u32, u32)> {
    external_pixels::get_pixels()
}

/// Check if external pixels are available.
pub fn has_external_pixels() -> bool {
    external_pixels::has_pixels()
}

/// Clear the external pixel data.
pub fn clear_external_pixels() {
    external_pixels::clear();
}

/// Embedded web browser view element.
///
/// Renders web content to a wgpu texture for display in the IR scene graph.
/// Supports both URL navigation and inline HTML content.
pub struct WebView {
    /// Layout rectangle
    pub rect: Rect,
    /// Current URL (if navigated via URL)
    pub url: Option<String>,
    /// Current HTML content (if loaded directly)
    pub html: Option<String>,
    /// Base URL for relative path resolution
    pub base_url: Option<String>,
    /// Device scale factor
    pub scale_factor: f32,
    /// Whether JavaScript is enabled
    pub javascript_enabled: bool,
    /// Custom user agent
    pub user_agent: Option<String>,
    /// Whether the element is focused
    pub focused: bool,
    /// Background color (shown while loading)
    pub bg_color: ColorLinPremul,
    /// Border color
    pub border_color: ColorLinPremul,
    /// Border width
    pub border_width: f32,
    /// Corner radius
    pub corner_radius: f32,
    /// Internal browser renderer (feature-gated)
    #[cfg(feature = "webview-cef")]
    renderer: Option<Box<dyn HeadlessRenderer>>,
    /// GPU texture target for frame upload
    #[cfg(feature = "webview-cef")]
    texture_target: Option<WgpuTextureTarget>,
    /// Last captured frame dimensions
    last_frame_size: Option<(u32, u32)>,
    /// Whether the browser is currently loading
    is_loading: bool,
    /// Error message if initialization failed
    error_message: Option<String>,
    /// Whether we need to reinitialize (e.g., after URL change)
    needs_reinit: bool,
}

impl WebView {
    /// Create a new WebView element.
    ///
    /// # Arguments
    /// * `rect` - Layout rectangle for the webview
    /// * `url` - Optional URL to navigate to
    /// * `html` - Optional HTML content to render directly
    pub fn new(rect: Rect, url: Option<String>, html: Option<String>) -> Self {
        Self {
            rect,
            url,
            html,
            base_url: None,
            scale_factor: 1.0,
            javascript_enabled: true,
            user_agent: None,
            focused: false,
            bg_color: ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
            border_color: ColorLinPremul::from_srgba_u8([200, 200, 200, 255]),
            border_width: 1.0,
            corner_radius: 4.0,
            #[cfg(feature = "webview-cef")]
            renderer: None,
            #[cfg(feature = "webview-cef")]
            texture_target: None,
            last_frame_size: None,
            is_loading: false,
            error_message: None,
            needs_reinit: true,
        }
    }

    /// Initialize or reinitialize the browser renderer.
    #[cfg(feature = "webview-cef")]
    pub fn ensure_initialized(&mut self, device: &wgpu::Device) {
        if !self.needs_reinit && self.renderer.is_some() {
            return;
        }

        self.needs_reinit = false;
        let width = self.rect.w.max(1.0) as u32;
        let height = self.rect.h.max(1.0) as u32;

        // Build the headless renderer
        let mut builder = HeadlessBuilder::new()
            .with_size(width, height)
            .with_scale_factor(self.scale_factor);

        if !self.javascript_enabled {
            builder = builder.disable_javascript();
        }

        if let Some(ref ua) = self.user_agent {
            builder = builder.with_user_agent(ua.clone());
        }

        // Initialize CEF
        match builder.build_cef() {
            Ok(renderer) => {
                self.renderer = Some(Box::new(renderer));
            }
            Err(e) => {
                self.error_message = Some(format!("CEF initialization failed: {}", e));
                return;
            }
        }

        // Navigate to URL or load HTML
        self.navigate_or_load();

        // Create texture target
        self.texture_target = Some(WgpuTextureTarget::new(
            device,
            width,
            height,
            Some("webview-texture"),
        ));
    }

    #[cfg(feature = "webview-cef")]
    fn navigate_or_load(&mut self) {
        let Some(ref mut renderer) = self.renderer else {
            return;
        };

        if let Some(ref url) = self.url {
            if let Err(e) = renderer.navigate(url) {
                self.error_message = Some(format!("Navigation failed: {}", e));
            } else {
                self.is_loading = true;
            }
        } else if let Some(ref html) = self.html {
            if let Err(e) = renderer.load_html(html, self.base_url.as_deref()) {
                self.error_message = Some(format!("HTML load failed: {}", e));
            } else {
                self.is_loading = true;
            }
        }
    }

    /// Navigate to a new URL.
    pub fn navigate(&mut self, url: &str) {
        self.url = Some(url.to_string());
        self.html = None;

        #[cfg(feature = "webview-cef")]
        if let Some(ref mut renderer) = self.renderer {
            if let Err(e) = renderer.navigate(url) {
                self.error_message = Some(format!("Navigation failed: {}", e));
            } else {
                self.is_loading = true;
                self.error_message = None;
            }
        } else {
            self.needs_reinit = true;
        }
    }

    /// Load HTML content directly.
    pub fn load_html(&mut self, html: &str, base_url: Option<&str>) {
        self.html = Some(html.to_string());
        self.base_url = base_url.map(|s| s.to_string());
        self.url = None;

        #[cfg(feature = "webview-cef")]
        if let Some(ref mut renderer) = self.renderer {
            if let Err(e) = renderer.load_html(html, base_url) {
                self.error_message = Some(format!("HTML load failed: {}", e));
            } else {
                self.is_loading = true;
                self.error_message = None;
            }
        } else {
            self.needs_reinit = true;
        }
    }

    /// Execute JavaScript in the webview.
    #[cfg(feature = "webview-cef")]
    pub fn execute_js(&mut self, script: &str) -> Option<String> {
        let renderer = self.renderer.as_mut()?;
        renderer.execute_js(script).ok().flatten()
    }

    /// Resize the webview.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.rect.w = width as f32;
        self.rect.h = height as f32;

        #[cfg(feature = "webview-cef")]
        if let Some(ref mut renderer) = self.renderer {
            let _ = renderer.resize(width, height);
        }
    }

    /// Update frame capture and upload to texture.
    #[cfg(feature = "webview-cef")]
    pub fn update_frame(&mut self, queue: &wgpu::Queue, device: &wgpu::Device) {
        let Some(ref mut renderer) = self.renderer else {
            return;
        };

        // Pump message loop
        renderer.pump_messages();

        // Update loading state
        self.is_loading = renderer.is_loading();

        // Capture frame
        let frame = match renderer.capture_frame() {
            Ok(f) => f,
            Err(_) => return,
        };

        if frame.data.is_empty() {
            return;
        }

        let frame_size = (frame.width, frame.height);

        // Recreate texture if size changed
        if self.last_frame_size != Some(frame_size) {
            self.texture_target = Some(WgpuTextureTarget::new(
                device,
                frame.width,
                frame.height,
                Some("webview-texture"),
            ));
            self.last_frame_size = Some(frame_size);
        }

        // Upload frame to texture
        if let Some(ref target) = self.texture_target {
            let _ = target.upload(queue, &frame);
        }
    }

    /// Render the webview element.
    ///
    /// Note: The actual browser texture is rendered separately via the image pipeline.
    /// This method draws the container (background, border, loading indicator).
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        // Draw background
        canvas.fill_rect(
            self.rect.x,
            self.rect.y,
            self.rect.w,
            self.rect.h,
            Brush::Solid(self.bg_color),
            z,
        );

        // Draw border
        if self.border_width > 0.0 {
            // Top
            canvas.fill_rect(
                self.rect.x,
                self.rect.y,
                self.rect.w,
                self.border_width,
                Brush::Solid(self.border_color),
                z + 1,
            );
            // Bottom
            canvas.fill_rect(
                self.rect.x,
                self.rect.y + self.rect.h - self.border_width,
                self.rect.w,
                self.border_width,
                Brush::Solid(self.border_color),
                z + 1,
            );
            // Left
            canvas.fill_rect(
                self.rect.x,
                self.rect.y,
                self.border_width,
                self.rect.h,
                Brush::Solid(self.border_color),
                z + 1,
            );
            // Right
            canvas.fill_rect(
                self.rect.x + self.rect.w - self.border_width,
                self.rect.y,
                self.border_width,
                self.rect.h,
                Brush::Solid(self.border_color),
                z + 1,
            );
        }

        // Show loading indicator or error message
        if self.is_loading {
            let text = "Loading...";
            let text_x = self.rect.x + self.rect.w * 0.5 - 30.0;
            let text_y = self.rect.y + self.rect.h * 0.5;
            canvas.draw_text_run(
                [text_x, text_y],
                text.to_string(),
                14.0,
                ColorLinPremul::from_srgba_u8([100, 100, 100, 255]),
                z + 2,
            );
        } else if let Some(ref error) = self.error_message {
            let text_x = self.rect.x + 10.0;
            let text_y = self.rect.y + 20.0;
            canvas.draw_text_run(
                [text_x, text_y],
                error.clone(),
                12.0,
                ColorLinPremul::from_srgba_u8([200, 50, 50, 255]),
                z + 2,
            );
        }

        // Focus outline
        if self.focused {
            let outline_color = ColorLinPremul::from_srgba_u8([63, 130, 246, 255]);
            // Top
            canvas.fill_rect(
                self.rect.x - 2.0,
                self.rect.y - 2.0,
                self.rect.w + 4.0,
                2.0,
                Brush::Solid(outline_color),
                z + 3,
            );
            // Bottom
            canvas.fill_rect(
                self.rect.x - 2.0,
                self.rect.y + self.rect.h,
                self.rect.w + 4.0,
                2.0,
                Brush::Solid(outline_color),
                z + 3,
            );
            // Left
            canvas.fill_rect(
                self.rect.x - 2.0,
                self.rect.y - 2.0,
                2.0,
                self.rect.h + 4.0,
                Brush::Solid(outline_color),
                z + 3,
            );
            // Right
            canvas.fill_rect(
                self.rect.x + self.rect.w,
                self.rect.y - 2.0,
                2.0,
                self.rect.h + 4.0,
                Brush::Solid(outline_color),
                z + 3,
            );
        }
    }

    /// Get the texture view for binding to shaders.
    #[cfg(feature = "webview-cef")]
    pub fn texture_view(&self) -> Option<&wgpu::TextureView> {
        self.texture_target.as_ref().map(|t| t.view())
    }

    /// Check if the webview has a valid texture.
    #[cfg(feature = "webview-cef")]
    pub fn has_texture(&self) -> bool {
        self.texture_target.is_some() && self.last_frame_size.is_some()
    }

    /// Get the frame buffer dimensions.
    pub fn frame_dimensions(&self) -> Option<(u32, u32)> {
        self.last_frame_size
    }

    // ===== Event Handling =====

    /// Hit test the webview
    pub fn hit_test(&self, x: f32, y: f32) -> bool {
        x >= self.rect.x
            && x <= self.rect.x + self.rect.w
            && y >= self.rect.y
            && y <= self.rect.y + self.rect.h
    }

    /// Check if this webview is focused
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Check if the point is inside this webview
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        self.hit_test(x, y)
    }

    /// Convert scene coordinates to webview-local coordinates
    fn to_local_coords(&self, x: f32, y: f32) -> (i32, i32) {
        let local_x = (x - self.rect.x) as i32;
        let local_y = (y - self.rect.y) as i32;
        (local_x, local_y)
    }

    /// Forward mouse event to the browser.
    #[cfg(feature = "webview-cef")]
    pub fn send_mouse_event(
        &mut self,
        x: f32,
        y: f32,
        kind: MouseEventKind,
        button: MouseButton,
        modifiers: Modifiers,
    ) {
        // Convert coords before borrowing renderer to avoid borrow conflict
        let (local_x, local_y) = self.to_local_coords(x, y);

        let Some(ref mut renderer) = self.renderer else {
            return;
        };

        let event = MouseEvent {
            x: local_x,
            y: local_y,
            kind,
            button,
            modifiers,
        };

        let _ = renderer.send_mouse_event(event);
    }

    /// Forward keyboard event to the browser.
    #[cfg(feature = "webview-cef")]
    pub fn send_key_event(&mut self, event: KeyEvent) {
        let Some(ref mut renderer) = self.renderer else {
            return;
        };
        let _ = renderer.send_key_event(event);
    }

    /// Shutdown the webview renderer.
    #[cfg(feature = "webview-cef")]
    pub fn shutdown(&mut self) {
        if let Some(ref mut renderer) = self.renderer {
            let _ = renderer.shutdown();
        }
        self.renderer = None;
        self.texture_target = None;
    }
}

impl Drop for WebView {
    fn drop(&mut self) {
        #[cfg(feature = "webview-cef")]
        self.shutdown();
    }
}

// ===== EventHandler Trait Implementation =====

impl crate::event_handler::EventHandler for WebView {
    fn handle_mouse_click(
        &mut self,
        event: crate::event_handler::MouseClickEvent,
    ) -> crate::event_handler::EventResult {
        use winit::event::ElementState;

        if !self.contains_point(event.x, event.y) {
            return crate::event_handler::EventResult::Ignored;
        }

        #[cfg(feature = "webview-cef")]
        {
            let button = match event.button {
                winit::event::MouseButton::Left => MouseButton::Left,
                winit::event::MouseButton::Right => MouseButton::Right,
                winit::event::MouseButton::Middle => MouseButton::Middle,
                _ => MouseButton::None,
            };

            let kind = match event.state {
                ElementState::Pressed => MouseEventKind::Down,
                ElementState::Released => MouseEventKind::Up,
            };

            self.send_mouse_event(event.x, event.y, kind, button, Modifiers::default());
        }

        crate::event_handler::EventResult::Handled
    }

    fn handle_keyboard(
        &mut self,
        event: crate::event_handler::KeyboardEvent,
    ) -> crate::event_handler::EventResult {
        if !self.focused {
            return crate::event_handler::EventResult::Ignored;
        }

        #[cfg(feature = "webview-cef")]
        {
            use winit::event::ElementState;

            let kind = match event.state {
                ElementState::Pressed => KeyEventKind::Down,
                ElementState::Released => KeyEventKind::Up,
            };

            // Convert winit key code to CEF key code (simplified mapping)
            let key_code = event.key as u32;

            let key_event = KeyEvent {
                key_code,
                char: None,
                kind,
                modifiers: Modifiers::default(),
            };

            self.send_key_event(key_event);
        }

        crate::event_handler::EventResult::Handled
    }

    fn handle_mouse_move(
        &mut self,
        event: crate::event_handler::MouseMoveEvent,
    ) -> crate::event_handler::EventResult {
        if !self.focused {
            return crate::event_handler::EventResult::Ignored;
        }

        #[cfg(feature = "webview-cef")]
        {
            self.send_mouse_event(
                event.x,
                event.y,
                MouseEventKind::Move,
                MouseButton::None,
                Modifiers::default(),
            );
        }

        crate::event_handler::EventResult::Handled
    }

    fn is_focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    fn contains_point(&self, x: f32, y: f32) -> bool {
        self.hit_test(x, y)
    }
}
