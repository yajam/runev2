# FileInput Element Usage Examples

The `FileInput` element provides a native file picker with single and multi-file upload support.

## Basic Single File Input

```rust
use rune_scene::elements::FileInput;
use engine_core::Rect;

// Create a single-file input
let mut file_input = FileInput::new(
    Rect {
        x: 100.0,
        y: 100.0,
        w: 400.0,
        h: 50.0,
    },
    false, // single file mode
)
.with_label("Picture".to_string())
.with_placeholder("Choose File".to_string());

// Render it
file_input.render(&mut canvas, z_index);

// Get selected files
if !file_input.selected_files().is_empty() {
    println!("Selected: {:?}", file_input.selected_files());
}
```

## Multi-File Input

```rust
use rune_scene::elements::FileInput;
use engine_core::Rect;

// Create a multi-file input
let mut file_input = FileInput::new(
    Rect {
        x: 100.0,
        y: 100.0,
        w: 400.0,
        h: 50.0,
    },
    true, // multi-file mode
)
.with_label("Documents".to_string())
.with_placeholder("Choose files".to_string());

// Render it
file_input.render(&mut canvas, z_index);
```

## File Type Filtering

```rust
use rune_scene::elements::FileInput;
use engine_core::Rect;

// Create a file input with file type filters
let mut file_input = FileInput::new(
    Rect {
        x: 100.0,
        y: 100.0,
        w: 400.0,
        h: 50.0,
    },
    false,
)
.with_label("Image".to_string())
.with_accept(vec!["png".to_string(), "jpg".to_string(), "jpeg".to_string(), "gif".to_string()]);

// Render it
file_input.render(&mut canvas, z_index);
```

## Event Handling

The FileInput element implements the `EventHandler` trait, so it handles events automatically:

```rust
use rune_scene::event_handler::{EventHandler, MouseClickEvent};
use winit::event::{ElementState, MouseButton};

// Handle click events
let click_event = MouseClickEvent {
    button: MouseButton::Left,
    state: ElementState::Pressed,
    x: 150.0,
    y: 120.0,
    click_count: 1,
};

let result = file_input.handle_mouse_click(click_event);
// If clicked on the file input, it automatically opens the file picker dialog
```

## Manual File Picker

You can also manually trigger the file picker:

```rust
// Open the file picker programmatically
file_input.open_file_picker();

// Check selected files
for file_path in file_input.selected_files() {
    println!("Selected: {}", file_path.display());
}

// Clear selection
file_input.clear_selection();
```

## Custom Styling

```rust
use engine_core::ColorLinPremul;

let mut file_input = FileInput::new(rect, false);

// Customize colors
file_input.bg_color = ColorLinPremul::from_srgba_u8([255, 255, 255, 255]);
file_input.button_bg_color = ColorLinPremul::from_srgba_u8([59, 130, 246, 255]);
file_input.button_text_color = ColorLinPremul::from_srgba_u8([255, 255, 255, 255]);
file_input.file_text_color = ColorLinPremul::from_srgba_u8([80, 80, 80, 255]);
file_input.radius = 8.0;

file_input.render(&mut canvas, z_index);
```

## Focus Management

```rust
// Set focus
file_input.set_focused(true);

// Check focus state
if file_input.is_focused() {
    println!("File input has focus");
}

// When focused, Space or Enter keys will open the file picker
```

## Hit Testing

```rust
// Check if a point is inside the file input
if file_input.contains_point(mouse_x, mouse_y) {
    println!("Mouse is over the file input");
}
```

## Features

- ✅ Single and multi-file selection
- ✅ Native file picker dialog (cross-platform via `rfd`)
- ✅ File type filtering
- ✅ Visual feedback (focus outline)
- ✅ Keyboard support (Space/Enter to open picker when focused)
- ✅ Mouse click support
- ✅ Displays selected file names
- ✅ Self-contained event handling via `EventHandler` trait
- ✅ Customizable styling
