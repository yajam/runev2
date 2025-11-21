# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rune Draw is an **AI-native runtime** built on a GPU-accelerated Intermediate Representation (IR) rendering engine. Built in Rust using wgpu, it provides two execution modes for AI-generated and traditional UIs:

### Dual-Mode Architecture

**1. Direct IR Mode (Native)**
- AI agents generate IR commands directly
- WebAssembly modules mutate the IR scene graph
- Zero-overhead rendering pipeline
- Native performance for AI-composed interfaces

**2. Web Standards Mode (Compatibility)**
- HTML, CSS, and JavaScript mapped to IR primitives
- DOM → IR translation layer
- CSS layout engine → IR geometry
- JavaScript runtime with WASM-based mutations
- Allows existing web content to run on the IR runtime

Both modes share the same GPU rendering backend: a display list architecture for rendering shapes, text, images, and SVG content with support for z-layering, hit-testing, and interactive scenes.

## Build & Run Commands

**Build workspace:**
```bash
cargo build --workspace
```

**Run demo app (primary test harness):**
```bash
cargo run -p demo-app
cargo run -p demo-app -- --scene=zones
cargo run -p demo-app -- --scene=images
cargo run -p demo-app -- --scene=overlay
cargo run -p demo-app -- --scene=shadow
cargo run -p demo-app -- --scene=linear
cargo run -p demo-app -- --scene=radial
```

Note: Some scenes (text_demo, cosmic_direct, harfrust_text) are currently disabled as they used legacy rendering methods.

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

- **`rune-scene`**: IR-based runtime environment with:
  - Direct IR rendering mode (AI-native)
  - WASM-based scene mutations
  - Web standards compatibility layer (HTML/CSS/JS → IR mapping)
  - Viewport, toolbar, sidebar, and devtools panels
  - Element inspector and IR scene graph visualization

### Utility Crates
- **`rune-surface`**: Canvas-style API on top of engine-core
- **`rune-text`**: Text layout and text editing utilities
- **`rune-window`**: Window management helpers

## Core Rendering Concepts

### IR (Intermediate Representation) Architecture

The runtime operates on an Intermediate Representation that can be:
- **Generated directly** by AI agents or Rust code via the `Painter` API
- **Translated** from web standards (HTML/CSS/JS) to IR primitives
- **Mutated** by WebAssembly modules for dynamic behavior

### Display List & Painter Pattern (IR Generation)

1. **Build a display list** using `Painter`:
   ```rust
   let mut p = Painter::begin_frame(Viewport { width, height });
   p.rect(rect, Brush::Solid(color), z_index);
   p.text(text_run, z_index);
   let dl = p.finish();
   ```

2. **Upload to GPU** (unified path):
   ```rust
   let unified_scene = upload_display_list_unified(allocator, &queue, &dl)?;
   ```

3. **Render via PassManager** (unified rendering):
   ```rust
   passes.render_unified(&mut encoder, allocator, &surface_view,
                        width, height, &unified_scene.gpu_scene,
                        &glyph_draws, &svg_draws, &image_draws,
                        clear_color, /*direct*/ false, &queue, preserve_surface);
   ```

### WASM Integration

WebAssembly modules can mutate the IR scene graph in both modes:
- **Direct mode**: WASM receives IR scene graph, returns modified IR
- **Web standards mode**: WASM operates on virtual DOM, translated to IR mutations
- Safe, sandboxed execution with controlled access to scene state

### Z-Ordering & Depth Buffer

The engine uses a depth buffer for proper z-layering across all element types:
- Z-index values map to depth values (higher z-index = closer to camera = lower depth)
- Depth testing ensures correct rendering order
- Background always renders at depth 1.0 (furthest back)
- All pipelines have depth_stencil configurations

**Current status**: Depth buffer infrastructure is complete. Unified rendering is fully implemented and is the only rendering path - all legacy rendering methods have been removed.

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
- `DEBUG_RADIAL=1`: Enable debug visualization for radial backgrounds

## Important Conventions

### Coordinate System
- Logical scene coordinates map 1:1 to device pixels
- Y axis points down
- Hit-testing uses device-space pixel positions directly

### Rendering Architecture
- **Unified IR rendering**: All element types (solids, text, images, SVGs) are rendered in a single depth-sorted pass via `PassManager::render_unified`
- **Direct vs Intermediate**: Can render directly to surface or via intermediate texture for smooth resizing (controlled by `direct` parameter)
- All rendering uses depth buffer for proper z-ordering across all element types
- Mode-agnostic: Same GPU backend for both direct IR and web standards modes

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
- `engine-core` public APIs exported in `src/lib.rs` (breaking changes require workspace-wide updates and may impact both IR modes)
- Shader layouts and binding interfaces in `engine-shaders` and `pass_manager` (must match upload shapes and pipeline definitions)
- IR primitive definitions (changes affect both direct mode and web standards translation layer)
- WASM interface contracts (breaking changes impact all WASM modules)

### Known Issues
- Text hit-testing not implemented (placeholder only)
- Per-side border widths not implemented
- Some demo scenes disabled (text_demo, cosmic_direct, harfrust_text) - these used legacy rendering methods and need conversion to unified path

## Testing Patterns

When making changes to rendering code:
1. Test with multiple scenes: `zones`, `images`, `overlay`, `shadow`, `linear`, `radial`
2. Toggle `USE_INTERMEDIATE` to test intermediate texture path
3. Test text rendering with different providers (`DEMO_FREETYPE=1`)
4. Verify hit-testing with interactive scenes (`zones`, `overlay`)
5. Test rune-scene app: `cargo run -p rune-scene`

## Key Documentation Files

- `docs/usage.md`: Comprehensive feature documentation and API examples
- `docs/z-layering-depth-buffer-implementation.md`: Depth buffer implementation status
- `docs/pass-manager-unified-refactoring-checklist.md`: Unified rendering refactoring plan
- `docs/how-to-run.md`: Quick reference for running different scenes
- `docs/rune-scene.md`: IR runtime architecture and dual-mode design
- `docs/FILE_INPUT_IMPLEMENTATION.md`: File input IR element implementation
- `docs/TABLE_IMPLEMENTATION.md`: Table IR element implementation

## AI-Native Runtime Goals

### Direct IR Mode Priorities
- Minimize latency between AI generation and visual output
- Provide rich IR primitives that match AI mental models
- Enable efficient WASM-based interactivity and animations
- Support visual debugging and IR inspection tools

### Web Standards Mode Priorities
- High-fidelity HTML/CSS layout translation to IR
- JavaScript runtime integration with WASM compilation
- DOM API compatibility for existing web libraries
- Progressive enhancement: web → IR optimization over time

### Shared Infrastructure
- Single GPU rendering backend for both modes
- Unified hit-testing and event routing
- Common text rendering and font management
- Shared resource management (images, gradients, patterns)
