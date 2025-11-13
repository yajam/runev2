# Rune Surface — Canvas-Oriented Rendering API

This document proposes and specifies the `rune-surface` module: a thin, ergonomic
wrapper that exposes a canvas‑style drawing flow on top of Rune’s existing
`Painter` + `PassManager` pipeline. It is designed for simple scenes where you
want to: begin a frame, clear, draw a few primitives and text, and end the
frame to present — without manually juggling display lists, uploads, and pass
selection.

## Goals

- A minimal “canvas” API over Rune’s core types.
- Safe lifecycle: `begin_frame(..) … end_frame()` does the right thing.
- Integrates text layout/rasterization via `TextProvider` or pre‑rasterized glyphs.
- Plays nicely with Rune’s offscreen→surface path and MSAA.

## Key Concepts

- Canvas: builder for a frame’s draw commands that internally uses `Painter`.
- Surface: the window’s swapchain surface and its `SurfaceTexture` frame.
- Scene: the logical list of draw commands collected during a frame.
- Provider: a `TextProvider` implementation used to layout/rasterize text runs
  when you opt into high‑level text drawing.

## API Shape (proposed)

```rust
// Construct once at startup
pub struct RuneSurface {
    device: std::sync::Arc<wgpu::Device>,
    queue: std::sync::Arc<wgpu::Queue>,
    surface: wgpu::Surface,
    pass: engine_core::PassManager,
    alloc: engine_core::allocator::RenderAllocator,
    direct: bool,                // direct-to-surface vs offscreen+composite
    preserve_surface: bool,      // Load vs Clear at frame start
}

pub struct Canvas {
    painter: engine_core::Painter,
    clear_color: Option<engine_core::scene::ColorLinPremul>,
    // Optional text provider (for TextRun based drawing)
    text_provider: Option<std::sync::Arc<dyn engine_core::TextProvider + Send + Sync>>,
}

impl RuneSurface {
    pub fn new(device: std::sync::Arc<wgpu::Device>,
               queue: std::sync::Arc<wgpu::Queue>,
               surface: wgpu::Surface,
               surface_format: wgpu::TextureFormat) -> Self { /* … */ }

    pub fn begin_frame(&mut self, width: u32, height: u32) -> anyhow::Result<(wgpu::SurfaceTexture, Canvas)> { /* … */ }

    pub fn end_frame(&mut self,
                     mut surface_frame: wgpu::SurfaceTexture,
                     canvas: Canvas) -> anyhow::Result<()> { /* … */ }
}

impl Canvas {
    pub fn clear(&mut self, c: engine_core::scene::ColorLinPremul) { /* store clear_color */ }

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32,
                     brush: engine_core::scene::Brush, z: i32) { /* painter.rect */ }

    pub fn stroke_path(&mut self, path: engine_core::scene::Path,
                       width: f32,
                       color: engine_core::scene::ColorLinPremul,
                       z: i32) { /* painter.stroke_path */ }

    // High-level text: rely on TextRun + provider
    pub fn draw_text_run(&mut self, origin: [f32; 2],
                         text: String, size_px: f32, color: engine_core::scene::ColorLinPremul,
                         z: i32) { /* painter.text(TextRun { … }) */ }

    // Low-level text: supply pre-rasterized glyph masks you already laid out
    pub fn draw_text_masks(&mut self,
        origin: [f32; 2],
        glyphs: &[engine_core::text::RasterizedGlyph],
        color: engine_core::scene::ColorLinPremul,
        z: i32,
    ) { /* record as a special command (or draw with PassManager at end) */ }
}
```

Notes:
- Internally, `Canvas` is simply a `Painter` plus optional `clear_color` and
  an optional `TextProvider` handle for high‑level text.
- `end_frame` finishes the painter → `DisplayList`, uploads via
  `engine_core::upload_display_list`, and calls into `PassManager` to render
  solids and text, then presents the surface texture.

## Minimal Usage

```rust
// Initialization (once)
let device: std::sync::Arc<wgpu::Device> = /* … */;
let queue: std::sync::Arc<wgpu::Queue> = /* … */;
let surface: wgpu::Surface = /* winit surface */;
let surface_format = /* chosen swapchain format */;

let mut rs = RuneSurface::new(device.clone(), queue.clone(), surface, surface_format);

// Per frame
let size = window.inner_size();
let (frame, mut canvas) = rs.begin_frame(size.width, size.height)?;

canvas.clear(engine_core::scene::ColorLinPremul::rgba(0.0, 0.0, 0.0, 0.0));

// Fill rect
canvas.fill_rect(
    20.0, 20.0, 200.0, 120.0,
    engine_core::scene::Brush::Solid(engine_core::scene::ColorLinPremul::rgba16f(0.8, 0.2, 0.4, 1.0)),
    0,
);

// Stroke a path
let mut path = engine_core::scene::Path::new();
path.move_to([50.0, 50.0]);
path.line_to([180.0, 80.0]);
path.quad_to([220.0, 120.0], [160.0, 160.0]);
canvas.stroke_path(
    path,
    2.0,
    engine_core::scene::ColorLinPremul::rgba(1.0, 1.0, 1.0, 1.0),
    1,
);

// Text via run + provider
canvas.draw_text_run(
    [32.0, 180.0],
    "Hello Rune".to_string(),
    20.0,
    engine_core::scene::ColorLinPremul::rgba(1.0, 1.0, 1.0, 1.0),
    2,
);

// Finish and present
rs.end_frame(frame, canvas)?;
```

Alternate call style (exactly as requested) can be expressed by exposing
helpers on `RuneSurface` that return a reusable `Canvas` bound to the surface
frame:

```rust
let mut frame = surface.acquire_texture()?; // wraps surface.get_current_texture()
let mut canvas = rs.canvas();              // creates an unattached Canvas
canvas.begin_frame(&mut frame, size.width, size.height);
canvas.clear(engine_core::scene::ColorLinPremul::rgba(0.0, 0.0, 0.0, 0.0));
canvas.fill_rect(10.0, 10.0, 100.0, 40.0,
                 engine_core::scene::Brush::Solid(engine_core::scene::ColorLinPremul::rgba(1.0, 0.0, 0.0, 1.0)), 0);
// … draw_text / stroke_path / etc …
canvas.end_frame();                         // internally renders and presents
```

## How It Maps to Existing Engine Code

- `Canvas::begin_frame` → `Painter::begin_frame(Viewport { width, height })`
  to create a `DisplayList` builder for this frame.
- `Canvas::clear(color)` sets a background that becomes either:
  - a pass‑manager clear color when rendering direct to surface; or
  - a background pass (solid or gradient) when going offscreen then compositing.
- `fill_rect`/`stroke_path` → append `Command`s into the `DisplayList` via
  `Painter` methods (`rect`, `fill_path`, `stroke_path`, etc.).
- `end_frame` does:
  1) `Painter::finish()` → `DisplayList`
  2) `engine_core::upload_display_list(..)` → `GpuScene`
  3) Call `PassManager::render_frame` or `render_frame_and_text` with the
     current size, clear/background, and direct/bypass options.
  4) Submit, then `surface_frame.present()`.

Text paths:
- High‑level: use `Painter::text(TextRun { … })` and call
  `PassManager::render_frame_and_text(.., &list, provider)`.
- Low‑level: for pre‑layouted glyphs, call `PassManager::draw_text_mask` for
  each glyph at `origin + glyph.offset` using the desired premultiplied color.

## Interop With rune-scene

`rune-scene` should be a “builder” that produces a `DisplayList` by applying a
layout algorithm over user widgets/data. The `Canvas` API gives `rune-scene` a
simple target:

- Build layout → emit draw calls through `Canvas` (which records via `Painter`).
- Use `TextProvider::line_metrics` to compute baselines/line heights.
- Split content into z‑layers using the `z` argument for proper stacking.
- On each frame: `let (_, mut canvas) = surface.begin_frame(w, h)?;` →
  draw scene → `surface.end_frame(frame, canvas)?;`.

For very simple scenes, `rune-scene` can be an adapter that only does:

- “Stack” layout (vertical column) by accumulating `y` with provider metrics.
- Optional `push_clip_rect`/`pop_clip` around scrollable regions.
- Optional transforms via `push_transform` for nested coordinate spaces.

## Example: Simple Scene With Layout

```rust
fn draw_scene(surface: &mut RuneSurface,
              frame: wgpu::SurfaceTexture,
              provider: &dyn engine_core::TextProvider,
              size: winit::dpi::PhysicalSize<u32>) -> anyhow::Result<()> {
    let (_, mut canvas) = surface.begin_frame(size.width, size.height)?;
    canvas.clear(engine_core::scene::ColorLinPremul::rgba(0.09, 0.12, 0.2, 1.0));

    let mut y = 24.0;
    for (i, line) in ["Title", "Subtitle", "Body text"].iter().enumerate() {
        let px = match i { 0 => 28.0, 1 => 18.0, _ => 14.0 };
        let col = engine_core::scene::ColorLinPremul::rgba(1.0, 1.0, 1.0, 1.0);
        canvas.draw_text_run([24.0, y], line.to_string(), px, col, 10 + i as i32);
        if let Some(m) = provider.line_metrics(px) {
            y += m.ascent + m.descent + m.line_gap;
        } else {
            y += px * 1.2;
        }
    }

    surface.end_frame(frame, canvas)
}
```

## Behavior and Defaults

- Default `direct = false`: render to offscreen then composite to surface for
  smooth resize behavior; set `direct = true` to bypass the compositor.
- When `preserve_surface = true`, the first pass uses `LoadOp::Load` to keep
  existing pixels (useful for incremental overlays), otherwise it clears.
- Colors are premultiplied linear. Use `ColorLinPremul::rgba/rgba16f` helpers.

## Error Handling

- `begin_frame` can fail if surface acquisition fails; return the error so the
  application can reconfigure/retry.
- If the surface size is zero (minimized), skip rendering gracefully.

## Mapping to Current Files

- Painter and commands: `crates/engine-core/src/painter.rs:1`
- Pass manager and frame rendering: `crates/engine-core/src/pass_manager.rs:2010`
- Text providers and glyph masks: `crates/engine-core/src/text.rs:186`
- Demo usage of `Painter::begin_frame`: `crates/demo-app/src/scenes/text_demo.rs:48`

## Status

- Extracted into its own crate: `crates/rune-surface`.
- Public types: `rune_surface::RuneSurface` and `rune_surface::Canvas`.
- Depends on `engine-core` and uses its wgpu re-export for type identity.
- API matches the spec above; supports both high-level `TextRun` (via `TextProvider`) and low-level pre‑rasterized glyph masks.
