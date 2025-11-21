# Taffy Integration with Coordinate System Management

## Decision: Keep Taffy, Fix Coordinate Handling

**Context**: Previous Taffy integration had bugs due to zone coordinate mismatches, not Taffy itself. We will keep Taffy and implement rigorous coordinate system management.

---

## Coordinate Systems in rune-draw

### 1. Taffy Layout Space
- **Origin**: Top-left of parent container
- **Units**: Logical pixels
- **Y-axis**: Down
- **Coordinates**: Parent-relative (local)
- **Output**: `taffy::Layout` with `location: Point`, `size: Size`, `order: u32`

### 2. Scene Space (rune-draw)
- **Origin**: Top-left of viewport
- **Units**: Logical pixels (1:1 with device pixels)
- **Y-axis**: Down
- **Coordinates**: Absolute (viewport-relative)
- **Output**: `Painter::rect(rect, brush, z_index)`

### 3. Zone Space (rune-scene)
- **Origin**: Zone-dependent (viewport, toolbar, sidebar, devtools have different origins)
- **Units**: Logical pixels
- **Y-axis**: Down
- **Coordinates**: Zone-relative
- **Output**: Zone-local positions that need transformation to scene space

---

## The Problem: Transform Chain

```
Taffy Layout (parent-relative)
    ↓
Zone Space (zone-relative)
    ↓
Scene Space (viewport-absolute)
    ↓
Display List (with z-index)
```

**Previous bug**: Skipped or double-applied transforms, causing layout positions to be wrong.

---

## Solution: Transform Stack Pattern

### Core Abstraction: `LayoutContext`

```rust
/// Tracks the transform chain from layout space to scene space.
pub struct LayoutContext {
    /// Current absolute position in scene space (accumulated transform)
    offset: Point,

    /// Current z-index base (depth in tree)
    z_base: i32,

    /// Transform stack for nested containers
    stack: Vec<Transform>,
}

impl LayoutContext {
    /// Create root context for a zone
    pub fn new(zone_origin: Point, z_base: i32) -> Self {
        Self {
            offset: zone_origin,
            z_base,
            stack: vec![],
        }
    }

    /// Push a transform for a container
    pub fn push(&mut self, taffy_layout: &taffy::Layout) {
        self.stack.push(Transform {
            offset: self.offset,
            z_base: self.z_base,
        });

        // Accumulate offset: parent offset + layout location
        self.offset.x += taffy_layout.location.x;
        self.offset.y += taffy_layout.location.y;

        // Increment z-base for depth ordering
        self.z_base += 1;
    }

    /// Pop transform when leaving a container
    pub fn pop(&mut self) {
        if let Some(prev) = self.stack.pop() {
            self.offset = prev.offset;
            self.z_base = prev.z_base;
        }
    }

    /// Convert Taffy layout to scene-space rect
    pub fn to_scene_rect(&self, taffy_layout: &taffy::Layout) -> Rect {
        Rect {
            x: self.offset.x + taffy_layout.location.x,
            y: self.offset.y + taffy_layout.location.y,
            width: taffy_layout.size.width,
            height: taffy_layout.size.height,
        }
    }

    /// Get current z-index (base + order from Taffy)
    pub fn z_index(&self, taffy_order: u32) -> i32 {
        self.z_base + taffy_order as i32
    }
}

#[derive(Clone, Copy)]
struct Transform {
    offset: Point,
    z_base: i32,
}
```

---

## Integration Pattern: IR Renderer with Taffy

### High-Level Flow

```rust
pub struct IrRenderer {
    taffy: taffy::TaffyTree<ViewNodeId>,
    context_stack: Vec<LayoutContext>,
}

impl IrRenderer {
    pub fn render(
        &mut self,
        painter: &mut Painter,
        data: &DataDocument,
        view: &ViewDocument,
        zone_origin: Point,
        z_base: i32,
    ) {
        // 1. Build Taffy tree from ViewDocument
        self.build_taffy_tree(view);

        // 2. Compute layout
        let available = taffy::Size {
            width: taffy::AvailableSpace::Definite(painter.viewport.width),
            height: taffy::AvailableSpace::Definite(painter.viewport.height),
        };
        self.taffy.compute_layout(root_node, available).unwrap();

        // 3. Render with transform tracking
        let mut ctx = LayoutContext::new(zone_origin, z_base);
        self.render_node(painter, data, view, view.root, &mut ctx);
    }

    fn render_node(
        &self,
        painter: &mut Painter,
        data: &DataDocument,
        view: &ViewDocument,
        node_id: &ViewNodeId,
        ctx: &mut LayoutContext,
    ) {
        let view_node = view.node(node_id).unwrap();
        let taffy_node = self.node_map[node_id];
        let layout = self.taffy.layout(taffy_node).unwrap();

        // Convert to scene space using current context
        let scene_rect = ctx.to_scene_rect(layout);
        let z = ctx.z_index(layout.order);

        // Render this node
        self.render_view_node(painter, data, view_node, scene_rect, z);

        // Render children with updated context
        if let Some(children) = self.get_children(view_node) {
            ctx.push(layout);  // Push transform

            for child_id in children {
                self.render_node(painter, data, view, child_id, ctx);
            }

            ctx.pop();  // Pop transform
        }
    }

    fn render_view_node(
        &self,
        painter: &mut Painter,
        data: &DataDocument,
        node: &ViewNode,
        rect: Rect,
        z: i32,
    ) {
        match &node.kind {
            ViewNodeKind::FlexContainer(spec) => {
                // Render background
                if let Some(bg) = &spec.background {
                    painter.rect(rect, brush_from_bg(bg), z);
                }
                // Children are rendered by caller's recursion
            }
            ViewNodeKind::Text(spec) => {
                // Resolve data binding
                let text = resolve_text(data, node.node_id.as_ref());
                painter.text(text, rect.origin(), style_from_spec(spec), z);
            }
            ViewNodeKind::Button(spec) => {
                // Render button background
                painter.rect(rect, brush_from_surface(spec), z);
                // Render label
                let label = resolve_label(data, node.node_id.as_ref());
                painter.text(label, rect.origin(), style_from_spec(&spec.label_style), z + 1);
            }
            // ... other node types
        }
    }
}
```

---

## Zone Integration in rune-scene

### Zone Definitions (from rune-scene)

```rust
pub struct ZoneLayout {
    pub viewport: Zone,
    pub toolbar: Zone,
    pub sidebar: Zone,
    pub devtools: Zone,
}

pub struct Zone {
    /// Scene-space bounds
    pub bounds: Rect,
    /// Z-base for this zone (ensures zones don't overlap in depth)
    pub z_base: i32,
}

impl ZoneLayout {
    pub fn new(window_size: (u32, u32)) -> Self {
        let (w, h) = (window_size.0 as f32, window_size.1 as f32);

        Self {
            // Main content area
            viewport: Zone {
                bounds: Rect { x: 0.0, y: TOOLBAR_HEIGHT, width: w, height: h - TOOLBAR_HEIGHT },
                z_base: 100,  // Background layer
            },

            // Top toolbar
            toolbar: Zone {
                bounds: Rect { x: 0.0, y: 0.0, width: w, height: TOOLBAR_HEIGHT },
                z_base: 500,  // Above viewport
            },

            // Right sidebar
            sidebar: Zone {
                bounds: Rect { x: w - SIDEBAR_WIDTH, y: TOOLBAR_HEIGHT,
                              width: SIDEBAR_WIDTH, height: h - TOOLBAR_HEIGHT },
                z_base: 600,  // Above toolbar
            },

            // Bottom devtools
            devtools: Zone {
                bounds: Rect { x: 0.0, y: h - DEVTOOLS_HEIGHT,
                              width: w, height: DEVTOOLS_HEIGHT },
                z_base: 700,  // Above sidebar
            },
        }
    }
}
```

### Rendering Each Zone

```rust
impl RuneSceneApp {
    fn render_zones(&mut self, painter: &mut Painter) {
        let zones = ZoneLayout::new(self.window_size);

        // Render viewport content (IR document)
        if let Some(package) = &self.package {
            let (data, view) = package.entrypoint_documents().unwrap();
            self.ir_renderer.render(
                painter,
                data,
                view,
                zones.viewport.bounds.origin(),  // Zone origin
                zones.viewport.z_base,           // Z-base
            );
        }

        // Render toolbar (static UI)
        self.toolbar.render(
            painter,
            zones.toolbar.bounds.origin(),
            zones.toolbar.z_base,
        );

        // Render sidebar (static UI)
        self.sidebar.render(
            painter,
            zones.sidebar.bounds.origin(),
            zones.sidebar.z_base,
        );

        // Render devtools (IR document or static UI)
        self.devtools.render(
            painter,
            zones.devtools.bounds.origin(),
            zones.devtools.z_base,
        );
    }
}
```

---

## Testing Strategy: Prevent Coordinate Bugs

### 1. Visual Regression Tests

```rust
#[test]
fn test_nested_flex_coordinates() {
    // Create a nested flex layout:
    // Container (100x100 at 0,0)
    //   ├─ Child1 (50x50 at 0,0)
    //   └─ Child2 (50x50 at 50,0)

    let view = create_test_view();
    let mut renderer = IrRenderer::new();
    let mut painter = Painter::begin_frame(Viewport { width: 800, height: 600 });

    let zone_origin = Point { x: 10.0, y: 20.0 };  // Offset zone
    renderer.render(&mut painter, &data, &view, zone_origin, 100);

    let dl = painter.finish();

    // Assert Child1 is at zone_origin (10, 20)
    assert_rect_at(&dl, 0, Rect { x: 10.0, y: 20.0, width: 50.0, height: 50.0 });

    // Assert Child2 is at zone_origin + (50, 0)
    assert_rect_at(&dl, 1, Rect { x: 60.0, y: 20.0, width: 50.0, height: 50.0 });
}

#[test]
fn test_zone_isolation() {
    // Ensure toolbar zone (z=500) renders above viewport zone (z=100)
    let zones = ZoneLayout::new((800, 600));

    // Viewport content at z=100..200
    renderer.render(&mut painter, data, view, zones.viewport.bounds.origin(), 100);

    // Toolbar content at z=500..600
    toolbar.render(&mut painter, zones.toolbar.bounds.origin(), 500);

    let dl = painter.finish();

    // Assert all toolbar elements have z > all viewport elements
    assert!(min_z_in_range(&dl, 500..600) > max_z_in_range(&dl, 100..200));
}
```

### 2. Coordinate Transform Validator

```rust
/// Debug helper to validate transform correctness
pub struct CoordinateValidator {
    transforms: Vec<(String, Point, i32)>,
}

impl CoordinateValidator {
    pub fn push(&mut self, name: &str, ctx: &LayoutContext) {
        self.transforms.push((
            name.to_string(),
            ctx.offset,
            ctx.z_base,
        ));
    }

    pub fn validate(&self) {
        // Check for common mistakes:
        // 1. Offsets should monotonically increase (no negative jumps)
        // 2. Z-bases should increase with nesting depth
        // 3. Pop should restore previous values exactly

        for window in self.transforms.windows(2) {
            let (name1, offset1, z1) = &window[0];
            let (name2, offset2, z2) = &window[1];

            // Validate z-base never decreases (except on pop)
            if name2.starts_with("pop:") {
                // Pop should restore
            } else {
                assert!(z2 >= z1, "Z-base decreased from {} to {} ({} -> {})",
                        z1, z2, name1, name2);
            }
        }
    }
}
```

### 3. Reference Layout Comparison

```rust
/// Compare Taffy output to CSS reference for identical specs
#[test]
fn test_flex_matches_css() {
    // Create identical flex layout in both systems
    let view = create_flex_layout();
    let css = "
        .container { display: flex; flex-direction: row; gap: 10px; }
        .child { width: 50px; height: 50px; }
    ";

    // Render with Taffy
    let taffy_layout = render_with_taffy(&view);

    // Render with CSS (using headless browser)
    let css_layout = render_with_css(css);

    // Compare positions (should match within 1px tolerance)
    assert_layouts_equal(&taffy_layout, &css_layout, tolerance: 1.0);
}
```

---

## Common Coordinate Bug Patterns to Avoid

### ❌ Bug 1: Double-Applying Zone Offset

```rust
// WRONG: Zone offset applied twice
let rect = Rect {
    x: zone_origin.x + taffy_layout.location.x,  // zone offset
    y: zone_origin.y + taffy_layout.location.y,
    ...
};
painter.rect(
    Rect {
        x: zone_origin.x + rect.x,  // ❌ DOUBLE OFFSET!
        y: zone_origin.y + rect.y,
        ...
    },
    brush, z
);

// CORRECT: Zone offset applied once in context
let rect = ctx.to_scene_rect(taffy_layout);  // Already includes zone offset
painter.rect(rect, brush, z);
```

### ❌ Bug 2: Forgetting to Push/Pop

```rust
// WRONG: Missing push/pop
fn render_container(ctx: &mut LayoutContext) {
    render_background(ctx);
    for child in children {
        render_child(ctx);  // ❌ Child inherits parent position incorrectly
    }
}

// CORRECT: Push before children, pop after
fn render_container(ctx: &mut LayoutContext) {
    render_background(ctx);
    ctx.push(layout);  // ✅ Push container transform
    for child in children {
        render_child(ctx);
    }
    ctx.pop();  // ✅ Restore parent context
}
```

### ❌ Bug 3: Z-Index Conflicts Between Zones

```rust
// WRONG: Z-indices overlap between zones
viewport.render(painter, zone_origin, 0);    // z = 0..100
toolbar.render(painter, zone_origin, 50);    // ❌ z = 50..150 overlaps!

// CORRECT: Non-overlapping z-ranges
viewport.render(painter, zone_origin, 100);  // z = 100..199
toolbar.render(painter, zone_origin, 500);   // z = 500..599
sidebar.render(painter, zone_origin, 600);   // z = 600..699
```

### ❌ Bug 4: Parent-Relative vs. Absolute Confusion

```rust
// WRONG: Mixing coordinate systems
let parent_layout = taffy.layout(parent).unwrap();
let child_layout = taffy.layout(child).unwrap();

// ❌ child_layout.location is parent-relative, not absolute!
painter.rect(
    Rect {
        x: child_layout.location.x,  // ❌ Missing parent offset!
        y: child_layout.location.y,
        ...
    },
    brush, z
);

// CORRECT: Accumulate offsets
let absolute_x = parent_layout.location.x + child_layout.location.x;
let absolute_y = parent_layout.location.y + child_layout.location.y;
```

---

## Implementation Checklist

### Phase 1: Taffy Integration (Week 1)
- [ ] Add `taffy = "0.3"` to `rune-ir/Cargo.toml`
- [ ] Implement `LayoutContext` with transform stack
- [ ] Wire `ViewDocument` → `TaffyTree` builder
- [ ] Test basic flex layout (single container, no nesting)

### Phase 2: Coordinate System (Week 1-2)
- [ ] Implement zone-aware rendering
- [ ] Test nested containers (push/pop transforms)
- [ ] Validate z-index isolation between zones
- [ ] Write visual regression tests

### Phase 3: Full IR Rendering (Week 2-3)
- [ ] Render all ViewNodeKind types
- [ ] Handle data bindings (resolve text from DataDocument)
- [ ] Implement widget rendering (buttons, inputs, etc.)
- [ ] Test complex layouts (3+ levels of nesting)

### Phase 4: Dynamic Updates (Week 3-4)
- [ ] Implement mutation handling (ReplaceText, IrDiff)
- [ ] Incremental layout (only re-layout dirty subtrees)
- [ ] Test coordinate stability during updates

---

## Debugging Tools

### 1. Layout Inspector

```rust
/// Visual overlay showing layout boxes and transforms
pub fn debug_render_layout(painter: &mut Painter, ctx: &LayoutContext, layout: &taffy::Layout) {
    let rect = ctx.to_scene_rect(layout);
    let z = ctx.z_index(layout.order);

    // Draw debug box
    painter.rect(
        rect,
        Brush::Solid([255, 0, 0, 50]),  // Semi-transparent red
        z + 10000,  // Always on top
    );

    // Draw coordinate label
    let label = format!("({:.0}, {:.0}) z={}", rect.x, rect.y, z);
    painter.text(label, rect.origin(), TextStyle::debug(), z + 10001);
}
```

### 2. Transform Stack Tracer

```rust
impl LayoutContext {
    pub fn trace(&self, msg: &str) {
        if cfg!(debug_assertions) {
            println!("[LayoutContext] {}: offset=({:.2}, {:.2}) z={} depth={}",
                     msg, self.offset.x, self.offset.y, self.z_base, self.stack.len());
        }
    }
}

// Usage:
ctx.trace("before push");
ctx.push(layout);
ctx.trace("after push");
```

---

## Summary

**Key Insight**: Taffy is solid, but coordinate transforms are the footgun.

**Solution**: Explicit `LayoutContext` that tracks the transform chain from Taffy's parent-relative coordinates to rune-draw's absolute scene coordinates.

**Three Rules**:
1. **Always use `LayoutContext::to_scene_rect()`** - Never manually compute positions
2. **Always push/pop around children** - Never skip the transform stack
3. **Allocate non-overlapping z-ranges per zone** - Never let zones interfere

**Result**: Robust Taffy integration with clear coordinate semantics and testable transforms.

---

**Ready to implement `LayoutContext` and wire up Taffy?**
