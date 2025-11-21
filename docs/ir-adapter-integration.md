# IR Adapter Integration

This document describes the integration of `rune-ir` with `rune-scene` through the `ir_adapter` module.

## Overview

The `ir_adapter` module provides conversion functions to map `rune-ir` ViewNodes to `rune-scene` rendering elements. This enables rendering UI defined in the rune-ir intermediate representation format.

## Architecture

### rune-ir Data-View Separation

rune-ir uses a two-document architecture:

- **ViewDocument**: Defines layout, styling, and visual structure
  - ViewNodes specify how elements should look and be positioned
  - References data via `node_id` field

- **DataDocument**: Contains actual content (text, values, etc.)
  - Stores the semantic data that fills the visual structure
  - Referenced by ViewNodes

### ir_adapter Role

The `ir_adapter` module focuses on converting ViewNode **visual specs** to rendering elements. Content resolution from DataDocument is handled separately (typically by `IrRenderer` or the caller).

## Usage

### Environment Variable

Set `USE_IR=1` to enable IR rendering mode:

```bash
USE_IR=1 cargo run -p rune-scene
```

When enabled, the application will print: `IR rendering enabled (USE_IR=1)`

### Feature Flag

A feature flag `ir-rendering` is available in `Cargo.toml` for future conditional compilation:

```toml
[features]
default = []
ir-rendering = []
```

## Components

### 1. ir_adapter Module (`src/ir_adapter.rs`)

Provides conversion functions for:

- **Buttons**: `IrAdapter::button_from_spec(spec, rect, label)`
  - Extracts background color, foreground color, radius from ButtonSpec
  - Label text should be resolved from DataDocument by caller

- **Checkboxes**: `IrAdapter::checkbox_from_spec(spec, rect)`
  - Minimal styling - CheckboxSpec only contains form metadata
  - Labels are typically separate Text nodes

- **Text Styles**: Helper functions for color and font size extraction
  - `color_from_text_style(style)` - Extract color from TextStyle
  - `font_size_from_text_style(style)` - Extract font size

- **Color Parsing**: `parse_color(color_str)`
  - Supports hex colors: `#RRGGBB`, `#RRGGBBAA`
  - Supports RGB/RGBA: `rgb(r, g, b)`, `rgba(r, g, b, a)`
  - Supports named colors: `red`, `blue`, `green`, etc.

### 2. ir_renderer Module (`src/ir_renderer.rs`)

Provides full layout integration using Taffy:

- Converts ViewDocument to Taffy layout tree
- Computes layout positions and sizes
- Renders elements using IrAdapter + Painter

### 3. viewport_ir Integration (`src/viewport_ir.rs`)

Contains example documentation for using IR rendering. The `render_from_ir_example()` function shows the integration pattern.

## Example

```rust
use rune_ir::view::{ViewDocument, ViewNodeKind};
use crate::ir_adapter::IrAdapter;
use engine_core::Rect;

// Parse ViewDocument from IR (JSON)
let view_doc: ViewDocument = serde_json::from_str(ir_json)?;

// Iterate through nodes
for node in &view_doc.nodes {
    match &node.kind {
        ViewNodeKind::Button(spec) => {
            // Define layout rect (from Taffy or manual layout)
            let rect = Rect { x: 40.0, y: 100.0, w: 160.0, h: 36.0 };

            // Resolve button text from DataDocument (not shown)
            let label = resolve_button_text(&node.node_id, &data_doc);

            // Convert to Button element
            let button = IrAdapter::button_from_spec(spec, rect, label);

            // Render
            button.render(&mut canvas, z_index);
        }
        _ => {}
    }
}
```

## Testing

Build and test the integration:

```bash
# Build rune-scene with IR adapter
cargo build -p rune-scene

# Run with IR rendering enabled
USE_IR=1 cargo run -p rune-scene

# Run tests
cargo test -p rune-scene
```

## Next Steps

To fully integrate IR rendering:

1. **Data Resolution**: Implement DataDocument text/value resolution
2. **Layout Integration**: Use `IrRenderer` for automatic Taffy layout
3. **Event Handling**: Map IR interaction intents to input events
4. **Additional Elements**: Extend `IrAdapter` for more ViewNode types:
   - InputBox, TextArea, Select
   - Radio buttons (already has basic support)
   - Tables, Images, Links

## Files Modified

- `crates/rune-scene/Cargo.toml` - Added feature flag
- `crates/rune-scene/src/lib.rs` - Added ir_adapter module, USE_IR check
- `crates/rune-scene/src/ir_adapter.rs` - New module for ViewNode conversion
- `crates/rune-scene/src/viewport_ir.rs` - Added example documentation

## References

- `docs/ir-wasm-io-integration-assessment.md` - Original integration assessment
- `crates/rune-ir/src/view/mod.rs` - rune-ir ViewDocument types
- `crates/rune-scene/src/ir_renderer.rs` - Full IrRenderer implementation
