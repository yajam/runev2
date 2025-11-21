# IR ViewNodeKind to Element Mapping

This document maps rune-ir `ViewNodeKind` types to rune-scene elements.

## Mapping Table

| ViewNodeKind | Element/Handler | Status | Notes |
|--------------|----------------|--------|-------|
| **Containers** |
| FlexContainer | IrRenderer (Taffy) | ✅ Complete | Background/border rendered, children via recursion |
| GridContainer | IrRenderer (Taffy) | ⚠️ Partial | Taffy layout, rendering TODO |
| FormContainer | IrRenderer (Taffy) | ⚠️ Partial | Taffy layout, rendering TODO |
| **Content** |
| Text | `elements::Text` or `elements::Label` | ✅ Complete | Static text rendering |
| Image | `elements::ImageBox` | ✅ Complete | Image rendering with fit modes |
| Spacer | N/A (layout only) | ❌ Missing | Empty space for layout |
| **Interactive Widgets** |
| Button | `elements::Button` | ✅ Complete | Clickable button with label |
| Link | `elements::Link` | ✅ Complete | Hyperlink with hover state |
| InputBox | `elements::InputBox` | ✅ Complete | Single-line text input |
| TextArea | `elements::TextArea` | ✅ Complete | Multi-line text input |
| Checkbox | `elements::Checkbox` | ✅ Complete | Toggle checkbox |
| Radio | `elements::Radio` | ✅ Complete | Radio button |
| Select | `elements::Select` | ✅ Complete | Dropdown select |
| FileInput | N/A | ❌ Missing | File picker widget |
| **Data Display** |
| Table | N/A | ❌ Missing | Data-driven table layout |
| **Overlays** |
| Alert | `elements::Alert` | ✅ Complete | Toast notification |
| Modal | `elements::Modal` | ✅ Complete | Modal dialog |
| Confirm | `elements::ConfirmDialog` | ✅ Complete | Confirmation dialog |

## Priority Fixes

### High Priority (blocking basic rendering)
1. **Spacer**: Simple empty rect for layout spacing
2. **Container backgrounds**: Ensure all containers render backgrounds/borders properly

### Medium Priority (enhanced functionality)
3. **FileInput**: File upload widget
4. **Table**: Data-driven table rendering
5. **GridContainer rendering**: Complete grid layout support

### Low Priority (polish)
6. DatePicker integration (element exists but not in IR spec)

## Implementation Strategy

### Phase 1: Basic Rendering Pipeline
1. Set up winit + wgpu in `ir_renderer::run()`
2. Use `IrRenderer` to build Taffy layout tree
3. Render containers (background, border)
4. Render basic elements (text, image, button)

### Phase 2: Interactive Elements
1. Wire up event_router for input handling
2. Connect element event handlers (click, hover, input)
3. Test with sample_first_node package

### Phase 3: Missing Elements
1. Implement Spacer (simple placeholder)
2. Implement FileInput widget
3. Implement Table rendering

## Element Requirements

Each element needs:
- **Render method**: Takes `Canvas`, rect, z-index, state
- **Event handlers**: Mouse/keyboard input (via event_router)
- **State management**: Local state (focus, hover, etc.)
- **Data binding**: Link to DataDocument via node_id

## Next Steps

1. ✅ Map ViewNodeKind to elements (this document)
2. ⏳ Implement rendering pipeline in `ir_renderer::run()`
3. ⏳ Add missing elements (Spacer, FileInput, Table)
4. ⏳ Test with sample packages
