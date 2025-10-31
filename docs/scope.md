# 🧭 Scope Document: Custom Graphics Engine + Cosmic-Text Patch

## 1. Objective

Design and implement a **custom GPU-native 2D graphics engine** in Rust with full control over precision, blending, and text rendering. The engine should serve as a rendering foundation for Rune/Wisp applications, replacing or extending Vello to address limitations in gradient precision, alpha handling, and subpixel text rendering.

---

## 2. Motivation

### Problems with Vello:

- Gradient interpolation artifacts (incorrect linear/sRGB blending, visible banding even with `Rgba16f`).
- Alpha transparency and premultiplied blending inconsistencies.
- Missing CSS-accurate box-shadow handling (blur, spread, offset).
- Limited text rendering quality at small font sizes — no subpixel AA.

### Desired Outcomes:

- Correct linear-space gradient interpolation.
- Physically correct alpha compositing with premultiplied alpha.
- Configurable 16-bit float pipeline from storage to framebuffer.
- Customizable blur + shadow pipeline matching CSS box-shadow semantics.
- Subpixel AA text rendering for small fonts using cosmic-text integration.

---

## 3. System Architecture Overview

### 3.1 Core Components

| Module                  | Responsibility                                                     |
| ----------------------- | ------------------------------------------------------------------ |
| **SceneGraph**          | Hierarchical node tree for shapes, text, images, and effects.      |
| **RenderAllocator**     | Manages GPU textures, buffers, and intermediate targets.           |
| **PassManager**         | Organizes render passes (fill, blur, composite, text).             |
| **Brush System**        | Implements solid, gradient, and pattern brushes.                   |
| **Compositor**          | Handles layer blending, ordering, and premultiplied alpha rules.   |
| **ShaderSuite**         | Set of WGSL shaders for fills, gradients, blurs, and text.         |
| **TextProvider**        | Integrates cosmic-text shaping and rasterization with subpixel AA. |
| **Backend Abstraction** | Based on `wgpu`, targeting Vulkan, Metal, and DX12.                |

### 3.2 Rendering Pipeline

1. **Scene Upload** — Convert IR to GPU commands (geometry, materials).
2. **Gradient Pass** — Compute linear interpolations in linear RGB, premultiplied.
3. **Shadow Pass** — Gaussian blur on alpha mask, composite with color and offset.
4. **Text Pass** — Render subpixel coverage glyphs with gamma-correct blending.
5. **Composition Pass** — Merge layers using premultiplied alpha blending.
6. **Output Pass** — Linear → sRGB conversion (if target requires it).

---

## 4. Gradient and Shadow Subsystem

### 4.1 Gradient Precision

- Store gradient stops in **linear RGB** space.
- Interpolate using 16f floats in WGSL:

  ```wgsl
  let color = mix(stop0, stop1, t); // all in linear space
  ```

- Optionally dither final color before write to reduce banding.

### 4.2 Alpha & Premultiplication

- All color data is stored **premultiplied by alpha** internally.
- Final un-premultiply before output only if target requires non-premultiplied format.

### 4.3 Box Shadow Pipeline

- Generate shadow geometry from base rect + spread + offset.
- Create an alpha mask, blur using separable Gaussian kernel.
- Multiply blurred alpha by shadow color.
- Composite beneath main object using premultiplied alpha blend.

---

## 5. Text Rendering and Cosmic-Text Patch

### 5.1 Current Status / Issue

- Cosmic-text shapes via HarfRust and rasterizes via Swash, which by default yields 8-bit grayscale coverage.
- Without subpixel AA, small sizes can look softer; we convert grayscale → RGB subpixel coverage in-engine.
- No true hinting yet; we apply baseline snap + small-size pseudo-hinting for stability.

### 5.2 Patch Goals

- Enable **RGB subpixel coverage masks**.
- Optionally **16-bit coverage precision**.
- Support **fractional glyph positioning**.

### 5.3 Implementation Steps

1. **Fork `fontdue`**:

   - Modify rasterizer to emit 3-channel (RGB) coverage per pixel.
   - Provide optional 16-bit mask generation.

2. **Engine integration with cosmic-text (done)**:

   - Default provider uses cosmic-text for shaping and swash for raster.
   - Convert grayscale coverage to RGB subpixel masks on CPU, respecting RGB/BGR.
   - Runtime toggle for RGB/BGR orientation.

   Optional (future): upstream cosmic-text extensions for native subpixel AA output and 16-bit masks.

3. **WGSL Shader Changes**:

   ```wgsl
   let coverage = textureSample(glyph_atlas, sampler, uv).rgb;
   let color = text_color.rgb * coverage;
   let alpha = dot(coverage, vec3<f32>(1.0/3.0));
   out_color = vec4<f32>(color, alpha);
   ```

4. **Optional hinting integration**:

   - Use FreeType through small FFI bridge for hinted glyph masks at small sizes.

---

## 6. Color Management

- All internal computations in **linear light**.
- Use `palette` crate for conversions.
- Per-pass color space awareness (no hidden sRGB clamps).
- Optional HDR-ready output if hardware supports 16f or 32f.

---

## 7. API Integration

### 7.1 Public Interface

```rust
struct GraphicsEngine {
    fn new(device: &wgpu::Device) -> Self;
    fn render_scene(&mut self, scene: &SceneGraph, target: &wgpu::TextureView);
}
```

### 7.2 IR Integration

- Direct conversion from Rune IR → SceneGraph nodes.
- Each IR node maps to `Shape`, `Text`, `Shadow`, `Group`, or `ClipLayer`.

### 7.3 Performance Goals

- Maintain <1ms GPU time per 1080p frame for 1k elements.
- Minimize texture reallocations.
- Persistent atlases for glyphs, gradients, and blurs.

---

## 8. Deliverables

| Deliverable                    | Description                                                          |
| ------------------------------ | -------------------------------------------------------------------- |
| **`engine-core` crate**        | SceneGraph, allocator, passes, compositing, color pipeline.          |
| **`engine-shaders` crate**     | WGSL shader suite for gradients, text, shadows.                      |
| **`cosmic-text-patch` branch** | Adds subpixel coverage and 16-bit glyph masks.                       |
| **Demo app**                   | Renders gradients, shadows, text comparison (grayscale vs subpixel). |

---

## 9. Future Extensions

- Signed Distance Field (SDF) text rendering.
- Vector outline rendering for SVG glyphs.
- GPU compute-based filters (blur, bloom, glow, inner shadow).
- Multi-threaded scene upload and batching.
- Skia‑compatible paint model for interoperability.

---

## 10. Licensing & Upstream Strategy

- Use permissive MIT/Apache-2.0 licensing to remain compatible with cosmic-text and wgpu.
- Maintain patch branches for `fontdue` and `cosmic-text` separately.
- Potential future contribution of linear-color and subpixel patches upstream to cosmic-text.

---

## 11. References

- [Linebender Vello](https://github.com/linebender/vello)
- [Cosmic-Text](https://github.com/pop-os/cosmic-text)
- [Fontdue](https://github.com/mooman219/fontdue)
- [Tiny-Skia](https://github.com/RazrFalcon/tiny-skia)
- [WGSL Specification](https://gpuweb.github.io/gpuweb/wgsl.html)

---

### ✅ Outcome

A fully controlled, precision-safe 2D graphics engine in Rust with:

- Correct gradient and alpha compositing.
- CSS‑accurate box‑shadow rendering.
- Subpixel and hinted text clarity.
- Seamless integration with Rune/Wisp IR and GPU runtime.
