# CEF-Rust Integration Implementation Plan

This document outlines the implementation plan for integrating CEF (Chromium Embedded Framework) with Rust for the Rune project.

## Overview

**Goal:** Create a Rust-based CEF integration that can render web content to a texture for use in Rune's GPU rendering pipeline.

**Architecture:**
- `crates/rune-cef/shim/` - C++ shim (in repo)
- `crates/rune-cef/` - Rust crate with FFI bindings to CEF
- `crates/cef-demo/` - Demo crate showing CEF integration with Rune
- `cef-app/` - Xcode project for macOS app bundling and code signing
- `vendor/cef/` or `CEF_ROOT` env var - CEF binaries (gitignored, download separately)

---

## Phase 1: C++ Shim Layer (DONE)

The shim is at `crates/rune-cef/shim/`:
- `rune_cef_shim.h` - C-compatible header
- `rune_cef_shim.cc` - Implementation
- `CMakeLists.txt` - Build configuration

### 1.1 Existing Shim API
- [x] `rune_cef_init()` - Initialize CEF with settings
- [x] `rune_cef_shutdown()` - Shutdown CEF
- [x] `rune_cef_do_message_loop_work()` - Process CEF messages
- [x] `rune_cef_create_browser()` - Create off-screen browser
- [x] `rune_cef_destroy_browser()` - Destroy browser instance
- [x] `rune_cef_navigate()` - Navigate to URL
- [x] `rune_cef_load_html()` - Load HTML string
- [x] `rune_cef_resize()` - Resize browser viewport
- [x] `rune_cef_send_mouse_event()` - Send mouse input
- [x] `rune_cef_send_key_event()` - Send keyboard input
- [x] `rune_cef_get_frame()` - Get pixel buffer
- [x] `rune_cef_is_loading()` - Check loading state

### 1.2 Existing Types
```c
// Already defined in rune_cef_shim.h
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

// Mouse and key event types also defined
```

### 1.3 Potential Enhancements (Optional)
- [ ] Add `rune_cef_execute_javascript()` for JS execution
- [ ] Add `rune_cef_set_focus()` for focus management
- [ ] Add callback for console messages

---

## Phase 2: Rust FFI Bindings (`rune-cef`) (DONE)

The Rust crate is at `crates/rune-cef/` with full FFI bindings and safe API.

### 2.1 Crate Setup
- [x] Create `crates/rune-cef/Cargo.toml`
- [x] Add dependencies: `libloading` for dynamic CEF loading, `wgpu`, `anyhow`, `thiserror`
- [x] Create `build.rs` for CEF library linking

### 2.2 build.rs Implementation
- [x] Dynamic loading via `libloading` crate (no compile-time CEF dependency)
- [x] Handle macOS framework paths
- [x] Support `CEF_PATH` environment variable

### 2.3 Raw FFI Bindings
- [x] `src/cef_sys.rs` - CEF C API type definitions
- [x] `src/shim_ffi.rs` - FFI bindings to C++ shim
- [x] FFI-safe struct representations

### 2.4 Safe Rust API
- [x] `HeadlessBuilder` - Builder pattern for renderer creation
- [x] `HeadlessRenderer` trait - Common API for different backends
- [x] `CefBackend` - Full CEF integration via shim (`src/cef_backend.rs`)
- [x] `ShimBackend` - Alternative shim-based backend (`src/shim_backend.rs`)
- [x] Error handling with `CefError` type

### 2.5 wgpu Integration
- [x] `WgpuTextureTarget` - Upload CEF frames to wgpu textures
- [x] `Frame` struct for BGRA pixel data
- [x] Bind group and texture creation helpers

### 2.6 Current API Design
```rust
// lib.rs - Actual implemented API
pub trait HeadlessRenderer {
    fn navigate(&mut self, url: &str) -> Result<()>;
    fn load_html(&mut self, html: &str, base_url: Option<&str>) -> Result<()>;
    fn resize(&mut self, width: u32, height: u32) -> Result<()>;
    fn capture_frame(&mut self) -> Result<Frame>;
    fn wait_for_load(&mut self, timeout_ms: u64) -> Result<bool>;
    fn tick(&mut self) -> Result<()>;
}

pub struct HeadlessBuilder { /* ... */ }

impl HeadlessBuilder {
    pub fn new() -> Self;
    pub fn with_size(self, width: u32, height: u32) -> Self;
    pub fn with_scale_factor(self, scale: f32) -> Self;
    pub fn build_cef(self) -> Result<CefBackend>;  // Requires CEF runtime
}

// wgpu integration
pub struct WgpuTextureTarget { /* ... */ }

impl WgpuTextureTarget {
    pub fn new(device: &wgpu::Device, width: u32, height: u32, label: Option<&str>) -> Self;
    pub fn upload(&mut self, queue: &wgpu::Queue, frame: &Frame) -> Result<()>;
    pub fn create_bind_group(&self, device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::BindGroup;
}
```

---

## Phase 3: Xcode Project Setup (DONE)

The `cef-app/` Xcode project is created and working with CEF (currently using Metal rendering from cef-test).

### 3.1 Create New Xcode Project ✅
- [x] Create `cef-app/` directory at project root
- [x] Created by copying and renaming cef-test
- [x] Configured for Objective-C++
- [x] Deployment target: macOS 12.0
- [x] C++ language standard: GNU++17

### 3.2 CEF Structure ✅
- [x] CEF binary distribution symlinked from `cef-test/cef/`
- [x] Framework already in versioned structure
- [x] Helper app targets: GPU, Renderer, Alerts, Plugin
- [x] Framework embedding configured
- [x] Library search paths configured

### 3.3 Current App Structure
```
cef-app/
├── cef-app.xcodeproj/
├── cef-app/
│   ├── main.mm                # App entry with CEF init
│   ├── AppDelegate.h/m        # App lifecycle
│   ├── ViewController.h/mm    # Metal rendering (to be replaced)
│   ├── Renderer.h/mm          # Metal renderer (to be replaced)
│   ├── Shaders.metal          # Metal shaders (to be replaced)
│   ├── Info.plist
│   └── Assets.xcassets/
├── cef-app Helper/
│   ├── process_helper_mac.cc  # Helper process entry
│   └── Info.plist (multiple)
└── cef -> ../cef-test/cef     # Symlink to CEF binaries
```

### 3.4 Code Signing ✅
- [x] App Sandbox disabled for development
- [x] Entitlements configured for helpers

---

## Phase 3.5: Rust/wgpu Integration into cef-app (DONE)

Replace Metal rendering with Rust/wgpu via the `cef-demo` crate.

### Implementation Notes (Completed)

**Key Design Decision:** CEF initialization and browser management stays in Xcode (Objective-C++), while Rust/wgpu handles only the GPU rendering. This avoids conflicts between Rust's rune-cef CefInitialize and Xcode's CefInitialize.

**Architecture:**
1. Xcode manages CEF lifecycle (init, browser creation, shutdown)
2. CEF's `OnPaint` callback sends pixel data to Rust via FFI
3. Rust/wgpu renders the pixels to a `CAMetalLayer` provided by Xcode
4. CVDisplayLink drives the render loop, calling Rust directly

**Key Technical Fixes:**
- Use `#[unsafe(no_mangle)]` syntax (Rust 2024 edition)
- Use `Rgba8UnormSrgb` texture format for correct sRGB color handling
- Convert BGRA (CEF native) to RGBA when uploading to texture
- Call `cef_demo_render()` directly from display link (no `dispatch_async`)
- Only render when texture size matches surface size to prevent stretching during resize

### 3.5.1 Modify cef-demo Crate for Static Library
- [x] Add `crate-type = ["staticlib", "rlib"]` to `Cargo.toml`
- [x] Create `src/lib.rs` with `AppRenderer` struct
- [x] Create `src/ffi.rs` with C-compatible exports
- [x] Implement FFI functions:

```rust
// src/ffi.rs - Actual implementation (simplified)
use std::ffi::c_void;

#[unsafe(no_mangle)]
pub extern "C" fn cef_demo_init(
    width: u32, height: u32, scale: f32, metal_layer: *mut c_void,
) -> bool {
    // Initialize wgpu with Metal layer from Xcode
    match AppRenderer::new(width, height, scale, metal_layer) {
        Ok(renderer) => { /* store in global */ true }
        Err(_) => false
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cef_demo_shutdown() { /* release renderer */ }

#[unsafe(no_mangle)]
pub extern "C" fn cef_demo_upload_pixels(
    pixels: *const u8, width: u32, height: u32, stride: u32,
) {
    // Upload BGRA pixels from CEF OnPaint callback
    // Converts BGRA->RGBA and uploads to wgpu texture
}

#[unsafe(no_mangle)]
pub extern "C" fn cef_demo_resize(width: u32, height: u32) {
    // Resize wgpu surface
}

#[unsafe(no_mangle)]
pub extern "C" fn cef_demo_render() {
    // Render textured quad to surface
}
```

**Note:** Mouse/keyboard events are handled directly by Xcode's CEF integration, not passed through Rust.

### 3.5.2 Create C Header (`cef_demo.h`) ✅
```c
#ifndef CEF_DEMO_H
#define CEF_DEMO_H

#include <stdbool.h>
#include <stdint.h>

// Initialize with CAMetalLayer from the view
bool cef_demo_init(uint32_t width, uint32_t height, float scale, void* metal_layer);
void cef_demo_shutdown(void);
void cef_demo_upload_pixels(const uint8_t* pixels, uint32_t width, uint32_t height, uint32_t stride);
void cef_demo_resize(uint32_t width, uint32_t height);
void cef_demo_render(void);

#endif
```

### 3.5.3 Build Rust Static Library ✅
```bash
# Build script: cef-app/build-rust.sh
#!/bin/bash
set -e
cd "$(dirname "$0")/.."
cargo build -p cef-demo --release
mkdir -p cef-app/lib
cp target/release/libcef_demo.a cef-app/lib/
echo "Built libcef_demo.a"
```

Run with: `./cef-app/build-rust.sh`

### 3.5.4 Xcode Project Changes ✅
1. ✅ **Add library to Link Binary**:
   - Add `libcef_demo.a` from `$(PROJECT_DIR)/lib/`

2. ✅ **Add Library Search Path**:
   - `$(PROJECT_DIR)/lib` (non-recursive)

3. ✅ **Add header** (`cef_demo.h`) to project root

4. ✅ **Modify ViewController.mm**:
   - Created `RustRenderView` with CAMetalLayer for wgpu
   - CEF's `OnPaint` calls `cef_demo_upload_pixels()`
   - CVDisplayLink calls `cef_demo_render()` directly
   - Mouse events forwarded to CEF via native CefHandler

### 3.5.5 Actual ViewController.mm Architecture ✅

The implementation uses a custom `RustRenderView` class:

```objc
// RustRenderView - provides CAMetalLayer for wgpu
@interface RustRenderView : NSView
- (void)onCefPaint:(const void*)buffer width:(int)width height:(int)height;
@end

@implementation RustRenderView
- (void)viewDidMoveToWindow {
    if (self.window && !_initialized) {
        // Initialize Rust renderer with CAMetalLayer
        cef_demo_init(width, height, scale, (__bridge void*)_metalLayer);

        // Start display link - calls render directly (no dispatch_async!)
        CVDisplayLinkCreateWithActiveCGDisplays(&_displayLink);
        CVDisplayLinkSetOutputCallback(_displayLink, &displayLinkCallback, ...);
        CVDisplayLinkStart(_displayLink);
    }
}

static CVReturn displayLinkCallback(...) {
    cef_demo_render();  // Direct call - wgpu/Metal is thread-safe
    return kCVReturnSuccess;
}

- (void)onCefPaint:(const void*)buffer width:(int)width height:(int)height {
    cef_demo_upload_pixels((const uint8_t*)buffer, width, height, width * 4);
}
@end

// CefHandler - forwards OnPaint to RustRenderView
void CefHandler::OnPaint(..., const void* buffer, int width, int height) {
    [view onCefPaint:buffer width:width height:height];
}
```

Mouse/keyboard events use native CEF forwarding via `CefBrowserHost::SendMouseClickEvent()` etc.

### 3.5.6 Implementation Steps Summary ✅
1. [x] Modify `crates/cef-demo/Cargo.toml` - add staticlib
2. [x] Create `crates/cef-demo/src/lib.rs` - AppRenderer struct
3. [x] Create `crates/cef-demo/src/ffi.rs` - FFI exports
4. [x] Create `cef-app/cef_demo.h` - C header
5. [x] Create `cef-app/build-rust.sh` - build script
6. [x] Add `libcef_demo.a` to Xcode Link Binary
7. [x] Add Library Search Path `$(PROJECT_DIR)/lib`
8. [x] Rewrite `ViewController.mm` with RustRenderView + CefHandler
9. [x] Test build and run - renders Google with correct colors

---

## Phase 4: Demo Crate (`cef-demo`) (DONE - Windowed Demo)

The demo crate exists at `crates/cef-demo/` as a standalone winit+wgpu application.

### 4.1 Crate Setup
- [x] Create `crates/cef-demo/Cargo.toml`
- [x] Depend on `rune-cef`, `wgpu`, `winit`, `pollster`
- [x] Implemented as binary (not staticlib) - runs standalone

### 4.2 Current Implementation
The demo is a windowed application that:
- Creates a winit window with wgpu rendering
- Uses `HeadlessBuilder` to create a CEF renderer
- Uploads CEF frames to a wgpu texture
- Renders the texture as a fullscreen quad

Run with: `cargo run -p cef-demo -- --cef-url=https://example.com`

### 4.3 For Xcode Integration (DONE - see Phase 3.5)
Completed as part of Phase 3.5:
- [x] Add `crate-type = ["staticlib"]` to Cargo.toml
- [x] Create `#[unsafe(no_mangle)] extern "C"` exported functions in `src/ffi.rs`
- [x] Create C header `cef-app/cef_demo.h`

Implemented FFI exports:
```rust
#[unsafe(no_mangle)]
pub extern "C" fn cef_demo_init(width: u32, height: u32, scale: f32, metal_layer: *mut c_void) -> bool
pub extern "C" fn cef_demo_shutdown()
pub extern "C" fn cef_demo_upload_pixels(pixels: *const u8, width: u32, height: u32, stride: u32)
pub extern "C" fn cef_demo_resize(width: u32, height: u32)
pub extern "C" fn cef_demo_render()
```

---

## Phase 5: Integration with Rune Engine

### 5.1 Texture Upload
- [ ] Create wgpu texture from CEF pixel buffer
- [ ] Implement efficient texture updates (partial updates if possible)
- [ ] Handle BGRA to RGBA conversion if needed

### 5.2 Event Routing
- [ ] Route mouse events from Rune hit-testing to CEF
- [ ] Route keyboard events to focused CEF browser
- [ ] Handle focus management

### 5.3 Integration Points
- [ ] Add CEF browser as IR element type
- [ ] Implement in `rune-scene` viewport rendering
- [ ] Create CEF element for IR renderer

---

## Phase 6: Testing & Polish

### 6.1 Basic Tests
- [ ] CEF initialization and shutdown
- [ ] Browser creation and URL loading
- [ ] Render callback receives pixels
- [ ] Mouse and keyboard input

### 6.2 Integration Tests
- [ ] CEF renders to Rune texture
- [ ] Interactive web content in Rune scene
- [ ] Multiple browser instances

### 6.3 Performance
- [ ] Profile texture upload path
- [ ] Optimize callback overhead
- [ ] Consider shared memory for pixel transfer

---

## File Checklist

### Completed Files (in repo)
```
crates/rune-cef/             # Rust CEF integration crate
├── Cargo.toml
├── build.rs
├── shim/                    # C++ shim (small, versioned)
│   ├── CMakeLists.txt
│   ├── rune_cef_shim.h
│   └── rune_cef_shim.cc
└── src/
    ├── lib.rs               # Public API exports
    ├── cef_backend.rs       # Full CEF backend via dynamic loading
    ├── cef_sys.rs           # CEF C API type definitions
    ├── shim_backend.rs      # Shim-based backend
    ├── shim_ffi.rs          # FFI bindings to shim
    ├── cdp_backend.rs       # Chrome DevTools Protocol backend (optional)
    ├── error.rs             # Error types
    ├── frame.rs             # Frame buffer types
    └── texture.rs           # wgpu texture target

crates/cef-demo/             # Demo windowed application
├── Cargo.toml
└── src/
    ├── main.rs              # winit+wgpu demo app
    └── shader.wgsl          # Textured quad shader

cef-test/                    # Reference Xcode project (working)
├── Testapp.xcodeproj/
├── testapp/
├── testapp Helper/
└── cef/                     # CEF binary distribution (gitignored)

cef-app/                     # Production Xcode project (Rust/wgpu rendering)
├── cef-app.xcodeproj/
├── cef_demo.h               # C header for Rust FFI
├── build-rust.sh            # Build script for libcef_demo.a
├── lib/
│   └── libcef_demo.a        # Built Rust static library
├── cef-app/
│   ├── main.mm              # CEF initialization
│   ├── AppDelegate.h/m
│   ├── ViewController.h/mm  # RustRenderView + CefHandler (Rust rendering)
│   └── Info.plist
├── cef-app Helper/
│   ├── process_helper_mac.cc
│   └── Info.plist (multiple)
└── cef -> ../cef-test/cef   # Symlink to CEF binaries
```

### CEF Binaries (gitignored, download separately)
```
cef-test/cef/                # CEF binary distribution
├── Debug/
│   └── Chromium Embedded Framework.framework/
├── Release/
│   └── Chromium Embedded Framework.framework/
├── include/
├── libcef_dll/
├── build/                   # Generated by cmake
│   └── libcef_dll_wrapper/Debug/libcef_dll_wrapper.a
└── README.txt
```

### Completed - Rust/wgpu Integration Files ✅
```
cef-app/                     # Files added for Rust integration
├── cef_demo.h               # ✅ C header for Rust FFI
├── build-rust.sh            # ✅ Script to build Rust static lib
└── lib/
    └── libcef_demo.a        # ✅ Built Rust library

crates/cef-demo/
├── src/
│   ├── main.rs              # Existing windowed demo
│   ├── lib.rs               # ✅ AppRenderer + library entry point
│   ├── ffi.rs               # ✅ FFI exports for Xcode
│   └── shader.wgsl          # Textured quad shader
└── Cargo.toml               # ✅ crate-type = ["staticlib", "rlib"]
```

### Workspace Files (already updated)
- [x] `Cargo.toml` (workspace) - `rune-cef` and `cef-demo` added
- [x] `.gitignore` - CEF binaries and Xcode artifacts excluded

---

## Dependencies

### Rust Crates
- `libc` - C type definitions
- `parking_lot` - Synchronization primitives
- `cc` - C++ compilation in build.rs
- `cmake` (optional) - If using CMake for shim

### System Requirements
- macOS 12.0+
- Xcode 13.5+
- CMake 3.21+
- Rust 1.70+
- CEF binary distribution (ARM64 or x64)

---

## Notes

### Thread Safety
- CEF must be initialized on the main thread
- Render callbacks come from CEF's render thread
- Use channels or mutexes to pass data to Rust main thread

### Memory Management
- CEF manages its own memory for pixel buffers
- Copy pixel data in OnPaint callback before returning
- Browser instances must be destroyed before CEF shutdown

### macOS Specific
- Helper apps required for multi-process architecture
- Framework must be properly signed for distribution
- Entitlements needed for certain features

---

## Status Summary

| Phase | Description | Status | Complexity |
|-------|-------------|--------|------------|
| 1 | C++ Shim Layer | ✅ DONE | Medium |
| 2 | Rust FFI Bindings | ✅ DONE | Medium |
| 3 | Xcode Project Setup | ✅ DONE | Medium |
| 3.5 | Rust/wgpu Integration | ✅ DONE | Medium |
| 4 | Demo Crate | ✅ DONE (windowed) | Low |
| 5 | Rune Integration | ⏳ TODO | High |
| 6 | Testing & Polish | ⏳ TODO | Medium |

**Current State:**
- Phases 1, 2, 3, 3.5, and 4 are complete
- `cef-app/` Xcode project works with CEF + Rust/wgpu rendering
- CEF runs in Xcode, Rust handles GPU rendering via `libcef_demo.a`
- Colors render correctly (sRGB), smooth animation via CVDisplayLink
- `cef-demo` also works as standalone windowed wgpu application
- `rune-cef` crate builds with full CEF FFI bindings

**Next Steps:**
- **Phase 5**: Integrate CEF as IR element type in rune-scene

---

## References

- [CEF C++ API Documentation](https://magpcss.org/ceforum/apidocs3/)
- [CEF Wiki - General Usage](https://bitbucket.org/chromiumembedded/cef/wiki/GeneralUsage)
- [Rust FFI Guide](https://doc.rust-lang.org/nomicon/ffi.html)
- [cef-test setup guide](./cef-test-setup.md)
