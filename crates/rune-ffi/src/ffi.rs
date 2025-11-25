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

    if let Some(()) = with_renderer(|r| {
        if r.needs_redraw() {
            r.render();
        }
    }) {
        // Render succeeded (or no redraw was needed)
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

/// Handle scroll (mouse wheel / trackpad) event.
///
/// # Arguments
/// * `delta_x` - Horizontal scroll delta (logical pixels)
/// * `delta_y` - Vertical scroll delta (logical pixels)
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_scroll(delta_x: f32, delta_y: f32) {
    with_renderer(|r| r.scroll(delta_x, delta_y));
}

/// Handle key event.
///
/// # Arguments
/// * `keycode` - Virtual keycode
/// * `modifiers` - Bitmask of modifier keys (RUNE_MODIFIER_*)
/// * `pressed` - true for key down, false for key up
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_key_event(keycode: u32, modifiers: u32, pressed: bool) {
    use winit::keyboard::ModifiersState;

    fn modifiers_from_bits(bits: u32) -> ModifiersState {
        let mut m = ModifiersState::empty();
        // Keep these in sync with RUNE_MODIFIER_* flags in rune_ffi.h
        if bits & (1 << 0) != 0 {
            m |= ModifiersState::SHIFT;
        }
        if bits & (1 << 1) != 0 {
            m |= ModifiersState::CONTROL;
        }
        if bits & (1 << 2) != 0 {
            m |= ModifiersState::ALT;
        }
        if bits & (1 << 3) != 0 {
            m |= ModifiersState::SUPER;
        }
        m
    }

    let modifiers_state = modifiers_from_bits(modifiers);
    with_renderer(|r| r.key_event(keycode, pressed, modifiers_state));
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
/// Returns false if no webview element exists in the current IR view.
/// When a webview element exists, CEF should be shown at that position.
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

    // Try rune_surface first (has transformed screen coords from webview element)
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

    // No webview element in the current IR - CEF should be hidden
    false
}

/// Check if navigation mode is Browser (CEF visible) vs Home/IRApp.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_is_browser_mode() -> bool {
    use rune_scene::navigation::{get_navigation_mode, NavigationMode};
    matches!(get_navigation_mode(), NavigationMode::Browser)
}

/// Check if the dock overlay is currently visible.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_is_dock_visible() -> bool {
    with_renderer(|r| r.is_dock_visible()).unwrap_or(false)
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

    eprintln!(
        "[rune_ffi] update_navigation_state: url={:?} back={} fwd={} loading={}",
        url_str, can_go_back, can_go_forward, is_loading
    );

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

/// Get the current page title from navigation state.
/// Returns NULL if no title is set.
/// The returned string must be freed with rune_ffi_free_string.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_get_current_title() -> *mut c_char {
    match rune_scene::navigation::get_current_title() {
        Some(title) => match std::ffi::CString::new(title) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        None => std::ptr::null_mut(),
    }
}

/// Set the current page title.
/// Called by the native side when CEF reports a title change.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_set_current_title(title: *const c_char) {
    let title_str = if title.is_null() {
        None
    } else {
        unsafe { CStr::from_ptr(title).to_str().ok().map(String::from) }
    };

    log::debug!("rune_ffi_set_current_title: {:?}", title_str);
    rune_scene::navigation::set_current_title(title_str);
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
///
/// IMPORTANT: This function checks the current navigation mode before updating
/// the address bar. When in IR mode (Home or IRApp), we ignore CEF URL updates
/// to prevent the address bar from flickering with stale CEF URLs.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_set_address_bar_url(url: *const c_char) {
    if url.is_null() {
        eprintln!("[rune_ffi] set_address_bar_url: url is null");
        return;
    }

    let url_str = match unsafe { CStr::from_ptr(url).to_str() } {
        Ok(s) => s,
        Err(_) => {
            eprintln!("[rune_ffi] set_address_bar_url: invalid UTF-8");
            return;
        }
    };

    // Check if we're in IR mode - if so, ignore CEF URL updates
    // to prevent overwriting the IR navigation URL in the address bar
    let nav_mode = rune_scene::navigation::get_navigation_mode();
    let in_ir_mode = matches!(
        nav_mode,
        rune_scene::navigation::NavigationMode::Home | rune_scene::navigation::NavigationMode::IRApp
    );

    if in_ir_mode {
        // Check if this is an IR URL (allow IR URLs to update even in IR mode)
        let render_target = rune_scene::navigation::determine_render_target(url_str);
        if render_target != rune_scene::navigation::RenderTarget::Ir {
            eprintln!(
                "[rune_ffi] set_address_bar_url: IGNORED (in IR mode, CEF URL: {})",
                url_str
            );
            return;
        }
    }

    eprintln!("[rune_ffi] set_address_bar_url: {}", url_str);

    let result = with_renderer(|r| {
        r.set_address_bar_text(url_str);
    });
    if result.is_none() {
        eprintln!("[rune_ffi] set_address_bar_url: renderer not available (wrong thread?)");
    } else {
        eprintln!("[rune_ffi] set_address_bar_url: SUCCESS");
    }
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

// ============================================================================
// DevTools FFI Functions
// ============================================================================

/// Flag for signaling DevTools toggle request.
static DEVTOOLS_TOGGLE_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Signal that Chrome DevTools should be toggled.
/// Called by the Rust side when the devtools button is clicked.
pub fn request_cef_devtools_toggle() {
    DEVTOOLS_TOGGLE_REQUESTED.store(true, Ordering::SeqCst);
    log::info!("CEF DevTools toggle requested");
}

/// Check if Chrome DevTools toggle was requested.
/// The native side should poll this and open DevTools when true.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_devtools_toggle_requested() -> bool {
    DEVTOOLS_TOGGLE_REQUESTED.swap(false, Ordering::SeqCst)
}

/// Get the height of the DevTools zone in logical pixels.
/// Returns 0.0 if DevTools is not visible or the renderer is not initialized.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_get_devtools_height() -> f32 {
    with_renderer(|r| r.get_devtools_height()).unwrap_or(0.0)
}

/// Log a message to the DevTools console.
/// Level: 0 = Log, 1 = Warn, 2 = Error.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_devtools_console_log(level: u32, msg: *const c_char) {
    if msg.is_null() {
        return;
    }

    let message = match unsafe { CStr::from_ptr(msg).to_str() } {
        Ok(s) => s.to_string(),
        Err(_) => return,
    };

    let level_enum = match level {
        1 => rune_scene::zones::ConsoleLevel::Warn,
        2 => rune_scene::zones::ConsoleLevel::Error,
        _ => rune_scene::zones::ConsoleLevel::Log,
    };

    with_renderer(|r| {
        r.devtools_console_log(level_enum, message);
    });
}

/// Clear all DevTools console entries.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_devtools_console_clear() {
    with_renderer(|r| {
        r.devtools_console_clear();
    });
}

// ============================================================================
// Bookmark FFI Functions
// ============================================================================

/// Add a bookmark for the current page.
/// Uses the current URL and title from navigation state.
/// Returns true if a bookmark was added, false otherwise (e.g., no URL available).
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_add_bookmark() -> bool {
    let current_url = rune_scene::navigation::get_current_url();

    if let Some(url) = current_url {
        if url.trim().is_empty() {
            return false;
        }

        // Use the CEF page title if available, otherwise derive from URL
        let title = rune_scene::navigation::get_current_title()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| {
                url.split('/')
                    .last()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Bookmark")
                    .to_string()
            });

        log::info!("Adding bookmark via FFI: {} -> {}", title, url);

        with_renderer(|r| {
            r.zone_manager.sidebar.add_bookmark(title.clone(), url.clone());
            r.needs_redraw = true;
        });

        true
    } else {
        false
    }
}

/// Open a new tab by navigating to a blank page.
/// Creates a new tab entry and makes it active.
/// This clears the current page and focuses the address bar for user input.
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_new_tab() {
    log::info!("New tab requested via FFI");

    // Create a new blank tab and make it active
    // Use empty string to indicate a new tab page (not a real URL)
    with_renderer(|r| {
        r.zone_manager.sidebar.new_tab("New Tab".to_string(), String::new());
    });

    // Don't navigate - just clear the address bar and focus it
    // The user will type a URL and hit Enter to navigate
    with_renderer(|r| {
        r.zone_manager.toolbar.address_bar.set_text("");
        r.zone_manager.toolbar.address_bar.focused = true;
        r.needs_redraw = true;
    });
}

/// Update the active tab with the current navigation URL and title.
/// Call this when CEF navigation state changes (URL or title update).
#[unsafe(no_mangle)]
pub extern "C" fn rune_ffi_update_active_tab() {
    let current_url = rune_scene::navigation::get_current_url();
    let current_title = rune_scene::navigation::get_current_title();

    if let Some(url) = current_url {
        if url.trim().is_empty() {
            return;
        }

        let title = current_title
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| {
                url.split('/')
                    .last()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Tab")
                    .to_string()
            });

        with_renderer(|r| {
            // If no active tab exists, create one
            if r.zone_manager.sidebar.active_tab().is_none() {
                r.zone_manager.sidebar.new_tab(title.clone(), url.clone());
                log::info!("Created initial tab: {} -> {}", title, url);
            } else {
                // Update the active tab
                if r.zone_manager.sidebar.update_active_tab(title.clone(), url.clone()) {
                    log::debug!("Updated active tab: {} -> {}", title, url);
                }
            }
            r.needs_redraw = true;
        });
    }
}
