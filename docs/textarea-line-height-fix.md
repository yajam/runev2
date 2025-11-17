# TextArea Line Height and Caret Rendering Fix

## Issues Identified

1. **Inconsistent caret positioning** when moving between lines
2. **Caret disappearing** during vertical navigation
3. **Selection highlighting misalignment** with text

## Root Cause

The coordinate system mismatch between:
- Text rendering (baseline-based coordinates)
- Selection rendering (box-based coordinates)
- Caret rendering (box-based coordinates from TextLayout)

## Understanding the Coordinate System

### TextLayout Coordinate System

Each `LineBox` in TextLayout has:
- `y_offset`: Top of the line box (relative to text start)
- `baseline_offset`: Distance from line box top to baseline
- `height`: Total line box height (ascent + descent + line_gap)

```
y_offset = 0.0     ┌─────────────────┐ ← Line box top
                   │                 │
baseline_offset    │─────────────────│ ← Baseline (where text sits)
                   │                 │
height             └─────────────────┘ ← Line box bottom
                   
y_offset = height  ┌─────────────────┐ ← Next line box top
                   │                 │
```

### Text Rendering

`draw_text_direct` expects **baseline coordinates**:
```rust
let text_baseline_y = content_y + line.y_offset + line.baseline_offset - scroll_y;
canvas.draw_text_direct([text_x, text_baseline_y], text, ...);
```

### Caret Rendering

`cursor_rect_at_position` returns coordinates where:
- `y = line.y_offset` (top of line box)
- `height = line.height` (full line box height)

The caret renderer transforms this:
```rust
let cy0 = content_rect.y - scroll_y + cursor_rect.y;  // Top of line box
let cy1 = cy0 + cursor_rect.height;                    // Bottom of line box
```

### Selection Rendering

Selection rectangles from `selection_rects()` use layout coordinates (relative to text start). The selection renderer needs to transform these to screen coordinates.

## The Fix

### 1. Improved Text Rendering (text_area.rs lines 766-788)

Added clearer variable names and better visibility checking:

```rust
// Text baseline Y position
let text_baseline_y = content_y + line.y_offset + line.baseline_offset - self.scroll_y;

// Visibility check using line box bounds
let line_top_y = content_y + line.y_offset - self.scroll_y;
let line_bottom_y = line_top_y + line.height;

if line_bottom_y >= content_y && line_top_y <= content_y + content_height {
    canvas.draw_text_direct([text_x, text_baseline_y], line_text, ...);
}
```

### 2. Fixed Selection Rendering (text_area.rs lines 745-770)

Corrected the `text_baseline_y` parameter to match the coordinate system:

**Before:**
```rust
text_baseline_y: content_y,  // Wrong! This causes misalignment
```

**After:**
```rust
// Get baseline offset from first line
let baseline_offset = layout.lines().first().map(|l| l.baseline_offset).unwrap_or(text_size * 0.8);

text_baseline_y: content_y + baseline_offset,  // Correct!
```

This ensures the selection renderer's coordinate transformation matches the text rendering:
```rust
// In selection_renderer.rs:
let highlight_y = config.text_baseline_y - baseline_offset + sel_rect.y - config.scroll_y;
// Expands to:
// = (content_y + baseline_offset) - baseline_offset + line.y_offset - scroll_y
// = content_y + line.y_offset - scroll_y  ✓ Correct!
```

## Why This Matters

### Line Height Consistency

The TextLayout correctly calculates line height as:
```rust
line_height = ascent + descent + line_gap
```

This is applied consistently across all lines (see `text_layout.rs` lines 2414, 2433, 2531).

### Vertical Navigation

With proper coordinate alignment:
- ✅ Caret stays visible when moving up/down
- ✅ Caret height matches line height
- ✅ Selection highlights align with text
- ✅ Smooth vertical scrolling

### Multi-line Rendering

Each line is positioned correctly:
```
Line 0: y_offset = 0.0
Line 1: y_offset = line_height
Line 2: y_offset = line_height * 2
...
```

## Testing

### Manual Test Cases

1. **Type multiple lines**: Press Enter several times
   - ✅ Lines should be evenly spaced
   - ✅ Caret should be visible on each line

2. **Navigate with Up/Down arrows**:
   - ✅ Caret should move smoothly between lines
   - ✅ Caret should not disappear
   - ✅ Caret height should be consistent

3. **Select text across lines**:
   - ✅ Selection should highlight entire lines
   - ✅ Selection should align with text
   - ✅ No gaps or overlaps

4. **Scroll with long text**:
   - ✅ Text should scroll smoothly
   - ✅ Caret should stay visible
   - ✅ Selection should scroll with text

### Run the Demo

```bash
cargo run
```

Then:
1. Click on the TextArea
2. Type several lines of text (press Enter between lines)
3. Use Up/Down arrows to navigate
4. Select text with Shift+Up/Down
5. Verify caret stays visible and aligned

## Technical Details

### Why baseline_offset is needed

Text rendering is baseline-based because:
- Fonts define glyphs relative to a baseline
- Different fonts/sizes have different ascent/descent
- Baseline ensures consistent text alignment

The baseline_offset tells us where the baseline is within the line box:
- Typically `baseline_offset ≈ ascent`
- This accounts for the space above the baseline (ascenders like 'h', 'k')

### Why the first line's baseline_offset

We use the first line's `baseline_offset` for the selection renderer because:
- All lines in a single-font layout have the same baseline_offset
- It provides the correct transformation from layout coordinates to screen coordinates
- It ensures selection highlights align with text regardless of line position

## Summary

The fix ensures:
- ✅ **Consistent coordinate system** across text, selection, and caret rendering
- ✅ **Proper line height** using TextLayout's calculated metrics
- ✅ **Smooth vertical navigation** with visible caret
- ✅ **Aligned selection highlights** that match text position
- ✅ **Correct scrolling behavior** for multi-line content

The TextArea now provides a professional multi-line editing experience with proper line spacing and caret behavior!
