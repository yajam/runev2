# Rune Text Shaping and Layout — Analysis and Recommendations

This document summarizes the current text approach, observed issues, and a concrete plan to reach browser‑grade text quality and control. If cosmic_text does not meet the desired results, this plan provides a low‑risk path to adopt HarfBuzz while keeping our rendering and layout stable.

## Goals

- One source of truth for text metrics that matches what we actually draw.
- Robust shaping and font features (OpenType, variation), with fine‑grained control.
- Consistent wrapping, baselines, and line spacing across platforms and providers.
- Clean integration with our renderer (subpixel AA) and future layout engine (Taffy).

## Current State

- Providers: cosmic_text (default), fontdue (simple), optional FreeType.
- Rendering: subpixel or grayscale glyph masks are rasterized, then tinted in a text shader (premultiplied linear color). This gives good compositing and matches our pipeline.
- Measurement: we’ve added a provider‑agnostic helper that measures runs from rasterized glyph masks (top, bottom, width), computes baselines and line spacing, and performs simple word wrap. This aligns layout with the final rendered result.

## Observed Issues

- Baseline mismatches with logical_px in cosmic_text for some fonts and sizes; visually rendered glyphs did not align with provider baselines.
- Differences between providers for line metrics (ascent, descent, gap).
- For UI text, we need predictable pixel alignment and cross‑provider consistency more than sophisticated typesetting defaults.

## Requirements for “Browser‑Class” Text

- Shaping control: per‑run script/direction, OpenType feature toggles, kerning, variation axes, language tags.
- Font fallback: robust mapping of codepoints to fonts (Latin + CJK + RTL + complex scripts).
- Bidi + script segmentation: correctness for mixed scripts and directions.
- Line break rules: UAX#14 (word boundaries, hyphenation optional), or an equivalent high‑quality breaker.
- Layout integration: Flex/Grid (Taffy), measurable exact sizes, padding, line heights, and pixel snapping.

## Strategy Overview

1) Keep raster‑driven measurement as the single source of truth.
   - Measure width/height/top/bottom directly from glyph masks (what we draw), not abstract baselines.
   - Use optional device scale_factor to snap baselines for crisp lines.

2) Use cosmic_text where it performs well; add HarfBuzz as a feature‑gated provider to unlock full control when needed.
   - Providers remain behind the `TextProvider` trait.
   - Layout and rendering code stay the same; only provider changes.

3) Wrap and layout in our engine; feed exact measurements into Taffy when adopted.
   - Implement a `LayoutBackend` abstraction; default to simple Stack/Row initially.
   - Introduce Taffy behind a feature and wire our text measure callback so Taffy computes sizes using our raster metrics.

## Provider Plan

- Phase A (Now): Cosmic Text + raster‑measure helper
  - Keep cosmic_text as the default provider for simplicity.
  - Use the raster‑based helper for baselines and wrapping (already implemented).

- Phase B (Opt‑In): HarfBuzz + FreeType provider
  - Add a `harfbuzz_shaper` feature in engine-core.
  - Implement `HarfbuzzProvider` that:
    - Shapes with HarfBuzz (per‑run direction/script/language, features optional).
    - Renders glyph masks with FreeType (LCD or grayscale), honoring subpixel orientation (RGB/BGR).
    - Caches glyph masks by (glyph_id, px, orientation, hinting).
    - Provides line_metrics using FreeType’s horizontal metrics.
  - Add a runtime switch (env var or CLI) to select provider: cosmic|harfbuzz|fontdue.

- Phase C (Advanced): Bidi, script segmentation, and fallback
  - Use unicode-bidi to compute embedding levels and split into directional runs.
  - Use unicode-script to assign script per run; set HarfBuzz buffer props accordingly.
  - Add font fallback via fontdb (or a configured fallback list). If a face lacks a glyph, map that codepoint to the first fallback that supports it.

## Wrapping and Line Height

- Continue to use our helper for wrapping, baselines, and line heights:
  - It already matches the final rendered pixels and works with any provider.
  - For long‑form text or multilingual content, consider a proper UAX#14 breaker (e.g., unicode-linebreak) while still measuring from glyph masks.

## Taffy Integration (Layout)

- Add a `taffy_layout` feature and build a `TaffyLayout` adapter implementing `LayoutBackend`.
- Map our Style to Taffy (size, min/max, padding/margin/border, gap, display: flex/grid, align/justify).
- Provide a measure callback that uses our raster helper (provider + scale_factor) so Taffy gets accurate sizes.
- Collect frames from Taffy and render with Canvas; baselines for text come from our helper.

## Rendering Quality (LCD)

- Keep using subpixel masks and premultiplied linear color in the shader.
- Ensure FreeType render flags match orientation (RGB/BGR) and consistent hinting.
- Optionally add small gamma/contrast adjustment if LCD fringing is observed; keep behind a toggle.

## Risks and Mitigation

- Complexity: HarfBuzz integration requires bidi/script handling and fallback fonts.
  - Mitigate by staging: single font + LTR first, then add bidi and fallback.
- Performance: Shaping + FT rasterization can be expensive.
  - Cache glyph masks aggressively and avoid re‑shaping unchanged runs.
- Consistency: Mixing provider baselines with raster metrics can drift.
  - Always measure from glyph masks for layout; treat provider line metrics as hints only.

## Validation Plan

- Visual test matrix across providers (cosmic_text vs harfbuzz) and scripts:
  - Latin, Arabic (RTL), Devanagari (complex), CJK; mixed LTR/RTL runs.
  - Compare baselines, line spacing, and wrap points; ensure no visual clipping.
- Subpixel AA checks on different orientations (RGB/BGR) and displays (macOS/Windows/Linux).
- Pixel snapping: baseline positions remain stable at various DPI scale factors.

## Rollout Steps

1. Keep cosmic_text as default; enable our raster‑measure helper (done).
2. Add `harfbuzz_shaper` feature and a `HarfbuzzProvider` prototype; runtime switch.
3. Stage bidi/script segmentation and font fallback.
4. Add `taffy_layout` feature and Taffy adapter; use our measure callback.
5. Expand tests; switch default to harfbuzz once quality criteria are met.

## Summary

- We can reach high‑quality, controllable text by keeping measurement tied to rasterized glyphs and swapping the shaper behind our `TextProvider` trait.
- If cosmic_text falls short, HarfBuzz + FreeType is the recommended path, staged behind a feature and validated with a clear test matrix.
- Taffy is warranted for browser‑class layout; integrate with our measurement callback so layout and rendering stay in sync.

