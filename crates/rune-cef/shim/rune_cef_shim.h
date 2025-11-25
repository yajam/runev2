#pragma once

#include <stdint.h>

#if defined(_WIN32)
  #define RUNE_CEF_SHIM_EXPORT __declspec(dllexport)
#else
  #define RUNE_CEF_SHIM_EXPORT __attribute__((visibility("default")))
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef void* rune_cef_browser_t;

typedef struct {
    uint32_t width;
    uint32_t height;
    float scale_factor;
    int enable_javascript;
    int disable_gpu;
    const char* user_agent;
} rune_cef_config_t;

typedef struct {
    const uint8_t* pixels;
    uint32_t width;
    uint32_t height;
    uint32_t stride;
} rune_cef_frame_t;

typedef enum {
    RUNE_MOUSE_NONE = 0,
    RUNE_MOUSE_LEFT = 1,
    RUNE_MOUSE_MIDDLE = 2,
    RUNE_MOUSE_RIGHT = 3,
} rune_mouse_button_t;

typedef enum {
    RUNE_MOUSE_MOVE = 0,
    RUNE_MOUSE_DOWN = 1,
    RUNE_MOUSE_UP = 2,
    RUNE_MOUSE_WHEEL = 3,
} rune_mouse_event_kind_t;

typedef struct {
    int32_t x;
    int32_t y;
    rune_mouse_event_kind_t kind;
    rune_mouse_button_t button;
    int32_t delta_x;
    int32_t delta_y;
    uint32_t modifiers;
} rune_mouse_event_t;

typedef enum {
    RUNE_KEY_DOWN = 0,
    RUNE_KEY_UP = 1,
    RUNE_KEY_CHAR = 2,
} rune_key_event_kind_t;

typedef struct {
    uint32_t key_code;
    uint32_t character;
    rune_key_event_kind_t kind;
    uint32_t modifiers;
} rune_key_event_t;

int rune_cef_init(const char* cache_path,
                  const char* root_cache_path,
                  const char* log_file_path,
                  int external_message_pump) RUNE_CEF_SHIM_EXPORT;

void rune_cef_shutdown(void) RUNE_CEF_SHIM_EXPORT;

rune_cef_browser_t rune_cef_create_browser(const rune_cef_config_t* config,
                                           const char* initial_url) RUNE_CEF_SHIM_EXPORT;

void rune_cef_destroy_browser(rune_cef_browser_t browser) RUNE_CEF_SHIM_EXPORT;

void rune_cef_navigate(rune_cef_browser_t browser, const char* url) RUNE_CEF_SHIM_EXPORT;

void rune_cef_load_html(rune_cef_browser_t browser,
                        const char* html,
                        const char* base_url) RUNE_CEF_SHIM_EXPORT;

void rune_cef_do_message_loop_work(void) RUNE_CEF_SHIM_EXPORT;

int rune_cef_is_loading(rune_cef_browser_t browser) RUNE_CEF_SHIM_EXPORT;

int rune_cef_get_frame(rune_cef_browser_t browser,
                       rune_cef_frame_t* out_frame) RUNE_CEF_SHIM_EXPORT;

void rune_cef_send_mouse_event(rune_cef_browser_t browser,
                               const rune_mouse_event_t* event) RUNE_CEF_SHIM_EXPORT;

void rune_cef_send_key_event(rune_cef_browser_t browser,
                             const rune_key_event_t* event) RUNE_CEF_SHIM_EXPORT;

void rune_cef_resize(rune_cef_browser_t browser,
                     uint32_t width,
                     uint32_t height) RUNE_CEF_SHIM_EXPORT;

#ifdef __cplusplus
}
#endif
