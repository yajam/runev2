//! FFI bindings to the C++ CEF shim (`rune_cef_shim`).
//!
//! The shim is expected to be provided as a dynamic library
//! (for example `librune_cef_shim.dylib` on macOS) that links
//! against the CEF framework and exposes a small `extern "C"`
//! API defined in `cef/rune_cef_shim/rune_cef_shim.h`.

use crate::error::{CefError, Result};
use libloading::{Library, Symbol};
use std::ffi::{c_char, c_int, c_uint, c_void};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

#[repr(C)]
pub struct RuneCefConfig {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub enable_javascript: c_int,
    pub disable_gpu: c_int,
    pub user_agent: *const c_char,
}

#[repr(C)]
pub struct RuneCefFrame {
    pub pixels: *const u8,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
}

pub type RuneCefBrowser = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
pub enum RuneMouseButton {
    RUNE_MOUSE_NONE = 0,
    RUNE_MOUSE_LEFT = 1,
    RUNE_MOUSE_MIDDLE = 2,
    RUNE_MOUSE_RIGHT = 3,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub enum RuneMouseEventKind {
    RUNE_MOUSE_MOVE = 0,
    RUNE_MOUSE_DOWN = 1,
    RUNE_MOUSE_UP = 2,
    RUNE_MOUSE_WHEEL = 3,
}

#[repr(C)]
pub struct RuneMouseEvent {
    pub x: i32,
    pub y: i32,
    pub kind: RuneMouseEventKind,
    pub button: RuneMouseButton,
    pub delta_x: i32,
    pub delta_y: i32,
    pub modifiers: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub enum RuneKeyEventKind {
    RUNE_KEY_DOWN = 0,
    RUNE_KEY_UP = 1,
    RUNE_KEY_CHAR = 2,
}

#[repr(C)]
pub struct RuneKeyEvent {
    pub key_code: u32,
    pub character: u32,
    pub kind: RuneKeyEventKind,
    pub modifiers: u32,
}

type RuneCefInitFn =
    unsafe extern "C" fn(*const c_char, *const c_char, *const c_char, c_int) -> c_int;
type RuneCefShutdownFn = unsafe extern "C" fn();
type RuneCefCreateBrowserFn =
    unsafe extern "C" fn(*const RuneCefConfig, *const c_char) -> RuneCefBrowser;
type RuneCefDestroyBrowserFn = unsafe extern "C" fn(RuneCefBrowser);
type RuneCefNavigateFn = unsafe extern "C" fn(RuneCefBrowser, *const c_char);
type RuneCefLoadHtmlFn =
    unsafe extern "C" fn(RuneCefBrowser, *const c_char, *const c_char);
type RuneCefDoMessageLoopWorkFn = unsafe extern "C" fn();
type RuneCefIsLoadingFn = unsafe extern "C" fn(RuneCefBrowser) -> c_int;
type RuneCefGetFrameFn =
    unsafe extern "C" fn(RuneCefBrowser, *mut RuneCefFrame) -> c_int;
type RuneCefSendMouseEventFn =
    unsafe extern "C" fn(RuneCefBrowser, *const RuneMouseEvent);
type RuneCefSendKeyEventFn =
    unsafe extern "C" fn(RuneCefBrowser, *const RuneKeyEvent);
type RuneCefResizeFn = unsafe extern "C" fn(RuneCefBrowser, c_uint, c_uint);

/// Dynamically loaded `rune_cef_shim` library.
pub(crate) struct ShimLibrary {
    #[allow(dead_code)]
    lib: Library,
    pub rune_cef_init: RuneCefInitFn,
    pub rune_cef_shutdown: RuneCefShutdownFn,
    pub rune_cef_create_browser: RuneCefCreateBrowserFn,
    pub rune_cef_destroy_browser: RuneCefDestroyBrowserFn,
    pub rune_cef_navigate: RuneCefNavigateFn,
    pub rune_cef_load_html: RuneCefLoadHtmlFn,
    pub rune_cef_do_message_loop_work: RuneCefDoMessageLoopWorkFn,
    pub rune_cef_is_loading: RuneCefIsLoadingFn,
    pub rune_cef_get_frame: RuneCefGetFrameFn,
    pub rune_cef_send_mouse_event: RuneCefSendMouseEventFn,
    pub rune_cef_send_key_event: RuneCefSendKeyEventFn,
    pub rune_cef_resize: RuneCefResizeFn,
}

impl ShimLibrary {
    fn load() -> Result<Self> {
        let lib_name = Self::library_name();

        let lib = if let Ok(path) = std::env::var("RUNE_CEF_SHIM_PATH") {
            let path = PathBuf::from(path);
            let lib_path = if path.is_file() {
                path
            } else {
                path.join(&lib_name)
            };
            eprintln!(
                "CEF shim: Loading rune_cef_shim from: {}",
                lib_path.display()
            );
            unsafe { Library::new(&lib_path) }
        } else {
            eprintln!(
                "CEF shim: Loading rune_cef_shim from system path: {}",
                lib_name
            );
            unsafe { Library::new(&lib_name) }
        }
        .map_err(|e| CefError::LibraryLoad(format!("rune_cef_shim: {}", e)))?;

        let (
            rune_cef_init,
            rune_cef_shutdown,
            rune_cef_create_browser,
            rune_cef_destroy_browser,
            rune_cef_navigate,
            rune_cef_load_html,
            rune_cef_do_message_loop_work,
            rune_cef_is_loading,
            rune_cef_get_frame,
            rune_cef_send_mouse_event,
            rune_cef_send_key_event,
            rune_cef_resize,
        ) = unsafe {
            let rune_cef_init: Symbol<RuneCefInitFn> =
                lib.get(b"rune_cef_init")
                    .map_err(|e| CefError::SymbolNotFound(format!("rune_cef_init: {}", e)))?;
            let rune_cef_shutdown: Symbol<RuneCefShutdownFn> =
                lib.get(b"rune_cef_shutdown")
                    .map_err(|e| CefError::SymbolNotFound(format!("rune_cef_shutdown: {}", e)))?;
            let rune_cef_create_browser: Symbol<RuneCefCreateBrowserFn> =
                lib.get(b"rune_cef_create_browser")
                    .map_err(|e| CefError::SymbolNotFound(format!("rune_cef_create_browser: {}", e)))?;
            let rune_cef_destroy_browser: Symbol<RuneCefDestroyBrowserFn> =
                lib.get(b"rune_cef_destroy_browser")
                    .map_err(|e| CefError::SymbolNotFound(format!("rune_cef_destroy_browser: {}", e)))?;
            let rune_cef_navigate: Symbol<RuneCefNavigateFn> =
                lib.get(b"rune_cef_navigate")
                    .map_err(|e| CefError::SymbolNotFound(format!("rune_cef_navigate: {}", e)))?;
            let rune_cef_load_html: Symbol<RuneCefLoadHtmlFn> =
                lib.get(b"rune_cef_load_html")
                    .map_err(|e| CefError::SymbolNotFound(format!("rune_cef_load_html: {}", e)))?;
            let rune_cef_do_message_loop_work: Symbol<RuneCefDoMessageLoopWorkFn> =
                lib.get(b"rune_cef_do_message_loop_work")
                    .map_err(|e| CefError::SymbolNotFound(format!("rune_cef_do_message_loop_work: {}", e)))?;
            let rune_cef_is_loading: Symbol<RuneCefIsLoadingFn> =
                lib.get(b"rune_cef_is_loading")
                    .map_err(|e| CefError::SymbolNotFound(format!("rune_cef_is_loading: {}", e)))?;
            let rune_cef_get_frame: Symbol<RuneCefGetFrameFn> =
                lib.get(b"rune_cef_get_frame")
                    .map_err(|e| CefError::SymbolNotFound(format!("rune_cef_get_frame: {}", e)))?;
            let rune_cef_send_mouse_event: Symbol<RuneCefSendMouseEventFn> =
                lib.get(b"rune_cef_send_mouse_event")
                    .map_err(|e| CefError::SymbolNotFound(format!("rune_cef_send_mouse_event: {}", e)))?;
            let rune_cef_send_key_event: Symbol<RuneCefSendKeyEventFn> =
                lib.get(b"rune_cef_send_key_event")
                    .map_err(|e| CefError::SymbolNotFound(format!("rune_cef_send_key_event: {}", e)))?;
            let rune_cef_resize: Symbol<RuneCefResizeFn> =
                lib.get(b"rune_cef_resize")
                    .map_err(|e| CefError::SymbolNotFound(format!("rune_cef_resize: {}", e)))?;

            (
                *rune_cef_init,
                *rune_cef_shutdown,
                *rune_cef_create_browser,
                *rune_cef_destroy_browser,
                *rune_cef_navigate,
                *rune_cef_load_html,
                *rune_cef_do_message_loop_work,
                *rune_cef_is_loading,
                *rune_cef_get_frame,
                *rune_cef_send_mouse_event,
                *rune_cef_send_key_event,
                *rune_cef_resize,
            )
        };

        Ok(Self {
            lib,
            rune_cef_init,
            rune_cef_shutdown,
            rune_cef_create_browser,
            rune_cef_destroy_browser,
            rune_cef_navigate,
            rune_cef_load_html,
            rune_cef_do_message_loop_work,
            rune_cef_is_loading,
            rune_cef_get_frame,
            rune_cef_send_mouse_event,
            rune_cef_send_key_event,
            rune_cef_resize,
        })
    }

    #[cfg(target_os = "windows")]
    fn library_name() -> String {
        "rune_cef_shim.dll".to_string()
    }

    #[cfg(target_os = "macos")]
    fn library_name() -> String {
        "librune_cef_shim.dylib".to_string()
    }

    #[cfg(target_os = "linux")]
    fn library_name() -> String {
        "librune_cef_shim.so".to_string()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    fn library_name() -> String {
        "librune_cef_shim.so".to_string()
    }
}

static SHIM_LIBRARY: OnceLock<Arc<ShimLibrary>> = OnceLock::new();

/// Get the globally loaded shim library instance.
pub(crate) fn get_shim_library() -> Result<Arc<ShimLibrary>> {
    if let Some(lib) = SHIM_LIBRARY.get() {
        return Ok(Arc::clone(lib));
    }

    let lib = ShimLibrary::load().map(Arc::new)?;
    // In case of concurrent initialization, prefer the one that won the race.
    if SHIM_LIBRARY.set(Arc::clone(&lib)).is_err() {
        if let Some(existing) = SHIM_LIBRARY.get() {
            return Ok(Arc::clone(existing));
        }
    }
    Ok(lib)
}
