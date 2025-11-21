# Using IR Renderer in rune-scene

This guide shows how to switch between the hardcoded `viewport_ir` rendering and IR-based rendering using `rune-ir`.

## Quick Start

### Enable IR Rendering

Set the `USE_IR=1` environment variable:

```bash
USE_IR=1 cargo run -p rune-scene
```

You should see: `IR rendering enabled (USE_IR=1)`

### Disable IR Rendering (Default)

Run without the environment variable to use the original hardcoded elements:

```bash
cargo run -p rune-scene
```

## How It Works

### Architecture

The integration uses a conditional branching approach in `lib.rs`:

1. **Initialization** (`lib.rs` line ~161-184):
   ```rust
   let use_ir = std::env::var("USE_IR").map(|v| v == "1").unwrap_or(false);

   // IR renderer and documents (only initialized if USE_IR=1)
   let ir_renderer = if use_ir {
       Some(Arc::new(Mutex::new(IrRenderer::new())))
   } else {
       None
   };
   let ir_view_doc = if use_ir {
       Some(Arc::new(sample_ir::create_sample_view_document()))
   } else {
       None
   };
   ```

2. **Rendering** (`lib.rs` line ~2542-2646):
   ```rust
   let content_height = if use_ir {
       // IR-based rendering
       // Uses IrAdapter to convert ViewNodes to rendering elements
       // Renders text, buttons, checkboxes from ViewDocument
   } else {
       // Original viewport_ir rendering
       viewport_ir_lock.render(...)
   };
   ```

### Components

#### 1. Sample IR Documents (`src/sample_ir.rs`)

Loads IR documents from the `rune-ir/home_tab` sample:
- `load_home_tab_view()` - Loads `home_tab/views/layout/home.vizr`
- `load_home_tab_data()` - Loads `home_tab/views/data/home.json`
- Falls back to hardcoded sample if files not found

The home_tab sample includes:
- Flex container with linear gradient background
- Hero image (references `images/home_tab.png`)

#### 2. IR Adapter (`src/ir_adapter.rs`)

Converts rune-ir specs to rendering elements:
- `button_from_spec()` - Convert ButtonSpec to Button
- `checkbox_from_spec()` - Convert CheckboxSpec to Checkbox
- `color_from_text_style()` - Extract color from TextStyle
- `font_size_from_text_style()` - Extract font size

#### 3. Current Implementation

The current implementation uses a **simplified manual layout** approach:
- Iterates through ViewDocument nodes
- Converts each node using IrAdapter
- Renders directly on Canvas with manual positioning

This is intentionally simple to demonstrate the integration. For full Taffy-based layout, see "Future Improvements" below.

## Example Output

With `USE_IR=1`, the viewport renders:

```
Rune Scene — IR Rendering           (title, white, 24px)
Powered by rune-ir with Taffy layout (subtitle, gray, 14px)

[Primary Button]  [Secondary Button]  (blue and gray buttons)

☐ Enable feature                    (checkbox with label)
```

## Customizing IR Content

### Modify Sample Document

Edit `src/sample_ir.rs` to change the UI:

```rust
// Add a new button
nodes.push(ViewNode {
    id: "button3".to_string(),
    node_id: Some("button3-data".to_string()),
    kind: ViewNodeKind::Button(ButtonSpec {
        style: SurfaceStyle {
            background: Some(ViewBackground::Solid {
                color: "#10b981".to_string(), // Green
            }),
            width: Some(140.0),
            height: Some(36.0),
            corner_radius: Some(8.0),
            ..Default::default()
        },
        label_style: TextStyle {
            color: Some("#ffffff".to_string()),
            font_size: Some(16.0),
            ..Default::default()
        },
        ..Default::default()
    }),
});

// Update get_text_content() to provide the label
pub fn get_text_content(node_id: &str) -> Option<String> {
    match node_id {
        "button3-data" => Some("Success".to_string()),
        // ... existing cases
    }
}
```

### Load from JSON

To load IR from a JSON file instead of hardcoded data:

```rust
// In lib.rs, replace sample_ir::create_sample_view_document()
let ir_json = std::fs::read_to_string("my-ui.json")?;
let ir_view_doc = serde_json::from_str::<ViewDocument>(&ir_json)?;
```

## Future Improvements

### Full Taffy Layout Integration

The current implementation uses manual positioning. To use full Taffy layout with `IrRenderer`:

```rust
// Instead of manual rendering in lib.rs:
let mut ir_renderer_lock = ir_renderer.lock().unwrap();

// Use Painter directly (requires Canvas API extension)
let mut painter = Painter::begin_frame(Viewport {
    width: viewport_rect.w as u32,
    height: viewport_rect.h as u32,
});

ir_renderer_lock.render(
    &mut painter,
    ir_data_doc.as_ref(),
    ir_view_doc.as_ref(),
    Point::new(0.0, 0.0),
    0,
    viewport_rect.w,
    viewport_rect.h,
)?;

let display_list = painter.finish();
// Render display_list...
```

This requires extending `Canvas` to expose its internal `Painter` or creating a parallel rendering path.

### DataDocument Integration

Currently, text content is resolved via `sample_ir::get_text_content()`. Full integration would:
1. Store text in DataDocument nodes
2. Query DataDocument using ViewNode's `node_id`
3. Bind data to view using DataBindings

### Event Handling

Map IR interaction intents to input events:
- `on_click_intent` from ButtonSpec → click handler
- `control_id` from CheckboxSpec → state management

## Comparison

| Feature | viewport_ir (Default) | IR Rendering (USE_IR=1) |
|---------|----------------------|-------------------------|
| **Layout** | Hardcoded positions | Manual (Taffy planned) |
| **Content** | Static Rust code | ViewDocument (JSON-loadable) |
| **Styling** | Rust structs | rune-ir specs |
| **Flexibility** | Low | High |
| **Performance** | Optimal | Similar (manual), varies (Taffy) |

## Troubleshooting

### IR Not Rendering

Check that:
1. `USE_IR=1` is set: `echo $USE_IR`
2. Console shows: "IR rendering enabled (USE_IR=1)"
3. No errors in terminal output

### Layout Issues

Current manual layout is limited. For complex layouts, implement full IrRenderer integration with Taffy.

### Missing Text Content

Ensure `sample_ir::get_text_content()` has an entry for every `node_id` referenced in the ViewDocument.

## Files Modified

- `crates/rune-scene/src/lib.rs` - Conditional IR rendering logic
- `crates/rune-scene/src/sample_ir.rs` - Sample ViewDocument/DataDocument
- `crates/rune-scene/src/ir_adapter.rs` - ViewNode to element conversion
- `docs/using-ir-renderer.md` - This guide

## Next Steps

1. **Add More Elements**: Extend `sample_ir.rs` with InputBox, Select, etc.
2. **Full Taffy Layout**: Integrate `IrRenderer` for automatic layout
3. **JSON Loading**: Load UI from external JSON files
4. **Hot Reload**: Watch JSON files and reload on change
