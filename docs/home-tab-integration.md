# Home Tab Integration Guide

This document describes how `rune-scene` loads and renders the `home_tab` sample from `rune-ir`.

## Overview

When you set `USE_IR=1`, rune-scene automatically loads the home_tab sample from the `rune-ir` crate, which demonstrates a complete ViewDocument/DataDocument structure with:
- Flex container layout
- Linear gradient background
- Image element with data binding

## File Structure

```
crates/rune-ir/home_tab/
├── RUNE.MANIFEST.json          # Package metadata
├── RUNE.TOC.json                # Table of contents
└── views/
    ├── data/
    │   └── home.json            # DataDocument (image source)
    └── layout/
        └── home.vizr            # ViewDocument (layout + styling)
```

## ViewDocument Structure

**File**: `home_tab/views/layout/home.vizr`

```json
{
  "view_id": "home",
  "root": "root",
  "nodes": [
    {
      "id": "root",
      "type": "flex_container",
      "layout": {
        "direction": "column",
        "justify": "start",
        "align": "center",
        "gap": 0
      },
      "background": {
        "type": "linear_gradient",
        "angle": 0,
        "stops": [
          ["#346fb4", 0],
          ["#1e4599", 1]
        ]
      },
      "children": ["hero_image"]
    },
    {
      "id": "hero_image",
      "node_id": "HMTB0001",
      "type": "image",
      "content_fit": "fill"
    }
  ]
}
```

### Key Features

1. **Root Container**: Flex container with column direction
2. **Gradient Background**: Linear gradient from #346fb4 to #1e4599
3. **Image Node**: References data via `node_id: "HMTB0001"`

## DataDocument Structure

**File**: `home_tab/views/data/home.json`

```json
{
  "document_id": "home-data",
  "nodes": [
    {
      "node_id": "HMTB0001",
      "kind": "image",
      "source": "images/home_tab.png",
      "description": "Rune home tab"
    }
  ]
}
```

### Data Binding

The ViewDocument's `hero_image` node references data node `HMTB0001`, which specifies:
- **Source**: `images/home_tab.png`
- **Kind**: Image
- **Description**: "Rune home tab"

## Loading Process

### 1. Automatic Loading (`sample_ir.rs`)

```rust
pub fn load_home_tab_view() -> ViewDocument {
    let home_tab_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("rune-ir/home_tab/views/layout/home.vizr");

    if let Ok(json) = std::fs::read_to_string(&home_tab_path) {
        if let Ok(view_doc) = serde_json::from_str::<ViewDocument>(&json) {
            println!("Loaded home_tab ViewDocument from: {:?}", home_tab_path);
            return view_doc;
        }
    }

    eprintln!("Failed to load home_tab, using fallback sample");
    create_sample_view_document()
}
```

### 2. Path Resolution

The loader resolves the path relative to `CARGO_MANIFEST_DIR`:
```
/Users/yppartha/PROJECTS/rune-draw/crates/rune-scene
  -> ../rune-ir/home_tab/views/layout/home.vizr
```

### 3. Fallback Behavior

If loading fails:
- Prints error: "Failed to load home_tab, using fallback sample"
- Falls back to hardcoded ViewDocument with buttons and text

## Rendering Integration

### Current Implementation (`lib.rs`)

```rust
let ir_view_doc = if use_ir {
    Some(Arc::new(sample_ir::load_home_tab_view()))
} else {
    None
};
```

The loaded ViewDocument is then rendered using:
1. **IrAdapter** - Converts ViewNodes to elements
2. **Manual layout** - Simple vertical positioning
3. **Canvas rendering** - Direct rendering on canvas

### Gradient Background Handling

The home_tab uses a linear gradient background:
```json
{
  "type": "linear_gradient",
  "angle": 0,
  "stops": [
    ["#346fb4", 0],
    ["#1e4599", 1]
  ]
}
```

**Current Status**: The home_tab linear gradient background is rendered via
`render_home_tab_viewport()` in `crates/rune-scene/src/lib.rs`, which maps
`ViewBackground::LinearGradient` to an engine-core `Brush::LinearGradient`.

### Image Rendering

The hero_image node references data via `node_id`:
```json
{
  "id": "hero_image",
  "node_id": "HMTB0001",
  "type": "image",
  "content_fit": "fill"
}
```

**Current Status**: Image rendering for the `hero_image` node is implemented:
1. `node_id` is resolved to the DataDocument node (`HMTB0001`)
2. The `source` path (`images/home_tab.png`) is used with Canvas `draw_image`
3. The IR `content_fit` value is mapped to `ImageFitMode` for correct sizing

## Testing

### Run with home_tab

```bash
USE_IR=1 cargo run -p rune-scene
```

Expected console output:
```
IR rendering enabled (USE_IR=1)
Loaded home_tab ViewDocument from: ".../rune-ir/home_tab/views/layout/home.vizr"
Loaded home_tab DataDocument from: ".../rune-ir/home_tab/views/data/home.json"
```

### Verify Fallback

To test the fallback behavior, temporarily rename the home_tab directory:
```bash
mv crates/rune-ir/home_tab crates/rune-ir/home_tab.bak
USE_IR=1 cargo run -p rune-scene
```

Expected output:
```
Failed to load home_tab, using fallback sample
```

## Extending home_tab

### Add New Elements

Edit `home_tab/views/layout/home.vizr`:

```json
{
  "id": "new_button",
  "type": "button",
  "style": {
    "background": { "type": "solid", "color": "#10b981" },
    "width": 160,
    "height": 36,
    "corner_radius": 8
  },
  "label_style": {
    "color": "#ffffff",
    "font_size": 16
  }
}
```

Add to root's children array:
```json
"children": ["hero_image", "new_button"]
```

### Add Data Bindings

Edit `home_tab/views/data/home.json`:

```json
{
  "node_id": "BTN001",
  "kind": "text",
  "content": "Click Me",
  "description": "Button label"
}
```

Reference in ViewDocument:
```json
{
  "id": "new_button",
  "node_id": "BTN001",
  ...
}
```

## Implementation Status

### ✅ Implemented
- ViewDocument JSON loading
- DataDocument JSON loading
- Fallback to hardcoded sample
- Path resolution
- Basic error handling
- Gradient background rendering for home_tab
- Hero image rendering with DataDocument binding

### 🚧 In Progress
- Data binding resolution beyond the home_tab hero image

### 📋 TODO
- Full Taffy layout integration (currently manual)
- Image asset loading
- Complete ViewNode type support
- Hot reload on JSON file changes

## Troubleshooting

### File Not Found

If you see "Failed to load home_tab":
1. Verify the home_tab directory exists: `ls crates/rune-ir/home_tab/`
2. Check file paths in console output
3. Ensure working directory is project root

### JSON Parse Errors

If JSON fails to parse:
1. Validate JSON syntax: `jsonlint home.vizr`
2. Check for trailing commas
3. Verify schema compliance

### Rendering Issues

If elements don't appear:
1. Check console for "Loaded home_tab" messages
2. Verify USE_IR=1 is set
3. Check node types are supported in `lib.rs` rendering loop

## Related Files

- **`crates/rune-scene/src/sample_ir.rs`** - Loading logic
- **`crates/rune-scene/src/lib.rs`** - Integration and rendering
- **`crates/rune-scene/src/ir_adapter.rs`** - ViewNode conversion
- **`crates/rune-ir/home_tab/`** - Sample documents
- **`docs/using-ir-renderer.md`** - User guide
