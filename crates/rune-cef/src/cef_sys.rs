//! Minimal CEF C API definitions needed for the dynamic backend.
//!
//! These are trimmed down versions of the structures from the official
//! headers so we can keep the dynamic loading surface small while still
//! matching the layout expected by the CEF runtime.
#![allow(non_camel_case_types, non_upper_case_globals, dead_code)]

use std::ffi::{c_char, c_int, c_uint, c_void};

pub type cef_color_t = u32;

/// UTF-16 string container used by CEF.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct cef_string_t {
    pub str_: *mut u16,
    pub length: usize,
    pub dtor: Option<unsafe extern "C" fn(*mut u16)>,
}

/// Reference-counted base used by all handler structs.
#[repr(C)]
pub struct cef_base_ref_counted_t {
    pub size: usize,
    pub add_ref: Option<unsafe extern "C" fn(*mut cef_base_ref_counted_t)>,
    pub release: Option<unsafe extern "C" fn(*mut cef_base_ref_counted_t) -> c_int>,
    pub has_one_ref: Option<unsafe extern "C" fn(*mut cef_base_ref_counted_t) -> c_int>,
    pub has_at_least_one_ref: Option<unsafe extern "C" fn(*mut cef_base_ref_counted_t) -> c_int>,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct cef_rect_t {
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
}

#[repr(C)]
pub struct cef_main_args_t {
    pub argc: c_int,
    pub argv: *mut *mut c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub enum cef_runtime_style_t {
    CEF_RUNTIME_STYLE_DEFAULT = 0,
    CEF_RUNTIME_STYLE_CHROME = 1,
    CEF_RUNTIME_STYLE_ALLOY = 2,
}

pub type cef_cursor_handle_t = *mut c_void;
pub type cef_cursor_type_t = c_int;
pub type cef_drag_operations_mask_t = c_uint;
pub type cef_horizontal_alignment_t = c_int;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct cef_range_t {
    pub from: c_uint,
    pub to: c_uint,
}

/// Dummy opaque types to satisfy function signatures.
#[repr(C)]
pub struct cef_screen_info_t {
    _unused: [u8; 0],
}

#[repr(C)]
pub struct cef_accelerated_paint_info_t {
    _unused: [u8; 0],
}

#[repr(C)]
pub struct cef_touch_handle_state_t {
    _unused: [u8; 0],
}

#[repr(C)]
pub struct cef_cursor_info_t {
    _unused: [u8; 0],
}

#[repr(C)]
pub struct cef_accessibility_handler_t {
    _unused: [u8; 0],
}

#[repr(C)]
pub struct cef_drag_data_t {
    _unused: [u8; 0],
}

#[repr(C)]
pub struct cef_string_visitor_t {
    _unused: [u8; 0],
}

#[repr(C)]
pub struct cef_browser_t {
    pub base: cef_base_ref_counted_t,
    pub get_host: Option<unsafe extern "C" fn(self_: *mut cef_browser_t) -> *mut c_void>,
    pub can_go_back: Option<unsafe extern "C" fn(self_: *mut cef_browser_t) -> c_int>,
    pub go_back: Option<unsafe extern "C" fn(self_: *mut cef_browser_t)>,
    pub can_go_forward: Option<unsafe extern "C" fn(self_: *mut cef_browser_t) -> c_int>,
    pub go_forward: Option<unsafe extern "C" fn(self_: *mut cef_browser_t)>,
    pub is_loading: Option<unsafe extern "C" fn(self_: *mut cef_browser_t) -> c_int>,
    pub reload: Option<unsafe extern "C" fn(self_: *mut cef_browser_t)>,
    pub reload_ignore_cache: Option<unsafe extern "C" fn(self_: *mut cef_browser_t)>,
    pub stop_load: Option<unsafe extern "C" fn(self_: *mut cef_browser_t)>,
    pub get_identifier: Option<unsafe extern "C" fn(self_: *mut cef_browser_t) -> c_int>,
    pub is_same: Option<unsafe extern "C" fn(self_: *mut cef_browser_t, that: *mut cef_browser_t) -> c_int>,
    pub is_popup: Option<unsafe extern "C" fn(self_: *mut cef_browser_t) -> c_int>,
    pub has_document: Option<unsafe extern "C" fn(self_: *mut cef_browser_t) -> c_int>,
    pub get_main_frame: Option<unsafe extern "C" fn(self_: *mut cef_browser_t) -> *mut cef_frame_t>,
    // We don't need further fields for now.
}

#[repr(C)]
pub struct cef_frame_t {
    pub base: cef_base_ref_counted_t,
    pub is_valid: Option<unsafe extern "C" fn(self_: *mut cef_frame_t) -> c_int>,
    pub undo: Option<unsafe extern "C" fn(self_: *mut cef_frame_t)>,
    pub redo: Option<unsafe extern "C" fn(self_: *mut cef_frame_t)>,
    pub cut: Option<unsafe extern "C" fn(self_: *mut cef_frame_t)>,
    pub copy: Option<unsafe extern "C" fn(self_: *mut cef_frame_t)>,
    pub paste: Option<unsafe extern "C" fn(self_: *mut cef_frame_t)>,
    pub del: Option<unsafe extern "C" fn(self_: *mut cef_frame_t)>,
    pub select_all: Option<unsafe extern "C" fn(self_: *mut cef_frame_t)>,
    pub view_source: Option<unsafe extern "C" fn(self_: *mut cef_frame_t)>,
    pub get_source: Option<unsafe extern "C" fn(self_: *mut cef_frame_t, visitor: *mut c_void)>,
    pub get_text: Option<unsafe extern "C" fn(self_: *mut cef_frame_t, visitor: *mut c_void)>,
    pub load_request: Option<unsafe extern "C" fn(self_: *mut cef_frame_t, request: *mut c_void)>,
    pub load_url: Option<unsafe extern "C" fn(self_: *mut cef_frame_t, url: *const cef_string_t)>,
    // We don't need further fields for now.
}

pub type cef_string_list_t = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
pub enum cef_paint_element_type_t {
    PET_VIEW = 0,
    PET_POPUP = 1,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub enum cef_state_t {
    STATE_DEFAULT = 0,
    STATE_ENABLED = 1,
    STATE_DISABLED = 2,
}

pub type cef_log_severity_t = c_int;
pub const LOGSEVERITY_DEFAULT: cef_log_severity_t = 0;

pub type cef_log_items_t = c_int;
pub const LOG_ITEMS_DEFAULT: cef_log_items_t = 0;

#[repr(C)]
pub struct cef_window_info_t {
    pub size: usize,
    pub window_name: cef_string_t,
    pub bounds: cef_rect_t,
    pub hidden: c_int,
    pub parent_view: *mut c_void,
    pub windowless_rendering_enabled: c_int,
    pub shared_texture_enabled: c_int,
    pub external_begin_frame_enabled: c_int,
    pub view: *mut c_void,
    pub runtime_style: cef_runtime_style_t,
}

#[repr(C)]
pub struct cef_settings_t {
    pub size: usize,
    pub no_sandbox: c_int,
    pub browser_subprocess_path: cef_string_t,
    pub framework_dir_path: cef_string_t,
    pub main_bundle_path: cef_string_t,
    pub multi_threaded_message_loop: c_int,
    pub external_message_pump: c_int,
    pub windowless_rendering_enabled: c_int,
    pub command_line_args_disabled: c_int,
    pub cache_path: cef_string_t,
    pub root_cache_path: cef_string_t,
    pub persist_session_cookies: c_int,
    pub user_agent: cef_string_t,
    pub user_agent_product: cef_string_t,
    pub locale: cef_string_t,
    pub log_file: cef_string_t,
    pub log_severity: cef_log_severity_t,
    pub log_items: cef_log_items_t,
    pub javascript_flags: cef_string_t,
    pub resources_dir_path: cef_string_t,
    pub locales_dir_path: cef_string_t,
    pub remote_debugging_port: c_int,
    pub uncaught_exception_stack_size: c_int,
    pub background_color: cef_color_t,
    pub accept_language_list: cef_string_t,
    pub cookieable_schemes_list: cef_string_t,
    pub cookieable_schemes_exclude_defaults: c_int,
    pub chrome_policy_id: cef_string_t,
    pub chrome_app_icon_id: c_int,
    pub disable_signal_handlers: c_int,
}

#[repr(C)]
pub struct cef_browser_settings_t {
    pub size: usize,
    pub windowless_frame_rate: c_int,
    pub standard_font_family: cef_string_t,
    pub fixed_font_family: cef_string_t,
    pub serif_font_family: cef_string_t,
    pub sans_serif_font_family: cef_string_t,
    pub cursive_font_family: cef_string_t,
    pub fantasy_font_family: cef_string_t,
    pub default_font_size: c_int,
    pub default_fixed_font_size: c_int,
    pub minimum_font_size: c_int,
    pub minimum_logical_font_size: c_int,
    pub default_encoding: cef_string_t,
    pub remote_fonts: cef_state_t,
    pub javascript: cef_state_t,
    pub javascript_close_windows: cef_state_t,
    pub javascript_access_clipboard: cef_state_t,
    pub javascript_dom_paste: cef_state_t,
    pub image_loading: cef_state_t,
    pub image_shrink_standalone_to_fit: cef_state_t,
    pub text_area_resize: cef_state_t,
    pub tab_to_links: cef_state_t,
    pub databases: cef_state_t,
    pub webgl: cef_state_t,
    pub background_color: cef_color_t,
    pub chrome_status_bubble: cef_state_t,
    pub chrome_zoom_bubble: cef_state_t,
}

#[repr(C)]
pub struct cef_load_handler_t {
    pub base: cef_base_ref_counted_t,
    pub on_loading_state_change: Option<
        unsafe extern "C" fn(
            self_: *mut cef_load_handler_t,
            browser: *mut cef_browser_t,
            is_loading: c_int,
            can_go_back: c_int,
            can_go_forward: c_int,
        ),
    >,
    pub on_load_start: Option<
        unsafe extern "C" fn(
            self_: *mut cef_load_handler_t,
            browser: *mut cef_browser_t,
            frame: *mut c_void,
            transition_type: c_int,
        ),
    >,
    pub on_load_end: Option<
        unsafe extern "C" fn(
            self_: *mut cef_load_handler_t,
            browser: *mut cef_browser_t,
            frame: *mut c_void,
            http_status_code: c_int,
        ),
    >,
    pub on_load_error: Option<
        unsafe extern "C" fn(
            self_: *mut cef_load_handler_t,
            browser: *mut cef_browser_t,
            frame: *mut c_void,
            error_code: c_int,
            error_text: *const cef_string_t,
            failed_url: *const cef_string_t,
        ),
    >,
}

#[repr(C)]
pub struct cef_render_handler_t {
    pub base: cef_base_ref_counted_t,
    pub get_accessibility_handler:
        Option<unsafe extern "C" fn(self_: *mut cef_render_handler_t) -> *mut cef_accessibility_handler_t>,
    pub get_root_screen_rect: Option<
        unsafe extern "C" fn(
            self_: *mut cef_render_handler_t,
            browser: *mut cef_browser_t,
            rect: *mut cef_rect_t,
        ) -> c_int,
    >,
    pub get_view_rect: Option<
        unsafe extern "C" fn(
            self_: *mut cef_render_handler_t,
            browser: *mut cef_browser_t,
            rect: *mut cef_rect_t,
        ),
    >,
    pub get_screen_point: Option<
        unsafe extern "C" fn(
            self_: *mut cef_render_handler_t,
            browser: *mut cef_browser_t,
            view_x: c_int,
            view_y: c_int,
            screen_x: *mut c_int,
            screen_y: *mut c_int,
        ) -> c_int,
    >,
    pub get_screen_info: Option<
        unsafe extern "C" fn(
            self_: *mut cef_render_handler_t,
            browser: *mut cef_browser_t,
            screen_info: *mut cef_screen_info_t,
        ) -> c_int,
    >,
    pub on_popup_show:
        Option<unsafe extern "C" fn(self_: *mut cef_render_handler_t, browser: *mut cef_browser_t, show: c_int)>,
    pub on_popup_size: Option<
        unsafe extern "C" fn(
            self_: *mut cef_render_handler_t,
            browser: *mut cef_browser_t,
            rect: *const cef_rect_t,
        ),
    >,
    pub on_paint: Option<
        unsafe extern "C" fn(
            self_: *mut cef_render_handler_t,
            browser: *mut cef_browser_t,
            paint_type: cef_paint_element_type_t,
            dirty_rects_count: usize,
            dirty_rects: *const cef_rect_t,
            buffer: *const c_void,
            width: c_int,
            height: c_int,
        ),
    >,
    pub on_accelerated_paint: Option<
        unsafe extern "C" fn(
            self_: *mut cef_render_handler_t,
            browser: *mut cef_browser_t,
            paint_type: cef_paint_element_type_t,
            dirty_rects_count: usize,
            dirty_rects: *const cef_rect_t,
            info: *const cef_accelerated_paint_info_t,
        ),
    >,
    pub get_touch_handle_size: Option<
        unsafe extern "C" fn(
            self_: *mut cef_render_handler_t,
            browser: *mut cef_browser_t,
            orientation: cef_horizontal_alignment_t,
            size: *mut cef_size_t,
        ),
    >,
    pub on_touch_handle_state_changed: Option<
        unsafe extern "C" fn(
            self_: *mut cef_render_handler_t,
            browser: *mut cef_browser_t,
            state: *const cef_touch_handle_state_t,
        ),
    >,
    pub start_dragging: Option<
        unsafe extern "C" fn(
            self_: *mut cef_render_handler_t,
            browser: *mut cef_browser_t,
            drag_data: *mut cef_drag_data_t,
            allowed_ops: cef_drag_operations_mask_t,
            x: c_int,
            y: c_int,
        ) -> c_int,
    >,
    pub update_drag_cursor: Option<
        unsafe extern "C" fn(
            self_: *mut cef_render_handler_t,
            browser: *mut cef_browser_t,
            operation: cef_drag_operations_mask_t,
        ),
    >,
    pub on_scroll_offset_changed: Option<
        unsafe extern "C" fn(self_: *mut cef_render_handler_t, browser: *mut cef_browser_t, x: f64, y: f64),
    >,
    pub on_ime_composition_range_changed: Option<
        unsafe extern "C" fn(
            self_: *mut cef_render_handler_t,
            browser: *mut cef_browser_t,
            selected_range: *const cef_range_t,
            character_bounds_count: usize,
            character_bounds: *const cef_rect_t,
        ),
    >,
    pub on_text_selection_changed: Option<
        unsafe extern "C" fn(
            self_: *mut cef_render_handler_t,
            browser: *mut cef_browser_t,
            selected_text: *const cef_string_t,
            selected_range: *const cef_range_t,
        ),
    >,
    pub on_virtual_keyboard_requested: Option<
        unsafe extern "C" fn(
            self_: *mut cef_render_handler_t,
            browser: *mut cef_browser_t,
            input_mode: c_int,
        ),
    >,
}

#[repr(C)]
pub struct cef_client_t {
    pub base: cef_base_ref_counted_t,
    pub get_audio_handler:
        Option<unsafe extern "C" fn(self_: *mut cef_client_t) -> *mut c_void /* cef_audio_handler_t */>,
    pub get_command_handler:
        Option<unsafe extern "C" fn(self_: *mut cef_client_t) -> *mut c_void /* cef_command_handler_t */>,
    pub get_context_menu_handler:
        Option<unsafe extern "C" fn(self_: *mut cef_client_t) -> *mut c_void /* cef_context_menu_handler_t */>,
    pub get_dialog_handler:
        Option<unsafe extern "C" fn(self_: *mut cef_client_t) -> *mut c_void /* cef_dialog_handler_t */>,
    pub get_display_handler:
        Option<unsafe extern "C" fn(self_: *mut cef_client_t) -> *mut c_void /* cef_display_handler_t */>,
    pub get_download_handler:
        Option<unsafe extern "C" fn(self_: *mut cef_client_t) -> *mut c_void /* cef_download_handler_t */>,
    pub get_drag_handler:
        Option<unsafe extern "C" fn(self_: *mut cef_client_t) -> *mut c_void /* cef_drag_handler_t */>,
    pub get_find_handler:
        Option<unsafe extern "C" fn(self_: *mut cef_client_t) -> *mut c_void /* cef_find_handler_t */>,
    pub get_focus_handler:
        Option<unsafe extern "C" fn(self_: *mut cef_client_t) -> *mut c_void /* cef_focus_handler_t */>,
    pub get_frame_handler:
        Option<unsafe extern "C" fn(self_: *mut cef_client_t) -> *mut c_void /* cef_frame_handler_t */>,
    pub get_permission_handler:
        Option<unsafe extern "C" fn(self_: *mut cef_client_t) -> *mut c_void /* cef_permission_handler_t */>,
    pub get_jsdialog_handler:
        Option<unsafe extern "C" fn(self_: *mut cef_client_t) -> *mut c_void /* cef_jsdialog_handler_t */>,
    pub get_keyboard_handler:
        Option<unsafe extern "C" fn(self_: *mut cef_client_t) -> *mut c_void /* cef_keyboard_handler_t */>,
    pub get_life_span_handler:
        Option<unsafe extern "C" fn(self_: *mut cef_client_t) -> *mut c_void /* cef_life_span_handler_t */>,
    pub get_load_handler: Option<unsafe extern "C" fn(self_: *mut cef_client_t) -> *mut cef_load_handler_t>,
    pub get_print_handler:
        Option<unsafe extern "C" fn(self_: *mut cef_client_t) -> *mut c_void /* cef_print_handler_t */>,
    pub get_render_handler: Option<unsafe extern "C" fn(self_: *mut cef_client_t) -> *mut cef_render_handler_t>,
    pub get_request_handler:
        Option<unsafe extern "C" fn(self_: *mut cef_client_t) -> *mut c_void /* cef_request_handler_t */>,
    pub on_process_message_received: Option<
        unsafe extern "C" fn(
            self_: *mut cef_client_t,
            browser: *mut cef_browser_t,
            frame: *mut c_void,
            source_process: c_int,
            message: *mut c_void,
        ) -> c_int,
    >,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct cef_size_t {
    pub width: c_int,
    pub height: c_int,
}
