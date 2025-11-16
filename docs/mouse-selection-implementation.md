# Mouse Selection Implementation

This document describes the mouse-based text selection functionality implemented in the `rune-text` crate.

## Overview

Mouse selection allows users to select text by clicking and dragging with the mouse. The implementation supports:
- **Click**: Position cursor at clicked location
- **Click + Drag**: Select text from click position to current mouse position
- **Double-click**: Select entire word at clicked location
- **Double-click + Drag**: Extend selection word-by-word
- **Triple-click**: Select entire line at clicked location
- **Triple-click + Drag**: Extend selection line-by-line

## API Methods

### Basic Mouse Selection

#### `start_mouse_selection(point: Point) -> Option<Selection>`

Starts a new selection at a mouse position. Called on mouse down.

```rust
let point = Point::new(x, y); // Mouse position in zone-local coordinates
if let Some(selection) = layout.start_mouse_selection(point) {
    // Selection started at clicked position
}
```

#### `extend_mouse_selection(selection: &Selection, point: Point) -> Selection`

Extends a selection to a new mouse position. Called on mouse move while button is held.

```rust
// On mouse move
let extended = layout.extend_mouse_selection(&current_selection, current_point);
```

### Word Selection (Double-click)

#### `start_word_selection(point: Point) -> Option<Selection>`

Selects the entire word at the clicked position.

```rust
// On double-click
if let Some(selection) = layout.start_word_selection(point) {
    // Entire word is selected
}
```

#### `extend_word_selection(selection: &Selection, point: Point) -> Selection`

Extends word selection as the mouse moves. Selection always includes complete words.

```rust
// On mouse move after double-click
let extended = layout.extend_word_selection(&current_selection, current_point);
```

### Line Selection (Triple-click)

#### `start_line_selection(point: Point) -> Option<Selection>`

Selects the entire line at the clicked position.

```rust
// On triple-click
if let Some(selection) = layout.start_line_selection(point) {
    // Entire line is selected
}
```

#### `extend_line_selection(selection: &Selection, point: Point) -> Selection`

Extends line selection as the mouse moves. Selection always includes complete lines.

```rust
// On mouse move after triple-click
let extended = layout.extend_line_selection(&current_selection, current_point);
```

## Usage Example

Here's a complete example of handling mouse selection in a text editor:

```rust
use rune_text::layout::{TextLayout, Selection, Point};

enum SelectionMode {
    Character,  // Normal click
    Word,       // Double-click
    Line,       // Triple-click
}

struct TextEditor {
    layout: TextLayout,
    selection: Selection,
    mode: SelectionMode,
    is_dragging: bool,
}

impl TextEditor {
    fn on_mouse_down(&mut self, x: f32, y: f32, click_count: u32) {
        let point = Point::new(x, y);
        
        match click_count {
            1 => {
                // Single click - start character selection
                if let Some(sel) = self.layout.start_mouse_selection(point) {
                    self.selection = sel;
                    self.mode = SelectionMode::Character;
                }
            }
            2 => {
                // Double-click - select word
                if let Some(sel) = self.layout.start_word_selection(point) {
                    self.selection = sel;
                    self.mode = SelectionMode::Word;
                }
            }
            3 => {
                // Triple-click - select line
                if let Some(sel) = self.layout.start_line_selection(point) {
                    self.selection = sel;
                    self.mode = SelectionMode::Line;
                }
            }
            _ => {}
        }
        
        self.is_dragging = true;
    }
    
    fn on_mouse_move(&mut self, x: f32, y: f32) {
        if !self.is_dragging {
            return;
        }
        
        let point = Point::new(x, y);
        
        self.selection = match self.mode {
            SelectionMode::Character => {
                self.layout.extend_mouse_selection(&self.selection, point)
            }
            SelectionMode::Word => {
                self.layout.extend_word_selection(&self.selection, point)
            }
            SelectionMode::Line => {
                self.layout.extend_line_selection(&self.selection, point)
            }
        };
    }
    
    fn on_mouse_up(&mut self) {
        self.is_dragging = false;
    }
}
```

## Coordinate System

All mouse positions are in **zone-local coordinates**, meaning they are relative to the text layout's origin (0, 0). If your text is positioned at a different location on screen, you must translate screen coordinates to zone-local coordinates before calling these methods.

```rust
// Example: Convert screen coordinates to zone-local
let text_x = 100.0; // Text position on screen
let text_y = 50.0;

let mouse_screen_x = 250.0;
let mouse_screen_y = 150.0;

let zone_local_point = Point::new(
    mouse_screen_x - text_x,
    mouse_screen_y - text_y
);

let selection = layout.start_mouse_selection(zone_local_point);
```

## Selection Direction

Selections maintain both an **anchor** (where the selection started) and an **active** end (current position). This allows:
- Forward selection: anchor < active
- Backward selection: anchor > active

The selection automatically handles both directions, so you can drag left or right from the initial click position.

## Hit Testing

Mouse selection uses the `hit_test` method with `HitTestPolicy::Clamp`, which means:
- Clicks before the text start are clamped to offset 0
- Clicks after the text end are clamped to the last position
- Clicks between lines are assigned to the nearest line

This provides a forgiving user experience where clicks near but not exactly on text still work correctly.

## BiDi Text Support

Mouse selection correctly handles bidirectional text (e.g., mixing English and Arabic). The hit testing takes into account the visual order of runs, ensuring that clicking on visually adjacent characters selects the correct logical range.

## Ligatures and Clusters

The implementation respects grapheme cluster boundaries and ligatures:
- Selections always snap to grapheme cluster boundaries
- Ligatures (like "fi" → "ﬁ") are treated as indivisible units
- Clicking within a ligature selects the entire cluster

## Performance

Mouse selection operations are O(1) for single-line text and O(n) for multi-line text where n is the number of lines. The hit testing uses binary search for line lookup and linear search within runs, making it efficient even for large documents.

## Testing

The implementation includes comprehensive tests covering:
- Basic click and drag selection
- Forward and backward selection
- Word selection and extension
- Line selection and extension
- Multi-line selection
- Empty text edge cases
- Selection direction handling

Run tests with:
```bash
cargo test --package rune-text -- mouse
```
