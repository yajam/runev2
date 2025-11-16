# Selection Management Implementation (Phase 6.4)

## Overview

The selection management system provides comprehensive text selection capabilities for editing, including range selection, multi-line selection, word/line/paragraph selection, and selection extension with keyboard navigation. All selection operations work correctly with BiDi text, ligatures, and complex Unicode.

## Features Implemented

### 1. Selection Data Structure

**Selection Type**
- Tracks anchor (start point) and active (current end) positions
- Supports both forward and backward selections
- Maintains selection direction for proper extension behavior
- Provides logical range (start..end) regardless of direction

**API:**
```rust
pub struct Selection {
    anchor: usize,  // Where selection started
    active: usize,  // Current end of selection
}
```

**Key Methods:**
```rust
// Creation
Selection::new(anchor, active)
Selection::collapsed(offset)

// Queries
selection.range() -> Range<usize>
selection.is_collapsed() -> bool
selection.is_forward() -> bool
selection.text(source) -> &str

// Modification
selection.extend_to(offset)
selection.collapse_to_start()
selection.collapse_to_end()
```

### 2. Selection Rendering

**Selection Rectangles**
- Calculates visual rectangles for rendering selection highlight
- Handles multi-line selections (one rectangle per line)
- Works with wrapped text
- Accounts for BiDi text reordering

**API:**
```rust
pub fn selection_rects(&self, selection: &Selection) -> Vec<SelectionRect>
```

**Example:**
```rust
let layout = TextLayout::with_wrap(text, &font, 16.0, Some(200.0), WrapMode::BreakWord);
let selection = Selection::new(10, 50);
let rects = layout.selection_rects(&selection);

// Render each rectangle with highlight color
for rect in rects {
    draw_rect(rect.x, rect.y, rect.width, rect.height, HIGHLIGHT_COLOR);
}
```

### 3. Selection Extension (Shift+Movement)

**Extending with Movement**
- Anchor stays fixed, active end moves
- Works with all cursor movement operations
- Supports both horizontal and vertical movement
- Maintains column position for vertical movement

**API:**
```rust
// Horizontal extension
pub fn extend_selection<F>(&self, selection: &Selection, move_fn: F) -> Selection

// Vertical extension (with column tracking)
pub fn extend_selection_vertical<F>(
    &self,
    selection: &Selection,
    move_fn: F,
    preferred_x: Option<f32>,
) -> (Selection, f32)
```

**Example:**
```rust
let mut sel = Selection::collapsed(10);

// Shift+Right (extend by character)
sel = layout.extend_selection(&sel, |offset| layout.move_cursor_right(offset));

// Shift+Ctrl+Right (extend by word)
sel = layout.extend_selection(&sel, |offset| layout.move_cursor_right_word(offset));

// Shift+Down (extend down one line)
let (new_sel, x) = layout.extend_selection_vertical(
    &sel,
    |offset, x| layout.move_cursor_down(offset, x),
    None,
);
sel = new_sel;
```

### 4. Word Selection (Double-Click)

**Select Word at Position**
- Selects the entire word containing the given offset
- Uses Unicode word segmentation (UAX-29)
- Handles punctuation and whitespace
- Works with multilingual text

**API:**
```rust
pub fn select_word_at(&self, byte_offset: usize) -> Selection
```

**Example:**
```rust
let layout = TextLayout::new("The quick brown fox", &font, 16.0);

// Simulate double-click at position 10 (in "brown")
let selection = layout.select_word_at(10);
println!("Selected: {}", selection.text(text)); // "brown"
```

### 5. Line Selection (Triple-Click)

**Select Line at Position**
- Selects the entire line containing the given offset
- Works with multi-line text
- Handles wrapped lines correctly

**API:**
```rust
pub fn select_line_at(&self, byte_offset: usize) -> Selection
```

**Example:**
```rust
let layout = TextLayout::new("Line 1\nLine 2\nLine 3", &font, 16.0);

// Simulate triple-click at position 10 (in "Line 2")
let selection = layout.select_line_at(10);
println!("Selected: {}", selection.text(text)); // "Line 2"
```

### 6. Paragraph Selection

**Select Paragraph at Position**
- Selects text between newlines
- Useful for Ctrl+A-like operations on paragraphs
- Works with multi-paragraph documents

**API:**
```rust
pub fn select_paragraph_at(&self, byte_offset: usize) -> Selection
```

**Example:**
```rust
let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
let layout = TextLayout::new(text, &font, 16.0);

let selection = layout.select_paragraph_at(20);
println!("Selected: {}", selection.text(text)); // "Second paragraph."
```

### 7. Multi-Line Selection

**Spanning Multiple Lines**
- Selections can span any number of lines
- Selection rectangles calculated per line
- Handles different line lengths
- Works with wrapped text

**Example:**
```rust
let text = "Line 1\nLine 2\nLine 3";
let layout = TextLayout::new(text, &font, 16.0);

// Select from middle of Line 1 to middle of Line 3
let selection = Selection::new(3, 17);
let rects = layout.selection_rects(&selection);

println!("Selection spans {} lines", rects.len());
```

### 8. BiDi Selection Handling

**Bidirectional Text**
- Selection range always in logical order (byte order)
- Rendering accounts for visual reordering
- Selection rectangles follow visual layout
- Works seamlessly with mixed LTR/RTL text

**Example:**
```rust
let layout = TextLayout::new("Hello مرحبا World", &font, 16.0);
let selection = Selection::new(0, 10);

// Range is logical (byte order)
println!("Range: {:?}", selection.range());

// Rectangles follow visual rendering
let rects = layout.selection_rects(&selection);
```

### 9. Selection Boundary Snapping

**Grapheme Cluster Boundaries**
- Ensures selections align to grapheme boundaries
- Prevents invalid UTF-8 ranges
- Handles multi-byte characters, emoji, combining marks

**API:**
```rust
pub fn snap_selection_to_boundaries(&self, selection: &Selection) -> Selection
```

**Example:**
```rust
let text = "Hello 世界";
let layout = TextLayout::new(text, &font, 16.0);

// Try to select in middle of multi-byte character
let sel = Selection::new(0, 8); // 8 is inside "世"
let snapped = layout.snap_selection_to_boundaries(&sel);

println!("Original: {:?}", sel.range());    // 0..8
println!("Snapped: {:?}", snapped.range()); // 0..6 (before "世")
```

## Implementation Details

### Module Structure

```
layout/
├── selection.rs         # Selection data structure
└── text_layout.rs       # Selection methods integration
```

### Key Components

**Selection** (in `selection.rs`):
- Core selection data structure
- Anchor/active tracking for proper extension
- Range calculation and queries
- Comprehensive unit tests

**TextLayout methods** (in `text_layout.rs`):
- `selection_rects()` - Calculate rendering rectangles
- `select_word_at()` - Word selection
- `select_line_at()` - Line selection
- `select_paragraph_at()` - Paragraph selection
- `extend_selection()` - Extend with movement
- `snap_selection_to_boundaries()` - Boundary validation

### Selection Direction

The selection maintains both anchor and active positions to support proper extension behavior:

- **Forward selection**: anchor < active (selecting to the right/down)
- **Backward selection**: anchor > active (selecting to the left/up)
- **Range**: Always returns start..end where start <= end

This allows extending the selection in either direction while maintaining the original anchor point.

### Dependencies

- `unicode-segmentation`: Word boundary detection
- Existing cursor movement infrastructure (Phase 6.3)
- Line layout system (Phase 2)
- BiDi reordering (Phase 3)

## Testing

Comprehensive tests cover:

1. **Basic selection**: Creation, range, direction
2. **Selection extension**: Forward, backward, multi-line
3. **Word selection**: Various word boundaries
4. **Line selection**: Multi-line documents
5. **Paragraph selection**: Multi-paragraph text
6. **Multi-line rendering**: Selection rectangles
7. **BiDi selection**: Mixed LTR/RTL text
8. **Emoji selection**: ZWJ sequences
9. **Boundary snapping**: Multi-byte characters

Run tests:
```bash
cargo test --lib selection
```

Run the demo:
```bash
cargo run --example selection_demo
```

## Usage Examples

### Basic Selection

```rust
use rune_text::font::FontFace;
use rune_text::layout::{Selection, TextLayout};

let font = FontFace::from_path("font.ttf", 0)?;
let layout = TextLayout::new("Hello, World!", &font, 16.0);

// Create selection
let selection = Selection::new(0, 5);
println!("Selected: {}", selection.text("Hello, World!")); // "Hello"

// Get rendering rectangles
let rects = layout.selection_rects(&selection);
for rect in rects {
    // Draw selection highlight
    draw_rect(rect.x, rect.y, rect.width, rect.height, BLUE);
}
```

### Selection Extension

```rust
let mut sel = Selection::collapsed(0);

// Extend right by word
sel = layout.extend_selection(&sel, |offset| {
    layout.move_cursor_right_word(offset)
});

// Extend down one line
let (new_sel, x) = layout.extend_selection_vertical(
    &sel,
    |offset, x| layout.move_cursor_down(offset, x),
    None,
);
sel = new_sel;
```

### Word/Line Selection

```rust
// Double-click word selection
fn on_double_click(layout: &TextLayout, click_pos: Point) {
    if let Some(hit) = layout.hit_test(click_pos, HitTestPolicy::Clamp) {
        let selection = layout.select_word_at(hit.byte_offset);
        // Update UI with new selection
    }
}

// Triple-click line selection
fn on_triple_click(layout: &TextLayout, click_pos: Point) {
    if let Some(hit) = layout.hit_test(click_pos, HitTestPolicy::Clamp) {
        let selection = layout.select_line_at(hit.byte_offset);
        // Update UI with new selection
    }
}
```

### Keyboard Selection Extension

```rust
fn handle_key(layout: &TextLayout, selection: &mut Selection, key: Key, shift: bool) {
    if shift {
        // Extend selection
        match key {
            Key::Right => {
                *selection = layout.extend_selection(selection, |offset| {
                    layout.move_cursor_right(offset)
                });
            }
            Key::End => {
                *selection = layout.extend_selection(selection, |offset| {
                    layout.move_cursor_line_end(offset)
                });
            }
            // ... other keys
        }
    } else {
        // Move cursor (collapse selection)
        let new_offset = match key {
            Key::Right => layout.move_cursor_right(selection.active()),
            Key::End => layout.move_cursor_line_end(selection.active()),
            // ... other keys
        };
        *selection = Selection::collapsed(new_offset);
    }
}
```

## Performance Considerations

- **Selection creation**: O(1)
- **Range calculation**: O(1)
- **Rectangle calculation**: O(L) where L = number of lines spanned
- **Word selection**: O(W) where W = number of words
- **Line selection**: O(1) with line lookup
- **Paragraph selection**: O(P) where P = paragraph length
- **Boundary snapping**: O(G) where G = number of grapheme clusters

All operations are efficient enough for real-time interactive use.

## Future Enhancements

Potential improvements for future phases:

1. **Block selection**: Rectangular selection mode
2. **Multiple selections**: Multiple cursors/selections
3. **Selection styling**: Custom colors, underlines, etc.
4. **Smart selection**: Expand selection by semantic units
5. **Selection history**: Undo/redo for selections

## Related Phases

- **Phase 6.1**: Cursor Management (cursor positioning)
- **Phase 6.2**: Hit Testing (point-to-offset for mouse selection)
- **Phase 6.3**: Cursor Movement (used for selection extension)
- **Phase 6.5**: Text Insertion (replace selection with text)
- **Phase 6.6**: Text Deletion (delete selected text)
- **Phase 6.7**: Clipboard Operations (copy/cut selected text)

## References

- [Unicode Text Segmentation (UAX-29)](https://www.unicode.org/reports/tr29/)
- [Unicode Bidirectional Algorithm (UAX-9)](https://www.unicode.org/reports/tr9/)
- [Text Selection Best Practices](https://www.w3.org/TR/selection-api/)
