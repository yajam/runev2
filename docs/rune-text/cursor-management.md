# Cursor Management (Phase 6.1)

Implementation of cursor management for text editing support in rune-text.

## Overview

The cursor management system provides:
- Cursor position tracking at byte offsets
- Blinking animation support
- Visibility toggling
- Grapheme cluster boundary handling
- Cursor affinity (upstream/downstream)
- Visual cursor rectangle calculation

## Components

### `CursorPosition`

Represents a cursor position in text with byte offset and affinity.

```rust
pub struct CursorPosition {
    pub byte_offset: usize,
    pub affinity: CursorAffinity,
}
```

**Key Methods:**
- `new(byte_offset)` - Create cursor at position
- `with_affinity(byte_offset, affinity)` - Create with explicit affinity
- `snap_to_grapheme_boundary(text)` - Ensure cursor is at valid grapheme boundary

### `CursorAffinity`

Determines which side of a character the cursor is on.

```rust
pub enum CursorAffinity {
    Upstream,   // Left/before character
    Downstream, // Right/after character (default)
}
```

This is important for:
- BiDi text rendering
- Line boundary positions
- Ambiguous cursor positions

### `Cursor`

Manages cursor state including position, visibility, and blinking animation.

```rust
pub struct Cursor {
    position: CursorPosition,
    visible: bool,
    blink_time: f32,
    blink_interval: f32,
}
```

**Key Methods:**
- `new()` - Create cursor at start (position 0)
- `at_position(position)` - Create at specific position
- `set_position(position)` - Move cursor (resets blink)
- `is_visible()` - Check visibility
- `set_visible(visible)` - Set visibility
- `toggle_visibility()` - Toggle visibility
- `update_blink(delta_time)` - Update blink animation
- `snap_to_grapheme_boundary(text)` - Ensure valid position

### `CursorRect`

Visual rectangle for rendering a cursor.

```rust
pub struct CursorRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,  // Typically 1-2px
    pub height: f32, // Typically line height
}
```

## TextLayout Integration

The `TextLayout` type provides cursor-related methods:

### Cursor Creation
```rust
let cursor_start = layout.cursor_at_start();
let cursor_end = layout.cursor_at_end();
```

### Cursor Rectangle Calculation
```rust
// Get visual rectangle for rendering
if let Some(rect) = layout.cursor_rect(&cursor) {
    // Render cursor at (rect.x, rect.y) with size (rect.width, rect.height)
}

// Or use a specific position
let pos = CursorPosition::new(10);
if let Some(rect) = layout.cursor_rect_at_position(pos) {
    // ...
}
```

### Grapheme Boundary Snapping
```rust
// Ensure cursor is at a valid grapheme boundary
let snapped = layout.snap_cursor_to_boundary(position);
```

## Usage Example

```rust
use rune_text::{Cursor, CursorPosition, FontCache};
use rune_text::layout::TextLayout;

// Load font and create layout
let mut font_cache = FontCache::new();
let font = font_cache.get_or_load("path/to/font.ttf", 0)?;
let layout = TextLayout::new("Hello, World!", &font, 16.0);

// Create and position cursor
let mut cursor = layout.cursor_at_start();
cursor.set_byte_offset(7); // After "Hello, "

// Get cursor rectangle for rendering
if let Some(rect) = layout.cursor_rect(&cursor) {
    println!("Cursor at ({}, {})", rect.x, rect.y);
}

// Update blink animation (in render loop)
cursor.update_blink(delta_time);
if cursor.is_visible() {
    // Render cursor
}
```

## Grapheme Cluster Handling

The cursor system ensures positions are always at grapheme cluster boundaries:

```rust
let text = "Hello 👨‍👩‍👧‍👦 World";
let layout = TextLayout::new(text, &font, 16.0);

// Try to position in middle of emoji
let pos = CursorPosition::new(10);
let snapped = layout.snap_cursor_to_boundary(pos);

// snapped.byte_offset will be at a valid boundary
```

This prevents:
- Splitting multi-byte characters (UTF-8)
- Breaking combining mark sequences
- Splitting emoji with ZWJ sequences
- Invalid cursor positions

## Blink Animation

The cursor supports automatic blinking:

```rust
let mut cursor = Cursor::new();
cursor.set_blink_interval(0.5); // 0.5 seconds

// In your render loop
cursor.update_blink(delta_time);
if cursor.is_visible() {
    // Render the cursor
}
```

The cursor automatically:
- Resets to visible when moved
- Toggles visibility at the blink interval
- Can be manually controlled with `set_visible()` and `toggle_visibility()`

## Implementation Notes

### Current Limitations

1. **X Position Calculation**: The current implementation approximates cursor X position by distributing run width evenly across grapheme clusters. A more accurate implementation would track cluster-to-glyph mapping from the shaping engine.

2. **Multiple Cursors**: The optional multiple cursor support is not yet implemented.

3. **BiDi Cursor Movement**: While cursor affinity is supported, BiDi-aware cursor movement (Phase 6.3) is not yet implemented.

### Future Enhancements

- Accurate X position using cluster-to-glyph mapping
- Multiple cursor support
- BiDi-aware cursor positioning
- Cursor movement operations (left/right/up/down)
- Hit testing (point to cursor position)

## Testing

The cursor module includes comprehensive unit tests:

```bash
cargo test --package rune-text cursor
```

Run the example to see cursor functionality:

```bash
cargo run --package rune-text --example cursor_demo
```

## Related Phases

- **Phase 6.2**: Hit Testing & Positioning (next)
- **Phase 6.3**: Cursor Movement
- **Phase 6.4**: Selection Management
