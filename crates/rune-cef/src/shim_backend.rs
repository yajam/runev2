use crate::error::{CefError, Result};
use crate::frame::{FrameBuffer, PixelFormat};
use crate::{
    HeadlessConfig, HeadlessRenderer, KeyEvent, KeyEventKind, Modifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use crate::shim_ffi::{
    get_shim_library, RuneCefBrowser, RuneCefConfig, RuneCefFrame, RuneKeyEvent,
    RuneKeyEventKind, RuneMouseButton, RuneMouseEvent, RuneMouseEventKind,
};
use std::ffi::CString;
use std::path::PathBuf;
use std::ptr;
use std::slice;
use std::sync::Arc;
use std::time::Duration;
use std::{thread, time::Instant};

fn log_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CEF_LOG_DIR") {
        return PathBuf::from(dir);
    }

    if let Ok(mut path) = std::env::current_exe() {
        for _ in 0..6 {
            if path.join("Cargo.toml").exists() || path.join(".git").exists() {
                return path;
            }
            if !path.pop() {
                break;
            }
        }
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn encode_modifiers(_mods: Modifiers) -> u32 {
    // For now we don't translate modifiers to CEF flags.
    0
}

/// Headless CEF renderer backed by the C++ shim.
pub struct ShimHeadless {
    library: Arc<crate::shim_ffi::ShimLibrary>,
    config: HeadlessConfig,
    browser: RuneCefBrowser,
    current_url: Option<String>,
}

unsafe impl Send for ShimHeadless {}

impl ShimHeadless {
    pub fn new(config: HeadlessConfig) -> Result<Self> {
        let library = get_shim_library()?;

        // Initialize CEF via the shim (idempotent in the shim implementation).
        let root_cache = PathBuf::from("/tmp/rune_cef_root");
        let cache = root_cache.join("cache");
        let log_file_path = log_dir().join("cef_cef.log");

        let cache_c = CString::new(cache.to_string_lossy().as_bytes())
            .map_err(|e| CefError::InitFailed(e.to_string()))?;
        let root_cache_c = CString::new(root_cache.to_string_lossy().as_bytes())
            .map_err(|e| CefError::InitFailed(e.to_string()))?;
        let log_file_c = CString::new(log_file_path.to_string_lossy().as_bytes())
            .map_err(|e| CefError::InitFailed(e.to_string()))?;

        let init_result = unsafe {
            (library.rune_cef_init)(
                cache_c.as_ptr(),
                root_cache_c.as_ptr(),
                log_file_c.as_ptr(),
                1, // external_message_pump
            )
        };

        if init_result == 0 {
            return Err(CefError::InitFailed(
                "rune_cef_init returned failure".to_string(),
            ));
        }

        let shim_config = RuneCefConfig {
            width: config.width,
            height: config.height,
            scale_factor: config.scale_factor,
            enable_javascript: if config.javascript_enabled { 1 } else { 0 },
            disable_gpu: if config.disable_gpu { 1 } else { 0 },
            user_agent: ptr::null(),
        };

        let initial_url = CString::new("about:blank")
            .map_err(|e| CefError::InitFailed(e.to_string()))?;

        let browser = unsafe {
            (library.rune_cef_create_browser)(&shim_config, initial_url.as_ptr())
        };

        if browser.is_null() {
            return Err(CefError::InitFailed(
                "rune_cef_create_browser returned null".to_string(),
            ));
        }

        Ok(Self {
            library,
            config,
            browser,
            current_url: Some("about:blank".to_string()),
        })
    }

    fn assert_initialized(&self) -> Result<()> {
        if self.browser.is_null() {
            Err(CefError::NotInitialized)
        } else {
            Ok(())
        }
    }
}

impl HeadlessRenderer for ShimHeadless {
    fn navigate(&mut self, url: &str) -> Result<()> {
        self.assert_initialized()?;

        let url_c =
            CString::new(url).map_err(|e| CefError::NavigationFailed(e.to_string()))?;

        unsafe {
            (self.library.rune_cef_navigate)(self.browser, url_c.as_ptr());
        }

        self.current_url = Some(url.to_string());
        Ok(())
    }

    fn load_html(&mut self, html: &str, base_url: Option<&str>) -> Result<()> {
        self.assert_initialized()?;

        let html_c =
            CString::new(html).map_err(|e| CefError::NavigationFailed(e.to_string()))?;
        let base_c = base_url
            .map(|b| CString::new(b).map_err(|e| CefError::NavigationFailed(e.to_string())))
            .transpose()?;

        unsafe {
            (self.library.rune_cef_load_html)(
                self.browser,
                html_c.as_ptr(),
                base_c
                    .as_ref()
                    .map(|c| c.as_ptr())
                    .unwrap_or(std::ptr::null()),
            );
        }

        self.current_url = base_url.map(|s| s.to_string());
        Ok(())
    }

    fn capture_frame(&mut self) -> Result<FrameBuffer> {
        self.assert_initialized()?;

        self.pump_messages();

        let mut frame = RuneCefFrame {
            pixels: ptr::null(),
            width: 0,
            height: 0,
            stride: 0,
        };

        let has_frame =
            unsafe { (self.library.rune_cef_get_frame)(self.browser, &mut frame) } != 0;

        if !has_frame || frame.pixels.is_null() || frame.width == 0 || frame.height == 0 {
            return Ok(FrameBuffer::new(self.config.width, self.config.height));
        }

        let size = (frame.stride * frame.height) as usize;
        let data = unsafe { slice::from_raw_parts(frame.pixels, size) }.to_vec();

        Ok(FrameBuffer::from_raw(
            data,
            frame.width,
            frame.height,
            frame.stride,
            PixelFormat::Bgra8,
        ))
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        self.assert_initialized()?;
        self.config.width = width;
        self.config.height = height;

        unsafe {
            (self.library.rune_cef_resize)(self.browser, width, height);
        }

        Ok(())
    }

    fn execute_js(&mut self, _script: &str) -> Result<Option<String>> {
        // Not yet exposed via the shim.
        Ok(None)
    }

    fn is_loading(&self) -> bool {
        if self.browser.is_null() {
            return false;
        }
        let loading =
            unsafe { (self.library.rune_cef_is_loading)(self.browser) } != 0;
        loading
    }

    fn wait_for_load(&mut self, timeout_ms: u64) -> Result<()> {
        let start = Instant::now();
        while self.is_loading() {
            if start.elapsed().as_millis() as u64 > timeout_ms {
                return Err(CefError::Timeout(timeout_ms));
            }
            self.pump_messages();
            thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    fn send_mouse_event(&mut self, event: MouseEvent) -> Result<()> {
        self.assert_initialized()?;

        let (kind, delta_x, delta_y) = match event.kind {
            MouseEventKind::Move => (RuneMouseEventKind::RUNE_MOUSE_MOVE, 0, 0),
            MouseEventKind::Down => (RuneMouseEventKind::RUNE_MOUSE_DOWN, 0, 0),
            MouseEventKind::Up => (RuneMouseEventKind::RUNE_MOUSE_UP, 0, 0),
            MouseEventKind::Wheel { delta_x, delta_y } => {
                (RuneMouseEventKind::RUNE_MOUSE_WHEEL, delta_x, delta_y)
            }
        };

        let button = match event.button {
            MouseButton::None => RuneMouseButton::RUNE_MOUSE_NONE,
            MouseButton::Left => RuneMouseButton::RUNE_MOUSE_LEFT,
            MouseButton::Middle => RuneMouseButton::RUNE_MOUSE_MIDDLE,
            MouseButton::Right => RuneMouseButton::RUNE_MOUSE_RIGHT,
        };

        let ffi_event = RuneMouseEvent {
            x: event.x,
            y: event.y,
            kind,
            button,
            delta_x,
            delta_y,
            modifiers: encode_modifiers(event.modifiers),
        };

        unsafe {
            (self.library.rune_cef_send_mouse_event)(self.browser, &ffi_event);
        }

        Ok(())
    }

    fn send_key_event(&mut self, event: KeyEvent) -> Result<()> {
        self.assert_initialized()?;

        let kind = match event.kind {
            KeyEventKind::Down => RuneKeyEventKind::RUNE_KEY_DOWN,
            KeyEventKind::Up => RuneKeyEventKind::RUNE_KEY_UP,
            KeyEventKind::Char => RuneKeyEventKind::RUNE_KEY_CHAR,
        };

        let character = event
            .char
            .map(|c| c as u32)
            .unwrap_or(0);

        let ffi_event = RuneKeyEvent {
            key_code: event.key_code,
            character,
            kind,
            modifiers: encode_modifiers(event.modifiers),
        };

        unsafe {
            (self.library.rune_cef_send_key_event)(self.browser, &ffi_event);
        }

        Ok(())
    }

    fn pump_messages(&mut self) {
        unsafe {
            (self.library.rune_cef_do_message_loop_work)();
        }
    }

    fn shutdown(&mut self) -> Result<()> {
        if !self.browser.is_null() {
            unsafe {
                (self.library.rune_cef_destroy_browser)(self.browser);
            }
            self.browser = ptr::null_mut();
        }
        Ok(())
    }
}

impl Drop for ShimHeadless {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

