//! FFI exports for embedding cef-demo in native applications.
//!
//! These functions are exported as C symbols for use from Objective-C/Swift.
//! CEF initialization and browser management is handled by the Xcode side;
//! this library just handles the wgpu rendering.

use std::ffi::c_void;

use crate::{get_renderer, AppRenderer};

/// Initialize the renderer with a CAMetalLayer.
///
/// # Arguments
/// * `width` - Initial width in physical pixels
/// * `height` - Initial height in physical pixels
/// * `scale` - Device scale factor (e.g., 2.0 for Retina)
/// * `metal_layer` - Pointer to CAMetalLayer
///
/// # Returns
/// `true` on success, `false` on failure
#[unsafe(no_mangle)]
pub extern "C" fn cef_demo_init(
    width: u32,
    height: u32,
    scale: f32,
    metal_layer: *mut c_void,
) -> bool {
    // Initialize logging (ignore errors if already initialized)
    let _ = env_logger::try_init();

    log::info!("cef_demo_init: width={} height={} scale={} layer={:p}",
               width, height, scale, metal_layer);

    if metal_layer.is_null() {
        log::error!("cef_demo_init: metal_layer is null");
        return false;
    }

    match AppRenderer::new(width, height, scale, metal_layer) {
        Ok(renderer) => {
            let mut guard = get_renderer().lock();
            *guard = Some(renderer);
            log::info!("cef_demo_init: success");
            true
        }
        Err(e) => {
            log::error!("cef_demo_init: failed: {:?}", e);
            false
        }
    }
}

/// Shutdown the renderer and release resources.
#[unsafe(no_mangle)]
pub extern "C" fn cef_demo_shutdown() {
    log::info!("cef_demo_shutdown");
    let mut guard = get_renderer().lock();
    *guard = None;
}

/// Upload pixel data from CEF OnPaint callback.
///
/// # Arguments
/// * `pixels` - Pointer to pixel data in BGRA format
/// * `width` - Width in pixels
/// * `height` - Height in pixels
/// * `stride` - Bytes per row (usually width * 4)
#[unsafe(no_mangle)]
pub extern "C" fn cef_demo_upload_pixels(
    pixels: *const u8,
    width: u32,
    height: u32,
    stride: u32,
) {
    if pixels.is_null() || width == 0 || height == 0 {
        return;
    }

    let byte_count = (stride * height) as usize;
    let pixel_slice = unsafe { std::slice::from_raw_parts(pixels, byte_count) };

    let mut guard = get_renderer().lock();
    if let Some(renderer) = guard.as_mut() {
        renderer.upload_pixels(pixel_slice, width, height);
    }
}

/// Resize the viewport.
///
/// # Arguments
/// * `width` - New width in physical pixels
/// * `height` - New height in physical pixels
#[unsafe(no_mangle)]
pub extern "C" fn cef_demo_resize(width: u32, height: u32) {
    log::debug!("cef_demo_resize: width={} height={}", width, height);

    let mut guard = get_renderer().lock();
    if let Some(renderer) = guard.as_mut() {
        renderer.resize(width, height);
    }
}

/// Render a single frame.
///
/// Call this from the display link callback or MTKView draw callback.
#[unsafe(no_mangle)]
pub extern "C" fn cef_demo_render() {
    let mut guard = get_renderer().lock();
    if let Some(renderer) = guard.as_mut() {
        renderer.render();
    }
}
