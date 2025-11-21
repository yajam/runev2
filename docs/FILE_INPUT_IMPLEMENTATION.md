# FileInput Element Implementation Summary

## Overview

A complete FileInput UI element has been added to `rune-scene` with self-contained event handlers, supporting both single and multi-file upload with native file picker dialogs.

## Visual Design

The FileInput element matches the design shown in the reference image:

```
┌─────────────────────────────────────────────────────┐
│ Picture                                             │
│ ┌──────────────┬────────────────────────────────┐  │
│ │ Choose File  │ Tasklist.txt                   │  │
│ └──────────────┴────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

- **Label**: Optional label displayed above the input (e.g., "Picture")
- **Button**: "Choose File" button that triggers the native file picker
- **Display Area**: Shows selected file name(s)
- **Focus Indicator**: Blue outline when element has focus

## Files Created/Modified

### New Files
1. **`crates/rune-scene/src/elements/file_input.rs`** - Complete FileInput element implementation
2. **`examples/file_input_usage.md`** - Comprehensive usage examples and API documentation

### Modified Files
1. **`Cargo.toml`** - Added `rfd = "0.15"` to workspace dependencies
2. **`crates/rune-scene/Cargo.toml`** - Added `rfd` dependency
3. **`crates/rune-scene/src/elements/mod.rs`** - Added FileInput module and exports
4. **`crates/rune-scene/src/ir_renderer/elements.rs`** - Updated IR renderer to use new FileInput element

## Features

✅ **Single and Multi-file Selection**
- Single file mode: `FileInput::new(rect, false)`
- Multi-file mode: `FileInput::new(rect, true)`

✅ **Native File Picker Dialog**
- Cross-platform via `rfd` crate
- Automatic platform-native UI (macOS, Windows, Linux)

✅ **File Type Filtering**
- Optional `accept` parameter for filtering by file extension
- Example: `.with_accept(vec!["png".to_string(), "jpg".to_string()])`

✅ **Visual Feedback**
- Focus outline when element has keyboard focus
- Displays selected file names
- Customizable styling (colors, radius, sizes)

✅ **Event Handling (EventHandler trait)**
- Mouse click support - opens file picker on click
- Keyboard support - Space/Enter opens picker when focused
- Hit testing for focus management
- Self-contained event handling logic

✅ **Customizable Styling**
- Background colors
- Button colors
- Text colors
- Corner radius
- Label configuration

## Quick Start

### Basic Usage

```rust
use rune_scene::elements::FileInput;
use engine_core::Rect;

// Create a single-file input
let mut file_input = FileInput::new(
    Rect { x: 100.0, y: 100.0, w: 400.0, h: 50.0 },
    false, // single file mode
)
.with_label("Picture".to_string());

// Render it
file_input.render(&mut canvas, z_index);

// Get selected files
for file_path in file_input.selected_files() {
    println!("Selected: {}", file_path.display());
}
```

### Multi-file Example

```rust
let mut file_input = FileInput::new(rect, true) // multi-file mode
    .with_label("Documents".to_string())
    .with_placeholder("Choose files".to_string());
```

### With File Type Filtering

```rust
let mut file_input = FileInput::new(rect, false)
    .with_label("Image".to_string())
    .with_accept(vec![
        "png".to_string(),
        "jpg".to_string(),
        "jpeg".to_string(),
        "gif".to_string(),
    ]);
```

## Event Handling Integration

The FileInput implements the `EventHandler` trait for automatic event processing:

```rust
use rune_scene::event_handler::EventHandler;

// Mouse click handling (automatically opens file picker)
let result = file_input.handle_mouse_click(click_event);

// Keyboard handling (Space/Enter when focused)
let result = file_input.handle_keyboard(keyboard_event);

// Focus management
file_input.set_focused(true);
if file_input.is_focused() {
    // Element has focus
}

// Hit testing
if file_input.contains_point(mouse_x, mouse_y) {
    // Mouse is over the element
}
```

## API Reference

### Constructor

```rust
FileInput::new(rect: Rect, multi: bool) -> Self
```

### Builder Methods

- `.with_label(label: String)` - Set the label text
- `.with_placeholder(placeholder: String)` - Set placeholder text
- `.with_accept(accept: Vec<String>)` - Set file type filters

### Methods

- `.render(&self, canvas: &mut Canvas, z: i32)` - Render the element
- `.open_file_picker(&mut self)` - Manually trigger file picker
- `.selected_files(&self) -> &[PathBuf]` - Get selected file paths
- `.clear_selection(&mut self)` - Clear current selection
- `.is_focused(&self) -> bool` - Check focus state
- `.set_focused(&mut self, focused: bool)` - Set focus state
- `.contains_point(&self, x: f32, y: f32) -> bool` - Hit test

### Public Fields (for customization)

```rust
pub struct FileInput {
    pub rect: Rect,
    pub label: Option<String>,
    pub label_size: f32,
    pub label_color: ColorLinPremul,
    pub selected_files: Vec<PathBuf>,
    pub multi: bool,
    pub placeholder: String,
    pub accept: Option<Vec<String>>,
    pub focused: bool,
    pub bg_color: ColorLinPremul,
    pub button_bg_color: ColorLinPremul,
    pub button_text_color: ColorLinPremul,
    pub file_text_color: ColorLinPremul,
    pub radius: f32,
}
```

## Custom Styling Example

```rust
use engine_core::ColorLinPremul;

let mut file_input = FileInput::new(rect, false);

// Customize appearance
file_input.bg_color = ColorLinPremul::from_srgba_u8([255, 255, 255, 255]);
file_input.button_bg_color = ColorLinPremul::from_srgba_u8([59, 130, 246, 255]); // Blue
file_input.button_text_color = ColorLinPremul::from_srgba_u8([255, 255, 255, 255]); // White
file_input.file_text_color = ColorLinPremul::from_srgba_u8([80, 80, 80, 255]); // Dark gray
file_input.radius = 8.0; // Larger corner radius
```

## IR Rendering Integration

The FileInput element is now integrated with the IR rendering system:

```rust
// In ir_renderer/elements.rs
pub(super) fn render_file_input_element(
    canvas: &mut rune_surface::Canvas,
    spec: &rune_ir::view::FileInputSpec,
    rect: engine_core::Rect,
    z: i32,
)
```

This allows FileInput to be rendered from IR specifications, though full interactivity requires managing the element in application state with the EventHandler trait.

## Dependencies

The implementation uses the `rfd` (Rust File Dialog) crate version 0.15 for cross-platform native file picker dialogs. This has been added to the workspace dependencies.

## Testing

Build verification:
```bash
cargo build -p rune-scene
```

Run the rune-scene app to see elements in action:
```bash
cargo run -p rune-scene
```

## Architecture Notes

### Rendering vs State Management

The FileInput element has two usage contexts:

1. **Stateful Usage** (Full Interactivity)
   - Create and manage FileInput instances in application state
   - Implement EventHandler trait for event processing
   - File selection persists across frames
   - Full access to selected files via `.selected_files()`

2. **IR Rendering** (Visual Only)
   - Used by `render_file_input_element()` for IR-driven UIs
   - Creates temporary FileInput for rendering
   - No persistent state between frames
   - For full interactivity, manage separately with EventHandler

### Event Flow

```
User Click
    ↓
EventHandler::handle_mouse_click()
    ↓
FileInput::handle_click()
    ↓
FileInput::open_file_picker()
    ↓
Native File Dialog (rfd)
    ↓
User Selects File(s)
    ↓
selected_files updated
    ↓
Re-render shows file names
```

## Future Enhancements

Potential improvements for the FileInput element:

- [ ] Add `accept` field to `FileInputSpec` in rune-ir for IR-driven file type filtering
- [ ] Drag-and-drop file support
- [ ] File size validation
- [ ] Preview thumbnails for image files
- [ ] Progress indication for large file uploads
- [ ] Multiple file deletion/removal UI
- [ ] Custom file name formatting

## References

- Main implementation: `crates/rune-scene/src/elements/file_input.rs`
- Usage examples: `examples/file_input_usage.md`
- EventHandler trait: `crates/rune-scene/src/event_handler.rs`
- Similar elements: `Button`, `Checkbox`, `Select` in `crates/rune-scene/src/elements/`
