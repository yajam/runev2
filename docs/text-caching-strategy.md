# Text Caching and Layout Strategy

## Goals

- [ ] Reliable rendering for long and multi-paragraph text (no hangs, no dropouts).
- [ ] Static text is fast by default; only focused/editing regions do dynamic work.
- [ ] One canonical layout/baseline source (rune-text/harfrust); `PassManager` only renders.
- [ ] Shared caching of shaping + glyph masks across the engine.

## High-Level Approach

1. Make text rendering static by default with cached glyph batches.
2. Add explicit IDs and “dynamic” flags so only active editors re-layout.
3. Move paragraph layout and baselines into `rune-text`, remove ad-hoc baseline math from `PassManager`.
4. Centralize caching (layout + glyphs) so scenes and UI don’t reimplement the same pattern.

---

## Phase 1: Static Text Caching in engine-core

Target: fast static text like `harfrust_text` everywhere, not just that scene.

- [x] Introduce a cache key type in `engine-core`, e.g.:
  - `TextCacheKey { text_hash, size_px, color, dpi, provider_kind, maybe width }`
- [x] Add a `GlyphBatch` struct:
  - `Vec<(SubpixelMask, [f32; 2], ColorLinPremul)>` plus metadata.
- [x] Create a `TextCache` (LRU or size-limited `HashMap<TextCacheKey, GlyphBatch>`).
- [x] Refactor `PassManager::render_text_for_list`:
  - [x] For each `DrawText` command, build a `TextCacheKey`.
  - [x] On cache hit: skip `provider.rasterize_run`, feed the cached `GlyphBatch` to `draw_text_mask`.
  - [x] On miss: call `provider.rasterize_run`, build `GlyphBatch`, insert into cache, then draw.
- [x] Keep `draw_text_mask` as the single GPU entry point:
  - [x] It only handles “batch of glyphs → atlas + draw”.
  - [x] No shaping, wrapping, or baseline math.

## Phase 2: Text Node Identity and Static/Dynamic Flags

Target: distinguish static labels vs. editable text without scene-specific hacks.

- [x] Extend the display list text representation (`Command::DrawText` or a new `TextNode`) with:
  - [x] A stable `id: u64` (or `u32`) per logical text node.
  - [x] A `dynamic: bool` flag (static by default).
- [x] Incorporate `id` and `dynamic` into `TextCacheKey`:
  - [x] Static: cache can be keyed primarily by `id` + text hash.
  - [x] Dynamic: either skip caching or keep a short-lived cache that invalidates on edits.
- [x] Update scenes / `rune-scene` to assign deterministic IDs to text in layouts:
  - [x] From node tree indices, explicit IDs, or IR-level handles.

## Phase 3: Editable Text and Focused Regions

Target: only focused editors pay the cost of re-layout and re-shape.

- [ ] Design a small “text node state” for editable areas:
  - `{ id, text, selection, focused: bool, layout_state }`
- [ ] When a node becomes focused:
  - [ ] Mark it `dynamic = true`.
  - [ ] Invalidate any static cache entries for that `id`.
- [ ] On edits (insert/delete, IME, style changes):
  - [ ] Re-run `rune-text` layout for that node only.
  - [ ] Update its `GlyphBatch` in the cache.
- [ ] When focus leaves:
  - [ ] Optionally “freeze” the last glyph batch and flip `dynamic = false`
    so it is treated as static again.

## Phase 4: Paragraph Layout and Baselines via rune-text

Target: multi-line paragraphs use rune-text, not `PassManager` heuristics.

- [ ] Identify one paragraph path to convert first:
  - [ ] e.g. a demo scene’s long text or a `rune-scene` text element.
- [ ] Use `rune-text`’s layout API to get:
  - [ ] Per-line glyphs, x/y positions.
  - [ ] Per-line baselines from rune-text metrics.
- [ ] Produce a `GlyphBatch` directly from rune-text’s layout output:
  - [ ] No extra baseline math in `PassManager`.
- [ ] Gradually deprecate:
  - [ ] `render_text_for_list` baseline heuristics (using `line_metrics` + transforms).
  - [ ] Ad-hoc multi-line baseline logic in `crates/rune-scene/src/text.rs` that conflicts with rune-text.

## Phase 5: Integrations and Cleanups

Target: consistent behavior and easy debugging.

- [ ] Wire `TextCache` into the `RuneSurface` canvas path:
  - [ ] `Canvas::draw_text_run` → `PassManager` text path → cache.
- [ ] Add optional debug overlays:
  - [ ] Toggle to draw baselines and glyph boxes from cached batches.
  - [ ] Counters: cache hits/misses, glyphs per frame, time spent in shaping vs. drawing.
- [ ] Once confident:
  - [ ] Remove scene-specific glyph caching in demo scenes in favor of the central cache.
  - [ ] Simplify `PassManager` text code so it clearly only does GPU work and simple transforms.

## Near-Term TODO (Refactor-Oriented)

- [x] Extract the `harfrust_text` glyph-batch pattern into a reusable `GlyphBatch` type in `engine-core`.
- [x] Add a minimal in-memory `TextCache` and use it inside `render_text_for_list` for simple runs.
- [ ] Extend `DrawText` commands with a stable `id` and hook that into the cache key.
- [ ] Convert one multi-line paragraph demo to rune-text layout + cached glyph batch (no `PassManager` baseline math).
- [ ] Measure long-text + resize behavior; iterate before expanding the pattern across all scenes.

## Debounced Resize Strategy

Target: keep resize smooth while avoiding repeated heavy text layout during continuous drags.

- [ ] Track resize state in the window loop:
  - [ ] `last_resize_at: Option<Instant>`
  - [ ] `needs_text_layout: bool`
- [ ] On `WindowEvent::Resized(new_size)`:
  - [ ] Reconfigure the surface / swapchain to `new_size`.
  - [ ] Update logical width/height used for text layout.
  - [ ] Set `needs_text_layout = true`.
  - [ ] Set `last_resize_at = Some(Instant::now())`.
- [ ] In `RedrawRequested` or `AboutToWait`:
  - [ ] If `needs_text_layout` is true and
        `last_resize_at` is older than a debounce window (e.g. 100–150 ms),
        then:
    - [ ] Re-run `rune-text` layout for paragraphs using the new logical width.
    - [ ] Rebuild glyph batches or text cache entries that depend on width.
    - [ ] Clear `needs_text_layout`.
- [ ] While actively resizing (time since `last_resize_at` < debounce window):
  - [ ] Continue to draw using the previous frame’s glyph batches, accepting a brief mismatch between text wrap and window width.
  - [ ] Optionally lower redraw frequency (e.g. via a timer or throttled `request_redraw`) to reduce GPU/CPU churn.
- [ ] On `ScaleFactorChanged` (DPI change):
  - [ ] Rebuild glyph batches immediately for text that depends on pixel size.
  - [ ] Still reuse the debounce mechanism for width-only changes to avoid extra work.
