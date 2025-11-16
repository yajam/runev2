# InputBox Implementation with rune-text

## Overview

The `InputBox` element provides full text editing support using the `rune-text` library. It includes:

- **Cursor with blinking animation** - Automatic cursor blinking at 500ms intervals
- **Text selection support** - Foundation for future selection features
- **Horizontal scrolling** - Automatically scrolls when text exceeds the input box width
- **Text editing operations** - Insert, delete, backspace with grapheme-aware cursor movement
- **Placeholder text** - Shows placeholder when input is empty
- **Grapheme cluster handling** - Properly handles multi-byte characters, emojis, and combining marks

## Architecture

### Integration with rune-text

The InputBox uses several components from `rune-text`:

1. **TextLayout** - Handles text shaping, line breaking, and cursor positioning
2. **Cursor** - Manages cursor position, visibility, and blinking animation
3. **Selection** - Tracks text selection (currently used for cursor position only)
4. **FontFace** - Provides font metrics and glyph information

### Key Features

#### Cursor Blinking

The cursor automatically blinks at a 500ms interval when the input box is focused:

```rust
// In your render loop
input_box.update_blink(delta_time);
```

The cursor visibility is managed by the `Cursor` struct from rune-text, which:
- Resets to visible when moved
- Toggles visibility at the blink interval
- Can be manually controlled if needed

#### Horizontal Scrolling

When text exceeds the input box width, the view automatically scrolls to keep the cursor visible:

```rust
// Scroll logic in update_scroll_to_cursor()
if cursor_x < self.scroll_x + margin {
    self.scroll_x = (cursor_x - margin).max(0.0);
} else if cursor_x > self.scroll_x + viewport_width - margin {
    self.scroll_x = cursor_x - viewport_width + margin;
}
```

The scrolling includes a 10px margin for better UX, ensuring the cursor isn't right at the edge.

#### Grapheme-Aware Editing

All cursor movement and deletion operations respect grapheme cluster boundaries:

```rust
// Delete before cursor finds the previous grapheme boundary
let mut new_offset = 0;
for (idx, _) in text.grapheme_indices(true) {
    if idx >= offset {
        break;
    }
    new_offset = idx;
}
```

This ensures:
- Multi-byte UTF-8 characters are never split
- Emoji sequences (including ZWJ sequences) are treated as single units
- Combining marks stay with their base characters

## Usage

### Basic Setup

```rust
use rune_scene::elements::input_box::InputBox;
use rune_text::font::FontCache;
use engine_core::{Color, Rect};

// Load a font
let mut font_cache = FontCache::new();
let font = font_cache.get_or_load("path/to/font.ttf", 0)?;

// Create an input box
let mut input_box = InputBox::new(
    Rect { x: 50.0, y: 50.0, w: 300.0, h: 40.0 },
    "Initial text".to_string(),
    16.0,                                    // text size
    Color::rgba(240, 240, 240, 255),        // text color
    Some("Placeholder...".to_string()),      // placeholder
    true,                                    // focused
    &font,
);
```

### Rendering

```rust
// In your render loop
input_box.update_blink(delta_time);
input_box.render(&mut canvas, z_index);
```

### Handling Keyboard Input

```rust
match key_event {
    KeyEvent::Char(ch) => {
        input_box.insert_char(ch, &font);
    }
    KeyEvent::Backspace => {
        input_box.delete_before_cursor(&font);
    }
    KeyEvent::Delete => {
        input_box.delete_after_cursor(&font);
    }
    KeyEvent::Left => {
        input_box.move_cursor_left();
    }
    KeyEvent::Right => {
        input_box.move_cursor_right();
    }
    KeyEvent::Home => {
        input_box.move_cursor_to_start();
    }
    KeyEvent::End => {
        input_box.move_cursor_to_end();
    }
}
```

### Getting/Setting Text

```rust
// Get current text
let text = input_box.text();

// Set new text
input_box.set_text("New text".to_string(), &font);
```

## Implementation Details

### Structure

```rust
pub struct InputBox {
    pub rect: Rect,
    pub text_size: f32,
    pub text_color: ColorLinPremul,
    pub placeholder: Option<String>,
    pub focused: bool,
    
    // Text editing state (using rune-text)
    layout: Option<TextLayout>,
    cursor: Cursor,
    selection: Selection,
    
    // Horizontal scrolling
    scroll_x: f32,
    
    // Padding
    padding_x: f32,
    padding_y: f32,
}
```

### Rendering Pipeline

1. **Background & Border** - Rounded rectangle with focus-dependent styling
2. **Clipping** - Clip to content area (rect minus padding)
3. **Text** - Render with horizontal scroll offset applied
4. **Cursor** - Render if focused and visible (blinking)
5. **Placeholder** - Render if text is empty

### Coordinate System

- Input box uses absolute screen coordinates for `rect`
- Text rendering uses scrolled coordinates: `text_x = content_x - scroll_x`
- Cursor position is calculated by rune-text's `TextLayout`
- Clipping ensures text doesn't overflow the input box bounds

## Future Enhancements

### Planned Features

1. **Text Selection** - Visual selection with mouse drag and Shift+arrow keys
2. **Copy/Paste** - Clipboard integration
3. **Undo/Redo** - Using rune-text's `UndoStack`
4. **Word Movement** - Ctrl+Left/Right for word-by-word navigation
5. **Select All** - Ctrl+A to select all text
6. **IME Support** - Input method editor for CJK languages
7. **Validation** - Optional validation callbacks
8. **Max Length** - Character/byte limit enforcement

### Possible Improvements

- **Performance** - Cache TextLayout when text doesn't change
- **Styling** - Customizable colors, borders, and padding
- **Accessibility** - ARIA attributes and screen reader support
- **Multi-line** - Extend to support textarea-style multi-line input

## Example

See `crates/rune-scene/examples/inputbox_demo.rs` for a complete working example.

Run it with:
```bash
cargo run --package rune-scene --example inputbox_demo
```

## Related Documentation

- [rune-text Cursor Management](./rune-text/cursor-management.md)
- [rune-text Hit Testing](./rune-text/hit-testing.md)
- [rune-text Cursor Movement](./rune-text/cursor-movement.md)
