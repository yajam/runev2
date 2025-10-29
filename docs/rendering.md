# Rendering Pipeline Notes

This document summarizes the current render paths, the `BYPASS_COMPOSITOR` flag, and the recommended approach to keep geometry stable on resize without re‑uploading buffers.

## Compositor Bypass
- Purpose: Allow lightweight widgets to render directly to the surface without the offscreen compositing pass.
- Toggle: Set the environment variable `BYPASS_COMPOSITOR`.
  - Direct to surface: `BYPASS_COMPOSITOR=1 cargo run -p demo-app`
  - Through compositor (default): `cargo run -p demo-app`
- Behavior:
  - When bypassed, `PassManager` uses a surface‑format solid pipeline and draws the scene straight to the swapchain.
  - Otherwise, it renders into a linear offscreen target and composites to the surface (with premultiplied alpha and a V‑flip in the compositor FS to correct orientation).

## Resize Policy: Keep Shapes from Scaling
Today the demo re‑uploads geometry on resize to convert from pixels → NDC using the new viewport. This works, but it’s not ideal.

Recommended approach going forward:

- Static geometry: Store vertices in local/layout units (IR units or pixels), not in NDC. Avoid touching vertex/index buffers on resize.
- Dynamic uniform (viewport): Compute a viewport transform every frame and pass it to the vertex shader as a uniform. This maps local/layout units to NDC.
- Per‑layer transform: Supply an additional uniform for scroll offset, scale, rotation per layer/group. The final transform is the product of viewport × layer × model transforms.

Conceptually:

- `clip_pos = Projection(viewport) * View(scroll/zoom) * Model(layer) * vec3(local_xy, 1)`
- Projection(viewport): Converts local/layout units into NDC, accounting for framebuffer size and Y direction.
- View(scroll/zoom): Optional global panning and zoom for the scene.
- Model(layer): Per‑layer translation/rotation/scale.

## Implementation Sketch

1) Add a per‑frame uniform for viewport
- CPU side: write width/height and any origin/top‑left conventions.
- WGSL uniform:

```
struct ViewportUniform {
  scale: vec2<f32>;   // 2/W, -2/H (for Y‑up NDC)
  translate: vec2<f32>; // (-1, +1)
}
@group(0) @binding(0) var<uniform> vp: ViewportUniform;
```

- Vertex path uses local positions, then:

```
let ndc = vec2<f32>(pos.x * vp.scale.x + vp.translate.x,
                    pos.y * vp.scale.y + vp.translate.y);
```

2) Add a per‑draw (or per‑layer) uniform for model
- Translation/rotation/scale per batch/layer. Multiplied before viewport.
- For a simple 2D affine:

```
struct Affine2D { m: mat3x3<f32>; } // or a 2x3 packed struct
@group(1) @binding(0) var<uniform> model: Affine2D;
```

- Apply: `let p = (model.m * vec3<f32>(pos, 1.0)).xy;`

3) CPU changes
- Keep geometry buffers static after initial upload (local units).
- On resize: update the viewport uniform only; do not re‑upload geometry.
- For scrolled/animated layers: update the per‑layer uniform.

4) Migration from current demo
- Today: CPU converts to NDC and re‑uploads on resize.
- Target: Stop doing CPU NDC conversion; instead, send local positions and bind viewport + model uniforms.
- Compositor remains optional via `BYPASS_COMPOSITOR`.

## When to Use Which Path
- Direct to surface (`BYPASS_COMPOSITOR=1`):
  - Simple, fully‑opaque widgets
  - No post‑processing or multipass effects
  - Latency sensitive paths where one pass matters
- Through compositor (default):
  - Gradients, blurs, shadows, text AA, or any effect requiring offscreen targets
  - Mixed content where ordering and alpha blending across layers is important

## Orientation Notes
- Offscreen → surface path samples a texture in the compositor. Because clip space is Y‑up and texture coords are Y‑down by default, the compositor flips V in its fragment shader.
- Direct path renders without that sampling step, so no flip is applied there.

## Roadmap Tasks
- Switch solid pipeline to consume local coordinates.
- Add viewport uniform to solids + text pipelines.
- Add per‑layer uniform (scroll/scale/rotation) and bind at draw time.
- Update `PassManager::render_frame` to write the viewport uniform each frame.
- Remove geometry re‑upload on resize in the demo once uniforms land.

