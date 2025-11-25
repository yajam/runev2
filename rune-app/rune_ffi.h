/*
 * Rune FFI Header
 *
 * C interface for the Rust rune-ffi library.
 * This library provides the rune-scene IR renderer with wgpu.
 * CEF initialization and browser management is handled by the Xcode side.
 */

#ifndef RUNE_FFI_H
#define RUNE_FFI_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Initialize the renderer with a CAMetalLayer.
 *
 * @param width Initial width in physical pixels
 * @param height Initial height in physical pixels
 * @param scale Device scale factor (e.g., 2.0 for Retina)
 * @param metal_layer Pointer to CAMetalLayer (cast to void*)
 * @param package_path Optional path to IR package directory (NULL for default)
 * @return true on success, false on failure
 */
bool rune_ffi_init(uint32_t width, uint32_t height, float scale, void* metal_layer, const char* package_path);

/*
 * Shutdown the renderer and release resources.
 */
void rune_ffi_shutdown(void);

/*
 * Upload pixel data from CEF OnPaint callback for WebView element.
 *
 * @param webview_id Identifier for the WebView element
 * @param pixels Pointer to pixel data in BGRA format
 * @param width Width in pixels
 * @param height Height in pixels
 * @param stride Bytes per row (usually width * 4)
 */
void rune_ffi_upload_webview_pixels(const char* webview_id, const uint8_t* pixels, uint32_t width, uint32_t height, uint32_t stride);

/*
 * Resize the viewport.
 *
 * @param width New width in physical pixels
 * @param height New height in physical pixels
 */
void rune_ffi_resize(uint32_t width, uint32_t height);

/*
 * Render a single frame.
 *
 * Call this from the display link callback or MTKView draw callback.
 */
void rune_ffi_render(void);

/*
 * Handle mouse click event.
 *
 * @param x X coordinate in physical pixels
 * @param y Y coordinate in physical pixels
 * @param pressed true if mouse button pressed, false if released
 */
void rune_ffi_mouse_click(float x, float y, bool pressed);

/*
 * Handle mouse move event.
 *
 * @param x X coordinate in physical pixels
 * @param y Y coordinate in physical pixels
 */
void rune_ffi_mouse_move(float x, float y);

/*
 * Handle scroll (mouse wheel / trackpad) event.
 *
 * @param delta_x Horizontal scroll delta (logical pixels)
 * @param delta_y Vertical scroll delta (logical pixels)
 */
void rune_ffi_scroll(float delta_x, float delta_y);

/*
 * Handle key event.
 *
 * @param keycode Virtual key code
 * @param modifiers Bitmask of modifier keys (see RUNE_MODIFIER_* flags)
 * @param pressed true if key pressed, false if released
 */
void rune_ffi_key_event(uint32_t keycode, uint32_t modifiers, bool pressed);

/*
 * Modifier key bitmask flags for rune_ffi_key_event.
 *
 * These map directly to winit's ModifiersState on the Rust side.
 */
#define RUNE_MODIFIER_SHIFT   (1u << 0)
#define RUNE_MODIFIER_CONTROL (1u << 1)
#define RUNE_MODIFIER_ALT     (1u << 2)
#define RUNE_MODIFIER_SUPER   (1u << 3)

/*
 * Check if redraw is needed.
 *
 * @return true if the scene needs to be redrawn
 */
bool rune_ffi_needs_redraw(void);

/*
 * Request a redraw on next frame.
 */
void rune_ffi_request_redraw(void);

/*
 * Deliver committed text input to the IR runtime.
 *
 * @param text UTF-8 encoded, null-terminated string
 */
void rune_ffi_text_input(const char* text);

/*
 * Get the WebView URL from the loaded IR package.
 *
 * @return URL string (must be freed with rune_ffi_free_string), or NULL if no WebView exists
 */
char* rune_ffi_get_webview_url(void);

/*
 * Free a string allocated by rune_ffi_get_webview_url.
 *
 * @param s String to free (may be NULL)
 */
void rune_ffi_free_string(char* s);

/*
 * Get the WebView element size from the loaded IR package.
 *
 * @param width Output: WebView width in logical pixels
 * @param height Output: WebView height in logical pixels
 * @return true if a WebView element exists, false otherwise
 */
bool rune_ffi_get_webview_size(uint32_t* width, uint32_t* height);

/*
 * Get the WebView element position from the loaded IR package.
 *
 * @param x Output: WebView x position in logical pixels
 * @param y Output: WebView y position in logical pixels
 * @return true if a WebView element exists, false otherwise
 */
bool rune_ffi_get_webview_position(float* x, float* y);

/*
 * Set the native CEF view handle (NSView*) for the renderer.
 * This is used for native NSView-based CEF rendering instead of OSR.
 *
 * @param cef_view Pointer to NSView (cast to void*)
 */
void rune_ffi_set_cef_view(void* cef_view);

/*
 * Update the position of the native CEF view based on viewport layout.
 * Call this after layout changes to reposition the CEF view.
 *
 * @param x X position in logical pixels
 * @param y Y position in logical pixels
 * @param width Width in logical pixels
 * @param height Height in logical pixels
 */
void rune_ffi_position_cef_view(float x, float y, float width, float height);

/*
 * Get the current WebView rect for positioning the native CEF view.
 * This combines position and size in a single call.
 *
 * @param x Output: X position in logical pixels
 * @param y Output: Y position in logical pixels
 * @param width Output: Width in logical pixels
 * @param height Output: Height in logical pixels
 * @return true if a WebView element exists, false otherwise
 */
bool rune_ffi_get_webview_rect(float* x, float* y, float* width, float* height);

/*
 * Check if navigation mode is Browser (CEF visible) vs Home/IRApp (CEF hidden).
 *
 * When this returns false, the native CEF view should be hidden regardless
 * of any previously reported WebView rect.
 *
 * @return true if in Browser mode, false otherwise
 */
bool rune_ffi_is_browser_mode(void);

/* ==========================================================================
 * Navigation API
 * ========================================================================== */

/*
 * Navigation command structure returned by rune_ffi_pop_navigation_command.
 */
typedef struct {
    /* Command type: 0=LoadUrl, 1=GoBack, 2=GoForward, 3=Reload, 4=Stop, 255=None */
    uint32_t command_type;
    /* URL for LoadUrl command (must be freed with rune_ffi_free_string), NULL otherwise */
    char* url;
} RuneNavigationCommand;

/* Navigation command type constants */
#define RUNE_NAV_LOAD_URL    0
#define RUNE_NAV_GO_BACK     1
#define RUNE_NAV_GO_FORWARD  2
#define RUNE_NAV_RELOAD      3
#define RUNE_NAV_STOP        4
#define RUNE_NAV_NONE        255

/* Render target constants */
#define RUNE_RENDER_IR       0
#define RUNE_RENDER_CEF      1

/*
 * Check if there are pending navigation commands.
 *
 * @return true if there are commands waiting to be processed
 */
bool rune_ffi_has_navigation_command(void);

/*
 * Pop the next navigation command from the queue.
 * If command_type is RUNE_NAV_LOAD_URL, the url field must be freed with rune_ffi_free_string.
 *
 * @return Navigation command structure (command_type=RUNE_NAV_NONE if no command)
 */
RuneNavigationCommand rune_ffi_pop_navigation_command(void);

/*
 * Get the render target for a URL.
 *
 * @param url The URL to check
 * @return RUNE_RENDER_IR (0) for IR rendering, RUNE_RENDER_CEF (1) for CEF rendering
 */
uint32_t rune_ffi_get_render_target(const char* url);

/*
 * Update navigation state from CEF.
 * Call this when CEF reports navigation state changes (e.g., after page load).
 *
 * @param url Current URL (may be NULL)
 * @param can_go_back Whether browser can go back
 * @param can_go_forward Whether browser can go forward
 * @param is_loading Whether a page is currently loading
 */
void rune_ffi_update_navigation_state(const char* url, bool can_go_back, bool can_go_forward, bool is_loading);

/*
 * Get the current URL from navigation state.
 *
 * @return URL string (must be freed with rune_ffi_free_string), or NULL
 */
char* rune_ffi_get_current_url(void);

/*
 * Get the current page title from navigation state.
 *
 * @return Title string (must be freed with rune_ffi_free_string), or NULL
 */
char* rune_ffi_get_current_title(void);

/*
 * Set the current page title.
 * Call this when CEF reports a title change (OnTitleChange).
 *
 * @param title The page title (may be NULL)
 */
void rune_ffi_set_current_title(const char* title);

/*
 * Get the current render target.
 *
 * @return RUNE_RENDER_IR (0) or RUNE_RENDER_CEF (1)
 */
uint32_t rune_ffi_get_current_render_target(void);

/*
 * Check if the Rune dock overlay is currently visible.
 *
 * @return true if dock overlay is visible, false otherwise
 */
bool rune_ffi_is_dock_visible(void);

/*
 * Update the address bar URL text.
 * Called when CEF navigates to a new URL to keep the address bar in sync.
 *
 * @param url The URL to display in the address bar
 */
void rune_ffi_set_address_bar_url(const char* url);

/*
 * Check if a page is currently loading.
 *
 * @return true if loading, false otherwise
 */
bool rune_ffi_is_loading(void);

/*
 * Update the toolbar loading state and spinner animation.
 * Call this each frame to animate the spinner while loading.
 */
void rune_ffi_update_toolbar_loading(void);

/* ==========================================================================
 * DevTools API
 * ========================================================================== */

/*
 * Check if Chrome DevTools toggle was requested.
 * Poll this after render to detect when the devtools button was clicked.
 * The flag is automatically cleared after being read.
 *
 * @return true if DevTools toggle was requested, false otherwise
 */
bool rune_ffi_devtools_toggle_requested(void);

/*
 * Get the height of the DevTools zone in logical pixels.
 * Returns 0.0 if DevTools is not visible.
 */
float rune_ffi_get_devtools_height(void);

/*
 * Log a message to the Rune DevTools console.
 *
 * @param level 0=Log, 1=Warn, 2=Error
 * @param msg   Null-terminated UTF-8 string
 */
void rune_ffi_devtools_console_log(uint32_t level, const char* msg);

/*
 * Clear all entries from the Rune DevTools console.
 */
void rune_ffi_devtools_console_clear(void);

/* ==========================================================================
 * Bookmark API
 * ========================================================================== */

/*
 * Add a bookmark for the current page.
 * Uses the current URL and title from navigation state.
 *
 * @return true if a bookmark was added, false otherwise (e.g., no URL available)
 */
bool rune_ffi_add_bookmark(void);

/*
 * Open a new tab by navigating to a blank page.
 * Creates a new tab entry and makes it active.
 * Clears the address bar and focuses it for user input.
 */
void rune_ffi_new_tab(void);

/*
 * Update the active tab with the current navigation URL and title.
 * Call this when CEF navigation state changes (URL or title update).
 */
void rune_ffi_update_active_tab(void);

#ifdef __cplusplus
}
#endif

#endif /* RUNE_FFI_H */
