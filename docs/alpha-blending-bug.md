# Alpha Blending Bug on macOS/Metal

## Summary
Premultiplied alpha blending does not work correctly on macOS with wgpu's Metal backend. Semi-transparent geometry renders as completely opaque.

## Environment
- **Platform**: macOS
- **Backend**: Metal (via wgpu)
- **wgpu version**: (check your Cargo.toml)
- **Formats tested**: 
  - `Rgba16Float` - doesn't blend
  - `Rgba8UnormSrgb` - doesn't blend
  - `Rgba8Unorm` - doesn't blend

## Expected Behavior
When rendering geometry with premultiplied alpha colors (e.g., `rgba(0.1, 0.1, 0.1, 0.1)`) using blend state:
```rust
blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING)
// Which expands to:
// color: { src: One, dst: OneMinusSrcAlpha, op: Add }
// alpha: { src: One, dst: OneMinusSrcAlpha, op: Add }
```

The geometry should blend with the background according to the formula:
```
result = src + dst * (1 - src.alpha)
```

## Actual Behavior
The geometry renders as completely opaque, as if alpha blending is disabled entirely. The alpha channel appears to be ignored or forced to 1.0.

## Reproduction
1. Clear render target to a solid color (e.g., dark blue `#0b1220`)
2. Render a rectangle with premultiplied alpha color (e.g., white at 10% alpha: `[0.102, 0.102, 0.102, 0.102]`)
3. Expected: Light gray-blue rectangle (blended)
4. Actual: Solid gray rectangle (opaque)

## Debug Findings
- Verified vertex colors contain correct alpha values (e.g., 0.102)
- Verified shader outputs correct premultiplied colors
- Visualizing alpha channel shows correct values in offscreen buffer
- Blend state is correctly configured
- Issue persists across multiple texture formats
- Same code works correctly on other platforms (needs verification)

## Workarounds Attempted
1. ✅ **Pre-calculate blended colors** - Works but defeats purpose of GPU blending
2. ❌ Switching to straight alpha (`SrcAlpha` instead of `One`) - Doesn't work
3. ❌ Un-premultiplying in shader - Doesn't work
4. ❌ Different texture formats - Doesn't work

## Related Issues
- Similar issue reported in Vello renderer (also uses wgpu on macOS)
- May be related to Metal's handling of premultiplied alpha

## Minimal Reproduction Code
```rust
// Render pipeline with premultiplied alpha blending
let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
    // ... vertex state ...
    fragment: Some(wgpu::FragmentState {
        module: &shader,
        entry_point: "fs_main",
        targets: &[Some(wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })],
    }),
    // ...
});

// Shader outputs premultiplied color
@fragment
fn fs_main(inp: VsOut) -> @location(0) vec4<f32> {
    return inp.color; // premultiplied: [0.102, 0.102, 0.102, 0.102]
}

// Render to offscreen buffer cleared to solid color
// Then composite to surface
// Result: Opaque gray instead of semi-transparent
```

## Impact
- Prevents proper transparency rendering on macOS
- Affects any application using wgpu for 2D graphics with alpha blending
- Requires manual color pre-calculation workarounds

## Next Steps
1. Verify if this is a wgpu bug or Metal driver issue
2. Test on different macOS versions and GPU hardware
3. Compare with WebGPU behavior in browsers on macOS
4. Report to wgpu if confirmed as wgpu bug
