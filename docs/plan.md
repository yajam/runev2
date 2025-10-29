# Plan: Custom GPU-Native 2D Engine (Rune/Wisp)

> Phased checklist distilled from docs/scope.md. Keep items small and verifiable. Tests and performance appear last by design.

## Phase 0 — Project Setup
- [x] Create `engine-core` and `engine-shaders` crates in a Cargo workspace
- [x] Add deps: `wgpu`, `palette`, `bytemuck`, `anyhow`, `thiserror`
- [x] Set up CI (build + fmt + clippy) and lint gates
- [x] Add example/demo crate scaffold with `wgpu` surface (headless init for now)

## Phase 1 — Backend & Allocation
- [x] Initialize `wgpu` device/queue/swapchain abstraction
- [x] Implement `RenderAllocator` for buffers/textures with pooling
- [x] Create transient targets (intermediate textures) lifecycle API

## Phase 2 — DisplayList + Painter (IR-agnostic)
- [x] Define primitives: `Rect`, `RoundedRect`, `TextRun`, `Clip`, `Transform`
- [x] Materials: `Brush::Solid`, `Brush::LinearGradient` (structure only)
- [x] Immediate-mode `Painter` → `DisplayList` builder
- [x] Scene upload path: `DisplayList` → CPU mesh → GPU buffers
- [ ] Optional adapters (Rune IR, Taffy) — deferred

## Phase 3 — Shader Suite (Foundations)
- [x] Establish WGSL module layout and common include (types, constants)
- [x] Implement linear-space solid fill shader (premultiplied alpha)
- [x] Implement gradient evaluation in linear RGB (initial f32; f16 later)
- [ ] Optional dithering toggle before write-out

## Phase 4 — Compositor & Pass Manager
- [x] `PassManager` orchestrates: solid → composite → output (initial)
- [x] Implement premultiplied alpha blending rules in compositor
- [x] Output transform via sRGB surface write

## Phase 5 — Box Shadow Pipeline
- [ ] Generate shadow mask from rect + spread + offset
- [ ] Separable Gaussian blur (alpha mask) with configurable radius
- [ ] Multiply blurred alpha by shadow color (premultiplied)
- [ ] Composite beneath source layer using compositor

## Phase 6 — Text Rendering (Subpixel AA)
- [ ] Fork `fontdue` to emit RGB coverage masks (optionally 16-bit)
- [ ] Extend `cosmic-text`: `RenderSettings::SubpixelAA` + RGB/BGR toggle
- [ ] GPU text pass consumes RGB coverage; supports fractional positioning
- [ ] (Optional) Add FreeType FFI path for hinted masks at small sizes

## Phase 7 — Color Management
- [ ] Enforce linear-light internal computations across passes
- [ ] Use `palette` for conversions and encoding management
- [ ] Add HDR-ready paths when 16f/32f targets available

## Phase 8 — Public API
- [ ] Implement `GraphicsEngine::new(device: &wgpu::Device) -> Self`
- [ ] Implement `render_scene(scene: &SceneGraph, target: &wgpu::TextureView)`
- [ ] Document safety/ownership and lifetime expectations

## Phase 9 — Demo Application
- [ ] Render gradient gallery (banding comparison + dithering toggle)
- [ ] Render shadow playground (blur/spread/offset parameters)
- [ ] Text compare: grayscale vs subpixel AA (RGB/BGR switch)
- [ ] Showcase persistent atlases (glyphs/gradients/blurs) behavior

## Phase 10 — Documentation
- [ ] API docs for core modules and passes
- [ ] WGSL shader notes (precision, blending, color space)
- [ ] Integration guide for Rune IR → SceneGraph

## Phase 11 — Tests (Keep last)
- [ ] Unit: linear gradient interpolation correctness (CPU reference)
- [ ] Unit: premultiplied alpha compositing math
- [ ] GPU: render tests for gradients, shadows (image-based thresholds)
- [ ] Text: grayscale vs subpixel AA golden comparisons
- [ ] Allocator: pooling/reuse invariants and leak checks

## Phase 12 — Performance (Keep last)
- [ ] Benchmark: <1ms GPU time @1080p for ~1k elements
- [ ] Profile pass timings; minimize texture reallocations
- [ ] Validate atlas policies (glyph/gradient/blur) and cache hit rates
- [ ] Pipeline/state caching to reduce command overhead

## Phase 13 — Release & Upstream
- [ ] Dual-license MIT/Apache-2.0; add NOTICE
- [ ] Maintain `cosmic-text` and `fontdue` patch branches
- [ ] Prepare upstreamable PRs (linear color, subpixel AA)
- [ ] Tag crates and publish demo instructions

## Backlog / Future Extensions
- [ ] SDF text and SVG glyph outlines
- [ ] Compute-based filters (blur, bloom, glow, inner shadow)
- [ ] Multithreaded scene upload and batching
- [ ] Skia-compatible paint model layer
