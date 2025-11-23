Continue implementing native NSView-based CEF rendering for the rune-v3 project.

## Context

We tried OSR (off-screen rendering) CEF → wgpu texture path but it was slow and flickery due to:

- 8MB+ CPU buffer copies per frame
- BGRA/RGBA format conversions
- Mutex contention between CEF paint callback and render thread
- No viable zero-copy path between CEF and wgpu

## Decision

Switch to native NSView-based CEF that macOS composites over the wgpu surface.

## Requirements

1. CEF renders to its own NSView (not OSR)
2. NSView is positioned within a viewport rect from the layout system
3. NSView frame updates when viewport moves/resizes
4. Keep full CEF API access: navigation interception, DOM extraction, JS injection
5. Integrate with existing rune-scene WebView element for rect tracking

## Key Files

- crates/rune-cef/src/cef_backend.rs - current OSR implementation
- crates/rune-cef/src/lib.rs - HeadlessRenderer trait
- crates/rune-scene/src/elements/webview.rs - WebView element, has set_webview_rect()
- crates/rune-ffi/src/lib.rs - FFI bridge to Objective-C
- rune-app/rune-app/ViewController.mm - Objective-C side

## Implementation Plan

1. Add native CEF view creation in Objective-C (ViewController.mm)
2. Create FFI functions: rune_create_cef_view(), rune_position_cef_view(x,y,w,h)
3. Add NativeViewRenderer trait/struct in rune-cef (alternative to HeadlessRenderer)
4. Update rune-scene to call positioning FFI when viewport rect changes
5. Remove OSR pixel upload code path

## Architecture

NSWindow
├── MTKView (wgpu surface) - your UI
└── NSView (CEF) - positioned at viewport rect, macOS composites

CEF handlers (OnBeforeNavigation, GetSource, ExecuteJavaScript) work the same regardless of rendering mode.
