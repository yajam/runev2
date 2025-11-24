//! Core IR renderer implementation with Taffy integration.

use std::collections::HashMap;

use anyhow::{Context as AnyhowContext, Result};
use rune_ir::{
    data::document::DataDocument,
    view::{
        FlexContainerSpec, FormContainerSpec, LayoutDirection, ViewDocument, ViewNode, ViewNodeId,
        ViewNodeKind,
    },
};
use taffy::prelude::*;

use crate::layout::{LayoutContext, Point};

use super::elements;
use super::style::{
    auto_dimension, dimension, layout_align_to_taffy, layout_justify_to_taffy, length,
    length_percentage,
};

/// Maps ViewDocument nodes to Taffy nodes.
type NodeId = taffy::NodeId;

/// IR renderer that converts ViewDocument + DataDocument to display list.
///
/// # Coordinate System
///
/// The renderer uses `LayoutContext` to manage the transform chain:
/// - Taffy computes parent-relative layout
/// - LayoutContext tracks absolute scene-space positions
/// - Painter receives absolute scene-space rects with z-indices
///
/// # Interactivity
///
/// The renderer maintains stateful elements (inputs, buttons, etc.) in `element_state`
/// and generates hit regions for interactive elements using `hit_registry`.
///
/// # Usage
///
/// ```ignore
/// let mut renderer = IrRenderer::new();
/// renderer.render(
///     &mut painter,
///     &data_document,
///     &view_document,
///     Point::new(0.0, 0.0),  // zone origin
///     100,                   // z-base
/// )?;
/// ```
pub struct IrRenderer {
    /// Taffy layout tree (context stores the ViewNodeId for each layout node)
    pub(super) taffy: TaffyTree<Option<ViewNodeId>>,

    /// Maps ViewNodeId to Taffy NodeId
    pub(super) node_map: HashMap<ViewNodeId, NodeId>,

    /// Root node for the current Taffy tree (stable across frames)
    pub(super) root_node: Option<NodeId>,

    /// Signature of the last built ViewDocument (root id + node count)
    pub(super) last_view_signature: Option<(ViewNodeId, usize)>,

    /// Last layout size used for layout computation
    pub(super) last_layout_size: Option<(u32, u32)>,

    /// Last rendered content height in scene space (used for scrolling)
    pub(super) last_content_height: f32,
    /// Last rendered content width in scene space (used for horizontal scrolling)
    pub(super) last_content_width: f32,

    /// Current content height (layout extent) for element positioning like popups
    pub(super) current_content_height: f32,

    /// State for interactive IR elements (inputs, buttons, etc.)
    pub(super) element_state: super::state::IrElementState,

    /// Registry for mapping ViewNodeId ↔ hit region IDs
    pub(super) hit_registry: super::hit_region::HitRegionRegistry,
}

impl IrRenderer {
    /// Create a new IR renderer.
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
            node_map: HashMap::new(),
            root_node: None,
            last_view_signature: None,
            last_layout_size: None,
            last_content_height: 0.0,
            last_content_width: 0.0,
            current_content_height: 800.0,
            element_state: super::state::IrElementState::new(),
            hit_registry: super::hit_region::HitRegionRegistry::new(),
        }
    }

    /// Get mutable access to element state (for event handling)
    pub fn element_state_mut(&mut self) -> &mut super::state::IrElementState {
        &mut self.element_state
    }

    /// Get access to element state (for reading)
    pub fn element_state(&self) -> &super::state::IrElementState {
        &self.element_state
    }

    /// Get access to hit region registry
    pub fn hit_registry(&self) -> &super::hit_region::HitRegionRegistry {
        &self.hit_registry
    }

    /// Render IR content into a `Canvas` at a given offset/size.
    ///
    /// This is the primary rendering method for embedding IR content within
    /// a viewport. Returns (content_height, content_width) on success.
    pub fn render_canvas_at_offset(
        &mut self,
        canvas: &mut rune_surface::Canvas,
        data_doc: &DataDocument,
        view_doc: &ViewDocument,
        offset_x: f32,
        offset_y: f32,
        width: f32,
        layout_height: f32,
        viewport_height: f32,
        scroll_offset_x: f32,
        scroll_offset_y: f32,
        provider: &dyn engine_core::TextProvider,
    ) -> Result<(f32, f32)> {
        let debug_logging = std::env::var("RUNE_IR_DEBUG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
            .unwrap_or(false);

        if debug_logging {
            eprintln!("=== render_ir_document START ===");
            eprintln!(
                "  viewport: {}x{} at offset ({}, {})",
                width, layout_height, offset_x, offset_y
            );
            eprintln!("  root: {}", view_doc.root);
            eprintln!("  total nodes: {}", view_doc.nodes.len());
        }

        let view_signature = (view_doc.root.clone(), view_doc.nodes.len());
        let needs_rebuild = self
            .last_view_signature
            .as_ref()
            .map(|sig| sig != &view_signature)
            .unwrap_or(true);

        if needs_rebuild {
            // Rebuild tree only when view changes to avoid churn on every redraw.
            self.taffy = TaffyTree::new();
            self.node_map.clear();
            self.hit_registry.clear();

            if debug_logging {
                eprintln!("Building Taffy layout tree (view changed)...");
            }
            let root_id = self
                .build_taffy_tree(view_doc, data_doc, &view_doc.root, true) // true = is root node
                .context("Failed to build Taffy tree")?;
            self.root_node = Some(root_id);
            self.last_view_signature = Some(view_signature.clone());
            if debug_logging {
                eprintln!("✓ Taffy tree built, root_id: {:?}", root_id);
            }
        }

        let root_id = self
            .root_node
            .context("Taffy root node missing; build step did not run")?;

        // Compute layout
        let available = taffy::prelude::Size {
            width: taffy::prelude::AvailableSpace::Definite(width),
            height: taffy::prelude::AvailableSpace::Definite(layout_height),
        };
        self.last_layout_size = Some((width.round() as u32, layout_height.round() as u32));

        if debug_logging {
            eprintln!("Computing layout...");
        }
        self.compute_layout_with_measure(root_id, available, view_doc, data_doc)
            .context("Failed to compute layout")?;
        if debug_logging {
            eprintln!("✓ Layout computed");
        }

        // Render root node and children recursively using elements
        // Use the viewport offset as the origin
        let mut ctx = LayoutContext::new(Point::new(offset_x, offset_y), 0);

        if debug_logging {
            eprintln!("Starting recursive render...");
        }
        self.last_content_height = 0.0;
        self.last_content_width = 0.0;
        // Base popup positioning on the visible viewport height so it doesn't depend
        // on prior content measurements or render feedback.
        self.current_content_height = viewport_height;

        self.render_view_node_with_elements(
            canvas,
            data_doc,
            view_doc,
            &view_doc.root,
            &mut ctx,
            provider,
        )?;

        // Render active overlays on top
        self.render_active_overlays(
            canvas,
            data_doc,
            view_doc,
            width,
            viewport_height,
            scroll_offset_x,
            scroll_offset_y,
            provider,
        )?;

        let content_height = (self.last_content_height - offset_y).max(layout_height);
        let content_width = (self.last_content_width - offset_x).max(width);

        // Ensure the root container background extends to the full IR content
        // height, not just the initial viewport height, so page backgrounds
        // fill the entire scrollable area.
        if let Some(root_node) = view_doc.node(&view_doc.root) {
            use engine_core::Rect;
            use rune_ir::view::ViewNodeKind;

            let background = match &root_node.kind {
                ViewNodeKind::FlexContainer(spec) => &spec.background,
                ViewNodeKind::GridContainer(spec) => &spec.background,
                ViewNodeKind::FormContainer(spec) => &spec.background,
                _ => &None,
            };

            if background.is_some() {
                let bg_rect = Rect {
                    x: offset_x,
                    y: offset_y,
                    w: width,
                    h: content_height,
                };
                // Use a low z so all content and overlays render above it.
                super::elements::render_background_element(canvas, background, bg_rect, 0);
            }
        }

        if debug_logging {
            eprintln!("=== render_ir_document END ===\n");
        }
        Ok((content_height, content_width))
    }

    /// Render IR content into a `Canvas` at origin (0,0).
    pub(crate) fn render_canvas(
        &mut self,
        canvas: &mut rune_surface::Canvas,
        data_doc: &DataDocument,
        view_doc: &ViewDocument,
        width: f32,
        height: f32,
        provider: &dyn engine_core::TextProvider,
    ) -> Result<(f32, f32)> {
        self.render_canvas_at_offset(
            canvas, data_doc, view_doc, 0.0, 0.0, width, height, height, 0.0, 0.0, provider,
        )
    }

    /// Build Taffy tree from ViewDocument recursively.
    pub(crate) fn build_taffy_tree(
        &mut self,
        view: &ViewDocument,
        data: &DataDocument,
        view_node_id: &ViewNodeId,
        is_root: bool,
    ) -> Result<NodeId> {
        let view_node = view
            .node(view_node_id)
            .with_context(|| format!("View node not found: {}", view_node_id))?;

        // Convert ViewNode to Taffy style
        let mut style = self.view_node_to_taffy_style(view_node, data)?;

        // Force root node to fill viewport width and be at least viewport height,
        // while still expanding to fit content (including padding) when taller.
        if is_root {
            style.size.width = percent(1.0);
            style.size.height = percent(1.0);
            style.min_size.width = percent(1.0);
            style.min_size.height = percent(1.0);
        }

        // Recursively build children
        let child_ids: Vec<ViewNodeId> = if matches!(
            view_node.kind,
            ViewNodeKind::Alert(_) | ViewNodeKind::Modal(_) | ViewNodeKind::Confirm(_)
        ) {
            // Overlays should not influence normal layout; skip their children here.
            Vec::new()
        } else {
            self.get_children(view_node).into_iter().cloned().collect()
        };
        let mut child_nodes = Vec::new();

        for child_id in &child_ids {
            let child_node_id = self.build_taffy_tree(view, data, child_id, false)?; // children are not root
            child_nodes.push(child_node_id);
        }

        // Create Taffy node. Leaf nodes (no children) use `new_leaf` so they can
        // participate in custom measurement (text/table/link) and avoid the
        // zero-height collapse that was happening when empty containers were
        // built with `new_with_children`.
        let taffy_node = if child_nodes.is_empty() {
            self.taffy
                .new_leaf(style)
                .context("Failed to create Taffy leaf node")?
        } else {
            self.taffy
                .new_with_children(style, &child_nodes)
                .context("Failed to create Taffy node")?
        };

        // Store mapping
        self.node_map.insert(view_node_id.clone(), taffy_node);

        // Store view node id as Taffy context (used for measurement)
        if matches!(
            view_node.kind,
            ViewNodeKind::Text(_) | ViewNodeKind::Table(_)
        ) {
            self.taffy
                .set_node_context(taffy_node, Some(Some(view_node_id.clone())))
                .context("Failed to attach context to measured node")?;
        }

        Ok(taffy_node)
    }

    /// Convert ViewNode to Taffy Style.
    fn view_node_to_taffy_style(&self, node: &ViewNode, data_doc: &DataDocument) -> Result<Style> {
        match &node.kind {
            ViewNodeKind::FlexContainer(spec) => self.flex_container_style(spec),
            ViewNodeKind::FormContainer(spec) => self.form_container_style(spec),
            ViewNodeKind::Text(spec) => Ok(self.text_style_from_spec(spec)),
            ViewNodeKind::Button(spec) => Ok(self.button_style(spec)),
            ViewNodeKind::Image(spec) => Ok(self.image_style(spec)),
            ViewNodeKind::Table(spec) => Ok(self.table_style(spec)),
            ViewNodeKind::Link(spec) => Ok(self.link_style(spec)),
            ViewNodeKind::InputBox(spec) => Ok(self.input_box_style(spec)),
            ViewNodeKind::TextArea(spec) => Ok(self.text_area_style(spec)),
            ViewNodeKind::Checkbox(spec) => Ok(self.checkbox_style(spec, node, data_doc)),
            ViewNodeKind::Radio(spec) => Ok(self.radio_style(spec, node, data_doc)),
            ViewNodeKind::Select(spec) => Ok(self.select_style(spec)),
            ViewNodeKind::FileInput(spec) => Ok(self.file_input_style(spec)),
            ViewNodeKind::DatePicker(spec) => Ok(self.date_picker_style(spec)),
            ViewNodeKind::WebView(spec) => Ok(self.webview_style(spec)),
            ViewNodeKind::Alert(_) | ViewNodeKind::Modal(_) | ViewNodeKind::Confirm(_) => {
                Ok(Style {
                    // Overlays are rendered separately; exclude them from flow layout.
                    display: Display::None,
                    ..Default::default()
                })
            }
            // Add more as needed
            _ => Ok(Style::default()),
        }
    }

    /// Convert FlexContainerSpec to Taffy Style.
    fn flex_container_style(&self, spec: &FlexContainerSpec) -> Result<Style> {
        Ok(Style {
            display: Display::Flex,
            flex_direction: match spec.layout.direction {
                LayoutDirection::Row => FlexDirection::Row,
                LayoutDirection::Column => FlexDirection::Column,
            },
            align_items: layout_align_to_taffy(spec.layout.align),
            justify_content: layout_justify_to_taffy(spec.layout.justify),
            flex_wrap: if spec.layout.wrap {
                FlexWrap::Wrap
            } else {
                FlexWrap::NoWrap
            },
            gap: Size {
                width: length_percentage(spec.layout.gap as f32),
                height: length_percentage(spec.layout.gap as f32),
            },
            padding: taffy::Rect {
                left: length_percentage(spec.padding.left as f32),
                right: length_percentage(spec.padding.right as f32),
                top: length_percentage(spec.padding.top as f32),
                bottom: length_percentage(spec.padding.bottom as f32),
            },
            margin: taffy::Rect {
                left: self.margin_value(spec.margin.left as f32, spec.margin_left_auto),
                right: self.margin_value(spec.margin.right as f32, spec.margin_right_auto),
                top: length(spec.margin.top as f32),
                bottom: length(spec.margin.bottom as f32),
            },
            size: Size {
                width: spec
                    .width
                    .map(|w| dimension(w as f32))
                    .unwrap_or(auto_dimension()),
                height: spec
                    .height
                    .map(|h| dimension(h as f32))
                    .unwrap_or(auto_dimension()),
            },
            min_size: Size {
                width: spec
                    .min_width
                    .map(|w| dimension(w as f32))
                    .unwrap_or(auto_dimension()),
                height: spec
                    .min_height
                    .map(|h| dimension(h as f32))
                    .unwrap_or(auto_dimension()),
            },
            max_size: Size {
                width: spec
                    .max_width
                    .map(|w| dimension(w as f32))
                    .unwrap_or(auto_dimension()),
                height: spec
                    .max_height
                    .map(|h| dimension(h as f32))
                    .unwrap_or(auto_dimension()),
            },
            flex_shrink: 0.0,
            ..Default::default()
        })
    }

    /// Handle margin auto values.
    fn margin_value(&self, value: f32, auto: bool) -> LengthPercentageAuto {
        if auto {
            LengthPercentageAuto::Auto
        } else {
            length(value)
        }
    }

    /// Text style with intrinsic height derived from font metrics so text nodes
    /// take up space in flex layouts instead of collapsing to zero height.
    fn text_style_from_spec(&self, spec: &rune_ir::view::TextSpec) -> Style {
        let font_size = spec.style.font_size.unwrap_or(16.0) as f32;
        let line_height = spec
            .style
            .line_height
            .map(|lh| lh as f32)
            .unwrap_or(font_size * 1.2);
        let pad_top = spec.style.padding.top as f32;
        let pad_bottom = spec.style.padding.bottom as f32;
        let min_height = (line_height + pad_top + pad_bottom).max(font_size);

        Style {
            padding: taffy::Rect {
                left: length_percentage(spec.style.padding.left as f32),
                right: length_percentage(spec.style.padding.right as f32),
                top: length_percentage(pad_top),
                bottom: length_percentage(pad_bottom),
            },
            margin: taffy::Rect {
                left: length(spec.style.margin.left as f32),
                right: length(spec.style.margin.right as f32),
                top: length(spec.style.margin.top as f32),
                bottom: length(spec.style.margin.bottom as f32),
            },
            size: Size {
                width: auto_dimension(),
                height: auto_dimension(), // allow measurement to drive height
            },
            min_size: Size {
                width: auto_dimension(),
                height: dimension(min_height),
            },
            flex_shrink: 0.0,
            ..Default::default()
        }
    }

    /// Convert SurfaceStyle to Taffy Style.
    fn surface_style(&self, spec: &rune_ir::view::SurfaceStyle) -> Style {
        Style {
            padding: taffy::Rect {
                left: length_percentage(spec.padding.left as f32),
                right: length_percentage(spec.padding.right as f32),
                top: length_percentage(spec.padding.top as f32),
                bottom: length_percentage(spec.padding.bottom as f32),
            },
            margin: taffy::Rect {
                left: length(spec.margin.left as f32),
                right: length(spec.margin.right as f32),
                top: length(spec.margin.top as f32),
                bottom: length(spec.margin.bottom as f32),
            },
            size: Size {
                width: spec
                    .width
                    .map(|w| dimension(w as f32))
                    .unwrap_or(auto_dimension()),
                height: spec
                    .height
                    .map(|h| dimension(h as f32))
                    .unwrap_or(auto_dimension()),
            },
            min_size: Size {
                width: spec
                    .min_width
                    .map(|w| dimension(w as f32))
                    .unwrap_or(auto_dimension()),
                height: spec
                    .min_height
                    .map(|h| dimension(h as f32))
                    .unwrap_or(auto_dimension()),
            },
            max_size: Size {
                width: spec
                    .max_width
                    .map(|w| dimension(w as f32))
                    .unwrap_or(auto_dimension()),
                height: spec
                    .max_height
                    .map(|h| dimension(h as f32))
                    .unwrap_or(auto_dimension()),
            },
            align_self: None, // Let parent align_items decide unless explicitly set in style.
            flex_shrink: 0.0,
            ..Default::default()
        }
    }

    /// Links behave like text with padding, so give them an intrinsic height
    /// based on label metrics to keep them in normal flow.
    fn link_style(&self, spec: &rune_ir::view::LinkSpec) -> Style {
        let mut style = self.surface_style(&spec.style);

        let font_size = spec.label_style.font_size.unwrap_or(16.0) as f32;
        let line_height = spec
            .label_style
            .line_height
            .map(|lh| lh as f32)
            .unwrap_or(font_size * 1.2);
        let intrinsic_height =
            line_height + spec.style.padding.top as f32 + spec.style.padding.bottom as f32;

        if matches!(style.min_size.height, Dimension::Auto) {
            style.min_size.height = dimension(intrinsic_height.max(font_size));
        }

        style
    }

    /// Convert FormContainerSpec to Taffy Style (same flex defaults as containers).
    fn form_container_style(&self, spec: &FormContainerSpec) -> Result<Style> {
        Ok(Style {
            display: Display::Flex,
            flex_direction: match spec.layout.direction {
                LayoutDirection::Row => FlexDirection::Row,
                LayoutDirection::Column => FlexDirection::Column,
            },
            align_items: layout_align_to_taffy(spec.layout.align),
            justify_content: layout_justify_to_taffy(spec.layout.justify),
            flex_wrap: if spec.layout.wrap {
                FlexWrap::Wrap
            } else {
                FlexWrap::NoWrap
            },
            gap: Size {
                width: length_percentage(spec.layout.gap as f32),
                height: length_percentage(spec.layout.gap as f32),
            },
            padding: taffy::Rect {
                left: length_percentage(spec.padding.left as f32),
                right: length_percentage(spec.padding.right as f32),
                top: length_percentage(spec.padding.top as f32),
                bottom: length_percentage(spec.padding.bottom as f32),
            },
            margin: taffy::Rect {
                left: length(spec.margin.left as f32),
                right: length(spec.margin.right as f32),
                top: length(spec.margin.top as f32),
                bottom: length(spec.margin.bottom as f32),
            },
            size: Size {
                width: spec
                    .width
                    .map(|w| dimension(w as f32))
                    .unwrap_or(auto_dimension()),
                height: spec
                    .height
                    .map(|h| dimension(h as f32))
                    .unwrap_or(auto_dimension()),
            },
            flex_shrink: 0.0,
            ..Default::default()
        })
    }

    /// InputBox style combines SurfaceStyle with width fallback and default height.
    fn input_box_style(&self, spec: &rune_ir::view::InputBoxSpec) -> Style {
        let mut style = self.surface_style(&spec.style);

        if let Some(w) = spec.width {
            style.size.width = dimension(w as f32);
        }

        if matches!(style.size.height, Dimension::Auto) {
            let h = spec.style.height.unwrap_or(44.0) as f32;
            style.size.height = dimension(h);
            style.min_size.height = dimension(h);
        }

        style.flex_shrink = 0.0;
        style
    }

    /// Button style: use flex properties for proper sizing in flex containers.
    fn button_style(&self, spec: &rune_ir::view::ButtonSpec) -> Style {
        let mut style = self.surface_style(&spec.style);

        // Don't force 100% width - let flex container manage sizing
        // If width is explicitly set in spec, it will be applied via surface_style
        // Otherwise, use flex_grow to distribute space
        if matches!(style.size.width, Dimension::Auto) {
            style.flex_grow = 1.0; // Grow to fill available space
        }

        if matches!(style.size.height, Dimension::Auto) {
            let h = spec.style.height.unwrap_or(44.0) as f32;
            style.size.height = dimension(h);
            style.min_size.height = dimension(h);
        }

        style.flex_shrink = 1.0; // Allow shrinking if needed
        style
    }

    /// TextArea style combines SurfaceStyle with width/height defaults.
    fn text_area_style(&self, spec: &rune_ir::view::TextAreaSpec) -> Style {
        let mut style = self.surface_style(&spec.style);

        if let Some(w) = spec.width {
            style.size.width = dimension(w as f32);
        }

        // Give text areas a reasonable height if not specified in style.
        if matches!(style.size.height, Dimension::Auto) {
            let h = spec.style.height.unwrap_or(140.0) as f32;
            style.size.height = dimension(h);
            style.min_size.height = dimension(h);
        }

        style.flex_shrink = 0.0;
        style
    }

    /// Checkbox style: fix box size while reserving horizontal room for label text.
    fn checkbox_style(
        &self,
        spec: &rune_ir::view::CheckboxSpec,
        node: &ViewNode,
        data_doc: &DataDocument,
    ) -> Style {
        let box_size = spec.size.unwrap_or(18.0) as f32;

        let mut style = Style {
            size: Size {
                width: dimension(box_size),
                height: dimension(box_size),
            },
            min_size: Size {
                width: dimension(box_size),
                height: dimension(box_size),
            },
            flex_shrink: 0.0,
            ..Default::default()
        };

        if let Some(w) = spec.style.width {
            style.size.width = dimension(w as f32);
        }
        if let Some(h) = spec.style.height {
            style.size.height = dimension(h as f32);
            style.min_size.height = dimension(h as f32);
        }

        // Reserve horizontal space using right margin so labels don't overlap siblings.
        if let Some(label) = node
            .node_id
            .as_ref()
            .and_then(|nid| elements::resolve_text_from_data(data_doc, nid))
        {
            let label_size = 16.0_f32;
            let char_width = label_size * 0.5;
            let label_width = label.len() as f32 * char_width;
            let label_height = label_size * 1.2;

            style.min_size.height = dimension(label_height.max(box_size));
            style.margin.right = length(label_width + 8.0);
        }

        style
    }

    /// Radio style: reserve diameter and label spacing similar to checkboxes.
    fn radio_style(
        &self,
        spec: &rune_ir::view::RadioSpec,
        node: &ViewNode,
        data_doc: &DataDocument,
    ) -> Style {
        let diameter = spec.size.unwrap_or(18.0) as f32;

        let mut style = Style {
            size: Size {
                width: dimension(diameter),
                height: dimension(diameter),
            },
            min_size: Size {
                width: dimension(diameter),
                height: dimension(diameter),
            },
            flex_shrink: 0.0,
            ..Default::default()
        };

        if let Some(w) = spec.style.width {
            style.size.width = dimension(w as f32);
        }
        if let Some(h) = spec.style.height {
            style.size.height = dimension(h as f32);
            style.min_size.height = dimension(h as f32);
        }

        if let Some(label) = node
            .node_id
            .as_ref()
            .and_then(|nid| elements::resolve_text_from_data(data_doc, nid))
        {
            let label_size = 16.0_f32;
            let char_width = label_size * 0.5;
            let label_width = label.len() as f32 * char_width;
            let label_height = label_size * 1.2;

            style.min_size.height = dimension(label_height.max(diameter));
            style.margin.right = length(label_width + 8.0);
        }

        style
    }

    /// Select style: honor explicit width, otherwise stretch; give a default height and include surface styling.
    fn select_style(&self, spec: &rune_ir::view::SelectSpec) -> Style {
        let mut style = self.surface_style(&spec.style);

        if let Some(w) = spec.width {
            style.size.width = dimension(w as f32);
        } else if matches!(style.size.width, Dimension::Auto) {
            style.size.width = percent(1.0);
        }

        if matches!(style.size.height, Dimension::Auto) {
            let h = 44.0_f32;
            style.size.height = dimension(h);
            style.min_size.height = dimension(h);
        }

        style.flex_shrink = 0.0;
        style
    }

    /// File input style: stretch by default with a sensible height.
    fn file_input_style(&self, spec: &rune_ir::view::FileInputSpec) -> Style {
        let mut style = Style {
            align_self: Some(AlignSelf::Stretch),
            flex_shrink: 0.0,
            ..Default::default()
        };

        if let Some(w) = spec.width {
            style.size.width = dimension(w as f32);
        } else {
            style.size.width = percent(1.0);
        }

        // File inputs should keep a consistent field height similar to selects.
        let h = 44.0_f32;
        style.size.height = dimension(h);
        style.min_size.height = dimension(h);

        style
    }

    /// Date picker style: honor explicit width, otherwise stretch; give a default height.
    fn date_picker_style(&self, spec: &rune_ir::view::DatePickerSpec) -> Style {
        let mut style = self.surface_style(&spec.style);

        if let Some(w) = spec.width {
            style.size.width = dimension(w as f32);
        } else if matches!(style.size.width, Dimension::Auto) {
            style.size.width = percent(1.0);
        }

        if matches!(style.size.height, Dimension::Auto) {
            let h = 44.0_f32;
            style.size.height = dimension(h);
            style.min_size.height = dimension(h);
        }

        style.flex_shrink = 0.0;
        style
    }

    /// Table style uses the surface defaults but enforces a sensible minimum
    /// height so the layout keeps space for headers/rows even before data is
    /// measured.
    fn table_style(&self, spec: &rune_ir::view::TableSpec) -> Style {
        let mut style = self.surface_style(&spec.style);

        if matches!(style.min_size.height, Dimension::Auto) {
            let border = spec.style.border_width.unwrap_or(1.0) as f32;
            let padding = spec.style.padding.top as f32 + spec.style.padding.bottom as f32;
            let base_row_height = 44.0 + spec.row_gap as f32;
            style.min_size.height = dimension(padding + base_row_height + border * 2.0);
        }

        style
    }

    /// WebView style: honor explicit width/height, otherwise fill available space
    fn webview_style(&self, spec: &rune_ir::view::WebViewSpec) -> Style {
        let mut style = self.surface_style(&spec.style);

        // Use explicit dimensions if provided, otherwise default to full width/height
        if matches!(style.size.width, Dimension::Auto) {
            style.size.width = percent(1.0); // Full width by default
        }
        if matches!(style.size.height, Dimension::Auto) {
            // Fill the parent height (viewport) by default so WebView
            // content occupies the full vertical space of its zone.
            style.size.height = percent(1.0);
        }
        if matches!(style.min_size.width, Dimension::Auto) {
            style.min_size.width = dimension(100.0);
        }
        if matches!(style.min_size.height, Dimension::Auto) {
            style.min_size.height = dimension(100.0);
        }

        style.flex_shrink = 0.0;
        style
    }

    /// Compute layout with custom measurement for text nodes (wrapping support).
    pub(crate) fn compute_layout_with_measure(
        &mut self,
        root_id: NodeId,
        available: Size<AvailableSpace>,
        view_doc: &ViewDocument,
        data_doc: &DataDocument,
    ) -> Result<()> {
        self.taffy
            .compute_layout_with_measure(
                root_id,
                available,
                |known, available_space, _, context, _| {
                    if let Some(Some(view_node_id)) = context.map(|ctx| ctx.clone()) {
                        if let Some(view_node) = view_doc.node(&view_node_id) {
                            match &view_node.kind {
                                ViewNodeKind::Text(spec) => {
                                    let text = view_node
                                        .node_id
                                        .as_ref()
                                        .and_then(|nid| {
                                            super::elements::resolve_text_from_data(data_doc, nid)
                                        })
                                        .unwrap_or_default();
                                    return super::text_measure::measure_text_node(
                                        spec,
                                        &text,
                                        known,
                                        available_space,
                                    );
                                }
                                ViewNodeKind::Table(spec) => {
                                    return Self::measure_table_node(
                                        spec,
                                        view_node,
                                        data_doc,
                                        known,
                                        available_space,
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                    Size::ZERO
                },
            )
            .context("Failed to compute layout")
    }

    /// Intrinsic measurement for table nodes so they reserve space according
    /// to their data (header + rows) and padding instead of collapsing to zero
    /// height in flex flow.
    fn measure_table_node(
        spec: &rune_ir::view::TableSpec,
        view_node: &ViewNode,
        data_doc: &DataDocument,
        known: Size<Option<f32>>,
        available_space: Size<AvailableSpace>,
    ) -> Size<f32> {
        // Honor explicitly known dimensions from the layout engine.
        let resolved_width = known.width.or_else(|| match available_space.width {
            AvailableSpace::Definite(w) => Some(w),
            _ => None,
        });
        if let Some(h) = known.height {
            return Size {
                width: resolved_width.unwrap_or(0.0),
                height: h,
            };
        }

        // Derive table payload from data bindings.
        let table_data = view_node.node_id.as_ref().and_then(|node_id| {
            data_doc.node(node_id).and_then(|n| {
                if let rune_ir::data::document::DataNodeKind::Table(table) = &n.kind {
                    Some(table.clone())
                } else {
                    None
                }
            })
        });

        let (row_count, has_header) = if let Some(table) = table_data {
            let derived_columns_len = if !table.columns.is_empty() {
                table.columns.len()
            } else {
                table.rows.first().map(|r| r.len()).unwrap_or(0)
            };
            (table.rows.len(), derived_columns_len > 0)
        } else {
            (0, true) // fallback to a single header row placeholder
        };

        // Match render_table_element defaults so paint + layout stay in sync.
        let mut row_height = 44.0_f32;
        if spec.row_gap > 0.0 {
            row_height += spec.row_gap as f32;
        }

        let total_rows = row_count as f32 + if has_header { 1.0 } else { 0.0 };
        let effective_rows = if total_rows > 0.0 { total_rows } else { 1.0 };

        let padding = spec.style.padding.top as f32 + spec.style.padding.bottom as f32;
        let border = spec.style.border_width.unwrap_or(1.0) as f32;
        let height = padding + effective_rows * row_height + border * 2.0;

        Size {
            width: resolved_width.unwrap_or(0.0),
            height,
        }
    }

    /// Convert ImageSpec to Taffy Style.
    fn image_style(&self, spec: &rune_ir::view::ImageSpec) -> Style {
        // Default to filling available space when dimensions are unspecified so
        // unsized images (like the home_tab hero) get a non-zero layout.
        let mut style = Style {
            size: Size {
                width: spec
                    .width
                    .map(|w| dimension(w as f32))
                    .unwrap_or(auto_dimension()),
                height: spec
                    .height
                    .map(|h| dimension(h as f32))
                    .unwrap_or(auto_dimension()),
            },
            ..Default::default()
        };

        if spec.width.is_none() {
            style.size.width = percent(1.0);
        }

        if spec.height.is_none() {
            style.flex_grow = 1.0;
            style.min_size.height = dimension(240.0); // ensure visible presence
        }

        style
    }

    /// Get children ViewNodeIds from a ViewNode.
    pub(crate) fn get_children<'a>(&self, node: &'a ViewNode) -> Vec<&'a ViewNodeId> {
        match &node.kind {
            ViewNodeKind::FlexContainer(spec) => spec.children.iter().collect(),
            ViewNodeKind::GridContainer(spec) => spec.children.iter().collect(),
            ViewNodeKind::FormContainer(spec) => spec.children.iter().collect(),
            ViewNodeKind::Alert(spec) => spec.children.iter().collect(),
            ViewNodeKind::Modal(spec) => spec.children.iter().collect(),
            ViewNodeKind::Confirm(spec) => spec.children.iter().collect(),
            _ => vec![],
        }
    }

    /// Render a single ViewNode tree into a `Canvas` using element helpers.
    ///
    /// This function now uses stateful elements for interactive components
    /// and adds hit regions for click detection.
    fn render_view_node_with_elements(
        &mut self,
        canvas: &mut rune_surface::Canvas,
        data_doc: &DataDocument,
        view_doc: &ViewDocument,
        view_node_id: &ViewNodeId,
        ctx: &mut LayoutContext,
        provider: &dyn engine_core::TextProvider,
    ) -> Result<()> {
        use rune_ir::view::ViewNodeKind;

        let debug_logging = std::env::var("RUNE_IR_DEBUG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
            .unwrap_or(false);

        let view_node = view_doc
            .node(view_node_id)
            .with_context(|| format!("View node not found: {}", view_node_id))?;

        let taffy_node = self
            .node_map
            .get(view_node_id)
            .with_context(|| format!("Taffy node not found for: {}", view_node_id))?;

        let layout = self
            .taffy
            .layout(*taffy_node)
            .context("Failed to get layout")?;

        // Convert to scene space using current context
        let scene_rect = ctx.to_scene_rect(layout);
        let z = ctx.z_index(layout.order);

        // Track rendered bounds to compute scrollable content height.
        self.last_content_height = self.last_content_height.max(scene_rect.y + scene_rect.h);
        self.last_content_width = self.last_content_width.max(scene_rect.x + scene_rect.w);

        let node_type = match &view_node.kind {
            ViewNodeKind::FlexContainer(_) => "FlexContainer",
            ViewNodeKind::GridContainer(_) => "GridContainer",
            ViewNodeKind::FormContainer(_) => "FormContainer",
            ViewNodeKind::Text(_) => "Text",
            ViewNodeKind::Button(_) => "Button",
            ViewNodeKind::Image(_) => "Image",
            ViewNodeKind::Spacer(_) => "Spacer",
            ViewNodeKind::Link(_) => "Link",
            ViewNodeKind::InputBox(_) => "InputBox",
            ViewNodeKind::TextArea(_) => "TextArea",
            ViewNodeKind::Checkbox(_) => "Checkbox",
            ViewNodeKind::Radio(_) => "Radio",
            ViewNodeKind::Select(_) => "Select",
            ViewNodeKind::FileInput(_) => "FileInput",
            ViewNodeKind::DatePicker(_) => "DatePicker",
            ViewNodeKind::Table(_) => "Table",
            ViewNodeKind::WebView(_) => "WebView",
            ViewNodeKind::Alert(_) => "Alert",
            ViewNodeKind::Modal(_) => "Modal",
            ViewNodeKind::Confirm(_) => "Confirm",
        };

        if debug_logging {
            eprintln!(
                "Rendering {} '{}' at ({:.1}, {:.1}) {:.1}x{:.1} z={}",
                node_type, view_node_id, scene_rect.x, scene_rect.y, scene_rect.w, scene_rect.h, z
            );
        }

        // Render this node using appropriate element
        match &view_node.kind {
            ViewNodeKind::FlexContainer(spec) => {
                elements::render_container_element(canvas, spec, scene_rect, z);
            }
            ViewNodeKind::GridContainer(spec) => {
                elements::render_background_element(canvas, &spec.background, scene_rect, z);
            }
            ViewNodeKind::FormContainer(spec) => {
                elements::render_background_element(canvas, &spec.background, scene_rect, z);
            }
            ViewNodeKind::Text(spec) => {
                elements::render_text_element(canvas, data_doc, view_node, spec, scene_rect, z);
            }
            ViewNodeKind::Button(spec) => {
                // Use stateful element for interactivity
                let button = self.element_state.get_or_create_button(
                    view_node_id,
                    view_node,
                    spec,
                    scene_rect,
                    data_doc,
                );
                button.render(canvas, z);

                // Add hit region for click detection
                let region_id = self.hit_registry.register(view_node_id);
                canvas.hit_region_rect(region_id, scene_rect, z + 10);
            }
            ViewNodeKind::Image(spec) => {
                elements::render_image_element(canvas, data_doc, view_node, spec, scene_rect, z);
            }
            ViewNodeKind::Spacer(_spec) => {
                // Spacer is layout-only; no visual.
            }
            ViewNodeKind::Link(spec) => {
                elements::render_link_element(canvas, data_doc, view_node, spec, scene_rect, z);
            }
            ViewNodeKind::InputBox(spec) => {
                // Use stateful element for interactivity
                let input_box = self.element_state.get_or_create_input_box(
                    view_node_id,
                    spec,
                    scene_rect,
                    data_doc,
                );
                input_box.render(canvas, z, provider);

                // Add hit region for click detection
                let region_id = self.hit_registry.register(view_node_id);
                canvas.hit_region_rect(region_id, scene_rect, z + 10);
            }
            ViewNodeKind::TextArea(spec) => {
                // Use stateful element for interactivity
                let text_area = self.element_state.get_or_create_text_area(
                    view_node_id,
                    spec,
                    scene_rect,
                    data_doc,
                );
                text_area.render(canvas, z, provider);

                // Add hit region for click detection
                let region_id = self.hit_registry.register(view_node_id);
                canvas.hit_region_rect(region_id, scene_rect, z + 10);
            }
            ViewNodeKind::Checkbox(spec) => {
                let checkbox = self.element_state.get_or_create_checkbox(
                    view_node_id,
                    view_node,
                    spec,
                    scene_rect,
                    data_doc,
                );
                checkbox.render(canvas, z);

                let region_id = self.hit_registry.register(view_node_id);
                let hit_rect = elements::checkbox_hit_rect(checkbox);
                canvas.hit_region_rect(region_id, hit_rect, z + 10);
            }
            ViewNodeKind::Radio(spec) => {
                let radio = self.element_state.get_or_create_radio(
                    view_node_id,
                    view_node,
                    spec,
                    scene_rect,
                    data_doc,
                );
                radio.render(canvas, z);

                let region_id = self.hit_registry.register(view_node_id);
                let hit_rect = elements::radio_hit_rect(radio);
                canvas.hit_region_rect(region_id, hit_rect, z + 10);
            }
            ViewNodeKind::Select(spec) => {
                // Use stateful select so dropdown toggles and focus persist
                let select =
                    self.element_state
                        .get_or_create_select(view_node_id, spec, scene_rect);
                select.render(canvas, z);

                // Register hit regions for the field and (when open) the dropdown overlay
                // so clicks on either surface route to the same select node.
                let field_region_z = z + 10;
                let region_id = self.hit_registry.register(view_node_id);
                canvas.hit_region_rect(region_id, scene_rect, field_region_z);

                if let Some(overlay) = select.get_overlay_bounds().filter(|_| select.is_open()) {
                    // Use a higher z so the overlay remains topmost for hit testing.
                    canvas.hit_region_rect(region_id, overlay, field_region_z + 1000);
                }
            }
            ViewNodeKind::FileInput(spec) => {
                let file_input =
                    self.element_state
                        .get_or_create_file_input(view_node_id, spec, scene_rect);
                file_input.render(canvas, z);

                let region_id = self.hit_registry.register(view_node_id);
                canvas.hit_region_rect(region_id, scene_rect, z + 10);
            }
            ViewNodeKind::DatePicker(spec) => {
                // Use stateful date picker so popup toggles and focus persist
                let picker = self.element_state.get_or_create_date_picker(
                    view_node_id,
                    spec,
                    scene_rect,
                    self.current_content_height,
                );
                picker.render(canvas, z);

                // Register hit regions for the field and (when open) the popup
                // so clicks on either surface route to the same date picker node.
                let field_region_z = z + 10;
                let region_id = self.hit_registry.register(view_node_id);
                canvas.hit_region_rect(region_id, scene_rect, field_region_z);

                if let Some(popup) = picker.get_popup_bounds().filter(|_| picker.is_open()) {
                    // Use a higher z so the popup remains topmost for hit testing.
                    canvas.hit_region_rect(region_id, popup, field_region_z + 1000);
                }
            }
            ViewNodeKind::Table(spec) => {
                elements::render_table_element(canvas, data_doc, view_node, spec, scene_rect, z);
            }
            #[cfg(feature = "webview-cef")]
            ViewNodeKind::WebView(spec) => {
                // Get or create stateful WebView element
                let webview =
                    self.element_state
                        .get_or_create_webview(view_node_id, spec, scene_rect);

                // Render the WebView container (background, border, loading indicator)
                webview.render(canvas, z);

                // Register hit region for mouse events
                let region_id = self.hit_registry.register(view_node_id);
                canvas.hit_region_rect(region_id, scene_rect, z + 10);

                // Note: The actual browser texture is rendered via the image pipeline
                // after the CEF frame is captured and uploaded. This requires additional
                // integration in the render loop to call webview.update_frame() and
                // render the texture via the image shader.
            }
            #[cfg(not(feature = "webview-cef"))]
            ViewNodeKind::WebView(_spec) => {
                // Store the WebView rect for FFI queries (size and position).
                // Apply the canvas transform to get screen coordinates (same as draw_raw_image).
                // This allows FFI hit testing to work correctly even before pixels arrive.
                let transform = canvas.current_transform();
                let [a, b, c, d, e, f] = transform.m;
                let screen_x = a * scene_rect.x + c * scene_rect.y + e;
                let screen_y = b * scene_rect.x + d * scene_rect.y + f;
                crate::elements::webview::set_webview_rect(
                    screen_x,
                    screen_y,
                    scene_rect.w,
                    scene_rect.h,
                );

                // Native NSView-based CEF rendering: CEF renders to its own native view
                // which is composited by macOS on top of our wgpu surface. We just need
                // to register a hit region for any non-webview hit testing and leave a
                // transparent hole in the wgpu surface where the webview appears.
                //
                // The native CEF view is positioned by ViewController.mm based on the
                // webview rect we set above via set_webview_rect().
                let webview_z = 500;

                // Check if native CEF view is active (set by FFI from Objective-C side)
                if crate::elements::webview::has_native_cef_view() {
                    // Native mode: CEF renders to its own NSView, composited by macOS.
                    // We don't render anything here - just register a hit region so
                    // clicks outside the webview are handled by our hit testing.
                    // Clicks inside the webview go to CEF's native view directly.
                    let region_id = self.hit_registry.register(view_node_id);
                    canvas.hit_region_rect(region_id, scene_rect, webview_z + 10);
                } else {
                    // No native CEF view - show placeholder until one is created.
                    let placeholder_color =
                        engine_core::ColorLinPremul::from_srgba_u8([240, 240, 240, 255]);
                    canvas.fill_rect(
                        scene_rect.x,
                        scene_rect.y,
                        scene_rect.w,
                        scene_rect.h,
                        engine_core::Brush::Solid(placeholder_color),
                        z,
                    );
                    let text_color =
                        engine_core::ColorLinPremul::from_srgba_u8([100, 100, 100, 255]);
                    canvas.draw_text_run(
                        [
                            scene_rect.x + scene_rect.w * 0.5 - 50.0,
                            scene_rect.y + scene_rect.h * 0.5,
                        ],
                        "WebView loading...".to_string(),
                        14.0,
                        text_color,
                        z + 1,
                    );

                    // Register hit region even when placeholder is shown.
                    let region_id = self.hit_registry.register(view_node_id);
                    canvas.hit_region_rect(region_id, scene_rect, z + 10);
                }
            }
            ViewNodeKind::Alert(_) | ViewNodeKind::Modal(_) | ViewNodeKind::Confirm(_) => {
                // Overlays are rendered separately in a post-render pass
                // only when they are active. Skip rendering here.
                // The children will also be skipped since we return early.
                return Ok(());
            }
        }

        // Render children recursively
        let children = self.get_children(view_node);
        if !children.is_empty() {
            ctx.push(layout); // Push transform

            for child_id in children {
                self.render_view_node_with_elements(
                    canvas, data_doc, view_doc, child_id, ctx, provider,
                )?;
            }

            ctx.pop(); // Pop transform
        }

        Ok(())
    }

    /// Render active overlays (modals, alerts, confirms) on top of all content
    fn render_active_overlays(
        &mut self,
        canvas: &mut rune_surface::Canvas,
        data_doc: &DataDocument,
        view_doc: &ViewDocument,
        viewport_width: f32,
        viewport_height: f32,
        scroll_offset_x: f32,
        scroll_offset_y: f32,
        _provider: &dyn engine_core::TextProvider,
    ) -> Result<()> {
        use engine_core::Transform2D;
        use rune_ir::view::ViewNodeKind;

        let active_overlays = self.element_state.get_active_overlays().to_vec();
        if active_overlays.is_empty() {
            return Ok(());
        }

        // Z-index base for overlays (very high to be on top of everything)
        let overlay_base_z = 9000;

        // Counteract the scroll transform so overlays stay fixed relative to the viewport.
        // The viewport position is handled by the canvas transform stack (set by runner.rs).
        canvas.push_transform(Transform2D::translate(scroll_offset_x, scroll_offset_y));

        for (overlay_index, active_overlay) in active_overlays.iter().enumerate() {
            let overlay_z = overlay_base_z + (overlay_index as i32 * 100);

            // Look up the view node for this overlay
            let view_node = match view_doc.node(&active_overlay.view_node_id) {
                Some(node) => node,
                None => continue,
            };

            // Get overlay spec
            let spec = match &view_node.kind {
                ViewNodeKind::Alert(spec)
                | ViewNodeKind::Modal(spec)
                | ViewNodeKind::Confirm(spec) => spec,
                _ => continue,
            };

            // Get title from data document if available
            let title_text = view_node
                .node_id
                .as_ref()
                .and_then(|node_id| elements::resolve_text_from_data(data_doc, node_id))
                .unwrap_or_else(|| match active_overlay.overlay_type {
                    super::state::OverlayType::Modal => "Modal".to_string(),
                    super::state::OverlayType::Alert => "Alert".to_string(),
                    super::state::OverlayType::Confirm => "Confirm".to_string(),
                });

            match active_overlay.overlay_type {
                super::state::OverlayType::Modal => {
                    elements::render_modal_overlay(
                        canvas,
                        &mut self.hit_registry,
                        &active_overlay.view_node_id,
                        spec,
                        &title_text,
                        overlay_z,
                        viewport_width,
                        viewport_height,
                        view_doc,
                        data_doc,
                    );
                }
                super::state::OverlayType::Alert => {
                    elements::render_alert_overlay(
                        canvas,
                        &mut self.hit_registry,
                        &active_overlay.view_node_id,
                        spec,
                        &title_text,
                        overlay_z,
                        viewport_width,
                        viewport_height,
                        view_doc,
                        data_doc,
                    );
                }
                super::state::OverlayType::Confirm => {
                    elements::render_confirm_overlay(
                        canvas,
                        &mut self.hit_registry,
                        &active_overlay.view_node_id,
                        spec,
                        &title_text,
                        overlay_z,
                        viewport_width,
                        viewport_height,
                        view_doc,
                        data_doc,
                    );
                }
            }
        }

        canvas.pop_transform();
        Ok(())
    }
}

impl Default for IrRenderer {
    fn default() -> Self {
        Self::new()
    }
}
