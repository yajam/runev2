# macOS Window Resize Flash Issue

## The Problem

When double-clicking the window title bar to maximize on macOS, you may see brief white/black flashes during the smooth resize animation. This happens because:

1. macOS performs a smooth animated resize (unlike manual dragging which is instant)
2. The Metal/wgpu rendering pipeline can't always produce frames fast enough to keep up
3. macOS either stretches the previous frame or shows the window background color

This is **not a bug in our code** - it's a fundamental limitation of how `winit` and `MTKView` work on macOS.

## Why Setting Window Background Color Doesn't Work

Setting the window background to match the scene background seems like a simple fix, but:
- It only works for solid color backgrounds
- For gradients, you'd see the solid color which looks wrong (e.g., dark gray on a blue gradient)
- The stretched/juddering frame is still visible, just less obvious

## How Other Engines Handle This

Based on research of professional graphics applications:

### 1. **Accept the Limitation** (Most Common)
Many applications (including iTerm2, various Metal apps) just live with occasional glitches during resize. Users rarely notice because:
- The animation is fast
- It only happens on double-click maximize, not manual resize
- The immediate render on resize completion looks correct

### 2. **Use CAMetalLayer Directly** (Complex)
Some apps bypass `MTKView` and use `CAMetalLayer` with specific synchronization:
```swift
layer.presentsWithTransaction = true
commandBuffer.waitUntilScheduled()
commandBuffer.present(drawable)
layer.layerContentsPlacement = .topLeft  // Hides glitches at edge
```

This requires:
- Not using `winit`'s default Metal view
- Custom Objective-C/Swift integration
- Significant complexity for marginal improvement

Reference: https://thume.ca/2019/06/19/glitchless-metal-window-resizing/

### 3. **Disable Resize Animation** (User Experience Trade-off)
Make window resizing instant instead of animated:
```bash
defaults write -g NSWindowResizeTime -float 0.001
```
This eliminates the problem but removes the smooth macOS feel.

### 4. **Our Current Approach: Immediate Render** (Best for wgpu/winit)
Our code already does the right thing:

```rust
Event::WindowEvent {
    event: WindowEvent::Resized(new_size),
    ...
} => {
    // Reconfigure surface
    surface.configure(&engine.device(), &config);
    
    // Immediately draw a full frame to avoid white flash
    if let Ok(frame) = surface.get_current_texture() {
        // ... render immediately ...
        frame.present();
    }
}
```

This minimizes flashing by rendering as quickly as possible after each resize event.

## How Vello Solves This

Vello handles resize smoothly by using an **intermediate texture approach**:

1. **Render to an intermediate texture** (not directly to the surface)
2. **Blit the texture to the surface** (a very fast operation)
3. During resize, only the blit needs to complete, not the full render

Key code from Vello:
```rust
// In RedrawRequested:
renderer.render_to_texture(device, queue, scene, &intermediate_texture, params);
blitter.copy(device, encoder, &intermediate_texture, &surface_texture);
```

The blit operation is orders of magnitude faster than rendering, so it completes before the next resize frame, eliminating flashes.

## Our Current Approach

We simplified the resize handler to match Vello's pattern:
- Remove immediate rendering on `Resized` events
- Let the normal `RedrawRequested` cycle handle rendering
- Rely on `AboutToWait` continuously requesting redraws

This is simpler and closer to Vello's approach, though without the intermediate texture optimization.

## Future Improvement

To fully match Vello's smooth resizing, we would need to:
1. Add an intermediate texture/render target
2. Implement a blit pass to copy to the surface
3. This adds complexity but provides the smoothest possible resize experience

For now, the simplified approach provides good results with minimal complexity.

## Related Issues

- [wgpu #249: How to handle window resizing on macOS](https://github.com/gfx-rs/wgpu/issues/249)
- [sokol #248: Bug and Possible Fix: Judder on Window Resize](https://github.com/floooh/sokol/issues/248)
- [Tristan Hume's article on glitchless Metal resizing](https://thume.ca/2019/06/19/glitchless-metal-window-resizing/)
