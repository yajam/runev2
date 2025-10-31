# 🧭 Renderer Completion Checklist

### 🎯 Goal

Achieve a production-grade, GPU-native 2D renderer (Rune Engine Core) with full support for gradients, alpha, shadows, shapes, and text — matching or exceeding Skia/Vello capabilities.

---

## ✅ Stage 1: Core Rendering Foundation _(Done or Near-Complete)_

| Feature                     | Status | Difficulty | Notes                                         |
| --------------------------- | ------ | ---------- | --------------------------------------------- |
| Linear gradients (16f)      | ✅     | ⚙️ Medium  | Banding-free, linear-space interpolation      |
| Radial gradients            | ✅     | ⚙️ Medium  | Aspect-correct circle scaling                 |
| Texture format fallback     | ✅     | ⚙️ Easy    | Default RGBA16F → fallback to RGBA8           |
| GPU viewport / DPI handling | 🟢     | ⚙️ Easy    | Implement `set_scale_factor()` in engine-core |

---

## 🟡 Stage 2: Compositing & Transparency

| Feature                         | Status | Difficulty | Notes                                              |
| ------------------------------- | ------ | ---------- | -------------------------------------------------- |
| Premultiplied alpha compositing | 🟡     | ⚙️ Medium  | Ensure src-over correctness, linear light blending |
| Box-shadow rendering            | 🟡     | ⚙️ Hard    | Gaussian blur on alpha mask + spread/offset        |
| Group opacity                   | ⏳     | ⚙️ Medium  | Composite offscreen layer per group                |
| Clipping (rect, rounded)        | ⏳     | ⚙️ Medium  | Scissor or stencil buffer                          |
| Arbitrary path clip             | ⏳     | ⚙️ Hard    | Use mask texture                                   |

---

## 🔵 Stage 3: Shape & Stroke System

| Feature                        | Status | Difficulty | Notes                         |
| ------------------------------ | ------ | ---------- | ----------------------------- |
| Basic stroke rendering         | ⏳     | ⚙️ Medium  | Line width, join, cap, dash   |
| Vector path fill rules         | ⏳     | ⚙️ Medium  | Non-zero / even-odd winding   |
| Shape transforms               | ⏳     | ⚙️ Medium  | Affine 2D transform hierarchy |
| Path flattening / tessellation | ⏳     | ⚙️ Hard    | Adaptive precision curves     |

---

## 🟣 Stage 4: Text Rendering System

| Feature                        | Status | Difficulty | Notes                               |
| ------------------------------ | ------ | ---------- | ----------------------------------- |
| Subpixel AA (RGB mask)         | ✅     | ⚙️ Medium  | Provider + GPU pass integrated      |
| Fractional glyph positioning   | 🟡     | ⚙️ Medium  | Needed for small font crispness     |
| Baseline alignment             | ✅     | ⚙️ Easy    | Fixed via run baseline snapping     |
| Font hinting (FreeType bridge) | 🟡     | ⚙️ Medium  | Implemented via `freetype_ffi` (bytes only); demo toggle `DEMO_FREETYPE=1` |
| Decorations (underline/strike) | ⏳     | ⚙️ Easy    | Vector line or shader pass          |

---

## 🟢 Stage 5: Image & SVG Integration

| Feature                     | Status | Difficulty | Notes                               |
| --------------------------- | ------ | ---------- | ----------------------------------- |
| PNG / JPEG raster sampling  | 🟡     | ⚙️ Easy    | GPU texture upload, sRGB correction |
| SVG rasterization + caching | 🟢     | ⚙️ Medium  | usvg+resvg raster; bucketed scale cache in engine-core |
| SVG path import             | ⏳     | ⚙️ Hard    | Convert `usvg` → geometry IR        |
| Gradient & mask integration | ⏳     | ⚙️ Medium  | SVG paint servers mapped to brushes |
| Image caching / atlas       | ⏳     | ⚙️ Medium  | Prevent re-uploads on redraw        |

---

## 🔶 Stage 6: Filters & Effects

| Feature                        | Status | Difficulty | Notes                          |
| ------------------------------ | ------ | ---------- | ------------------------------ |
| Drop shadow                    | ⏳     | ⚙️ Medium  | Outer shadow via blur + offset |
| Inner shadow                   | ⏳     | ⚙️ Hard    | Inverted mask blur             |
| Glow                           | ⏳     | ⚙️ Medium  | Additive blur pass             |
| Gaussian blur (general filter) | 🟡     | ⚙️ Medium  | Reuse box-shadow blur kernel   |
| Brightness / contrast filters  | ⏳     | ⚙️ Easy    | Simple color matrix shader     |

---

## 🔷 Stage 7: Color & Output Quality

| Feature                   | Status | Difficulty | Notes                           |
| ------------------------- | ------ | ---------- | ------------------------------- |
| Linear ↔ sRGB conversions | ✅     | ⚙️ Medium  | Avoid double-gamma on swapchain |
| Dithering pass            | ⏳     | ⚙️ Medium  | Blue-noise or Bayer matrix      |
| HDR pipeline support      | ⏳     | ⚙️ Hard    | Optional RGBA16F → scRGB output |
| Color profile awareness   | ⏳     | ⚙️ Medium  | Integrate ICC/gamut metadata    |

---

## 🧱 Stage 8: Performance Infrastructure

| Feature                           | Status | Difficulty | Notes                           |
| --------------------------------- | ------ | ---------- | ------------------------------- |
| Draw batching by pipeline         | ⏳     | ⚙️ Medium  | Minimize pipeline binds         |
| GPU buffer reuse                  | ⏳     | ⚙️ Easy    | Persistent vertex/index buffers |
| Texture atlas for glyphs & images | ⏳     | ⚙️ Medium  | Reduce bind overhead            |
| Dirty rect tracking               | ⏳     | ⚙️ Medium  | Partial redraw optimization     |
| GPU timing / profiling            | ⏳     | ⚙️ Easy    | For perf HUD or logs            |

---

## 🧩 Stage 9: Developer & Debug Tools

| Feature                 | Status | Difficulty | Notes                                 |
| ----------------------- | ------ | ---------- | ------------------------------------- |
| Wireframe overlay       | ⏳     | ⚙️ Easy    | Visualize tessellation                |
| Layer visualization     | ⏳     | ⚙️ Medium  | Show blend passes / offscreen buffers |
| GPU capture integration | ⏳     | ⚙️ Easy    | RenderDoc marker scopes               |
| Validation scenes       | ⏳     | ⚙️ Medium  | Automated diff vs. reference images   |

---

## 🔟 Stage 10: SVG Animation Runtime

| Feature                                  | Status | Difficulty | Notes                                                              |
| ---------------------------------------- | ------ | ---------- | ------------------------------------------------------------------ |
| Declarative SMIL subset                   | ⏳     | ⚙️ Hard    | Support `<animate>` and `<animateTransform>`                        |
| Properties: opacity, transform, dashoffset| ⏳     | ⚙️ Medium  | Translate to engine primitives; linear RGB for color, if added     |
| Timing model (dur/repeat/keyTimes/splines)| ⏳     | ⚙️ Medium  | Timeline + easing; per-node state updates per frame                |
| Scheduler + integration                   | ⏳     | ⚙️ Medium  | Vsync-driven loop, pause/resume, fixed timestep option             |
| Non-goal: JS/CSS animation                | 🚫     | —          | No script execution; fallback to raster for unsupported features   |

---

## 🧭 Final Integration Milestones

| Milestone                      | Definition of Done                                             |
| ------------------------------ | -------------------------------------------------------------- |
| **M1 – Core Renderer Stable**  | Gradients, alpha, box-shadow, transforms, text baseline stable |
| **M2 – Full 2D Engine Parity** | Strokes, images, SVGs, filters, clipping all working           |
| **M3 – GPU Optimized Passes**  | Batching, caching, dirty rects                                 |
| **M4 – QA & Validation**       | No banding, color-correct, accurate DPI across OS              |
| **M5 – Integration Ready**     | Seamless plug-in for Rune/Wisp IR and event system             |
