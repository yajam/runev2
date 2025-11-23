//! CEF backend using dynamic loading via libloading.
//!
//! This implementation wires CEF's off-screen rendering callbacks into a
//! shared wgpu-friendly frame buffer so the demo app can render real web pages
//! even when Chrome/CDP isn't available.

use crate::cef_sys::*;
use crate::error::{CefError, Result};
use crate::frame::{FrameBuffer, PixelFormat};
use crate::{HeadlessConfig, HeadlessRenderer, KeyEvent, MouseEvent};
use libloading::{Library, Symbol};
use std::ffi::{c_int, c_void};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::Duration;

type CefInitializeFn =
    unsafe extern "C" fn(*const cef_main_args_t, *const cef_settings_t, *mut c_void, *mut c_void) -> c_int;
type CefShutdownFn = unsafe extern "C" fn();
type CefDoMessageLoopWorkFn = unsafe extern "C" fn();
type CefCreateBrowserSyncFn = unsafe extern "C" fn(
    *const cef_window_info_t,
    *mut cef_client_t,
    *const cef_string_t,
    *const cef_browser_settings_t,
    *mut c_void,
    *mut c_void,
) -> *mut c_void;

/// CEF library wrapper for dynamic loading.
struct CefLibrary {
    #[allow(dead_code)]
    lib: Library,
    cef_initialize: CefInitializeFn,
    cef_shutdown: CefShutdownFn,
    cef_do_message_loop_work: CefDoMessageLoopWorkFn,
    cef_browser_host_create_browser_sync: CefCreateBrowserSyncFn,
}

impl CefLibrary {
    fn load(cef_path: Option<&PathBuf>) -> Result<Self> {
        let lib_name = Self::library_name();

        let lib = if let Some(path) = cef_path {
            let lib_path = if path.is_file() { path.clone() } else { path.join(&lib_name) };
            eprintln!("CEF: Loading library from: {}", lib_path.display());
            unsafe { Library::new(&lib_path) }
        } else {
            eprintln!("CEF: Loading library from system path: {}", lib_name);
            unsafe { Library::new(&lib_name) }
        }
        .map_err(|e| CefError::LibraryLoad(e.to_string()))?;

        let (cef_initialize, cef_shutdown, cef_do_message_loop_work, cef_browser_host_create_browser_sync) = unsafe {
            let init: Symbol<CefInitializeFn> = lib
                .get(b"cef_initialize")
                .map_err(|e| CefError::SymbolNotFound(format!("cef_initialize: {}", e)))?;
            let shutdown: Symbol<CefShutdownFn> = lib
                .get(b"cef_shutdown")
                .map_err(|e| CefError::SymbolNotFound(format!("cef_shutdown: {}", e)))?;
            let msg_loop: Symbol<CefDoMessageLoopWorkFn> = lib
                .get(b"cef_do_message_loop_work")
                .map_err(|e| CefError::SymbolNotFound(format!("cef_do_message_loop_work: {}", e)))?;
            let create_browser: Symbol<CefCreateBrowserSyncFn> = lib
                .get(b"cef_browser_host_create_browser_sync")
                .map_err(|e| CefError::SymbolNotFound(format!("cef_browser_host_create_browser_sync: {}", e)))?;

            (*init, *shutdown, *msg_loop, *create_browser)
        };

        Ok(Self {
            lib,
            cef_initialize,
            cef_shutdown,
            cef_do_message_loop_work,
            cef_browser_host_create_browser_sync,
        })
    }

    #[cfg(target_os = "windows")]
    fn library_name() -> String {
        "libcef.dll".to_string()
    }

    #[cfg(target_os = "macos")]
    fn library_name() -> String {
        "Chromium Embedded Framework.framework/Chromium Embedded Framework".to_string()
    }

    #[cfg(target_os = "linux")]
    fn library_name() -> String {
        "libcef.so".to_string()
    }
}

fn cef_string_from_str(s: &str) -> cef_string_t {
    let utf16: Vec<u16> = s.encode_utf16().collect();
    let len = utf16.len();
    let ptr = utf16.leak().as_mut_ptr();
    cef_string_t {
        str_: ptr,
        length: len,
        dtor: None,
    }
}

fn cef_string_from_path(path: &Path) -> cef_string_t {
    cef_string_from_str(&path.to_string_lossy())
}

fn log_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CEF_LOG_DIR") {
        return PathBuf::from(dir);
    }

    // Try to locate the repo root by walking up from the current executable.
    if let Ok(mut path) = std::env::current_exe() {
        for _ in 0..6 {
            if path.join("Cargo.toml").exists() || path.join(".git").exists() {
                return path;
            }
            if !path.pop() {
                break;
            }
        }
    }

    // Fallback to current working directory.
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn log_cef(msg: &str) {
    use std::io::Write;
    let primary = log_dir().join("cef_debug.log");
    let fallback = PathBuf::from("/tmp/cef_debug.log");

    let mut wrote = false;
    for target in [&primary, &fallback] {
        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(target) {
            let _ = writeln!(f, "{}", msg);
            let _ = f.flush();
            wrote = true;
            break;
        }
    }

    if !wrote {
        eprintln!("CEF LOG WRITE FAILED: {}", msg);
    } else {
        eprintln!("{}", msg);
    }
}

#[derive(Debug)]
struct RenderState {
    width: u32,
    height: u32,
    buffer: Vec<u8>,
    dirty: bool,
}

impl RenderState {
    fn new(width: u32, height: u32) -> Self {
        let size = (width * height * 4) as usize;
        Self {
            width,
            height,
            buffer: vec![0u8; size],
            dirty: false,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        let size = (width * height * 4) as usize;
        self.buffer.resize(size, 0);
        self.dirty = true;
    }

    fn update_from_paint(&mut self, data: &[u8], width: u32, height: u32) {
        if self.width != width || self.height != height {
            self.resize(width, height);
        }
        if data.len() == self.buffer.len() {
            self.buffer.copy_from_slice(data);
            self.dirty = true;
        }
    }

    fn take_frame(&mut self) -> Option<FrameBuffer> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        Some(FrameBuffer::from_raw(
            self.buffer.clone(),
            self.width,
            self.height,
            self.width * 4,
            PixelFormat::Bgra8,
        ))
    }
}

macro_rules! ref_counted {
    ($add_ref:ident, $release:ident, $has_one:ident, $has_at_least_one:ident, $ty:ty) => {
        unsafe extern "C" fn $add_ref(base: *mut cef_base_ref_counted_t) {
            unsafe {
                if let Some(wrapper) = (base as *mut $ty).as_ref() {
                    wrapper.ref_count.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        unsafe extern "C" fn $release(base: *mut cef_base_ref_counted_t) -> c_int {
            unsafe {
                let wrapper = base as *mut $ty;
                if let Some(inner) = wrapper.as_ref() {
                    if inner.ref_count.fetch_sub(1, Ordering::Release) == 1 {
                        std::sync::atomic::fence(Ordering::Acquire);
                        drop(Box::from_raw(wrapper));
                        return 1;
                    }
                }
            }
            0
        }
        unsafe extern "C" fn $has_one(base: *mut cef_base_ref_counted_t) -> c_int {
            unsafe {
                (base as *mut $ty)
                    .as_ref()
                    .map(|w| w.ref_count.load(Ordering::Acquire) == 1)
                    .unwrap_or(false) as c_int
            }
        }
        unsafe extern "C" fn $has_at_least_one(base: *mut cef_base_ref_counted_t) -> c_int {
            unsafe {
                (base as *mut $ty)
                    .as_ref()
                    .map(|w| w.ref_count.load(Ordering::Acquire) > 0)
                    .unwrap_or(false) as c_int
            }
        }
    };
}

#[repr(C)]
struct LoadHandler {
    handler: cef_load_handler_t,
    ref_count: AtomicUsize,
    loading: Arc<AtomicBool>,
}

ref_counted!(
    load_handler_add_ref,
    load_handler_release,
    load_handler_has_one_ref,
    load_handler_has_at_least_one_ref,
    LoadHandler
);

unsafe extern "C" fn on_loading_state_change(
    self_: *mut cef_load_handler_t,
    _browser: *mut cef_browser_t,
    is_loading: c_int,
    _can_go_back: c_int,
    _can_go_forward: c_int,
) {
    unsafe {
        if let Some(wrapper) = (self_ as *mut LoadHandler).as_ref() {
            wrapper.loading.store(is_loading != 0, Ordering::Relaxed);
            log_cef(&format!("on_loading_state_change: loading={}", is_loading));
        }
    }
}

impl LoadHandler {
    fn new(loading: Arc<AtomicBool>) -> *mut cef_load_handler_t {
        let handler = cef_load_handler_t {
            base: cef_base_ref_counted_t {
                size: std::mem::size_of::<cef_load_handler_t>(),
                add_ref: Some(load_handler_add_ref),
                release: Some(load_handler_release),
                has_one_ref: Some(load_handler_has_one_ref),
                has_at_least_one_ref: Some(load_handler_has_at_least_one_ref),
            },
            on_loading_state_change: Some(on_loading_state_change),
            on_load_start: None,
            on_load_end: None,
            on_load_error: None,
        };

        let wrapper = Box::new(LoadHandler {
            handler,
            ref_count: AtomicUsize::new(1),
            loading,
        });
        Box::into_raw(wrapper) as *mut cef_load_handler_t
    }
}

#[repr(C)]
struct RenderHandlerWrapper {
    handler: cef_render_handler_t,
    ref_count: AtomicUsize,
    state: Arc<Mutex<RenderState>>,
    loading: Arc<AtomicBool>,
    paint_count: AtomicUsize,
}

ref_counted!(
    render_handler_add_ref,
    render_handler_release,
    render_handler_has_one_ref,
    render_handler_has_at_least_one_ref,
    RenderHandlerWrapper
);

unsafe extern "C" fn render_get_view_rect(
    self_: *mut cef_render_handler_t,
    _browser: *mut cef_browser_t,
    rect: *mut cef_rect_t,
) {
    unsafe {
        if let Some(wrapper) = (self_ as *mut RenderHandlerWrapper).as_ref() {
            if let Ok(state) = wrapper.state.lock() {
                if let Some(rect) = rect.as_mut() {
                    rect.x = 0;
                    rect.y = 0;
                    rect.width = state.width as c_int;
                    rect.height = state.height as c_int;
                }
            }
        }
    }
}

unsafe extern "C" fn render_get_root_screen_rect(
    self_: *mut cef_render_handler_t,
    _browser: *mut cef_browser_t,
    rect: *mut cef_rect_t,
) -> c_int {
    unsafe {
        if let Some(wrapper) = (self_ as *mut RenderHandlerWrapper).as_ref() {
            if let Ok(state) = wrapper.state.lock() {
                if let Some(rect) = rect.as_mut() {
                    rect.x = 0;
                    rect.y = 0;
                    rect.width = state.width as c_int;
                    rect.height = state.height as c_int;
                    return 1;
                }
            }
        }
    }
    0
}

unsafe extern "C" fn render_on_paint(
    self_: *mut cef_render_handler_t,
    _browser: *mut cef_browser_t,
    _paint_type: cef_paint_element_type_t,
    _dirty_rects_count: usize,
    _dirty_rects: *const cef_rect_t,
    buffer: *const c_void,
    width: c_int,
    height: c_int,
) {
    unsafe {
        let wrapper = self_ as *mut RenderHandlerWrapper;
        let len = (width.max(0) as usize) * (height.max(0) as usize) * 4;
        if buffer.is_null() || len == 0 {
            return;
        }
        let slice = std::slice::from_raw_parts(buffer as *const u8, len);
        if let Some(wrapper) = wrapper.as_ref() {
            if let Ok(mut state) = wrapper.state.lock() {
                state.update_from_paint(slice, width as u32, height as u32);
            }
            wrapper.loading.store(false, Ordering::Relaxed);

            let count = wrapper.paint_count.fetch_add(1, Ordering::Relaxed) + 1;
            if count <= 5 {
                log_cef(&format!(
                    "render_on_paint #{} ({}x{}) dirty={}",
                    count,
                    width,
                    height,
                    len
                ));
            }
        }
    }
}

impl RenderHandlerWrapper {
    fn new(state: Arc<Mutex<RenderState>>, loading: Arc<AtomicBool>) -> *mut cef_render_handler_t {
        let handler = cef_render_handler_t {
            base: cef_base_ref_counted_t {
                size: std::mem::size_of::<cef_render_handler_t>(),
                add_ref: Some(render_handler_add_ref),
                release: Some(render_handler_release),
                has_one_ref: Some(render_handler_has_one_ref),
                has_at_least_one_ref: Some(render_handler_has_at_least_one_ref),
            },
            get_accessibility_handler: None,
            get_root_screen_rect: Some(render_get_root_screen_rect),
            get_view_rect: Some(render_get_view_rect),
            get_screen_point: None,
            get_screen_info: None,
            on_popup_show: None,
            on_popup_size: None,
            on_paint: Some(render_on_paint),
            on_accelerated_paint: None,
            get_touch_handle_size: None,
            on_touch_handle_state_changed: None,
            start_dragging: None,
            update_drag_cursor: None,
            on_scroll_offset_changed: None,
            on_ime_composition_range_changed: None,
            on_text_selection_changed: None,
            on_virtual_keyboard_requested: None,
        };

        let wrapper = Box::new(RenderHandlerWrapper {
            handler,
            ref_count: AtomicUsize::new(1),
            state,
            loading,
            paint_count: AtomicUsize::new(0),
        });
        Box::into_raw(wrapper) as *mut cef_render_handler_t
    }
}

#[repr(C)]
struct ClientWrapper {
    client: cef_client_t,
    ref_count: AtomicUsize,
    render_handler: *mut cef_render_handler_t,
    load_handler: *mut cef_load_handler_t,
}

ref_counted!(
    client_wrapper_add_ref,
    client_wrapper_release,
    client_wrapper_has_one_ref,
    client_wrapper_has_at_least_one_ref,
    ClientWrapper
);

unsafe extern "C" fn client_get_render_handler(self_: *mut cef_client_t) -> *mut cef_render_handler_t {
    unsafe {
        (self_ as *mut ClientWrapper)
            .as_ref()
            .map(|w| w.render_handler)
            .unwrap_or(ptr::null_mut())
    }
}

unsafe extern "C" fn client_get_load_handler(self_: *mut cef_client_t) -> *mut cef_load_handler_t {
    unsafe {
        (self_ as *mut ClientWrapper)
            .as_ref()
            .map(|w| w.load_handler)
            .unwrap_or(ptr::null_mut())
    }
}

impl ClientWrapper {
    fn new(render_handler: *mut cef_render_handler_t, load_handler: *mut cef_load_handler_t) -> *mut cef_client_t {
        let client = cef_client_t {
            base: cef_base_ref_counted_t {
                size: std::mem::size_of::<cef_client_t>(),
                add_ref: Some(client_wrapper_add_ref),
                release: Some(client_wrapper_release),
                has_one_ref: Some(client_wrapper_has_one_ref),
                has_at_least_one_ref: Some(client_wrapper_has_at_least_one_ref),
            },
            get_audio_handler: None,
            get_command_handler: None,
            get_context_menu_handler: None,
            get_dialog_handler: None,
            get_display_handler: None,
            get_download_handler: None,
            get_drag_handler: None,
            get_find_handler: None,
            get_focus_handler: None,
            get_frame_handler: None,
            get_permission_handler: None,
            get_jsdialog_handler: None,
            get_keyboard_handler: None,
            get_life_span_handler: None,
            get_load_handler: Some(client_get_load_handler),
            get_print_handler: None,
            get_render_handler: Some(client_get_render_handler),
            get_request_handler: None,
            on_process_message_received: None,
        };

        let wrapper = Box::new(ClientWrapper {
            client,
            ref_count: AtomicUsize::new(1),
            render_handler,
            load_handler,
        });
        Box::into_raw(wrapper) as *mut cef_client_t
    }
}

/// Headless CEF renderer with dynamic library loading.
pub struct CefHeadless {
    library: Arc<CefLibrary>,
    config: HeadlessConfig,
    render_state: Arc<Mutex<RenderState>>,
    loading: Arc<AtomicBool>,
    framework_path: Option<PathBuf>,
    render_handler: Option<*mut cef_render_handler_t>,
    load_handler: Option<*mut cef_load_handler_t>,
    client: Option<*mut cef_client_t>,
    browser: Option<*mut cef_browser_t>,
    initialized: bool,
    current_url: Option<String>,
}

// Safety: CefHeadless is driven from a single thread via the external message pump.
unsafe impl Send for CefHeadless {}

impl CefHeadless {
    pub fn new(config: HeadlessConfig) -> Result<Self> {
        let cef_path = std::env::var("CEF_PATH")
            .or_else(|_| std::env::var("CEF_LIBRARY_PATH"))
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                #[cfg(target_os = "macos")]
                {
                    Self::find_cef_in_bundle()
                }
                #[cfg(not(target_os = "macos"))]
                {
                    None
                }
            });

        let framework_path = cef_path.as_ref().and_then(|p| {
            if p.is_file() {
                p.parent().map(|p| p.to_path_buf())
            } else if p.extension().map_or(false, |e| e == "framework") {
                Some(p.clone())
            } else {
                let framework = p.join("Chromium Embedded Framework.framework");
                if framework.exists() {
                    Some(framework)
                } else {
                    None
                }
            }
        });

        let library = Arc::new(CefLibrary::load(cef_path.as_ref())?);
        let render_state = Arc::new(Mutex::new(RenderState::new(config.width, config.height)));
        let loading = Arc::new(AtomicBool::new(true));

        let mut renderer = Self {
            library,
            config,
            render_state,
            loading,
            framework_path,
            render_handler: None,
            load_handler: None,
            client: None,
            browser: None,
            initialized: false,
            current_url: None,
        };

        renderer.initialize("about:blank")?;
        Ok(renderer)
    }

    #[cfg(target_os = "macos")]
    fn find_cef_in_bundle() -> Option<PathBuf> {
        let exe_path = std::env::current_exe().ok()?;
        let contents_dir = exe_path.parent()?.parent()?;
        let framework_path = contents_dir
            .join("Frameworks")
            .join("Chromium Embedded Framework.framework")
            .join("Chromium Embedded Framework");
        if framework_path.exists() {
            eprintln!("CEF: Found framework in app bundle: {}", framework_path.display());
            Some(framework_path)
        } else {
            None
        }
    }

    fn create_handlers(&mut self) {
        let render = RenderHandlerWrapper::new(self.render_state.clone(), self.loading.clone());
        let load = LoadHandler::new(self.loading.clone());
        let client = ClientWrapper::new(render, load);

        self.render_handler = Some(render);
        self.load_handler = Some(load);
        self.client = Some(client);
    }

    fn release_handlers(&mut self) {
        unsafe {
            if let Some(client) = self.client.take() {
                if let Some(release) = (*client).base.release {
                    release(&mut (*client).base);
                }
            }
            if let Some(load) = self.load_handler.take() {
                let load_wrapper = load as *mut LoadHandler;
                if let Some(release) = (*load_wrapper).handler.base.release {
                    release(&mut (*load_wrapper).handler.base);
                }
            }
            if let Some(render) = self.render_handler.take() {
                let render_wrapper = render as *mut RenderHandlerWrapper;
                if let Some(release) = (*render_wrapper).handler.base.release {
                    release(&mut (*render_wrapper).handler.base);
                }
            }
        }
    }

    fn initialize(&mut self, url: &str) -> Result<()> {
        let _ = std::fs::write("/tmp/rune_init_debug.txt", "initialize called\n");
        self.loading.store(true, Ordering::Relaxed);
        self.release_handlers();
        self.create_handlers();

        #[cfg(not(target_os = "windows"))]
        let main_args = cef_main_args_t {
            argc: 0,
            argv: ptr::null_mut(),
        };

        #[cfg(target_os = "windows")]
        let main_args = cef_main_args_t { argc: 0, argv: ptr::null_mut() };

        let mut settings: cef_settings_t = unsafe { std::mem::zeroed() };
        settings.size = std::mem::size_of::<cef_settings_t>();
        settings.no_sandbox = 1;
        settings.windowless_rendering_enabled = 1;
        settings.external_message_pump = 1;
        settings.multi_threaded_message_loop = 0;
        settings.log_severity = LOGSEVERITY_DEFAULT;
        settings.command_line_args_disabled = 0;
        let log_file_path = log_dir().join("cef_cef.log");
        settings.log_file = cef_string_from_path(&log_file_path);
        // Avoid default singleton warnings and put cache somewhere writable.
        // Normalize paths so that cache_path is a child of root_cache_path
        // even when /tmp is a symlink to /private/tmp on macOS.
        let mut root_cache = std::path::PathBuf::from("/tmp/rune_cef_root");
        let _ = std::fs::create_dir_all(&root_cache);
        if let Ok(real_root) = std::fs::canonicalize(&root_cache) {
            root_cache = real_root;
        }
        let cache = root_cache.join("cache");
        let _ = std::fs::create_dir_all(&cache);
        log_cef(&format!(
            "CEF: root_cache_path={}, cache_path={}",
            root_cache.display(),
            cache.display()
        ));
        settings.root_cache_path = cef_string_from_path(&root_cache);
        settings.cache_path = cef_string_from_path(&cache);

        #[cfg(target_os = "macos")]
        {
            if let Some(ref fw) = self.framework_path {
                let path_str = fw.to_string_lossy();
                eprintln!("CEF: Setting framework_dir_path to: {}", path_str);
                settings.framework_dir_path = cef_string_from_str(&path_str);

                let resources_path = fw.join("Resources");
                // Commenting out manual resource path setting to let CEF use the bundle Resources
                /*
                if resources_path.exists() {
                    let resources_str = resources_path.to_string_lossy();
                    log_cef(&format!("CEF: Setting resources_dir_path to: {}", resources_str));
                    settings.resources_dir_path = cef_string_from_str(&resources_str);

                    let locales_path = resources_path.join("locales");
                    if locales_path.exists() {
                        let locales_str = locales_path.to_string_lossy();
                        log_cef(&format!("CEF: Setting locales_dir_path to: {}", locales_str));
                        settings.locales_dir_path = cef_string_from_str(&locales_str);
                    }
                }
                */

                if let Ok(exe_path) = std::env::current_exe() {
                    if let Some(bundle_path) = exe_path.parent().and_then(|p| p.parent()).and_then(|p| p.parent()) {
                        let bundle_str = bundle_path.to_string_lossy();
                        log_cef(&format!("CEF: Setting main_bundle_path to: {}", bundle_str));
                        settings.main_bundle_path = cef_string_from_str(&bundle_str);
                    }
                }
            }

            if let Some(helper) = Self::find_helper_executable(self.framework_path.as_ref()) {
                log_cef(&format!("CEF: Setting browser_subprocess_path to: {}", helper));
                settings.browser_subprocess_path = cef_string_from_str(&helper);
            } else {
                log_cef("CEF: Warning - no helper executable found. Set CEF_HELPER_PATH env var.");
            }
        }

        let mut window_info: cef_window_info_t = unsafe { std::mem::zeroed() };
        window_info.size = std::mem::size_of::<cef_window_info_t>();
        window_info.window_name = cef_string_from_str("cef");
        window_info.bounds = cef_rect_t {
            x: 0,
            y: 0,
            width: self.config.width as c_int,
            height: self.config.height as c_int,
        };
        window_info.windowless_rendering_enabled = 1;
        window_info.shared_texture_enabled = 0;
        window_info.external_begin_frame_enabled = 0;
        window_info.hidden = 0;
        window_info.parent_view = ptr::null_mut();
        window_info.view = ptr::null_mut();
        window_info.runtime_style = cef_runtime_style_t::CEF_RUNTIME_STYLE_ALLOY;

        let mut browser_settings: cef_browser_settings_t = unsafe { std::mem::zeroed() };
        browser_settings.size = std::mem::size_of::<cef_browser_settings_t>();
        browser_settings.windowless_frame_rate = 60;
        browser_settings.javascript = if self.config.javascript_enabled {
            cef_state_t::STATE_ENABLED
        } else {
            cef_state_t::STATE_DISABLED
        };
        browser_settings.webgl = if self.config.disable_gpu {
            cef_state_t::STATE_DISABLED
        } else {
            cef_state_t::STATE_ENABLED
        };

        // Only initialize CEF once per process
        static CEF_INITIALIZED: std::sync::Once = std::sync::Once::new();
        CEF_INITIALIZED.call_once(|| {
             unsafe {
                let init_result = (self.library.cef_initialize)(
                    &main_args,
                    &settings,
                    ptr::null_mut(),
                    ptr::null_mut(),
                );
                log_cef(&format!("cef_initialize returned {}", init_result));
            }
        });

        let url_str = cef_string_from_str(url);
        let browser = unsafe {
            (self.library.cef_browser_host_create_browser_sync)(
                &window_info,
                self.client.unwrap_or(ptr::null_mut()),
                &url_str,
                &browser_settings,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };

        log_cef("cef_browser_host_create_browser_sync returned");
        if browser.is_null() {
            return Err(CefError::InitFailed("failed to create browser".into()));
        }

        log_cef("browser created, waiting for first paint...");
        self.initialized = true;
        self.current_url = Some(url.to_string());
        self.browser = Some(browser as *mut cef_browser_t);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn find_helper_executable(framework_path: Option<&PathBuf>) -> Option<String> {
        if let Ok(path) = std::env::var("CEF_HELPER_PATH") {
            return Some(path);
        }

        if let Ok(exe_path) = std::env::current_exe() {
            log_cef(&format!("CEF: Current exe path: {}", exe_path.display()));
            if let Some(contents_dir) = exe_path.parent().and_then(|p| p.parent()) {
                log_cef(&format!("CEF: Contents dir: {}", contents_dir.display()));
                
                // Check for cef-demo Helper (created by bundle script)
                let helper = contents_dir.join("Frameworks/cef-demo Helper.app/Contents/MacOS/cef-demo Helper");
                log_cef(&format!("CEF: Checking helper at: {}", helper.display()));
                if helper.exists() {
                    return Some(helper.to_string_lossy().to_string());
                }
                
                let helper = contents_dir.join("Frameworks/demo-app Helper.app/Contents/MacOS/demo-app Helper");
                log_cef(&format!("CEF: Checking helper at: {}", helper.display()));
                if helper.exists() {
                    return Some(helper.to_string_lossy().to_string());
                }
                
                let helper = contents_dir.join("Frameworks/cefsimple Helper.app/Contents/MacOS/cefsimple Helper");
                log_cef(&format!("CEF: Checking helper at: {}", helper.display()));
                if helper.exists() {
                    return Some(helper.to_string_lossy().to_string());
                }
            }
        }

        framework_path.and_then(|fw| {
            let cef_root = fw.parent()?.parent()?; // up to cef/
            let helper =
                cef_root.join("build/tests/cefsimple/Release/cefsimple Helper.app/Contents/MacOS/cefsimple Helper");
            if helper.exists() {
                Some(helper.to_string_lossy().to_string())
            } else {
                None
            }
        })
    }

    fn shutdown_cef(&mut self) {
        unsafe {
            if let Some(browser) = self.browser.take() {
                 if let Some(release) = (*browser).base.release {
                     release(&mut (*browser).base);
                 }
            }
            // Note: We do NOT call cef_shutdown here because it can only be called once per process.
            // We leave it to the OS to clean up the process or call it in a dedicated shutdown hook if needed.
        }
        self.initialized = false;
        self.release_handlers();
    }
}

impl HeadlessRenderer for CefHeadless {
    fn navigate(&mut self, url: &str) -> Result<()> {
        self.loading.store(true, Ordering::Relaxed);
        if let Some(browser) = self.browser {
            unsafe {
                 if let Some(get_main_frame) = (*browser).get_main_frame {
                      let frame = get_main_frame(browser);
                      if !frame.is_null() {
                           if let Some(load_url) = (*frame).load_url {
                                let s = cef_string_from_str(url);
                                load_url(frame, &s);
                           }
                      }
                 }
            }
        } else {
             self.initialize(url)?;
        }
        Ok(())
    }

    fn load_html(&mut self, html: &str, base_url: Option<&str>) -> Result<()> {
        let data_url = if let Some(base) = base_url {
            format!("data:text/html;charset=utf-8,{}", percent_encode_with_base(html, base))
        } else {
            format!("data:text/html;charset=utf-8,{}", percent_encode(html))
        };
        self.navigate(&data_url)
    }

    fn capture_frame(&mut self) -> Result<FrameBuffer> {
        if !self.initialized {
            return Err(CefError::NotInitialized);
        }

        self.pump_messages();

        let frame = if let Ok(mut state) = self.render_state.lock() {
            state.take_frame()
        } else {
            None
        };

        let fb = frame.unwrap_or_else(|| FrameBuffer::new(self.config.width, self.config.height));
        if fb.is_empty() {
            // Log the first few empty captures to diagnose blank output.
            static EMPTY_COUNT: std::sync::OnceLock<AtomicUsize> = std::sync::OnceLock::new();
            let counter = EMPTY_COUNT.get_or_init(|| AtomicUsize::new(0));
            let count = counter.fetch_add(1, Ordering::Relaxed);
            if count < 5 {
                log_cef("capture_frame: empty frame");
            }
        }
        Ok(fb)
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        self.config.width = width;
        self.config.height = height;
        if let Ok(mut state) = self.render_state.lock() {
            state.resize(width, height);
        }
        if let Some(ref url) = self.current_url.clone() {
            self.navigate(url)?;
        }
        Ok(())
    }

    fn execute_js(&mut self, _script: &str) -> Result<Option<String>> {
        // TODO: expose executeJavaScript via frame/host bindings.
        Ok(None)
    }

    fn is_loading(&self) -> bool {
        self.loading.load(Ordering::Relaxed)
    }

    fn wait_for_load(&mut self, timeout_ms: u64) -> Result<()> {
        let start = std::time::Instant::now();
        while self.is_loading() {
            if start.elapsed().as_millis() as u64 > timeout_ms {
                return Err(CefError::Timeout(timeout_ms));
            }
            self.pump_messages();
            thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    fn send_mouse_event(&mut self, _event: MouseEvent) -> Result<()> {
        // Input forwarding not yet wired for the headless CEF path.
        Ok(())
    }

    fn send_key_event(&mut self, _event: KeyEvent) -> Result<()> {
        // Input forwarding not yet wired for the headless CEF path.
        Ok(())
    }

    fn pump_messages(&mut self) {
        if self.initialized {
            unsafe {
                (self.library.cef_do_message_loop_work)();
            }
        }
    }

    fn shutdown(&mut self) -> Result<()> {
        self.shutdown_cef();
        Ok(())
    }
}

fn percent_encode(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '<' => "%3C".to_string(),
            '>' => "%3E".to_string(),
            '#' => "%23".to_string(),
            '%' => "%25".to_string(),
            '"' => "%22".to_string(),
            '\n' => "%0A".to_string(),
            '\r' => "%0D".to_string(),
            _ if c.is_ascii_alphanumeric() || "-_.~".contains(c) => c.to_string(),
            _ => format!("%{:02X}", c as u32),
        })
        .collect()
}

fn percent_encode_with_base(input: &str, _base: &str) -> String {
    // Base URL is currently unused for simple data URL encoding but preserved
    // for API symmetry with the CDP backend.
    percent_encode(input)
}

impl Drop for CefHeadless {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
