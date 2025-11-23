## Features

- [ ] Text selection and highlight
- [x] Datepicker
- [x] Hyperlink element
- [NP] Box in elements - use canvas.draw_rect
- [x] File input was not added
- [x] All elements should take dynamic parameters for styling
- [x] Add Rune-ir
- [x] Rune-layout using taffy
- [ ] Address input, forward, backward, refresh implementation in toolbar
- [x] Port persistence.rs
- [ ] Bookmark, Tabs
- [ ] Port Boa, csslightning (canonical)
- [ ] Test gradients in layers
- [x] Full svg vector support
- [ ] Multi select
- [ ] Custom widgets through IR blocks using primitives
- [ ] wire to wasm, fetch, form submission
- [ ] Pdf export
- [ ] Elements, Console and Network implementation in devtools
- [ ] Light mode
- [ ] Root level font definition inherit
- [ ] Theming support dark mode, light mode system mode
- [ ] Native menus support
- [x] Web page rendering (NSView CEF via rune-ffi)
- [ ] Fallback theough headless CEF

## IR Porting

- [x] Input, text, textarea, buttons, label, hyperlinks, Table
- [x] Image, Select, datepicker, fileinput
- [x] Alert, modal, confirm
- [x] Checkboxes, Radio buttons
- [~] Scroll horizontal and vertical

## Bugs

- [ ] Text is still not crisp, specs and dust flickering
- [x] Textarea caret down is not working correctly
- [ ] Smooth scrolling in CEF/NSView rendering path
- [ ] Edge visbility fix using background expansion
- [x] Caret blink is not consistent
- [x] Change SVG to true vector
- [ ] Text editing textarea line selection multiple with keyboard doesnt work
- [ ] Double click triple click doesnt work right only highlights till cursor position
- [ ] Hyperlink underline mismatch
- [ ] Load delay for elements

## Improvements

- [x] Select needs a placeholder
- [ ] Implement a scrollbar
- [x] Refactor and move viewport_ir as a second demo-app - removed completely
- [ ] Text must ALWAYS be rendered in a linear (unorm) format, with premultiplied alpha, into a linear color attachment.
- [~] Add style parameter for all elements and make it robust

Testing

- BIDI testing

## Additional

- [ ] Video/Audio subsystem (core missing block)
- [ ] A11y subsystem completion
- [ ] Full input/focus model (pointer capture, inertial scroll, tab order)
- [ ] CEF main-thread initialization wired into winit backend (current rune-app + rune-ffi path is a stop-gap until CEF OSR is fully hosted from the winit-owned window/surface)
- [ ] JS runtime → DOM mutation + events integration
- [ ] Download manager
- [ ] GPU device lost recovery
- [ ] Telemetry (chrome traces, overlays)
- [ ] Platform parity for Wayland/macOS/Windows IME & scaling
- [ ] Testing suite (layout/text/gpu/event/IR)
- [ ] Developer tooling & CI
- [ ] Explore further `librune_ffi.a` size reductions (feature gating, symbols, archive tools) beyond current ~54M build

---

### CEF / rune-app integration summary

Problem: Today `rune-scene` assumes a winit-owned window/event loop and owns the `wgpu::Surface`, while the CEF sample (`cef-app` / `rune-app`) assumes a Cocoa-owned NSWindow, CEF main-thread initialization, and a `CAMetalLayer` provided to Rust. There is no unified path where Rune both owns the window and hosts CEF OSR on the same main thread, so we cannot yet run “Rune window + Rune compositor + CEF WebView element” purely from the winit backend.

Stopgap: `rune-app` embeds Rune via `rune-ffi` as a pure renderer: Cocoa/CEF own the window and message loop, and pass a `CAMetalLayer` plus mouse/keyboard/text events and an OSR pixel buffer (for WebView textures) into the Rune compositor. This keeps all of the complex CEF bootstrap and helper processes inside the existing Xcode project, at the cost of re-implementing the window/input glue that the winit runner already has.

Potential solution: Long term, move CEF initialization and OSR hosting into a dedicated “CEF backend” wired directly into `rune-scene`’s winit runner (via the `webview-cef` / `rune-cef` path). That means: initializing CEF on the main thread behind a reusable abstraction, integrating its message pumping with winit’s event loop, and wiring WebView frames into the Rune compositor and hit-testing from the winit-owned `wgpu::Surface`. Once that path is stable, `rune-app` can become a thin wrapper around the winit-based Rune binary, and the current `rune-ffi` + Cocoa glue can be retired or kept only as an embedding option.

Status: NSView-based CEF rendering is working through `rune-ffi`; the remaining known gap is smoothing out scroll behavior in that path.
