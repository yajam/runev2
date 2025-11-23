//! FFI exports for embedding rune-scene in native applications.
//!
//! These functions are exported as C symbols for use from Objective-C/Swift.

use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Once;

use crate::{set_renderer, with_renderer, AppRenderer};

/// Initialize the renderer with a CAMetalLayer.
///
/// # Arguments
/// * `width` - Initial width in physical pixels
/// * `height` - Initial height in physical pixels
/// * `scale` - Device scale factor (e.g., 2.0 for Retina)
/// * `metal_layer` - Pointer to CAMetalLayer
/// * `package_path` - Optional path to IR package directory (null for default)
///
/// # Returns
/// `true` on success, `false` on failure
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_init(
    width: u32,
    height: u32,
    scale: f32,
    metal_layer: *mut c_void,
    package_path: *const c_char,
) -> bool {
    // Initialize logging
    let _ = env_logger::try_init();

    log::info!(
        "rune_ffi_init: width={} height={} scale={} layer={:p}",
        width, height, scale, metal_layer
    );

    if metal_layer.is_null() {
        log::error!("rune_ffi_init: metal_layer is null");
        return false;
    }

    // Convert package path
    let path_str = if package_path.is_null() {
        None
    } else {
        unsafe { CStr::from_ptr(package_path).to_str().ok() }
    };

    match AppRenderer::new(width, height, scale, metal_layer, path_str) {
        Ok(renderer) => {
            set_renderer(Some(renderer));
            log::info!("rune_ffi_init: success");
            true
        }
        Err(e) => {
            log::error!("rune_ffi_init: failed: {:?}", e);
            false
        }
    }
}

/// Shutdown the renderer and release resources.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_shutdown() {
    log::info!("rune_ffi_shutdown");
    set_renderer(None);
}

/// Upload pixel data from CEF for a WebView element.
///
/// # Arguments
/// * `webview_id` - Identifier for the WebView element (currently unused, reserved for multi-webview support)
/// * `pixels` - Pointer to pixel data in BGRA format
/// * `width` - Width in pixels
/// * `height` - Height in pixels
/// * `stride` - Bytes per row (usually width * 4)
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_upload_webview_pixels(
    _webview_id: *const c_char,
    pixels: *const u8,
    width: u32,
    height: u32,
    stride: u32,
) {
    if pixels.is_null() || width == 0 || height == 0 {
        return;
    }

    // Calculate expected size
    let expected_size = (height * stride) as usize;

    // Create slice from raw pointer
    let pixel_slice = unsafe { std::slice::from_raw_parts(pixels, expected_size) };

    // Upload to the global WebView texture
    with_renderer(|r| {
        r.upload_webview_pixels(pixel_slice, width, height);
    });
}

/// Resize the viewport.
///
/// # Arguments
/// * `width` - New width in physical pixels
/// * `height` - New height in physical pixels
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_resize(width: u32, height: u32) {
    log::debug!("rune_ffi_resize: width={} height={}", width, height);
    with_renderer(|r| r.resize(width, height));
}

/// Render a single frame.
///
/// Call this from the display link callback.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_render() {
    // Optional debug path: allow freezing the scene after the initial load
    // to diagnose issues where repeated redraws might overwrite content.
    //
    // Enable by setting RUNE_FREEZE_AFTER_LOAD=1 in the environment before
    // launching the host app (e.g., in the Xcode scheme).
    static FREEZE_ENABLED: AtomicBool = AtomicBool::new(false);
    static INIT_FREEZE_FLAG: Once = Once::new();
    static FRAME_COUNT: AtomicU32 = AtomicU32::new(0);

    INIT_FREEZE_FLAG.call_once(|| {
        if let Ok(val) = std::env::var("RUNE_FREEZE_AFTER_LOAD") {
            let enabled = val == "1" || val.eq_ignore_ascii_case("true");
            FREEZE_ENABLED.store(enabled, Ordering::Relaxed);
            if enabled {
                log::info!("rune_ffi_render: RUNE_FREEZE_AFTER_LOAD enabled");
            }
        }
    });

    if FREEZE_ENABLED.load(Ordering::Relaxed) {
        let frame = FRAME_COUNT.fetch_add(1, Ordering::Relaxed);
        // After 120 frames (~2 seconds at 60fps), stop redrawing so the last
        // good frame remains visible for debugging.
        if frame > 120 {
            return;
        }
    }

    if let Some(()) = with_renderer(|r| r.render()) {
        // Render succeeded
    } else {
        log::warn!("rune_ffi_render: renderer not available (wrong thread?)");
    }
}

/// Handle mouse click event.
///
/// # Arguments
/// * `x` - X position in physical pixels
/// * `y` - Y position in physical pixels
/// * `pressed` - true for mouse down, false for mouse up
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_mouse_click(x: f32, y: f32, pressed: bool) {
    with_renderer(|r| r.mouse_click(x, y, pressed));
}

/// Handle mouse move event.
///
/// # Arguments
/// * `x` - X position in physical pixels
/// * `y` - Y position in physical pixels
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_mouse_move(x: f32, y: f32) {
    with_renderer(|r| r.mouse_move(x, y));
}

/// Handle key event.
///
/// # Arguments
/// * `keycode` - Virtual keycode
/// * `pressed` - true for key down, false for key up
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_key_event(keycode: u32, pressed: bool) {
    with_renderer(|r| r.key_event(keycode, pressed));
}

/// Handle committed text input (UTF-8).
///
/// # Arguments
/// * `text` - UTF-8 encoded text (null-terminated)
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_text_input(text: *const c_char) {
    if text.is_null() {
        return;
    }
    let s = unsafe { CStr::from_ptr(text) };
    if let Ok(str_slice) = s.to_str() {
        with_renderer(|r| r.text_input(str_slice));
    }
}

/// Check if redraw is needed.
///
/// # Returns
/// `true` if renderer needs redraw
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_needs_redraw() -> bool {
    with_renderer(|r| r.needs_redraw()).unwrap_or(false)
}

/// Request a redraw.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_request_redraw() {
    with_renderer(|r| r.request_redraw());
}

/// Get the WebView URL from the loaded IR package.
/// Returns NULL if no WebView element exists or no package is loaded.
/// The returned string must be freed by the caller using rune_ffi_free_string.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_get_webview_url() -> *mut c_char {
    let url = with_renderer(|r| r.get_webview_url()).flatten();

    match url {
        Some(url_str) => match std::ffi::CString::new(url_str) {
            Ok(cstr) => cstr.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        None => std::ptr::null_mut(),
    }
}

/// Free a string allocated by rune_ffi_get_webview_url.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = std::ffi::CString::from_raw(s);
        }
    }
}

/// Get the WebView element size from the loaded IR package.
/// Returns the width and height via out parameters.
/// Returns true if a WebView element exists, false otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_get_webview_size(width: *mut u32, height: *mut u32) -> bool {
    if width.is_null() || height.is_null() {
        return false;
    }

    let size = with_renderer(|r| r.get_webview_size()).flatten();

    match size {
        Some((w, h)) => {
            unsafe {
                *width = w;
                *height = h;
            }
            true
        }
        None => false,
    }
}

/// Get the WebView element position from the rendered scene.
///
/// # Safety
/// The x and y pointers must be valid.
#[no_mangle]
pub extern "C" fn rune_ffi_get_webview_position(x: *mut f32, y: *mut f32) -> bool {
    if x.is_null() || y.is_null() {
        return false;
    }

    // Try rune_surface first (has transformed screen coords when pixels are rendered)
    if let Some((rx, ry, _w, _h)) = rune_surface::get_last_raw_image_rect() {
        unsafe {
            *x = rx;
            *y = ry;
        }
        return true;
    }

    // Fall back to rune_scene (has viewport-local coords from layout)
    if let Some((rx, ry, _w, _h)) = rune_scene::elements::webview::get_webview_rect() {
        unsafe {
            *x = rx;
            *y = ry;
        }
        return true;
    }

    false
}
