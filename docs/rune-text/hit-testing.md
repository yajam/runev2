# Hit Testing & Positioning (Phase 6.2)

Implementation of hit testing and position mapping with zone-local coordinate support in rune-text.

## Overview

The hit testing system provides bidirectional mapping between:
- **Point → Offset**: Click/touch position to character offset (hit testing)
- **Offset → Position**: Character offset to visual position (cursor positioning)

All coordinates use **zone-local space** (relative to layout origin), ensuring the system works correctly regardless of where the text is positioned on screen.

## Zone-Local Coordinates

Zone-local coordinates are relative to the top-left corner of the text layout (0, 0), not absolute screen coordinates.

### Why Zone-Local?

1. **Decoupling**: Text layout doesn't need to know its screen position
2. **Reusability**: Same layout can be rendered at different positions
3. **Simplicity**: Caller handles coordinate transformation
4. **Correctness**: Prevents coordinate system bugs

### Usage Pattern

```rust
// User clicks at screen position (screen_x, screen_y)
// Text layout is at screen position (layout_x, layout_y)

// Convert to zone-local coordinates
let local_x = screen_x - layout_x;
let local_y = screen_y - layout_y;
let point = Point::new(local_x, local_y);

// Hit test in zone-local space
let result = layout.hit_test(point, HitTestPolicy::Clamp);
```

## Components

### `Point`

Represents a 2D point in zone-local coordinates.

```rust
pub struct Point {
    pub x: f32,  // Relative to layout origin
    pub y: f32,  // Relative to layout origin
}
```

**Methods:**
- `new(x, y)` - Create a point
- `zero()` - Create point at origin

### `Position`

Represents a visual position with line information.

```rust
pub struct Position {
    pub x: f32,          // Relative to layout origin
    pub y: f32,          // Relative to layout origin
    pub line_index: usize,
}
```

**Methods:**
- `new(x, y, line_index)` - Create a position
- `to_point()` - Convert to point (discarding line index)

### `HitTestResult`

Result of a hit test operation.

```rust
pub struct HitTestResult {
    pub byte_offset: usize,
    pub affinity: CursorAffinity,
    pub line_index: usize,
}
```

**Methods:**
- `new(byte_offset, affinity, line_index)` - Create result
- `to_cursor_position()` - Convert to cursor position

### `HitTestPolicy`

Controls behavior for out-of-bounds points.

```rust
pub enum HitTestPolicy {
    Clamp,  // Clamp to nearest valid position (default)
    Strict, // Return None if out of bounds
}
```

## TextLayout Integration

### Hit Testing (Point → Offset)

```rust
// Hit test with clamping (recommended for mouse clicks)
let point = Point::new(50.0, 20.0);
if let Some(result) = layout.hit_test(point, HitTestPolicy::Clamp) {
    println!("Clicked at byte offset: {}", result.byte_offset);
    println!("Line: {}", result.line_index);
}

// Hit test with strict bounds checking
if let Some(result) = layout.hit_test(point, HitTestPolicy::Strict) {
    // Point is within text bounds
} else {
    // Point is outside text bounds
}
```

### Position Mapping (Offset → Position)

```rust
// Get cursor position
let offset = 10;
if let Some(pos) = layout.offset_to_position(offset) {
    println!("Cursor at ({}, {})", pos.x, pos.y);
    println!("Line: {}", pos.line_index);
}

// Get baseline position (for IME candidate window)
if let Some(pos) = layout.offset_to_baseline_position(offset) {
    // Position cursor at baseline
}
```

## Hit Testing Behavior

### Line Boundaries

The system correctly handles clicks at line boundaries:

```rust
// Click before first line → clamped to start of first line
// Click after last line → clamped to end of last line
// Click between lines → snapped to nearest line
```

### Horizontal Boundaries

```rust
// Click before line start → clamped to line start
// Click after line end → clamped to line end
// Click within line → nearest grapheme cluster
```

### Grapheme Cluster Snapping

Hit testing snaps to grapheme cluster boundaries:

```rust
let text = "Hello 👨‍👩‍👧‍👦 World";
let layout = TextLayout::new(text, &font, 16.0);

// Click in middle of emoji
let point = Point::new(35.0, 0.0);
let result = layout.hit_test(point, HitTestPolicy::Clamp).unwrap();

// result.byte_offset will be at a valid grapheme boundary
// (either before or after the emoji, never in the middle)
```

## Usage Examples

### Mouse Click Handling

```rust
fn handle_mouse_click(layout: &TextLayout, screen_x: f32, screen_y: f32, 
                      layout_x: f32, layout_y: f32) {
    // Convert to zone-local coordinates
    let point = Point::new(screen_x - layout_x, screen_y - layout_y);
    
    // Hit test
    if let Some(result) = layout.hit_test(point, HitTestPolicy::Clamp) {
        // Move cursor to clicked position
        let cursor = Cursor::at_position(result.to_cursor_position());
        // ... update editor state
    }
}
```

### Text Selection

```rust
fn handle_mouse_drag(layout: &TextLayout, 
                     start_screen: (f32, f32),
                     end_screen: (f32, f32),
                     layout_pos: (f32, f32)) {
    let start_point = Point::new(
        start_screen.0 - layout_pos.0,
        start_screen.1 - layout_pos.1
    );
    let end_point = Point::new(
        end_screen.0 - layout_pos.0,
        end_screen.1 - layout_pos.1
    );
    
    let start = layout.hit_test(start_point, HitTestPolicy::Clamp)?;
    let end = layout.hit_test(end_point, HitTestPolicy::Clamp)?;
    
    // Create selection from start to end
    let selection = Selection::new(start.byte_offset, end.byte_offset);
}
```

### IME Candidate Window Positioning

```rust
fn position_ime_window(layout: &TextLayout, cursor_offset: usize,
                       layout_screen_pos: (f32, f32)) -> (f32, f32) {
    if let Some(pos) = layout.offset_to_baseline_position(cursor_offset) {
        // Convert zone-local to screen coordinates
        let screen_x = layout_screen_pos.0 + pos.x;
        let screen_y = layout_screen_pos.1 + pos.y;
        (screen_x, screen_y)
    } else {
        layout_screen_pos
    }
}
```

### Round-Trip Validation

```rust
// Verify offset → position → hit test → offset
let original_offset = 10;
let pos = layout.offset_to_position(original_offset).unwrap();
let point = pos.to_point();
let result = layout.hit_test(point, HitTestPolicy::Clamp).unwrap();
assert_eq!(result.byte_offset, original_offset);
```

## Implementation Details

### Current Approach

The implementation uses an approximation for X position calculation:
- Distributes run width evenly across grapheme clusters
- Simple and fast for most use cases
- Works well for monospaced and regular proportional fonts

### Ligature Support

The implementation uses HarfBuzz cluster information for accurate ligature handling:

```rust
// Text with ligatures (e.g., "fi" → single glyph in many fonts)
let text = "office";
let layout = TextLayout::new(text, &font, 16.0);

// Hit testing correctly handles ligature boundaries
let point = Point::new(15.0, 0.0); // Middle of "ffi" ligature
let result = layout.hit_test(point, HitTestPolicy::Clamp).unwrap();

// Result snaps to ligature boundary (not in the middle)
```

**How it works:**
- HarfBuzz provides cluster mapping (glyph → character offset)
- Multiple glyphs can map to same cluster (ligatures)
- Hit testing finds cluster boundaries, not individual glyphs
- Cursor positions snap to cluster start/end

### BiDi Support

The implementation handles bidirectional text correctly:

```rust
// Mixed LTR/RTL text
let text = "Hello שלום World";
let layout = TextLayout::new(text, &font, 16.0);

// Hit testing respects visual order
let point = Point::new(50.0, 0.0);
let result = layout.hit_test(point, HitTestPolicy::Clamp).unwrap();

// Result considers RTL run direction
```

**BiDi features:**
- Visual order hit testing (runs already reordered)
- RTL run coordinate reversal
- Visual start/end offset detection
- Cursor affinity at BiDi boundaries

### Limitations

1. **Complex Scripts**: May need additional tuning for complex shaping
2. **Performance**: O(n) line lookup (could use binary search)
3. **BiDi Affinity**: Basic implementation, could be more sophisticated

### Future Enhancements

- Binary search for line lookup (O(log n))
- More sophisticated BiDi affinity handling
- Improved accuracy for complex scripts
- Spatial indexing for very large documents

## Testing

Run the hit testing example:

```bash
cargo run --package rune-text --example hit_test_demo
```

Run unit tests:

```bash
cargo test --package rune-text hit_test
```

## Performance Considerations

### Hit Testing Complexity

- **Line lookup**: O(n) where n = number of lines (could be optimized to O(log n))
- **Run lookup**: O(m) where m = runs per line (typically small)
- **Cluster lookup**: O(k) where k = clusters per run (typically small)

For typical documents, hit testing is very fast. For very large documents with many lines, consider:
- Binary search for line lookup
- Spatial indexing for large layouts

### Position Mapping Complexity

- **Line lookup**: O(n) where n = number of lines
- **X calculation**: O(k) where k = clusters in run

Similar performance characteristics to hit testing.

## Related Phases

- **Phase 6.1**: Cursor Management (prerequisite)
- **Phase 6.3**: Cursor Movement (uses hit testing)
- **Phase 6.4**: Selection Management (uses hit testing)
- **Phase 8**: IME Support (uses baseline positioning)
