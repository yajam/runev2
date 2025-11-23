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

// NOTE: rune_ffi_upload_webview_pixels removed - now using native NSView-based CEF rendering
// instead of OSR pixel uploads. See docs/NSview.md for architecture details.

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

/// Set the native CEF view handle for positioning.
/// This is used for native NSView-based CEF rendering instead of OSR.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_set_cef_view(cef_view: *mut c_void) {
    log::info!("rune_ffi_set_cef_view: view={:p}", cef_view);
    rune_scene::elements::webview::set_native_cef_view(cef_view);
}

/// Update the position of the native CEF view based on viewport layout.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_position_cef_view(x: f32, y: f32, width: f32, height: f32) {
    log::debug!(
        "rune_ffi_position_cef_view: x={} y={} w={} h={}",
        x, y, width, height
    );
    rune_scene::elements::webview::position_native_cef_view(x, y, width, height);
}

/// Get the current WebView rect for positioning the native CEF view.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_get_webview_rect(
    x: *mut f32,
    y: *mut f32,
    width: *mut f32,
    height: *mut f32,
) -> bool {
    if x.is_null() || y.is_null() || width.is_null() || height.is_null() {
        return false;
    }

    // Try rune_surface first (has transformed screen coords)
    if let Some((rx, ry, rw, rh)) = rune_surface::get_last_raw_image_rect() {
        unsafe {
            *x = rx;
            *y = ry;
            *width = rw;
            *height = rh;
        }
        return true;
    }

    // Fall back to rune_scene (has viewport-local coords from layout)
    if let Some((rx, ry, rw, rh)) = rune_scene::elements::webview::get_webview_rect() {
        unsafe {
            *x = rx;
            *y = ry;
            *width = rw;
            *height = rh;
        }
        return true;
    }

    false
}

// ============================================================================
// Navigation FFI Functions
// ============================================================================

/// Navigation command types for the native side.
/// Must match the values expected by the Objective-C code.
#[repr(C)]
pub struct NavigationCommand {
    /// Command type: 0=LoadUrl, 1=GoBack, 2=GoForward, 3=Reload, 4=Stop
    pub command_type: u32,
    /// URL for LoadUrl command (null-terminated, must be freed with rune_ffi_free_string)
    pub url: *mut c_char,
}

/// Check if there are pending navigation commands.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_has_navigation_command() -> bool {
    rune_scene::navigation::has_pending_commands()
}

/// Pop the next navigation command from the queue.
/// Returns a NavigationCommand struct with command_type and optional url.
/// The url field must be freed with rune_ffi_free_string if not null.
/// Returns command_type=255 if no command is available.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_pop_navigation_command() -> NavigationCommand {
    use rune_scene::navigation::NavigationCommand as NavCmd;

    match rune_scene::navigation::pop_navigation_command() {
        Some(NavCmd::LoadUrl(url)) => {
            let url_cstr = match std::ffi::CString::new(url) {
                Ok(s) => s.into_raw(),
                Err(_) => std::ptr::null_mut(),
            };
            NavigationCommand {
                command_type: 0, // LoadUrl
                url: url_cstr,
            }
        }
        Some(NavCmd::GoBack) => NavigationCommand {
            command_type: 1,
            url: std::ptr::null_mut(),
        },
        Some(NavCmd::GoForward) => NavigationCommand {
            command_type: 2,
            url: std::ptr::null_mut(),
        },
        Some(NavCmd::Reload) => NavigationCommand {
            command_type: 3,
            url: std::ptr::null_mut(),
        },
        Some(NavCmd::Stop) => NavigationCommand {
            command_type: 4,
            url: std::ptr::null_mut(),
        },
        None => NavigationCommand {
            command_type: 255, // No command
            url: std::ptr::null_mut(),
        },
    }
}

/// Get the render target for a URL.
/// Returns 0 for IR rendering, 1 for CEF rendering.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_get_render_target(url: *const c_char) -> u32 {
    use rune_scene::navigation::{determine_render_target, RenderTarget};

    if url.is_null() {
        return 1; // Default to CEF
    }

    let url_str = match unsafe { CStr::from_ptr(url).to_str() } {
        Ok(s) => s,
        Err(_) => return 1,
    };

    match determine_render_target(url_str) {
        RenderTarget::Ir => 0,
        RenderTarget::Cef => 1,
    }
}

/// Update navigation state from CEF.
/// Called by the native side when CEF reports state changes.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_update_navigation_state(
    url: *const c_char,
    can_go_back: bool,
    can_go_forward: bool,
    is_loading: bool,
) {
    let url_str = if url.is_null() {
        None
    } else {
        unsafe { CStr::from_ptr(url).to_str().ok().map(String::from) }
    };

    rune_scene::navigation::update_state(url_str, can_go_back, can_go_forward, is_loading);
}

/// Get the current URL from navigation state.
/// Returns NULL if no URL is set.
/// The returned string must be freed with rune_ffi_free_string.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_get_current_url() -> *mut c_char {
    match rune_scene::navigation::get_current_url() {
        Some(url) => match std::ffi::CString::new(url) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        None => std::ptr::null_mut(),
    }
}

/// Get current render target.
/// Returns 0 for IR, 1 for CEF.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_get_current_render_target() -> u32 {
    use rune_scene::navigation::{get_render_target, RenderTarget};

    match get_render_target() {
        RenderTarget::Ir => 0,
        RenderTarget::Cef => 1,
    }
}

/// Update the address bar text.
/// Called by the native side when CEF navigates to a new URL.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_set_address_bar_url(url: *const c_char) {
    if url.is_null() {
        return;
    }

    let url_str = match unsafe { CStr::from_ptr(url).to_str() } {
        Ok(s) => s,
        Err(_) => return,
    };

    log::debug!("Setting address bar URL: {}", url_str);

    with_renderer(|r| {
        r.set_address_bar_text(url_str);
    });
}

/// Check if a page is currently loading.
/// Returns true if the browser is loading a page.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_is_loading() -> bool {
    rune_scene::navigation::get_state().is_loading
}

/// Update the toolbar loading state and spinner animation.
/// Call this each frame to keep the spinner animating while loading.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_update_toolbar_loading() {
    with_renderer(|r| {
        r.update_toolbar_loading();
    });
}
