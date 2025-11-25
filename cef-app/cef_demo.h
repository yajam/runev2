/*
 * CEF Demo FFI Header
 *
 * C interface for the Rust cef-demo library.
 * This library handles wgpu rendering of CEF content.
 * CEF initialization and browser management is handled by the Xcode side.
 */

#ifndef CEF_DEMO_H
#define CEF_DEMO_H

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
 * @return true on success, false on failure
 */
bool cef_demo_init(uint32_t width, uint32_t height, float scale, void* metal_layer);

/*
 * Shutdown the renderer and release resources.
 */
void cef_demo_shutdown(void);

/*
 * Upload pixel data from CEF OnPaint callback.
 *
 * @param pixels Pointer to pixel data in BGRA format
 * @param width Width in pixels
 * @param height Height in pixels
 * @param stride Bytes per row (usually width * 4)
 */
void cef_demo_upload_pixels(const uint8_t* pixels, uint32_t width, uint32_t height, uint32_t stride);

/*
 * Resize the viewport.
 *
 * @param width New width in physical pixels
 * @param height New height in physical pixels
 */
void cef_demo_resize(uint32_t width, uint32_t height);

/*
 * Render a single frame.
 *
 * Call this from the display link callback or MTKView draw callback.
 */
void cef_demo_render(void);

#ifdef __cplusplus
}
#endif

#endif /* CEF_DEMO_H */
