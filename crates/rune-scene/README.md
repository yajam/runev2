# Rune Scene - Zone-Based UI Architecture

## Overview

Rune Scene implements a zone-based rendering architecture that divides the application window into distinct zones with proper z-layering support. This architecture is built on top of the rune-draw engine's zone system.

## Zone System

### Architecture

The application window is divided into four main zones:

1. **Viewport** (Zone ID: 1000)
   - Main content area where IR-based rendering will occur
   - Located: Right of sidebar, below toolbar, above devtools
   - Background: Dark blue-gray (#1E2337)

2. **Sidebar** (Zone ID: 2000)
   - Left panel for tools and navigation
   - Located: Left edge, below toolbar
   - Background: Darker blue-gray (#161B2F)
   - Width: 280px (fixed)

3. **Toolbar** (Zone ID: 3000)
   - Top bar for actions and controls
   - Located: Full width at top
   - Background: Darkest blue-gray (#12172B)
   - Height: 48px (fixed)

4. **DevTools** (Zone ID: 4000)
   - Bottom panel for debugging and development tools
   - Located: Bottom right, below viewport, right of sidebar
   - Background: Medium dark blue-gray (#1A1F33)
   - Height: 200px (fixed)

### Z-Layering

The zone system uses z-layering to control rendering order:

- **Zone backgrounds**: z=0 (rendered first/behind)
- **Zone borders**: z=1
- **UI elements**: z=10+ (rendered on top)

All zones support the full z-layering system from engine-core, allowing elements within zones to have their own depth ordering.

### Hit Testing

Each zone can have hit regions defined using the engine-core hit testing system:

```rust
// Define a hit region for a zone
painter.hit_region_rect(ZoneId::Viewport.as_u32(), rect, z_layer);

// Check hits in event handlers
if let Some(hit) = hit_result {
    if hit.region_id == Some(ZoneId::Viewport.as_u32()) {
        // Handle viewport interaction
    }
}
```

## Module Structure

```
rune-scene/
├── src/
│   ├── lib.rs           # Main entry point with zone rendering
│   ├── zones.rs         # Zone layout and styling system
│   ├── viewport_ir.rs   # Viewport IR + UI elements
│   ├── elements/        # UI element implementations
│   └── text.rs          # Text rendering utilities
```

### Key Modules

#### `zones.rs`

Defines the zone system:

- `ZoneId`: Enum for zone identifiers
- `ZoneLayout`: Calculates zone rectangles based on window size
- `ZoneStyle`: Styling configuration for each zone
- `ZoneManager`: Manages layout and styling for all zones

```rust
use zones::{ZoneManager, ZoneId};

// Create zone manager
let mut zone_manager = ZoneManager::new(width, height);

// Get zone rectangle
let viewport_rect = zone_manager.layout.get_zone(ZoneId::Viewport);

// Resize zones
zone_manager.resize(new_width, new_height);
```

#### `viewport_ir.rs`

Contains the viewport IR + UI elements for demonstration (formerly `sample_ui.rs`). This module is evolving into the extended IR implementation for the viewport zone.

Current elements:
- Text, Checkboxes, Buttons, Radio buttons
- Input boxes, Text areas, Select dropdowns
- Labels, Images, Multiline text

## Integration with Engine-Core

Rune Scene leverages the underlying engine-core zone system:

### Display List Commands

All drawing commands support z-layering:

```rust
// From engine-core/src/display_list.rs
DrawRect { rect, brush, z, transform }
DrawRoundedRect { rrect, brush, z, transform }
DrawText { run, z, transform }
// ... etc
```

### Painter API

The `Painter` API (wrapped by `Canvas`) provides zone-aware drawing:

```rust
painter.rect(rect, brush, z_layer);
painter.rounded_rect(rrect, brush, z_layer);
painter.hit_region_rect(zone_id, rect, z_layer);
```

### Canvas API

The `Canvas` (from rune-surface) wraps the `Painter`:

```rust
canvas.fill_rect(x, y, w, h, brush, z_layer);
canvas.draw_text_run(pos, text, size, color, z_layer);
```

## Usage

### Running the Application

```bash
cargo run -p rune-scene
```

### Rendering Flow

1. **Zone Backgrounds** (z=0): Render all zone backgrounds
2. **Zone Borders** (z=1): Render zone separation borders
3. **UI Elements** (z=10+): Render UI elements with proper layering

```rust
// In RedrawRequested event
let mut canvas = surf.begin_frame(width, height);

// 1. Render zones
render_zones(&mut canvas, &zone_manager);

// 2. Render content (viewport IR / UI)
viewport_ir.render(&mut canvas, scale_factor, width);

surf.end_frame(frame, canvas);
```

## Future: IR-Based Rendering

The viewport zone is designed to be replaced with IR (Intermediate Representation) based rendering:

1. **Current**: Sample UI elements render directly to canvas
2. **Future**: IR tree → Layout → Paint commands → Viewport zone

The zone system provides the foundation for this transition:
- Viewport zone defines the rendering area
- Z-layering ensures proper depth ordering
- Hit regions enable interaction within the viewport

## Customization

### Changing Zone Layout

Modify `ZoneLayout::calculate()` in `zones.rs`:

```rust
impl ZoneLayout {
    pub fn calculate(window_width: u32, window_height: u32) -> Self {
        // Adjust constants
        const SIDEBAR_WIDTH: f32 = 320.0;  // Wider sidebar
        const TOOLBAR_HEIGHT: f32 = 60.0;  // Taller toolbar
        // ...
    }
}
```

### Changing Zone Styles

Modify style methods in `ZoneStyle`:

```rust
impl ZoneStyle {
    pub fn viewport() -> Self {
        Self {
            bg_color: ColorLinPremul::from_srgba_u8([40, 45, 65, 255]),
            border_color: ColorLinPremul::from_srgba_u8([80, 85, 105, 255]),
            border_width: 2.0,
        }
    }
}
```

## References

- **Engine-Core Display List**: `crates/engine-core/src/display_list.rs`
- **Engine-Core Painter**: `crates/engine-core/src/painter.rs`
- **Engine-Core Hit Testing**: `crates/engine-core/src/hit_test.rs`
- **Demo Zones Example**: `crates/demo-app/src/scenes/zones.rs`
- **Usage Documentation**: `docs/usage.md` (sections on Hit Regions and Z-layering)
