Strokes: support uniform stroke width for rect and rounded rect. Per-side widths are marked optional in the plan and not implemented.
BoxShadow in DisplayList is added but currently ignored by upload; the pass is invoked explicitly via PassManager::draw_box_shadow.
The blur pass uses a fixed maximum radius via sigma→radius mapping inside the shader. It’s a straightforward separable Gaussian, sufficient for initial correctness.
Next steps (optional)

Integrate BoxShadow commands into the geometry path by sequencing shadow passes with z-order.
Add per-side border widths and stroke alignment modes.
Add caching/reuse for R8 mask/ping textures to avoid per-frame allocations.
Expand demo to overlay the source rect on top of the shadow in the FullscreenBackground scene.

Interleave text with solids by z (requires splitting solids into batches).
Add a proper shaping path with cosmic-text.

What’s left for full hybrid SVG import

usvg → geometry importer

Convert usvg::Path data into our Path (PathCmd), then let upload.rs tessellate with lyon.
Map solid fills; fall back to resvg for gradients, patterns, complex clips/masks/filters.
Optional: Rect/RoundedRect/Circle/Ellipse import directly to Painter primitives.
Threshold policy (as discussed)

<1k paths → import geometry (usvg→Path/Lyon).
1k–10k paths → resvg rasterize once per scale, cache.
10k paths or filters/masks → resvg raster and cache.

If you want, I can implement the usvg→geometry importer next (solid fill support first, fallback otherwise) and add the threshold-based selection inside a new svg demo scene, or extend the images scene to mix geometry and raster based on the policy.

Implement subpixel rasterization in the fork:
In fontdue’s rasterizer, sample coverage at three X offsets per pixel (±1/3, center) using the existing edge coverage math; pack to RGB channels.
Add Rgba16 path by emitting u16 coverage values (0..65535) for higher precision.
Expose the new rasterize_rgb8_indexed and rasterize_rgb16_indexed APIs.
Wire your fork into this repo via fontdue-rgb-patch and verify parity with the current CPU conversion.
