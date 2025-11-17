# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rune Draw is a GPU-native 2D rendering engine built in Rust using wgpu. It provides a display list architecture for rendering shapes, text, images, and SVG content with support for z-layering, hit-testing, and interactive scenes.

## Build & Run Commands

**Build workspace:**
```bash
cargo build --workspace
```

**Run demo app (primary test harness):**
```bash
cargo run -p demo-app
cargo run -p demo-app -- --scene=zones
cargo run -p demo-app -- --scene=text
cargo run -p demo-app -- --scene=images
cargo run -p demo-app -- --scene=overlay
```

**Run rune-scene app:**
```bash
cargo run -p rune-scene
```

**Run tests:**
```bash
cargo test                    # All tests
cargo test -p engine-core     # Single crate
```

**Build single crate:**
```bash
cargo build -p engine-core
```

**Debug logging:**
```bash
RUST_LOG=debug cargo run -p demo-app
```

## Workspace Architecture

This is a Rust workspace with the following crates:

### Core Engine Crates
- **`engine-core`**: Core rendering engine with display list, upload paths, pass manager, hit-testing, and text rendering
  - Key files: `src/painter.rs` (display list builder), `src/display_list.rs` (command types), `src/pass_manager.rs` (rendering orchestration), `src/hit_test.rs` (click detection), `src/upload.rs` (CPU → GPU geometry)

- **`engine-shaders`**: WGSL shader modules for rendering passes (solid, text, image, compositor, background, blur/shadow)

### Application Crates
- **`demo-app`**: Primary test harness using winit + wgpu. Entry point: `src/main.rs`. Scenes in `src/scenes/`
  - Supports multiple scenes via `--scene=<name>` flag or `DEMO_SCENE` env var

- **`rune-scene`**: Higher-level zone-based UI app with viewport, toolbar, sidebar, and devtools panels

### Utility Crates
- **`rune-surface`**: Canvas-style API on top of engine-core
- **`rune-text`**: Text layout and text editing utilities
- **`rune-window`**: Window management helpers

## Core Rendering Concepts

### Display List & Painter Pattern

1. **Build a display list** using `Painter`:
   ```rust
   let mut p = Painter::begin_frame(Viewport { width, height });
   p.rect(rect, Brush::Solid(color), z_index);
   p.text(text_run, z_index);
   let dl = p.finish();
   ```

2. **Upload to GPU**:
   ```rust
   let gpu_scene = upload_display_list(allocator, &queue, &dl)?;
   ```

3. **Render via PassManager**:
   ```rust
   passes.render_frame_with_intermediate(&mut encoder, allocator, &surface_view,
                                         width, height, &gpu_scene, clear_color,
                                         /*direct*/ false, &queue);
   ```

### Z-Ordering & Depth Buffer

The engine uses a depth buffer for proper z-layering across all element types:
- Z-index values map to depth values (higher z-index = closer to camera = lower depth)
- Depth testing ensures correct rendering order
- Background always renders at depth 1.0 (furthest back)
- All pipelines have depth_stencil configurations

**Current status**: Depth buffer infrastructure is complete. Unified rendering (single pass for all element types) is partially implemented but has a critical bug where text/images/SVGs are skipped. See `docs/pass-manager-unified-refactoring-checklist.md` for details.

### Hit Testing

Build a hit-test index from a display list and query with device-space coordinates:
```rust
let index = HitIndex::build(&dl);
if let Some(hit) = index.topmost_at([x, y]) {
    // hit.id, hit.z, hit.kind, hit.region_id, hit.local_pos, hit.local_uv
}
```

- Hit regions allow defining interactive zones that don't render
- Root viewport hit region (id = `u32::MAX`) automatically added for background clicks
- Rebuild index whenever display list changes

### Text Rendering

Multiple text provider paths exist:
- **`SimpleFontdueProvider`**: Basic fontdue-based rendering
- **`CosmicTextProvider`**: HarfBuzz shaping + Swash rasterization (feature: `cosmic_text_shaper`)
- **`FreeTypeProvider`**: FreeType LCD rendering (feature: `freetype_ffi`)

Text is rendered with subpixel antialiasing (RGB or BGR orientation). GPU shader tints RGB coverage masks with premultiplied linear color.

## Environment Variables & Runtime Toggles

### Demo Scene Selection
- `DEMO_SCENE=<name>` or `--scene=<name>`: Choose scene (zones, text, images, overlay, shadow, etc.)

### Text Rendering
- `DEMO_FONT=/path/to/font.ttf`: Custom font path
- `DEMO_FREETYPE=1`: Use FreeType provider for LCD rendering
- `DEMO_SUBPIXEL_OFFSET=0.33`: Fractional X offset for subpixel text
- `DEMO_SNAP_X=1`: Snap text X position to integer pixels
- `DEMO_TEXT_SIZE=48`: Initial text size for text demo
- `DEMO_LINE_PAD=<px>`: Extra baseline spacing between text lines

### Rendering
- `USE_INTERMEDIATE=1` (default): Enable intermediate texture for smooth resizing
- `BYPASS_COMPOSITOR=1`: Force direct rendering path for solids
- `DEBUG_RADIAL=1`: Enable debug visualization for radial backgrounds

## Important Conventions

### Coordinate System
- Logical scene coordinates map 1:1 to device pixels
- Y axis points down
- Hit-testing uses device-space pixel positions directly

### Rendering Paths
- **Direct rendering**: Renders directly to surface (bypass intermediate texture)
- **Intermediate rendering**: Renders to offscreen texture then blits (smooth resizing)
- PassManager can be switched between paths via `direct` parameter

### Text Color Handling
- Text shaders expect **premultiplied linear color**
- Use `ColorLinPremul::from_srgba_u8([r, g, b, a])` to convert from sRGB bytes
- Subpixel orientation: `RGB` or `BGR` based on display layout

### Feature Flags
- `cosmic_text_shaper`: Enable cosmic-text integration (default)
- `freetype_ffi`: Enable FreeType FFI for LCD rendering
- `fontdue-rgb-patch`: Enable patched fontdue with direct RGB mask output

## Critical Constraints

### Do NOT Change Without Coordination
- `engine-core` public APIs exported in `src/lib.rs` (breaking changes require workspace-wide updates)
- Shader layouts and binding interfaces in `engine-shaders` and `pass_manager` (must match upload shapes and pipeline definitions)

### Known Issues
- **Unified rendering bug**: `viewport_ir` renders blank screen with `use_unified_rendering = true`. Root cause: `render_unified()` exits early after rendering solids, skipping text/images/SVGs. See `docs/pass-manager-unified-refactoring-checklist.md` for fix plan.
- Text hit-testing not implemented (placeholder only)
- Per-side border widths not implemented

## Testing Patterns

When making changes to rendering code:
1. Test with multiple scenes: `zones`, `text`, `images`, `overlay`, `shadow`
2. Toggle `USE_INTERMEDIATE` and `BYPASS_COMPOSITOR` to test different code paths
3. Test text rendering with different providers (`DEMO_FREETYPE=1`)
4. Verify hit-testing with interactive scenes (`zones`, `overlay`)

## Key Documentation Files

- `docs/usage.md`: Comprehensive feature documentation and API examples
- `docs/z-layering-depth-buffer-implementation.md`: Depth buffer implementation status
- `docs/pass-manager-unified-refactoring-checklist.md`: Unified rendering refactoring plan
- `docs/how-to-run.md`: Quick reference for running different scenes
