//! Coordinate system management for Taffy layout integration.
//!
//! This module provides the `LayoutContext` type which manages the transform
//! chain from Taffy's parent-relative layout coordinates to rune-draw's absolute
//! scene coordinates.

use engine_core::Rect;

/// A point in 2D space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A transform snapshot for push/pop operations.
#[derive(Debug, Clone, Copy)]
struct Transform {
    offset: Point,
    z_base: i32,
}

/// Manages coordinate transforms from Taffy layout space to scene space.
///
/// # Coordinate Systems
///
/// - **Taffy Layout Space**: Parent-relative coordinates from Taffy layout engine
/// - **Zone Space**: Zone-relative coordinates (each zone has its own origin)
/// - **Scene Space**: Absolute viewport coordinates used by the Painter
///
/// # Usage
///
/// ```ignore
/// let mut ctx = LayoutContext::new(zone_origin, z_base);
///
/// // Render parent container
/// let parent_rect = ctx.to_scene_rect(&parent_layout);
/// painter.rect(parent_rect, brush, ctx.z_index(parent_layout.order));
///
/// // Render children with accumulated transform
/// ctx.push(&parent_layout);
/// for child_layout in children {
///     let child_rect = ctx.to_scene_rect(&child_layout);
///     painter.rect(child_rect, brush, ctx.z_index(child_layout.order));
/// }
/// ctx.pop();
/// ```
#[derive(Debug)]
pub struct LayoutContext {
    /// Current absolute position in scene space (accumulated transform)
    offset: Point,

    /// Current z-index base (depth in tree)
    z_base: i32,

    /// Transform stack for nested containers
    stack: Vec<Transform>,
}

impl LayoutContext {
    /// Create a new root context for a zone.
    ///
    /// # Arguments
    ///
    /// - `zone_origin`: The top-left corner of the zone in scene space
    /// - `z_base`: The base z-index for this zone (ensures zones don't overlap in depth)
    pub fn new(zone_origin: Point, z_base: i32) -> Self {
        Self {
            offset: zone_origin,
            z_base,
            stack: Vec::new(),
        }
    }

    /// Push a transform for entering a container.
    ///
    /// This accumulates the container's position and increments the z-base for depth ordering.
    /// Must be paired with `pop()` when leaving the container.
    ///
    /// # Arguments
    ///
    /// - `layout`: The Taffy layout for the container being entered
    pub fn push(&mut self, layout: &taffy::Layout) {
        // Save current state
        self.stack.push(Transform {
            offset: self.offset,
            z_base: self.z_base,
        });

        // Accumulate offset: parent offset + layout location
        self.offset.x += layout.location.x;
        self.offset.y += layout.location.y;

        // Increment z-base for depth ordering (children render above parent)
        self.z_base += 1;
    }

    /// Pop a transform when leaving a container.
    ///
    /// Restores the coordinate system state from before the matching `push()`.
    pub fn pop(&mut self) {
        if let Some(prev) = self.stack.pop() {
            self.offset = prev.offset;
            self.z_base = prev.z_base;
        } else {
            #[cfg(debug_assertions)]
            eprintln!("[LayoutContext] Warning: pop() called with empty stack");
        }
    }

    /// Convert a Taffy layout to a scene-space rect.
    ///
    /// The returned rect has absolute coordinates in scene space, ready to pass to the Painter.
    pub fn to_scene_rect(&self, layout: &taffy::Layout) -> Rect {
        Rect {
            x: self.offset.x + layout.location.x,
            y: self.offset.y + layout.location.y,
            w: layout.size.width,
            h: layout.size.height,
        }
    }

    /// Get the current z-index for rendering.
    ///
    /// Combines the z-base (depth in container hierarchy) with Taffy's order value.
    ///
    /// # Arguments
    ///
    /// - `taffy_order`: The order value from Taffy layout (typically 0 for most nodes)
    pub fn z_index(&self, taffy_order: u32) -> i32 {
        self.z_base + taffy_order as i32
    }

    /// Get the current absolute offset in scene space.
    ///
    /// Useful for positioning elements that don't come from Taffy (e.g., text cursors).
    pub fn offset(&self) -> Point {
        self.offset
    }

    /// Get the current z-base.
    pub fn z_base(&self) -> i32 {
        self.z_base
    }

    /// Get the current nesting depth (number of pushed transforms).
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Debug trace for coordinate validation.
    ///
    /// Prints the current context state in debug builds. No-op in release builds.
    #[allow(unused_variables)]
    pub fn trace(&self, msg: &str) {
        #[cfg(debug_assertions)]
        {
            println!(
                "[LayoutContext] {}: offset=({:.2}, {:.2}) z={} depth={}",
                msg,
                self.offset.x,
                self.offset.y,
                self.z_base,
                self.stack.len()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_context() {
        let ctx = LayoutContext::new(Point::new(10.0, 20.0), 100);
        assert_eq!(ctx.offset(), Point::new(10.0, 20.0));
        assert_eq!(ctx.z_base(), 100);
        assert_eq!(ctx.depth(), 0);
    }

    #[test]
    fn test_push_pop() {
        let mut ctx = LayoutContext::new(Point::new(10.0, 20.0), 100);

        // Create a mock layout
        let layout = taffy::Layout {
            order: 0,
            size: taffy::Size {
                width: 100.0,
                height: 50.0,
            },
            location: taffy::Point { x: 5.0, y: 10.0 },
            ..Default::default()
        };

        ctx.push(&layout);

        // After push, offset should be accumulated
        assert_eq!(ctx.offset(), Point::new(15.0, 30.0)); // 10+5, 20+10
        assert_eq!(ctx.z_base(), 101); // 100+1
        assert_eq!(ctx.depth(), 1);

        ctx.pop();

        // After pop, state should be restored
        assert_eq!(ctx.offset(), Point::new(10.0, 20.0));
        assert_eq!(ctx.z_base(), 100);
        assert_eq!(ctx.depth(), 0);
    }

    #[test]
    fn test_nested_push_pop() {
        let mut ctx = LayoutContext::new(Point::ZERO, 0);

        let layout1 = taffy::Layout {
            location: taffy::Point { x: 10.0, y: 20.0 },
            ..Default::default()
        };

        let layout2 = taffy::Layout {
            location: taffy::Point { x: 5.0, y: 10.0 },
            ..Default::default()
        };

        ctx.push(&layout1);
        assert_eq!(ctx.offset(), Point::new(10.0, 20.0));
        assert_eq!(ctx.z_base(), 1);

        ctx.push(&layout2);
        assert_eq!(ctx.offset(), Point::new(15.0, 30.0)); // 10+5, 20+10
        assert_eq!(ctx.z_base(), 2);

        ctx.pop();
        assert_eq!(ctx.offset(), Point::new(10.0, 20.0));
        assert_eq!(ctx.z_base(), 1);

        ctx.pop();
        assert_eq!(ctx.offset(), Point::ZERO);
        assert_eq!(ctx.z_base(), 0);
    }

    #[test]
    fn test_to_scene_rect() {
        let ctx = LayoutContext::new(Point::new(100.0, 200.0), 500);

        let layout = taffy::Layout {
            location: taffy::Point { x: 10.0, y: 20.0 },
            size: taffy::Size {
                width: 50.0,
                height: 30.0,
            },
            ..Default::default()
        };

        let rect = ctx.to_scene_rect(&layout);

        assert_eq!(rect.x, 110.0); // 100 + 10
        assert_eq!(rect.y, 220.0); // 200 + 20
        assert_eq!(rect.w, 50.0);
        assert_eq!(rect.h, 30.0);
    }

    #[test]
    fn test_z_index() {
        let ctx = LayoutContext::new(Point::ZERO, 100);

        assert_eq!(ctx.z_index(0), 100);
        assert_eq!(ctx.z_index(5), 105);
        assert_eq!(ctx.z_index(10), 110);
    }

    #[test]
    fn test_pop_empty_stack() {
        let mut ctx = LayoutContext::new(Point::ZERO, 0);

        // Should not panic, just warn in debug mode
        ctx.pop();

        // State should remain unchanged
        assert_eq!(ctx.offset(), Point::ZERO);
        assert_eq!(ctx.z_base(), 0);
    }
}
