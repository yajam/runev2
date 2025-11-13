# `rune` — Rendering Architecture (wgpu + taffy + cosmic-text)

A unified 2D rendering + layout + text system built **without Vello**.

Rune-Draw replaces Vello and directly uses **wgpu** to draw shapes, images, and text using:

- **taffy** (layout)
- **cosmic-text** (font shaping)
- **wgpu** (GPU drawing)
- **Rune scene graph** (your own lightweight node model)

This document defines _exactly how the engine works_ so that the renderer is smooth, fast, and stable — even under continuous resize.

---

# 1. Core Goals

### 🎯 **Fast load time**

- defer heavy work
- load fonts once
- avoid unnecessary GPU/CPU resizes
- stable coordinate system

### 🎯 **Smooth resize**

- containers resize every pixel (no jitter)
- text only reshapes when necessary
- GPU keeps working at full speed

### 🎯 **Predictable layout**

- taffy is the single source of truth
- transforms preserve subpixel precision
- hit testing guarantees consistency

### 🎯 **Browser-grade correctness**

- freeze text during resize drag
- incremental shaping
- selective scene updates
- no redundant rasterization

---

# 2. Architecture Overview

```
         +----------------+
         |  Input Events  |
         +--------+-------+
                  |
                  v
        +---------+---------+
        |  Rune Layout Core |
        |    (Taffy)        |
        +---------+---------+
                  |
                  v
       +----------+----------+
       |   Rune Text Core    |
       | (cosmic-text shaping)|
       +----------+----------+
                  |
                  v
         +--------+--------+
         |   Scene Tree    |
         |  (Draw Commands)|
         +--------+--------+
                  |
                  v
         +--------+--------+
         |    GPU Backend   |
         |     (wgpu)       |
         +------------------+
```

---

# 3. Coordinate System (critical to avoid jitter)

Rune-Draw uses **one unified coordinate space** across layout, text, and rendering:

### ✔ `f32` floats everywhere

### ✔ No rounding until raster

### ✔ Scene tree uses the exact numbers from taffy

### ✔ Text uses fractional positions from layout

This eliminates:

- logical_px vs device_px mismatch
- hit testing drift
- jitter during resize

---

# 4. Layout System (Taffy)

### 4.1 Layout Pipeline

```
visual_width  → GPU drawing only (smooth)
layout_width  → Taffy + cosmic-text (debounced or bucketized)
```

### 4.2 Key Rules

- Taffy only recomputes on **layout_width** changes
- visual resize uses transform scaling (GPU)
- layout tree produces absolute rects
- rect values are stored as `f32` floats and passed to renderer unchanged

### 4.3 Layout Phases

1. Build/Update Taffy tree
2. Apply layout width
3. Let Taffy compute node rects
4. Emit absolute positions for scene nodes
5. Pass rects to Rune-Draw unmodified

---

# 5. Text System (cosmic-text)

### 5.1 Buffer lifecycle

Each text node has:

```
struct TextNode {
    buffer: cosmic_text::Buffer,
    style: TextStyle,
    last_layout_width: f32,
}
```

### 5.2 Shaping rules

- Only shape when `layout_width` changes enough (threshold ≈ 12px)
- Freeze text during active resize
- Apply shaping after final layout
- Extract glyph positions at fractional coords
- Font fallback handled inside cosmic-text

### 5.3 Text rendering output

Each shaped text block emits:

```
glyph: { pos: vec2<f32>, uv: vec2<f32>, size: f32, atlas_index: u32 }
```

You generate quads from these.

---

# 6. Scene Graph (lightweight custom)

A minimal, flattened scene tree:

```
enum DrawNode {
    Rect(RectNode),
    Image(ImageNode),
    Text(TextNode),
}
```

### Rules:

- Nodes reference absolute layout rects
- No nested transforms unless explicit
- Node ordering = paint order
- Only dirty nodes are re-rasterized

---

# 7. GPU Rendering Model (wgpu)

Rune-Draw uses **2 primary pipelines**:

## 7.1 Shape Pipeline

For rectangles, rounded rects, borders, shadows.

Features:

- solid colors
- gradients (linear/radial)
- corner radius
- box shadows (blur kernel on GPU)

Backed by a simple instanced quad system with uniforms:

```
struct ShapeUniform {
    rect: vec4<f32>,
    radius: f32,
    fill_color: vec4<f32>,
    shadow_color: vec4<f32>,
    shadow_offset: vec2<f32>,
    shadow_blur: f32,
}
```

---

## 7.2 Text Pipeline

For SDF-based or atlas-based text rendering.

### Requirements:

- Preload fonts into a GPU atlas
- Each glyph → quad
- Use a single fragment shader:

  - sample SDF or 8-bit alpha mask
  - apply subpixel AA if needed
  - apply transforms

---

# 8. Resize Strategy (smooth containers, stable text)

This is the secret to browser-like smooth resizing.

## 8.1 Track two width states

```
visual_width  = updated every pixel (smooth)
layout_width  = updated only when needed
```

## 8.2 Frozen text during resize

- Update GPU transform every frame
- Keep old text shaping
- Keep old layout rects
- Update only visual width

## 8.3 Trigger layout + text shaping only when:

- width change ≥ 12 px, OR
- 120ms since last resize event

---

# 9. Hit Testing (zero drift)

Hit testing uses:

- pointer position
- same transforms as GPU
- same float rects from Taffy

Workflow:

```
pointer → visual space → layout space → rect.contains()
```

### Key rule:

**DO NOT recompute hit-test rects manually.
Always read from layout tree.**

---

# 10. Render Loop

Pseudocode:

```rust
loop {
    handle_events();

    if resize_active {
        update_visual_width();
        render_visual_frame(); // no layout, no text reflow
        continue;
    }

    if layout_needed {
        compute_layout(layout_width);
        shape_text();
        update_scene_nodes();
    }

    render_full_frame();
}
```

---

# 11. Performance Strategies

### ✔ Constant-size offscreen texture

Resize only when absolutely needed.

### ✔ Batch updates

Rebuild only changed nodes.

### ✔ Separate shape/text pipelines

Parallelizable on GPU.

### ✔ No CPU-side clipping

Let fragment shader discard.

### ✔ Cache shaped paragraphs

### ✔ Use float positions everywhere

No rounding until raster.

---

# 12. Files & Modules

```
rune-draw/
  src/
    gpu/
      mod.rs
      pipelines/
        shape.rs
        text.rs
    layout/
      mod.rs
      resolver.rs
    text/
      mod.rs
      shaping.rs
    scene/
      mod.rs
    renderer.rs
    lib.rs

  shaders/
    shape.wgsl
    text.wgsl

docs/
  rune_draw.md   <--- this file
```

---

# 13. Future Enhancements

- GPU-driven text shaping (compute shader)
- Async shaping using rayon
- Glyph subpixel-positioning
- Blur layers for shadows
- Mesh caching for rounded rects
- CSS filter effects (blur, saturate, contrast)

---

# 14. Summary

Rune-Draw replaces Vello with a predictable, stable pipeline:

- **Taffy → layout**
- **Cosmic-text → shaping**
- **wgpu → rendering**
- **Scene Graph → integration**

Resize is smooth, text is stable, hit testing is perfect.

You avoid:

- logical_px chaos
- jittery containers
- cosmic-text overwork
- GPU stutter
- Vello opacity/format issues

This is the foundation of your fully custom 2D renderer.
