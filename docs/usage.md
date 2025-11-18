# Rune Draw Engine — Supported Features and Usage

This document describes what the current engine supports and how to use it in your own code or via the demo app.

## Overview

- Crates

  - `engine-core`: GPU device/surface helpers, scene and display list types, upload path, passes, compositor, hit testing.
  - `engine-shaders`: WGSL shader modules used by the passes (filled by the build).
  - `demo-app`: A small winit + wgpu application demonstrating backgrounds, shapes, shadows, and input events.

- Coordinate system
  - Logical scene coordinates map 1:1 to device pixels; Y axis points down.
  - A viewport uniform converts from scene to NDC during rendering.
  - Hit-testing takes device-space pixel positions (e.g., `winit` cursor coordinates) directly.

## Scene & Display List

- Core types: `Rect`, `RoundedRect`, `RoundedRadii`, `Brush`, `Stroke`, `TextRun`, `Transform2D`.
- Display list commands (see `crates/engine-core/src/display_list.rs:10`):

  - `DrawRect`, `DrawRoundedRect`, `DrawEllipse`, `DrawText`
  - `StrokeRect`, `StrokeRoundedRect`
  - `FillPath` (solid), `StrokePath` (width only; round cap/join)
  - `BoxShadow` for rounded rects (rendered by a dedicated pass)
  - `PushClip`/`PopClip` with `ClipRect`
  - `PushTransform`/`PopTransform` with `Transform2D`

- Brushes (see `crates/engine-core/src/scene.rs:44`):

  - `Brush::Solid(ColorLinPremul)`
  - `Brush::LinearGradient { start, end, stops }` (structure defined; currently used in rect/ellipse upload helpers)
  - `Brush::RadialGradient { center, radius, stops }` (ellipse path)

- Building a display list via `Painter` (see `crates/engine-core/src/painter.rs:1`):
  - Start: `let mut p = Painter::begin_frame(Viewport { width, height });`
  - Draw shapes: `p.rect(..)`, `p.rounded_rect(..)`, `p.stroke_rect(..)`, `p.ellipse(..)`, etc.
  - Optional state: `p.push_clip_rect(..)`, `p.push_transform(..)` and corresponding pops.
  - Finish: `let dl = p.finish();`

## Upload and Rendering

- Upload CPU geometry to GPU buffers:

  - `let gpu_scene = engine_core::upload_display_list(allocator, &queue, &dl)?;`
  - Returns `GpuScene` with vertex/index buffers used by renderers.
  - Lyon tessellation tolerance can be tuned via `LYON_TOLERANCE` (default `0.1`).

- Pass manager (`crates/engine-core/src/pass_manager.rs:1`):
  - `PassManager::new(device, target_format)` sets up pipelines for solids, compositor, blur/shadow, backgrounds.
  - Rendering options:
    - `render_frame_with_intermediate` renders to an intermediate texture and blits to the surface for smooth resizing.
    - `render_frame` renders either via offscreen+composite or directly to the surface when `direct = true`.
  - Background helpers:
    - `paint_root_color` or `paint_root_*gradient*` draw the window background.
  - Shape helpers for demonstration:
    - `draw_box_shadow(…, rrect, spec, …)` draws a CSS‑style shadow under a rounded rect.
    - `draw_filled_rounded_rect(…, rrect, color, …)` draws a filled rounded rect directly.

## Box Shadows

- Use `Command::BoxShadow` in a display list (via `Painter::box_shadow`) or call the pass manager helper.
- Pipeline steps: mask render → separable Gaussian blur → tint with color → composite under content.
- See `PassManager::draw_box_shadow` in `crates/engine-core/src/pass_manager.rs:240` for details.

## Hit Testing (Phase 6.5)

- Build a hit-test index from a `DisplayList` and query with device‑space coordinates:
  - `let index = engine_core::HitIndex::build(&dl);`
  - `let hit = index.topmost_at([x, y]); // Option<HitResult>`
- `HitResult` fields (see `crates/engine-core/src/hit_test.rs:5`):
  - `id`: monotonically increasing draw order id inside the list.
  - `z`: z-layer value; higher is visually on top.
  - `kind`: `HitKind` enum (Rect, RoundedRect, Ellipse, Text, StrokeRect, StrokeRoundedRect, BoxShadow).
  - `shape`: `HitShape` with geometry snapshot to support scene reactions.
  - `transform`: the `Transform2D` in effect for that command.
  - `region_id`: `Some(id)` when the hit corresponds to a `HitRegion*` item; otherwise `None`.
  - `local_pos`: local-space point relative to the shape/zones (after inverse transform).
  - `local_uv`: normalized [0..1] coordinates in shape’s bounding box when applicable.
- Supported hit tests:
  - Rect, RoundedRect (with corner radii), Ellipse, StrokeRect, StrokeRoundedRect.
  - Path (coarse bbox only).
  - Text currently returns no hit.
  - Nested clip rects are respected. Transforms are inverted to local space for geometric checks.
- Rebuild the `HitIndex` whenever your display list changes (including on resize).

### Hit Regions (Zones) and Scene Surface Hits

- Use hit-only commands to define interactive zones that don’t render:
  - `HitRegionRect { id, rect, z, transform }`
  - `HitRegionRoundedRect { id, rrect, z, transform }`
  - `HitRegionEllipse { id, center, radii, z, transform }`
- Painter helpers (non-rendering):
  - `p.hit_region_rect(id, Rect { .. }, z);`
  - `p.hit_region_rounded_rect(id, rrect, z);`
  - `p.hit_region_ellipse(id, center, radii, z);`
- A root viewport hit region is automatically added by `HitIndex::build` with `region_id = u32::MAX` and very low `z`, so clicks on empty scene surface are reported and can close overlays.
- When a hit region is matched, `HitResult.region_id` is `Some(id)`, and `local_pos`/`local_uv` are reported relative to that zone.

Example: scene-surface zone and relative coordinates

```rust
// In your display list build:
let mut p = Painter::begin_frame(Viewport { width, height });
// Explicit zone (optional — a root viewport zone is added automatically):
p.hit_region_rect(42, Rect { x: 0.0, y: 0.0, w: width as f32, h: height as f32 }, -10);

// Later in the event hook:
fn on_click(&mut self, _pos: [f32;2], hit: Option<&HitResult>) -> Option<DisplayList> {
    if let Some(h) = hit {
        if h.region_id == Some(42) || h.region_id == Some(u32::MAX) {
            // Close overlay using normalized zone UVs (0..1, y-down)
            if let Some(uv) = h.local_uv {
                // eprintln!("clicked in zone at uv={:?}", uv);
            }
            // ... mutate scene state and rebuild DisplayList
        }
    }
    None
}
```

Notes:

- `local_uv` origin is the zone’s top-left, with Y increasing downwards.
- For ellipses, `local_uv` is normalized to the ellipse’s bounding box.

## Scene Event Hooks (Phase 6.5)

- The `Scene` trait (demo app) exposes pointer hooks. Implement any subset to react to input and optionally return an updated `DisplayList`:
  - `on_pointer_move(pos, hit)`
  - `on_pointer_down(pos, hit)` / `on_pointer_up(pos, hit)`
  - `on_click(pos, hit)`
  - `on_drag(pos, hit)`
- The demo app wires winit events to these hooks, rebuilds the `HitIndex`, requests a redraw and, for geometry scenes, re-uploads the display list.
- Example: `CenteredRectScene` updates a `hovered` field on pointer move and draws a cyan overlay stroke around the hovered shape (see `crates/demo-app/src/scenes/centered_rect.rs:140`).

## Demo App

- Run: `cargo run -p demo-app` (supports flags or env vars below)
- Scenes (`--scene=...` or `DEMO_SCENE=...`): `default`, `circle`, `radial`, `linear`, `centered`, `shadow`.
- Scenes (`--scene=...` or `DEMO_SCENE=...`): `default`, `circle`, `radial`, `linear`, `centered`, `shadow`, `overlay`, `zones`, `text`, `images`.
  - `overlay` demonstrates a modal overlay and uses the root hit region (`region_id = u32::MAX`) to close when clicking the background.
  - `zones` demonstrates two-column layout (sidebar + main) with red and blue zone rectangles. Clicking a zone reports coordinates in that zone’s local coordinate system and places a marker at the clicked point.
  - `text` shows grayscale vs subpixel text side-by-side. Click the RGB/BGR buttons (top-right) to switch subpixel orientation. Requires `DEMO_FONT` to be set to a `.ttf` path.
  - `images` displays all images from the `images/` folder in a grid. Supports PNG, JPEG, SVG (rasterized via resvg), GIF (animated), and WebP (static and some animated files). Drop files into `images/` and run with `--scene=images`.
  - `svg` imports `.svg` files from the same `images/` folder as vector geometry using lyon tessellation (solid fills). Run with `--scene=svg`.
- Environment toggles:
  - `USE_INTERMEDIATE=1` (default) enables intermediate texture for smooth resizing.
  - `BYPASS_COMPOSITOR=1` forces direct rendering path for solids.
  - `DEBUG_RADIAL=1` enables internal debug visualization for some backgrounds.
  - `DEMO_FONT=/path/to/font.ttf` sets the font used by the demo text providers. With the default cosmic-text shaper, the demo uses system fonts when this is unset; when set, both the cosmic providers and the grayscale (fontdue) provider use the specified font.
- `DEMO_SUBPIXEL_OFFSET=0.33` controls a fractional X offset applied to subpixel text in the `text` scene to accentuate orientation differences. Default is `0.33`.
- `DEMO_SNAP_X=1` rounds the run X to integer pixels for stricter alignment (useful for small text or fonts with fractional advances).
- `DEMO_LINE_PAD=<px>` extra baseline spacing between the first and second sample lines in the text demo. Defaults to `size * 0.25`.
- Interactions
  - Hover: window title shows `kind`, `id`, and `z`. Centered scene highlights hovered elements.
  - Click/Drag: events are logged to stdout. Scenes can opt-in to react visually by returning updated `DisplayList`s.

## Limitations and Notes

- Text hit testing is not implemented; `HitShape::Text` is a placeholder.
- Per-side border widths are not yet implemented.
- Gradient support in upload helpers is basic and used selectively; shader suite focuses on correctness in linear space.
- The hit-test “spatial index” is a flat list; replaceable with BVH/quadtree if needed for large scenes.
- Hover overlays in the demo currently draw in scene space assuming no additional transforms; for transformed content, apply the same transform or use the recorded `transform` from `HitResult` when rebuilding the display list.
- Animated SVG is not supported (SMIL/CSS/SVG animations are ignored). For simple animation in the demo, use animated GIFs in the `images/` scene.

## Quick Start Snippets

- Build and upload:

  - `let mut p = Painter::begin_frame(Viewport { width, height });`
  - `p.rounded_rect(rrect, Brush::Solid(color), z);`
  - `let dl = p.finish();`
  - `let gpu = upload_display_list(allocator, &queue, &dl)?;`

- Render with passes:

  - `let mut passes = PassManager::new(device, surface_format);`
  - `passes.render_frame_with_intermediate(&mut encoder, allocator, &surface_view, width, height, &gpu, clear, /*direct*/ false, &queue);`

- Hit-test on cursor move (demo-style):
  - `let index = engine_core::HitIndex::build(&dl);`
  - `if let Some(hit) = index.topmost_at([x, y]) { /* react */ }`

If you need deeper integration help or want new primitives/passes added, open a task with your requirements and constraints.

## Text Rendering (Phase 7)

- GPU text pass tints an RGB subpixel coverage mask with a premultiplied linear color and blends it over your target.
- Utilities convert an 8-bit grayscale glyph mask to an RGB subpixel mask with `RGB` or `BGR` orientation.
- Baseline alignment: PassManager snaps each text run’s baseline using line metrics ascent so descenders render correctly and consistently.
- Small-size pseudo-hinting: positions snap to whole pixels when `size <= 15.0` to reduce shimmer.

Two ways to draw text

- Manual mask route

  - Build subpixel mask: `engine_core::grayscale_to_subpixel_rgb(width, height, &gray, SubpixelOrientation::RGB)` → `SubpixelMask`.
  - Draw mask: `PassManager::draw_text_mask(encoder, target_view, width, height, origin_xy, &mask, color, &queue)`.

- Provider route (recommended)
  - Implement `engine_core::TextProvider` or use the built-ins:
    - `SimpleFontdueProvider::from_bytes(bytes, SubpixelOrientation::RGB|BGR)`
    - `GrayscaleFontdueProvider::from_bytes(bytes)`
    - `CosmicTextProvider::from_bytes(bytes, SubpixelOrientation::RGB|BGR)` (feature `cosmic_text_shaper`)
  - Render: `passes.render_text_for_list(encoder, target_view, &display_list, &queue, provider)`.
  - Or render solids + text together:
    - `render_frame_and_text` (direct or offscreen)
    - `render_frame_with_intermediate_and_text` (offscreen then blit; smooth resizing)

Example: provider-based rendering

```rust
use engine_core::{ColorLinPremul, DisplayList, Painter, PassManager, SubpixelOrientation, TextProvider};

// Build a display list with text
let mut p = Painter::begin_frame(Viewport { width, height });
p.text(engine_core::TextRun { text: "Hello baseline!".into(), pos: [40.0, 160.0], size: 24.0, color: ColorLinPremul::from_srgba_u8([255,255,255,255]) }, 2);
let dl = p.finish();

// Create a provider from a TTF byte slice
let provider = engine_core::SimpleFontdueProvider::from_bytes(ttf_bytes, SubpixelOrientation::RGB)?;

// Draw solids first, then text (or use the *_with_intermediate_* helper)
passes.render_frame_and_text(
    &mut encoder,
    allocator,
    &surface_view,
    width,
    height,
    &gpu_scene,
    wgpu::Color::BLACK,
    /*direct*/ false,
    &queue,
    /*preserve_surface*/ false,
    &dl,
    &provider,
);
```

Notes

- The shader expects premultiplied linear text color. Use `ColorLinPremul::from_srgba_u8` to create it from sRGB bytes.
- Use `SubpixelOrientation::BGR` for displays with BGR subpixel layout.
- If you only have a grayscale mask, convert it with `grayscale_to_rgb_equal`.
- 16-bit masks are supported via `grayscale_to_subpixel_rgb16`/`grayscale_to_rgb_equal16`; `draw_text_mask` uploads as `Rgba16Unorm` when provided.
- For zero-copy subpixel masks, enable the `fontdue-rgb-patch` feature and use `PatchedFontdueProvider`.
- `TextRun::pos` is baseline-anchored. If you want to position using top-left, add the provider’s ascent: `pos_y = top_y + provider.line_metrics(px).map(|m| m.ascent).unwrap_or(px*0.8)`.

Enabling the cosmic-text shaper

- Add the feature to `engine-core` and include the dep:
  - `[dependencies] engine-core = { path = "...", features = ["cosmic_text_shaper"] }`
- Construct a provider from bytes or system fonts:
  - `let provider = engine_core::CosmicTextProvider::from_bytes(ttf_bytes, SubpixelOrientation::RGB)?;`
  - or `let provider = engine_core::CosmicTextProvider::from_system_fonts(SubpixelOrientation::RGB);`
- The provider uses cosmic-text for shaping (ligatures, bidi, fallback) and swash for rasterization,
  then converts grayscale coverage to RGB subpixel masks for the GPU text pass.

Enabling the FreeType FFI rasterizer

- Enable the feature on `engine-core` (and forward it from the demo if desired):
  - `[dependencies] engine-core = { path = "...", features = ["cosmic_text_shaper", "freetype_ffi"] }`
  - Demo: add `freetype_ffi` feature or run with `--features demo-app/freetype_ffi`
- Construct a provider from bytes using FreeType for hinted LCD masks, shaped by cosmic-text:
  - `let provider = engine_core::FreeTypeProvider::from_bytes(ttf_bytes, SubpixelOrientation::RGB)?;`
- Demo toggle: set `DEMO_FREETYPE=1` to use the FreeType-backed provider for the subpixel column when `DEMO_FONT` is provided.
- Notes:
  - Uses FreeType’s LCD filter (Normal) and hinting for small font clarity.
  - Orientation `RGB/BGR` is respected by channel order mapping.
  - Currently supports `from_bytes`; system font fallback remains via the cosmic-text path.

Enabling the patched fontdue provider

- In your Cargo, enable the feature on `engine-core`:
  - `[dependencies] engine-core = { path = "...", features = ["fontdue-rgb-patch"] }`
- Provide a `fontdue_rgb` crate (fork of `fontdue`) on your workspace or via git that exposes:
  - `Font::rasterize_rgb8_indexed(glyph_index, px) -> (u32, u32, Vec<u8>)` yielding RGBA8 RGB coverage
  - `Font::rasterize_rgb16_indexed(glyph_index, px) -> Option<(u32, u32, Vec<u8>)>` yielding RGBA16 (if supported)
- Env toggles in the demo: `DEMO_FONT=/path/to/font.ttf`, `DEMO_SUBPIXEL_OFFSET=0.33`, `DEMO_SNAP_X=1`.
- Additional demo env: `DEMO_TEXT_SIZE=48` to set the initial size for the text demo.
