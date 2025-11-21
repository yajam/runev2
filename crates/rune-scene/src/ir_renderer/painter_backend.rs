//! Optional Painter-based backend for `IrRenderer`.
//!
//! This is kept in a separate module so the core IR renderer stays
//! focused on Canvas-based rendering.

use anyhow::{Context as AnyhowContext, Result};
use engine_core::{Brush, ColorLinPremul, Painter, Rect};
use rune_ir::{
    data::document::DataDocument,
    view::{ViewDocument, ViewNodeId, ViewNodeKind},
};
use taffy::prelude::*;

use crate::layout::{LayoutContext, Point};

use super::core::IrRenderer;
use super::style::brush_from_background;

impl IrRenderer {
    /// Render a ViewDocument + DataDocument directly into a `Painter`.
    ///
    /// This lower-level API is primarily used by experimental integrations.
    #[allow(unused_variables)]
    pub fn render(
        &mut self,
        painter: &mut Painter,
        data: &DataDocument,
        view: &ViewDocument,
        zone_origin: Point,
        z_base: i32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Result<()> {
        // Clear previous state
        self.taffy.clear();
        self.node_map.clear();

        // Build Taffy tree from ViewDocument
        let root_id = self
            .build_taffy_tree(view, data, &view.root, true) // true = is root node
            .context("Failed to build Taffy tree")?;

        // Compute layout
        let available = Size {
            width: AvailableSpace::Definite(viewport_width),
            height: AvailableSpace::Definite(viewport_height),
        };

        self.compute_layout_with_measure(root_id, available, view, data)
            .context("Failed to compute layout")?;

        // Render with transform tracking
        let mut ctx = LayoutContext::new(zone_origin, z_base);
        render_node(self, painter, data, view, &view.root, &mut ctx)
    }
}

/// Render a single node and its children recursively into a Painter.
fn render_node(
    renderer: &IrRenderer,
    painter: &mut Painter,
    data: &DataDocument,
    view: &ViewDocument,
    view_node_id: &ViewNodeId,
    ctx: &mut LayoutContext,
) -> Result<()> {
    let view_node = view
        .node(view_node_id)
        .with_context(|| format!("View node not found: {}", view_node_id))?;

    let taffy_node = renderer
        .node_map
        .get(view_node_id)
        .with_context(|| format!("Taffy node not found for: {}", view_node_id))?;

    let layout = renderer
        .taffy
        .layout(*taffy_node)
        .context("Failed to get layout")?;

    // Convert to scene space using current context
    let scene_rect = ctx.to_scene_rect(layout);
    let z = ctx.z_index(layout.order);

    // Render this node directly with Painter.
    let rect = Rect {
        x: scene_rect.x,
        y: scene_rect.y,
        w: scene_rect.w,
        h: scene_rect.h,
    };

    match &view_node.kind {
        ViewNodeKind::FlexContainer(spec) => {
            // Render background if present
            if let Some(bg) = &spec.background {
                let brush = brush_from_background(bg);
                painter.rect(rect, brush, z);
            }
            // Children are rendered by recursion
        }
        ViewNodeKind::Text(_spec) => {
            // Placeholder: text rendering via Painter is not wired yet.
            painter.rect(
                rect,
                Brush::Solid(ColorLinPremul::from_srgba_u8([200, 200, 200, 255])),
                z,
            );
        }
        ViewNodeKind::Button(spec) => {
            // Render button background
            if let Some(bg) = &spec.style.background {
                let brush = brush_from_background(bg);
                painter.rect(rect, brush, z);
            }
            // TODO: Render label text
        }
        ViewNodeKind::Image(_spec) => {
            // TODO: Render image
            painter.rect(
                rect,
                Brush::Solid(ColorLinPremul::from_srgba_u8([150, 150, 200, 255])),
                z,
            );
        }
        _ => {
            // Placeholder for unimplemented node types
            painter.rect(
                rect,
                Brush::Solid(ColorLinPremul::from_srgba_u8([220, 220, 220, 255])),
                z,
            );
        }
    }

    // Render children recursively
    let children = renderer.get_children(view_node);
    if !children.is_empty() {
        ctx.push(layout);

        for child_id in children {
            render_node(renderer, painter, data, view, child_id, ctx)?;
        }

        ctx.pop();
    }

    Ok(())
}
