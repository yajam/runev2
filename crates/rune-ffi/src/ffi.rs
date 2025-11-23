//! FFI exports for embedding rune-scene in native applications.
//!
//! These functions are exported as C symbols for use from Objective-C/Swift.

use std::ffi::{c_char, c_void, CStr};

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
pub extern "C" fn rune_init(
    width: u32,
    height: u32,
    scale: f32,
    metal_layer: *mut c_void,
    package_path: *const c_char,
) -> bool {
    // Initialize logging
    let _ = env_logger::try_init();

    log::info!(
        "rune_init: width={} height={} scale={} layer={:p}",
        width, height, scale, metal_layer
    );

    if metal_layer.is_null() {
        log::error!("rune_init: metal_layer is null");
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
            log::info!("rune_init: success");
            true
        }
        Err(e) => {
            log::error!("rune_init: failed: {:?}", e);
            false
        }
    }
}

/// Shutdown the renderer and release resources.
#[unsafe(no_mangle)]
pub extern "C" fn rune_shutdown() {
    log::info!("rune_shutdown");
    set_renderer(None);
}

/// Resize the viewport.
///
/// # Arguments
/// * `width` - New width in physical pixels
/// * `height` - New height in physical pixels
#[unsafe(no_mangle)]
pub extern "C" fn rune_resize(width: u32, height: u32) {
    log::debug!("rune_resize: width={} height={}", width, height);
    with_renderer(|r| r.resize(width, height));
}

/// Render a single frame.
///
/// Call this from the display link callback.
#[unsafe(no_mangle)]
pub extern "C" fn rune_render() {
    with_renderer(|r| r.render());
}

/// Handle mouse click event.
///
/// # Arguments
/// * `x` - X position in physical pixels
/// * `y` - Y position in physical pixels
/// * `pressed` - true for mouse down, false for mouse up
#[unsafe(no_mangle)]
pub extern "C" fn rune_mouse_click(x: f32, y: f32, pressed: bool) {
    with_renderer(|r| r.mouse_click(x, y, pressed));
}

/// Handle mouse move event.
///
/// # Arguments
/// * `x` - X position in physical pixels
/// * `y` - Y position in physical pixels
#[unsafe(no_mangle)]
pub extern "C" fn rune_mouse_move(x: f32, y: f32) {
    with_renderer(|r| r.mouse_move(x, y));
}

/// Handle key event.
///
/// # Arguments
/// * `keycode` - Virtual keycode
/// * `pressed` - true for key down, false for key up
#[unsafe(no_mangle)]
pub extern "C" fn rune_key_event(keycode: u32, pressed: bool) {
    with_renderer(|r| r.key_event(keycode, pressed));
}

/// Check if redraw is needed.
///
/// # Returns
/// `true` if renderer needs redraw
#[unsafe(no_mangle)]
pub extern "C" fn rune_needs_redraw() -> bool {
    with_renderer(|r| r.needs_redraw()).unwrap_or(false)
}

/// Request a redraw.
#[unsafe(no_mangle)]
pub extern "C" fn rune_request_redraw() {
    with_renderer(|r| r.request_redraw());
}
