/*
 * Rune FFI Header
 *
 * C interface for the Rust rune-ffi library.
 * This library handles rune-scene IR rendering.
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
bool rune_init(uint32_t width, uint32_t height, float scale, void* metal_layer, const char* package_path);

/*
 * Shutdown the renderer and release resources.
 */
void rune_shutdown(void);

/*
 * Resize the viewport.
 *
 * @param width New width in physical pixels
 * @param height New height in physical pixels
 */
void rune_resize(uint32_t width, uint32_t height);

/*
 * Render a single frame.
 *
 * Call this from the display link callback.
 */
void rune_render(void);

/*
 * Handle mouse click event.
 *
 * @param x X position in physical pixels
 * @param y Y position in physical pixels
 * @param pressed true for mouse down, false for mouse up
 */
void rune_mouse_click(float x, float y, bool pressed);

/*
 * Handle mouse move event.
 *
 * @param x X position in physical pixels
 * @param y Y position in physical pixels
 */
void rune_mouse_move(float x, float y);

/*
 * Handle key event.
 *
 * @param keycode Virtual keycode
 * @param pressed true for key down, false for key up
 */
void rune_key_event(uint32_t keycode, bool pressed);

/*
 * Check if redraw is needed.
 *
 * @return true if renderer needs redraw
 */
bool rune_needs_redraw(void);

/*
 * Request a redraw.
 */
void rune_request_redraw(void);

#ifdef __cplusplus
}
#endif

#endif /* RUNE_FFI_H */
