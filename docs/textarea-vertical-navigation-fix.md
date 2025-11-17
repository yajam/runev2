# TextArea Vertical Navigation Fix

## Issues Fixed

1. **✅ Down arrow inconsistency**: Down arrow now works consistently to move to the next line
2. **✅ Preferred X position**: Cursor maintains horizontal position when moving vertically

## Root Cause

The `move_cursor_up` and `move_cursor_down` methods were passing `None` for the `preferred_x` parameter, causing the TextLayout to recalculate the X position on each vertical movement. This led to:
- Cursor "drifting" horizontally when moving vertically
- Inconsistent behavior when lines have different lengths
- Difficulty navigating through short lines

## The Fix

### 1. Added `preferred_x` Field

Added a new field to track the preferred horizontal position:

```rust
pub struct TextArea {
    // ... existing fields ...
    
    /// Preferred X position for vertical navigation
    preferred_x: Option<f32>,
}
```

### 2. Updated Vertical Navigation

Modified `move_cursor_up` and `move_cursor_down` to maintain preferred X:

**Before:**
```rust
pub fn move_cursor_down(&mut self) {
    let (new_pos, _) = layout.move_cursor_down(pos, None);  // ❌ Loses X position
    self.cursor_position = new_pos;
}
```

**After:**
```rust
pub fn move_cursor_down(&mut self) {
    let (new_pos, new_x) = layout.move_cursor_down(pos, self.preferred_x);  // ✅ Maintains X
    self.cursor_position = new_pos;
    self.preferred_x = Some(new_x);  // ✅ Save for next vertical move
}
```

### 3. Reset on Horizontal Movement

Reset `preferred_x` when moving horizontally to establish a new preferred position:

```rust
pub fn move_cursor_left(&mut self) {
    // ... movement logic ...
    self.preferred_x = None;  // ✅ Reset for new horizontal position
}
```

This is reset for:
- `move_cursor_left()` / `move_cursor_right()`
- `move_cursor_left_word()` / `move_cursor_right_word()`
- `move_cursor_line_start()` / `move_cursor_line_end()`
- `move_cursor_to_document_start()` / `move_cursor_to_document_end()`

## How It Works

### Preferred X Position Behavior

```
Line 1: "This is a long line of text"
                    ↑ cursor at X=150
        Press Down ↓
        
Line 2: "Short"
                ↑ cursor tries to stay at X=150, lands at end (X=50)
        Press Down ↓
        
Line 3: "Another long line here"
                    ↑ cursor returns to X=150 (preferred position)
```

### State Machine

```
Initial State: preferred_x = None

Vertical Movement (Up/Down):
  - Use preferred_x if available
  - Calculate new position
  - Save new X as preferred_x
  
Horizontal Movement (Left/Right/Home/End):
  - Move cursor
  - Reset preferred_x = None
  - Next vertical move will use new position
```

## About Line Spacing

### Current Behavior

The TextLayout uses the font's native metrics:
```rust
line_height = ascent + descent + line_gap
```

This includes the font's designed `line_gap` (leading), which can be quite large for some fonts. This is **intentional** and correct for:
- Maintaining proper typography
- Ensuring cursor positioning works correctly
- Keeping hit testing accurate

### Why We Don't Modify Line Height

Modifying line positions would break:
- ✅ `cursor_rect_at_position()` - relies on line.y_offset
- ✅ `hit_test()` - relies on line box positions
- ✅ `move_cursor_up/down()` - relies on line spacing
- ✅ Selection rectangles - rely on line coordinates

### Alternative Approaches (Not Implemented)

If tighter line spacing is desired in the future, options include:

1. **Custom TextLayout**: Fork rune-text to support custom line height multipliers
2. **Font Selection**: Use fonts with smaller native line_gap
3. **Post-processing**: Adjust line positions and update all coordinate calculations
4. **CSS-style line-height**: Add a line-height property that scales the font metrics

For now, we use the font's native metrics to ensure all text operations work correctly.

## Testing

### Test Cases

1. **Vertical Navigation**:
   ```
   Type: "Line 1\nShort\nLine 3 is longer"
   - Place cursor at end of "Line 1"
   - Press Down → should go to end of "Short"
   - Press Down → should go to similar X position in "Line 3"
   - Press Up → should return to end of "Short"
   - Press Up → should return to end of "Line 1"
   ```

2. **Horizontal Reset**:
   ```
   - Navigate vertically to establish preferred_x
   - Press Left or Right
   - Press Up/Down → should use new X position
   ```

3. **Line Start/End**:
   ```
   - Press Home → cursor to line start
   - Press Down → should go to start of next line (not previous X)
   - Press End → cursor to line end
   - Press Down → should go to end of next line
   ```

### Run the Demo

```bash
cargo run
```

Then:
1. Click the TextArea
2. Type several lines with varying lengths
3. Use Up/Down arrows - cursor should maintain horizontal position
4. Navigate through short lines - cursor should return to preferred X on longer lines
5. Use Home/End, then Up/Down - should use new position

## Summary

The fix ensures:
- ✅ **Consistent down arrow behavior** - always moves to next line
- ✅ **Maintained horizontal position** - cursor stays in same column when possible
- ✅ **Proper reset behavior** - horizontal movement establishes new preferred position
- ✅ **Professional navigation** - matches behavior of VS Code, Sublime Text, etc.

The TextArea now provides industry-standard vertical navigation! 🎯
