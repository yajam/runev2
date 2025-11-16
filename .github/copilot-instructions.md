<!-- Copilot instructions for AI coding agents working on Rune Draw -->
# Rune Draw — Copilot Instructions

Purpose: help an AI become productive quickly in this Rust workspace (multi-crate GPU 2D engine + demo).

- **Quick commands**
  - Build whole workspace: `cargo build --workspace`
  - Run demo app: `cargo run -p demo-app` (or `cargo run -p demo-app -- --scene=zones`)
  - Run a single crate build: `cargo build -p engine-core`
  - Run tests: `cargo test` (or `cargo test -p <crate>`)

- **Where to start (big picture)**
  - This is a Rust workspace (see top-level `Cargo.toml`) composed of UI/demo and engine crates:
    - `crates/engine-core`: core types, display list, upload paths, pass manager, hit-testing and text helpers. Key files: `src/painter.rs`, `src/display_list.rs`, `src/pass_manager.rs`, `src/hit_test.rs`, `src/text.rs`.
    - `crates/engine-shaders`: WGSL shader modules referenced by `engine-core` pipelines.
    - `crates/demo-app`: winit + wgpu demo that exercises scenes and providers. Entry: `src/main.rs`; scenes live under `src/scenes`.
    - `crates/rune-scene`, `crates/rune-surface`, `crates/rune-text` and `crates/rune-window` are higher-level/experimental pieces — check `crates/rune-scene/README.md` for scene-specific notes.

- **Core concepts & patterns**
  - DisplayList / Painter: construct scene commands via `Painter::begin_frame(...)`, call shape/text helpers, then `p.finish()` to obtain a `DisplayList`. See `crates/engine-core/src/painter.rs` and `docs/usage.md` for exact helpers used.
  - Upload path: CPU display list → `upload_display_list(...)` → `GpuScene` (vertex/index buffers) used by `PassManager`. See `crates/engine-core/src/upload.rs` and `docs/usage.md` snippets.
  - PassManager: central rendering orchestration (offscreen intermediate, compositing, blur/shadow passes). See `crates/engine-core/src/pass_manager.rs` and `docs/usage.md` for `render_frame_with_intermediate` vs `render_frame` behavior.
  - Hit testing: build `HitIndex::build(&dl)` from a `DisplayList` and query with device-space coords: `index.topmost_at([x,y])`. See `crates/engine-core/src/hit_test.rs`.
  - Text providers: multiple provider paths exist (cosmic-text, fontdue, freetype FFI). Look at demo flags and provider selection in `crates/demo-app/src/main.rs` to understand runtime toggles.

- **Important runtime toggles & env vars**
  - `DEMO_SCENE`, or `--scene=...` flag for `demo-app` (many scenes: `zones`, `text`, `images`, `svg`, `overlay`, etc.). See `crates/demo-app/src/main.rs`.
  - `DEMO_FONT`, `DEMO_FREETYPE`, `USE_INTERMEDIATE`, `BYPASS_COMPOSITOR`, `DEMO_SUBPIXEL_OFFSET`, `DEMO_SNAP_X` — these change rendering and provider selection; prefer matching the demo code when testing changes.

- **Conventions & guidelines specific to this repo**
  - Keep work within the workspace layout and avoid changing crate paths in `Cargo.toml` unless intentionally refactoring the workspace.
  - Rendering code distinguishes logical scene coordinates vs device pixels; `PassManager` may be switched between direct and intermediate render paths — preserve logical-pixel handling when modifying pipelines.
  - Text rendering has multiple code paths; when changing text shaping/rasterization, update both provider selection logic (demo) and the expected premultiplied linear color usage described in `docs/usage.md`.
  - Hit-testing is built from the display list; do not rely on GPU buffers for hit tests: modify `crates/engine-core/src/hit_test.rs` and `Painter` helpers when changing click/zone behavior.

- **Concrete examples to reference**
  - Build a `DisplayList`: `crates/engine-core/src/painter.rs` and usage snippets in `docs/usage.md`.
  - Demo scene selection and provider toggles: `crates/demo-app/src/main.rs` (search for `DEMO_SCENE`, `DEMO_FONT`).
  - Render orchestration and shadow pipeline: `crates/engine-core/src/pass_manager.rs` (search for `draw_box_shadow`, `render_frame_with_intermediate`).

- **Debugging & running locally**
  - For iterative work on the demo, run: `cargo run -p demo-app -- --scene=text` and toggle env vars in the same shell, e.g. `DEMO_FONT=/path/to/font.ttf DEMO_FREETYPE=1 cargo run -p demo-app`.
  - Use `RUST_LOG=debug` if adding logging; many modules use `anyhow` and standard logging patterns.
  - When testing GPU changes, inspect the demo scenes that exercise the pipeline (`shadow`, `zones`, `text`, `images`).

- **What NOT to change without coordination**
  - `engine-core` public APIs that other workspace crates import (types re-exported in `crates/engine-core/src/lib.rs`) — breaking changes require coordinated workspace updates.
  - Shader layouts and binding interfaces in `engine-shaders` and `pass_manager` — they must match the upload shapes and pipeline definitions.

If anything above is unclear or you'd like me to add more examples (small code snippets, a checklist for common PRs, or test commands), tell me which area to expand.
