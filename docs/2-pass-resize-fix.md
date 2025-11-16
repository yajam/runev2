# 2-Pass Rendering Resize Orchestration Fix

## Problem Summary

The 2-pass rendering system (background + foreground) had orchestration issues during window resize:

1. **Intermediate texture not preserved during resize** - White edges appeared when expanding
2. **Background rendering didn't match expansion** - Backgrounds were cleared on size change
3. **Layout freezing during expansion** - Full redraws forced even during debounce period

## Root Causes

### 1. MSAA Resolve Size Mismatch
The intermediate texture must match the exact render size for MSAA resolve to work. When using MSAA with a resolve target, both textures must have identical dimensions. The original code tried to preserve an oversized texture, causing validation errors:

```
Attachments have differing sizes: the color attachment at index 0's texture view has extent (1600, 1200, 1) 
but is followed by the color attachment at index 0's resolve texture view which has (3024, 1832, 1)
```

### 2. Forced Full Redraw on Size Change
The resize logic forced a full redraw whenever size changed:

```rust
// OLD CODE
let should_render_full = needs_redraw && (
    last_resize_time.is_none() || 
    max_debounce_exceeded || 
    size_changed  // <-- This forced full redraw
);
```

This prevented the background-only render path from working during expansion.

## Solution

### 1. Exact Size Matching with LoadOp::Load Preservation

The intermediate texture must always match the exact render size for MSAA compatibility. Content preservation is achieved through `LoadOp::Load` when the size hasn't changed:

```rust
// Check if size changed before ensuring texture
let size_changed = match &self.intermediate_texture {
    Some(tex) => tex.key.width != width || tex.key.height != height,
    None => true,
};

// Ensure texture matches exact size (required for MSAA resolve)
self.ensure_intermediate_texture(allocator, width, height);

// Only preserve if size didn't change (texture wasn't reallocated)
let actually_preserve = preserve_intermediate && !size_changed;
```

The `ensure_intermediate_texture` always allocates at exact size:

```rust
let needs_realloc = match &self.intermediate_texture {
    Some(tex) => {
        // Reallocate if size doesn't match exactly
        // MSAA resolve requires exact size match
        tex.key.width != width || tex.key.height != height
    }
    None => true,
};
```

### 2. Remove Forced Full Redraw on Size Change

Updated the redraw logic to not force full redraw on size change:

```rust
// NEW CODE
let should_render_full = needs_redraw && (
    last_resize_time.is_none() || 
    max_debounce_exceeded
    // size_changed removed - no longer forces full redraw
);
```

This allows background-only renders to continue during resize, with the intermediate texture preserved.

### 3. Preserve Content via LoadOp::Load

When the size doesn't change, content is preserved using `LoadOp::Load`:
- The intermediate texture keeps its content between frames
- Background renders use `LoadOp::Load` to preserve existing content
- Only new/changed areas need to be redrawn

When size changes, the texture is reallocated and cleared:
- MSAA requires exact size match, so we must reallocate
- Backgrounds are immediately rendered to fill the new size
- Debounce prevents expensive foreground rendering during rapid resize

## Benefits

1. **No MSAA validation errors** - Intermediate texture always matches render size exactly
2. **Smooth background rendering** - Backgrounds render immediately without layout calculations
3. **No layout freezing** - Full redraws only happen after debounce, not on every size change
4. **Content preservation when possible** - LoadOp::Load preserves content when size is stable

## Testing

To test the fix:
1. Run `cargo run --bin rune-scene`
2. Resize the window by dragging edges
3. Observe:
   - No MSAA validation errors
   - UI remains responsive (no freezing)
   - Full redraw happens after 200ms debounce

## Known Issues

⚠️ **Background zones not visually filling full width during rapid expansion**

During rapid window expansion, the background zones may not visually fill the entire window width. The zone layout calculations are correct and backgrounds render immediately, but there appears to be a timing or coordinate system issue.

Possible causes to investigate:
- Logical vs physical pixel coordinate mismatch
- Blit timing relative to surface resize
- Surface configuration lag

## Files Modified

- `crates/engine-core/src/pass_manager.rs` - Exact size matching for MSAA compatibility
- `crates/rune-scene/src/lib.rs` - Remove forced full redraw on size change
- `crates/demo-app/src/main.rs` - Add preserve_intermediate parameter

## Related Issues

- Background/foreground 2-pass rendering
- Vello-style smooth resizing
- Resize debouncing for performance
