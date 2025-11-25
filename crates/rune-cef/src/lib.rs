//! Headless CEF (Chromium Embedded Framework) renderer for wgpu surfaces.
//!
//! This crate provides off-screen rendering of web content via CEF,
//! capturing frames to wgpu textures for integration with the rune-draw
//! rendering pipeline.
//!
//! # Features
//!
//! - `cef-native`: Use native CEF FFI bindings (requires CEF distribution)
//! - `cdp`: Use Chrome DevTools Protocol via chromiumoxide (async, easier setup)

mod error;
mod frame;
mod texture;

#[cfg(feature = "cef-dynamic")]
mod cef_sys;

#[cfg(feature = "cef-dynamic")]
mod cef_backend;

#[cfg(feature = "cdp")]
mod cdp_backend;

pub use error::{CefError, Result};
pub use frame::{DirtyRect, FrameBuffer};
pub use texture::WgpuTextureTarget;

/// Configuration for headless CEF renderer.
#[derive(Debug, Clone)]
pub struct HeadlessConfig {
    /// Width of the virtual viewport in pixels.
    pub width: u32,
    /// Height of the virtual viewport in pixels.
    pub height: u32,
    /// Device scale factor (1.0 = 96 DPI, 2.0 = 192 DPI).
    pub scale_factor: f32,
    /// Enable JavaScript execution.
    pub javascript_enabled: bool,
    /// User agent string override.
    pub user_agent: Option<String>,
    /// Disable GPU acceleration in CEF (use software rendering).
    pub disable_gpu: bool,
}

impl Default for HeadlessConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            scale_factor: 1.0,
            javascript_enabled: true,
            user_agent: None,
            disable_gpu: false,
        }
    }
}

/// Trait for headless browser backends.
pub trait HeadlessRenderer: Send {
    /// Navigate to a URL.
    fn navigate(&mut self, url: &str) -> Result<()>;

    /// Load HTML content directly.
    fn load_html(&mut self, html: &str, base_url: Option<&str>) -> Result<()>;

    /// Capture the current frame to a buffer.
    fn capture_frame(&mut self) -> Result<FrameBuffer>;

    /// Resize the virtual viewport.
    fn resize(&mut self, width: u32, height: u32) -> Result<()>;

    /// Execute JavaScript and return the result as a string.
    fn execute_js(&mut self, script: &str) -> Result<Option<String>>;

    /// Check if the page has finished loading.
    fn is_loading(&self) -> bool;

    /// Wait for the page to finish loading (with timeout in milliseconds).
    fn wait_for_load(&mut self, timeout_ms: u64) -> Result<()>;

    /// Send a mouse event.
    fn send_mouse_event(&mut self, event: MouseEvent) -> Result<()>;

    /// Send a keyboard event.
    fn send_key_event(&mut self, event: KeyEvent) -> Result<()>;

    /// Perform a single iteration of the message loop.
    fn pump_messages(&mut self);

    /// Shutdown the renderer and release resources.
    fn shutdown(&mut self) -> Result<()>;
}

/// Mouse event for input simulation.
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    pub x: i32,
    pub y: i32,
    pub kind: MouseEventKind,
    pub button: MouseButton,
    pub modifiers: Modifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEventKind {
    Move,
    Down,
    Up,
    Wheel { delta_x: i32, delta_y: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MouseButton {
    #[default]
    None,
    Left,
    Middle,
    Right,
}

/// Keyboard event for input simulation.
#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub key_code: u32,
    pub char: Option<char>,
    pub kind: KeyEventKind,
    pub modifiers: Modifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventKind {
    Down,
    Up,
    Char,
}

/// Modifier keys state.
#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

/// Builder for creating headless renderers.
pub struct HeadlessBuilder {
    config: HeadlessConfig,
}

impl HeadlessBuilder {
    pub fn new() -> Self {
        Self {
            config: HeadlessConfig::default(),
        }
    }

    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.config.width = width;
        self.config.height = height;
        self
    }

    pub fn with_scale_factor(mut self, scale: f32) -> Self {
        self.config.scale_factor = scale;
        self
    }

    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.config.user_agent = Some(user_agent.into());
        self
    }

    pub fn disable_javascript(mut self) -> Self {
        self.config.javascript_enabled = false;
        self
    }

    pub fn disable_gpu(mut self) -> Self {
        self.config.disable_gpu = true;
        self
    }

    /// Build a CEF headless renderer (dynamically linked).
    #[cfg(feature = "cef-dynamic")]
    pub fn build_cef(self) -> Result<cef_backend::CefHeadless> {
        cef_backend::CefHeadless::new(self.config)
    }

    /// Build a CDP-based headless renderer (chromiumoxide).
    #[cfg(feature = "cdp")]
    pub async fn build_cdp(self) -> Result<cdp_backend::CdpHeadless> {
        cdp_backend::CdpHeadless::new(self.config).await
    }
}

impl Default for HeadlessBuilder {
    fn default() -> Self {
        Self::new()
    }
}
