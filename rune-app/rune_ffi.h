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
 * Handle key event.
 *
 * @param keycode Virtual key code
 * @param pressed true if key pressed, false if released
 */
void rune_ffi_key_event(uint32_t keycode, bool pressed);

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

#ifdef __cplusplus
}
#endif

#endif /* RUNE_FFI_H */
