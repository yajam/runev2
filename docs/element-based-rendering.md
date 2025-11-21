# Element-Based Rendering Architecture

## ✅ Implemented: Clean Separation of Concerns

The IR renderer now uses **self-contained elements** where each element owns its:
- Rendering logic
- Event handling (next step via event_router)
- Internal state

## Architecture

```
IR Documents → Taffy Layout → Element Creation → Element.render(canvas, z)
                                                        ↓
                                                  event_router (next)
                                                        ↓
                                                  Element.handle_click()
```

## Element Mapping (Implemented)

### ✅ Containers
- **FlexContainer**: Renders background/borders, children via recursion
- **GridContainer**: TODO (placeholder gray box)
- **FormContainer**: TODO (placeholder gray box)

### ✅ Content
- **Text**: Uses `elements::Text` - fully self-contained
- **Image**: Uses Canvas.draw_image (TODO: wrap in `elements::ImageBox`)

### ✅ Interactive
- **Button**: Uses `elements::Button` - self-contained rendering
  - Label from DataDocument via node_id binding
  - Event handling ready (via event_router next)

### ⏳ TODO
- Checkbox, Radio, Select
- InputBox, TextArea
- Alert, Modal, Confirm
- FileInput, Table, Spacer

## Code Structure

### ir_renderer.rs Functions

```rust
// High-level rendering
render_frame() → render_ir_document() → render_view_node_with_elements()

// Element renderers (self-contained)
render_container_element()  // FlexContainer background/border
render_text_element()        // elements::Text
render_button_element()      // elements::Button
render_image_element()       // Canvas image (TODO: elements::ImageBox)

// Data resolution
resolve_text_from_data()          // Get text from DataDocument
resolve_action_label_from_data()  // Get button label
resolve_image_source_from_data()  // Get image path

// Utilities
brush_from_view_background()      // Convert IR backgrounds to Brush
```

## Element Pattern

Each element follows this pattern:

```rust
// 1. Create element from IR spec + data
let element = elements::Button {
    rect,           // From Taffy layout
    label,          // From DataDocument
    bg, fg,         // From ViewSpec styling
    focused: false, // Internal state
    // ... other fields
};

// 2. Element renders itself
element.render(canvas, z);

// 3. Element handles its own events (via event_router)
// element.handle_click(x, y) → ButtonClickResult::Clicked
// element.handle_hover(x, y) → bool
```

## Benefits

✅ **Clean separation**: Each element is self-contained
✅ **Reusable**: Elements can be used in any rendering context
✅ **Testable**: Elements can be tested independently
✅ **Maintainable**: Event handling co-located with rendering
✅ **Extensible**: Easy to add new element types

## Data Binding

Elements get their content from DataDocument via node_id:

```
ViewNode                     DataDocument
--------                     ------------
Button {                     Action {
  node_id: "BTN001" ------→    node_id: "BTN001"
  style: { ... }               label: "Click Me"
}                              action: "submit"
                             }
```

Resolvers extract data:
- `resolve_text_from_data()` → text content
- `resolve_action_label_from_data()` → button labels
- `resolve_image_source_from_data()` → image paths

## Next Steps

### 1. Event Handling (High Priority)
- Wire up event_router to route mouse/keyboard events
- Connect element event handlers (handle_click, handle_hover, etc.)
- Track focus state for interactive elements
- Update elements based on event results

### 2. More Elements (Medium Priority)
- Checkbox, Radio, Select
- InputBox, TextArea with editing
- Alert, Modal, Confirm dialogs

### 3. Missing Element Types (Lower Priority)
- Spacer (empty space)
- FileInput (file picker)
- Table (data grid)

### 4. Polish (Nice to Have)
- Hover states
- Focus indicators
- Animations/transitions
- Scroll containers

## Usage

```bash
# Test with default home_tab
USE_IR=1 cargo run -p rune-scene

# Test with sample_first_node (buttons, text, images)
USE_IR=1 cargo run -p rune-scene -- crates/rune-scene/examples/sample_first_node
```

## Key Files

- `crates/rune-scene/src/ir_renderer.rs` - Main rendering logic
- `crates/rune-scene/src/elements/*.rs` - Self-contained element implementations
- `crates/rune-scene/src/event_router.rs` - Event routing (next to implement)
- `crates/rune-scene/src/ir_adapter.rs` - IR → Element conversion utilities

## Success Criteria

✅ Elements own their rendering
✅ Data binding works (ViewDocument + DataDocument)
✅ Taffy layout integration
✅ Clean, maintainable code structure
⏳ Event handling (next)
⏳ Full element coverage
