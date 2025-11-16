# Cursor Movement Implementation (Phase 6.3)

## Overview

The cursor movement system provides comprehensive navigation capabilities for text editing, supporting character-by-character, word-by-word, and line-by-line movement. All movement operations respect grapheme cluster boundaries, handle BiDi text correctly, and work seamlessly with ligatures and combining marks.

## Features Implemented

### 1. Character Movement (Grapheme Cluster)

**Left/Right by Character**
- Moves cursor by one grapheme cluster at a time
- Correctly handles multi-byte UTF-8 characters (e.g., Chinese, Arabic)
- Treats emoji ZWJ sequences as single units (e.g., 👨‍👩‍👧‍👦)
- Respects combining marks (e.g., é as e + combining acute)

**API:**
```rust
pub fn move_cursor_left(&self, byte_offset: usize) -> usize
pub fn move_cursor_right(&self, byte_offset: usize) -> usize
```

**Example:**
```rust
let layout = TextLayout::new("Hello 世界! 👨‍👩‍👧‍👦", &font, 16.0);
let offset = 0;
let next = layout.move_cursor_right(offset); // Moves to byte 1 (after 'H')
```

### 2. Word Movement

**Left/Right by Word Boundary**
- Uses Unicode word segmentation (UAX-29) via `unicode-segmentation`
- Moves to the start of the previous/next word
- Handles punctuation and whitespace correctly
- Works with multilingual text

**API:**
```rust
pub fn move_cursor_left_word(&self, byte_offset: usize) -> usize
pub fn move_cursor_right_word(&self, byte_offset: usize) -> usize
```

**Example:**
```rust
let layout = TextLayout::new("The quick brown fox", &font, 16.0);
let offset = 10; // Middle of "brown"
let prev = layout.move_cursor_left_word(offset); // Moves to start of "brown"
let next = layout.move_cursor_right_word(offset); // Moves to end of "brown"
```

### 3. Line Movement

**Up/Down by Line**
- Maintains horizontal position (column) when moving vertically
- Returns both the new offset and the preferred X position
- Handles lines of different lengths gracefully
- Works correctly with wrapped text

**API:**
```rust
pub fn move_cursor_up(&self, byte_offset: usize, preferred_x: Option<f32>) -> (usize, f32)
pub fn move_cursor_down(&self, byte_offset: usize, preferred_x: Option<f32>) -> (usize, f32)
```

**Example:**
```rust
let layout = TextLayout::with_wrap(text, &font, 16.0, Some(200.0), WrapMode::BreakWord);
let offset = 50; // Some position in the text
let (new_offset, x) = layout.move_cursor_down(offset, None);
// Move down again, maintaining column
let (next_offset, _) = layout.move_cursor_down(new_offset, Some(x));
```

**Column Preservation:**
The `preferred_x` parameter allows maintaining the visual column when moving up/down through lines of varying lengths. This matches standard text editor behavior.

### 4. Line Start/End Movement

**Home/End**
- Moves to the visual start/end of the current line
- Handles BiDi text correctly (visual vs logical order)
- Works with wrapped lines

**API:**
```rust
pub fn move_cursor_line_start(&self, byte_offset: usize) -> usize
pub fn move_cursor_line_end(&self, byte_offset: usize) -> usize
```

**Example:**
```rust
let layout = TextLayout::new("Line 1\nLine 2\nLine 3", &font, 16.0);
let offset = 10; // Middle of Line 2
let start = layout.move_cursor_line_start(offset); // Start of Line 2
let end = layout.move_cursor_line_end(offset); // End of Line 2
```

### 5. Document Start/End Movement

**Ctrl+Home/Ctrl+End**
- Moves to the very beginning or end of the document
- Simple and efficient

**API:**
```rust
pub fn move_cursor_document_start(&self) -> usize
pub fn move_cursor_document_end(&self) -> usize
```

**Example:**
```rust
let layout = TextLayout::new("Long document...", &font, 16.0);
let start = layout.move_cursor_document_start(); // Returns 0
let end = layout.move_cursor_document_end(); // Returns text.len()
```

## BiDi Support

All cursor movement operations handle bidirectional text correctly:

- **Character movement**: Follows logical order (byte order in the string)
- **Line start/end**: Uses visual order (leftmost/rightmost position on screen)
- **Word movement**: Follows logical order
- **Up/down movement**: Maintains visual column position

**Example with BiDi text:**
```rust
let layout = TextLayout::new("Hello مرحبا World", &font, 16.0);
// Character movement follows byte order
// Line start/end respects visual rendering
```

## Ligature Handling

Cursor movement correctly handles ligatures (e.g., "fi", "fl" in some fonts):

- Ligatures are treated as indivisible units
- Cursor positions snap to cluster boundaries
- Hit testing accounts for ligature width

This is handled automatically by the cluster-based positioning system inherited from HarfBuzz shaping.

## Combining Marks

Combining marks (diacritics) are handled correctly:

- Base character + combining marks = one grapheme cluster
- Cursor never stops in the middle of a combining sequence
- Examples: é (e + ◌́), ñ (n + ◌̃)

## Implementation Details

### Module Structure

```
layout/
├── cursor_movement.rs    # Core movement logic
├── cursor.rs            # Cursor state management
└── text_layout.rs       # Integration with layout
```

### Key Components

**CursorMovement** (in `cursor_movement.rs`):
- Static helper functions for movement operations
- Pure functions that operate on text and offsets
- No state, easily testable

**TextLayout methods** (in `text_layout.rs`):
- Public API that integrates movement with layout
- Uses line information for vertical movement
- Handles BiDi reordering for line start/end

### Dependencies

- `unicode-segmentation`: Grapheme cluster and word boundary detection
- `unicode-linebreak`: Line break opportunities (used for word wrapping)
- Existing hit testing infrastructure for up/down movement

## Testing

Comprehensive tests cover:

1. **Character movement**: ASCII, multi-byte, emoji, combining marks
2. **Word movement**: Multiple words, punctuation, multilingual
3. **Line movement**: Wrapped text, column preservation
4. **BiDi text**: Mixed LTR/RTL content
5. **Edge cases**: Empty text, single character, document boundaries

Run tests:
```bash
cargo test --lib cursor_movement
```

Run the demo:
```bash
cargo run --example cursor_movement_demo
```

## Usage Example

```rust
use rune_text::font::FontFace;
use rune_text::layout::{Cursor, TextLayout, WrapMode};

// Load font and create layout
let font = FontFace::from_path("font.ttf", 0)?;
let layout = TextLayout::with_wrap(
    "Multi-line text with wrapping",
    &font,
    16.0,
    Some(200.0),
    WrapMode::BreakWord
);

// Create cursor
let mut cursor = Cursor::new();

// Move right by character
let new_offset = layout.move_cursor_right(cursor.byte_offset());
cursor.set_byte_offset(new_offset);

// Move right by word
let new_offset = layout.move_cursor_right_word(cursor.byte_offset());
cursor.set_byte_offset(new_offset);

// Move down, maintaining column
let (new_offset, x) = layout.move_cursor_down(cursor.byte_offset(), None);
cursor.set_byte_offset(new_offset);

// Move down again, keeping same column
let (new_offset, _) = layout.move_cursor_down(cursor.byte_offset(), Some(x));
cursor.set_byte_offset(new_offset);

// Jump to line start
let new_offset = layout.move_cursor_line_start(cursor.byte_offset());
cursor.set_byte_offset(new_offset);

// Jump to document end
let new_offset = layout.move_cursor_document_end();
cursor.set_byte_offset(new_offset);
```

## Performance Considerations

- **Character movement**: O(n) where n is the number of grapheme clusters (typically very fast)
- **Word movement**: O(w) where w is the number of words (computed on demand)
- **Line movement**: O(1) with hit testing (uses existing infrastructure)
- **Line start/end**: O(1) lookup in line array
- **Document start/end**: O(1)

All operations are efficient enough for real-time interactive use.

## Future Enhancements

Potential improvements for future phases:

1. **Visual cursor movement in BiDi**: Option to move visually left/right instead of logically
2. **Paragraph boundaries**: Move by paragraph (Ctrl+Up/Down)
3. **Subword movement**: CamelCase and snake_case aware movement
4. **Custom word boundaries**: Configurable word segmentation rules
5. **Movement history**: Track cursor positions for undo/redo

## Related Phases

- **Phase 6.1**: Cursor Management (position tracking, rendering)
- **Phase 6.2**: Hit Testing & Positioning (point-to-offset conversion)
- **Phase 6.4**: Selection Management (will use movement for selection extension)
- **Phase 6.5-6.6**: Text Insertion/Deletion (will use movement for navigation)

## References

- [Unicode Text Segmentation (UAX-29)](https://www.unicode.org/reports/tr29/)
- [Unicode Line Breaking (UAX-14)](https://www.unicode.org/reports/tr14/)
- [Unicode Bidirectional Algorithm (UAX-9)](https://www.unicode.org/reports/tr9/)
