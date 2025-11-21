//! Element-level Canvas rendering helpers for IR nodes.

use super::hit_region::HitRegionRegistry;
use crate::elements::{Alert, AlertPosition, ConfirmDialog, Modal, ModalButton};
use engine_core::ColorLinPremul;
use rune_ir::data::document::DataDocument;
use rune_ir::view::{OverlayContainerSpec, OverlayPosition, ViewNodeId};

/// Render a generic background for any element that exposes a single
/// `ViewBackground` field (FlexContainer, GridContainer, FormContainer, overlays, etc).
pub(super) fn render_background_element(
    canvas: &mut rune_surface::Canvas,
    background: &Option<rune_ir::view::ViewBackground>,
    rect: engine_core::Rect,
    z: i32,
) {
    if let Some(bg) = background {
        let brush = brush_from_view_background(bg, rect);
        if std::env::var("RUNE_IR_DEBUG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
            .unwrap_or(false)
        {
            eprintln!(
                "Rendering background at ({}, {}) {}x{} z={}",
                rect.x, rect.y, rect.w, rect.h, z
            );
        }
        canvas.fill_rect(rect.x, rect.y, rect.w, rect.h, brush, z);
    }
}

/// Render FlexContainer using background/border (children handled recursively).
pub(super) fn render_container_element(
    canvas: &mut rune_surface::Canvas,
    spec: &rune_ir::view::FlexContainerSpec,
    rect: engine_core::Rect,
    z: i32,
) {
    // Render background if present
    render_background_element(canvas, &spec.background, rect, z);

    // TODO: Render border if present
    // Children are rendered by recursion from the caller.
}

/// Render Text element using elements::Text or elements::Label.
pub(super) fn render_text_element(
    canvas: &mut rune_surface::Canvas,
    data_doc: &DataDocument,
    view_node: &rune_ir::view::ViewNode,
    spec: &rune_ir::view::TextSpec,
    rect: engine_core::Rect,
    z: i32,
) {
    // Get text content from data document
    let text = if let Some(node_id) = &view_node.node_id {
        resolve_text_from_data(data_doc, node_id).unwrap_or_else(|| "[no text]".to_string())
    } else {
        "[no binding]".to_string()
    };

    // Get text styling
    let color = crate::ir_adapter::IrAdapter::color_from_text_style(&spec.style);
    let size = crate::ir_adapter::IrAdapter::font_size_from_text_style(&spec.style);
    let pad_x = spec.style.padding.left as f32;
    let pad_y = spec.style.padding.top as f32;
    let pad_right = spec.style.padding.right as f32;
    let wrap_width = (rect.w - pad_x - pad_right).max(0.0);

    if std::env::var("RUNE_IR_DEBUG")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
        .unwrap_or(false)
    {
        eprintln!(
            "Rendering text '{}' at ({}, {}) size={} color=({:.2},{:.2},{:.2},{:.2}) z={}",
            text.chars().take(30).collect::<String>(),
            rect.x,
            rect.y,
            size,
            color.r,
            color.g,
            color.b,
            color.a,
            z
        );
    }

    // Use shared line computation so measurement and render stay in sync
    let lines = super::text_measure::compute_lines(
        spec,
        &text,
        if wrap_width > 0.0 {
            Some(wrap_width)
        } else {
            None
        },
    );

    let base_y = (rect.y + pad_y + lines.ascent).round();
    let base_x = (rect.x + pad_x).round();
    for (i, line) in lines.lines.iter().enumerate() {
        let y = base_y + (i as f32) * lines.line_height;
        canvas.draw_text_run([base_x, y], line.clone(), size, color, z);
    }
}

/// Render Button element using elements::Button (self-contained).
#[allow(dead_code)]
pub(super) fn render_button_element(
    canvas: &mut rune_surface::Canvas,
    data_doc: &DataDocument,
    view_node: &rune_ir::view::ViewNode,
    spec: &rune_ir::view::ButtonSpec,
    rect: engine_core::Rect,
    z: i32,
) {
    // Get button label from data document
    let label = if let Some(node_id) = &view_node.node_id {
        resolve_action_label_from_data(data_doc, node_id).unwrap_or_else(|| "Button".to_string())
    } else {
        "Button".to_string()
    };

    // Create button element (self-contained rendering + event handling)
    let button = crate::elements::Button {
        rect,
        radius: spec.style.corner_radius.unwrap_or(4.0) as f32,
        bg: engine_core::ColorLinPremul::from_srgba_u8([59, 130, 246, 255]), // Blue
        fg: engine_core::ColorLinPremul::from_srgba_u8([255, 255, 255, 255]), // White
        label,
        label_size: 14.0,
        focused: false, // TODO: Track focus state
        on_click_intent: spec.on_click_intent.clone(),
    };

    button.render(canvas, z);
}

/// Render Image element using elements::ImageBox (self-contained).
pub(super) fn render_image_element(
    canvas: &mut rune_surface::Canvas,
    data_doc: &DataDocument,
    view_node: &rune_ir::view::ViewNode,
    spec: &rune_ir::view::ImageSpec,
    rect: engine_core::Rect,
    z: i32,
) {
    // Get image source from data document
    if let Some(node_id) = &view_node.node_id {
        if let Some(image_path) = resolve_image_source_from_data(data_doc, node_id) {
            if rect.w <= 0.0 || rect.h <= 0.0 {
                eprintln!(
                    "Image node '{}' has zero-sized layout ({:.1}x{:.1}); skipping draw",
                    view_node.id, rect.w, rect.h
                );
                return;
            }

            // Use canvas.draw_image directly for now
            // TODO: Use elements::ImageBox when it supports rendering
            let fit = match spec.content_fit {
                Some(rune_ir::view::ImageContentFit::Fill) => rune_surface::ImageFitMode::Fill,
                Some(rune_ir::view::ImageContentFit::Cover) => rune_surface::ImageFitMode::Cover,
                _ => rune_surface::ImageFitMode::Contain,
            };

            if image_path.exists() {
                canvas.draw_image(image_path, [rect.x, rect.y], [rect.w, rect.h], fit, z);
            } else {
                eprintln!(
                    "Image path not found for node '{}': {:?}",
                    view_node.id, image_path
                );
                canvas.fill_rect(
                    rect.x,
                    rect.y,
                    rect.w.max(1.0),
                    rect.h.max(1.0),
                    engine_core::Brush::Solid(engine_core::ColorLinPremul::from_srgba_u8([
                        80, 80, 90, 255,
                    ])),
                    z,
                );
            }
        }
    }
}

/// Render a hyperlink element from IR `LinkSpec` and bound `Action` data.
pub(super) fn render_link_element(
    canvas: &mut rune_surface::Canvas,
    data_doc: &DataDocument,
    view_node: &rune_ir::view::ViewNode,
    spec: &rune_ir::view::LinkSpec,
    rect: engine_core::Rect,
    z: i32,
) {
    use crate::ir_adapter::IrAdapter;

    // Label + href come from an Action data node.
    let (label, href) = if let Some(node_id) = &view_node.node_id {
        let label =
            resolve_action_label_from_data(data_doc, node_id).unwrap_or_else(|| "link".to_string());
        let href =
            resolve_action_href_from_data(data_doc, node_id).unwrap_or_else(|| "#".to_string());
        (label, href)
    } else {
        ("link".to_string(), "#".to_string())
    };

    let size = IrAdapter::font_size_from_text_style(&spec.label_style);
    let color = IrAdapter::color_from_text_style(&spec.label_style);

    let pos = [rect.x, rect.y + size * 0.8];

    let link = crate::elements::Link::new(label, href, pos, size).with_color(color);

    link.render(canvas, z);
}

/// Render a checkbox element from IR `CheckboxSpec`.
pub(super) fn render_checkbox_element(
    canvas: &mut rune_surface::Canvas,
    _data_doc: &DataDocument,
    _view_node: &rune_ir::view::ViewNode,
    spec: &rune_ir::view::CheckboxSpec,
    rect: engine_core::Rect,
    z: i32,
) {
    use crate::ir_adapter::IrAdapter;

    let checkbox = IrAdapter::checkbox_from_spec(spec, rect);
    checkbox.render(canvas, z);
}

/// Render a radio button element from IR `RadioSpec`.
pub(super) fn render_radio_element(
    canvas: &mut rune_surface::Canvas,
    _data_doc: &DataDocument,
    _view_node: &rune_ir::view::ViewNode,
    spec: &rune_ir::view::RadioSpec,
    rect: engine_core::Rect,
    z: i32,
) {
    let radius = spec
        .size
        .unwrap_or_else(|| (rect.w.min(rect.h) / 2.0) as f64) as f32;

    let radio = crate::elements::Radio {
        center: [rect.x + rect.w * 0.5, rect.y + rect.h * 0.5],
        radius,
        selected: spec.default_selected.unwrap_or(false),
        label: None,
        label_size: 16.0,
        label_color: engine_core::ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
        focused: false,
    };

    radio.render(canvas, z);
}

/// Render a select element from IR `SelectSpec`.
#[allow(dead_code)]
pub(super) fn render_select_element(
    canvas: &mut rune_surface::Canvas,
    spec: &rune_ir::view::SelectSpec,
    rect: engine_core::Rect,
    z: i32,
) {
    let options: Vec<String> = spec.options.iter().map(|opt| opt.label.clone()).collect();

    let selected_index = spec
        .options
        .iter()
        .position(|opt| opt.selected)
        .or_else(|| if options.is_empty() { None } else { Some(0) });

    let label = selected_index
        .and_then(|idx| options.get(idx).cloned())
        .or_else(|| spec.placeholder.clone())
        .unwrap_or_default();

    let mut select = crate::elements::Select {
        rect,
        label,
        label_size: 16.0,
        label_color: engine_core::ColorLinPremul::from_srgba_u8([20, 24, 30, 255]),
        open: false,
        focused: false,
        options,
        selected_index,
        padding_left: 14.0,
        padding_right: 14.0,
        padding_top: 10.0,
        padding_bottom: 10.0,
        bg_color: engine_core::ColorLinPremul::from_srgba_u8([245, 247, 250, 255]),
        border_color: engine_core::ColorLinPremul::from_srgba_u8([200, 208, 216, 255]),
        border_width: 1.0,
        radius: 8.0,
    };

    select.apply_surface_style(&spec.style);
    select.render(canvas, z);
}

/// Render a simple input box from IR `InputBoxSpec`.
///
/// This intentionally uses a lightweight visual representation (no caret/selection)
/// so it does not depend on the richer `elements::InputBox` text editing pipeline.
#[allow(dead_code)]
pub(super) fn render_input_box_element(
    canvas: &mut rune_surface::Canvas,
    data_doc: &DataDocument,
    view_node: &rune_ir::view::ViewNode,
    spec: &rune_ir::view::InputBoxSpec,
    rect: engine_core::Rect,
    z: i32,
) {
    use engine_core::{Brush, Color, RoundedRadii, RoundedRect};
    use rune_surface::shapes;

    // Styling driven by spec.style with sensible defaults
    let radius = spec.style.corner_radius.unwrap_or(6.0) as f32;
    let padding = spec.style.padding;
    let text_size = 16.0;

    let rrect = RoundedRect {
        rect,
        radii: RoundedRadii {
            tl: radius,
            tr: radius,
            br: radius,
            bl: radius,
        },
    };

    // Background
    let bg = spec
        .style
        .background
        .as_ref()
        .and_then(|bg| match bg {
            rune_ir::view::ViewBackground::Solid { color } => crate::ir_adapter::parse_color(color),
            _ => None,
        })
        .unwrap_or_else(|| Color::rgba(245, 248, 255, 255));
    canvas.rounded_rect(rrect, Brush::Solid(bg), z);

    // Border
    let border_color = spec
        .style
        .border_color
        .as_ref()
        .and_then(|c| crate::ir_adapter::parse_color(c))
        .unwrap_or_else(|| Color::rgba(148, 163, 184, 255));
    let border_width = spec.style.border_width.unwrap_or(1.0) as f32;
    if border_width > 0.0 {
        shapes::draw_rounded_rectangle(
            canvas,
            rrect,
            None,
            Some(border_width),
            Some(Brush::Solid(border_color)),
            z + 1,
        );
    }

    // Resolve current value or placeholder.
    let value = if let Some(node_id) = &view_node.node_id {
        resolve_text_from_data(data_doc, node_id)
            .or_else(|| spec.default_value.clone())
            .unwrap_or_default()
    } else {
        spec.default_value.clone().unwrap_or_default()
    };

    let placeholder = spec.placeholder.clone();
    // Center text vertically within the content box
    let content_height = rect.h - (padding.top as f32 + padding.bottom as f32);
    let text_y = rect.y + padding.top as f32 + (content_height - text_size) * 0.5 + text_size * 0.8;
    let text_x = rect.x + padding.left as f32;

    if !value.is_empty() {
        canvas.draw_text_run(
            [text_x, text_y],
            value,
            text_size,
            engine_core::ColorLinPremul::from_srgba_u8([51, 65, 85, 255]),
            z + 2,
        );
    } else if let Some(ph) = placeholder {
        canvas.draw_text_run(
            [text_x, text_y],
            ph,
            text_size,
            engine_core::ColorLinPremul::from_srgba_u8([148, 163, 184, 255]),
            z + 2,
        );
    }
}

/// Render a simple multi-line text area from IR `TextAreaSpec`.
#[allow(dead_code)]
pub(super) fn render_text_area_element(
    canvas: &mut rune_surface::Canvas,
    data_doc: &DataDocument,
    view_node: &rune_ir::view::ViewNode,
    spec: &rune_ir::view::TextAreaSpec,
    rect: engine_core::Rect,
    z: i32,
) {
    use engine_core::{Brush, Color, RoundedRadii, RoundedRect};
    use rune_surface::shapes;

    let radius = 6.0;
    let rrect = RoundedRect {
        rect,
        radii: RoundedRadii {
            tl: radius,
            tr: radius,
            br: radius,
            bl: radius,
        },
    };

    // Background
    let bg = Color::rgba(45, 52, 71, 255);
    canvas.rounded_rect(rrect, Brush::Solid(bg), z);

    // Border
    let border_color = Color::rgba(80, 90, 110, 255);
    shapes::draw_rounded_rectangle(
        canvas,
        rrect,
        None,
        Some(1.0),
        Some(Brush::Solid(border_color)),
        z + 1,
    );

    let value = if let Some(node_id) = &view_node.node_id {
        resolve_text_from_data(data_doc, node_id)
            .or_else(|| spec.default_value.clone())
            .unwrap_or_default()
    } else {
        spec.default_value.clone().unwrap_or_default()
    };

    let placeholder = spec.placeholder.clone();
    let text_size = 16.0;
    let line_height = text_size * 1.3;
    let mut y = rect.y + 12.0 + text_size;
    let x = rect.x + 12.0;

    if !value.is_empty() {
        for line in value.lines() {
            if y > rect.y + rect.h {
                break;
            }
            canvas.draw_text_run(
                [x, y],
                line.to_string(),
                text_size,
                engine_core::ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
                z + 2,
            );
            y += line_height;
        }
    } else if let Some(ph) = placeholder {
        canvas.draw_text_run(
            [x, y],
            ph,
            text_size,
            engine_core::ColorLinPremul::from_srgba_u8([120, 120, 130, 255]),
            z + 2,
        );
    }
}

/// Render a very simple data-driven table from IR `TableSpec` + `TableNodeData`.
pub(super) fn render_table_element(
    canvas: &mut rune_surface::Canvas,
    data_doc: &DataDocument,
    view_node: &rune_ir::view::ViewNode,
    spec: &rune_ir::view::TableSpec,
    rect: engine_core::Rect,
    z: i32,
) {
    use engine_core::{Brush, ColorLinPremul, RoundedRadii, RoundedRect};

    // Respect style padding by carving out the content rect.
    let padding = &spec.style.padding;
    let content_rect = engine_core::Rect {
        x: rect.x + padding.left as f32,
        y: rect.y + padding.top as f32,
        w: (rect.w - (padding.left + padding.right) as f32).max(0.0),
        h: (rect.h - (padding.top + padding.bottom) as f32).max(0.0),
    };

    let table_data = if let Some(node_id) = &view_node.node_id {
        resolve_table_from_data(data_doc, node_id)
    } else {
        None
    };

    let Some(table_data) = table_data else {
        // No data; draw a subtle placeholder.
        canvas.fill_rect(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            Brush::Solid(ColorLinPremul::from_srgba_u8([60, 60, 70, 255])),
            z + 1,
        );
        return;
    };

    // Build column definitions from data (or derive from first row).
    let columns: Vec<crate::elements::Column> = if !table_data.columns.is_empty() {
        table_data
            .columns
            .iter()
            .map(|c| crate::elements::Column::new(c.clone()))
            .collect()
    } else if let Some(first_row) = table_data.rows.first() {
        first_row
            .iter()
            .enumerate()
            .map(|(idx, _)| crate::elements::Column::new(format!("Column {}", idx + 1)))
            .collect()
    } else {
        Vec::new()
    };

    if columns.is_empty() {
        canvas.fill_rect(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            Brush::Solid(ColorLinPremul::from_srgba_u8([60, 60, 70, 255])),
            z + 1,
        );
        return;
    }

    // Map row data into TableRows/TableCells.
    let rows: Vec<crate::elements::TableRow> = table_data
        .rows
        .iter()
        .map(|row| {
            let cells: Vec<crate::elements::TableCell> = row
                .iter()
                .map(|value| crate::elements::TableCell::new(value.clone()))
                .collect();
            crate::elements::TableRow::new(cells)
        })
        .collect();

    let mut table = crate::elements::Table::new(content_rect, columns);
    table.rows = rows;
    table.zebra_striping = spec.zebra;

    // Apply style-driven spacing tweaks.
    if spec.column_gap > 0.0 {
        table.cell_padding_x += spec.column_gap as f32 * 0.5;
    }
    if spec.row_gap > 0.0 {
        table.row_height += spec.row_gap as f32;
    }

    // Map text styling.
    if let Some(size) = spec.header_style.font_size {
        table.header_text_size = size as f32;
    }
    if let Some(color) = spec
        .header_style
        .color
        .as_ref()
        .and_then(|c| crate::ir_adapter::parse_color(c))
    {
        table.header_text_color = color;
    }
    if let Some(size) = spec.cell_style.font_size {
        table.cell_text_size = size as f32;
    }
    if let Some(color) = spec
        .cell_style
        .color
        .as_ref()
        .and_then(|c| crate::ir_adapter::parse_color(c))
    {
        table.cell_text_color = color;
    }

    // Map surface styling (background + border).
    if let Some(bg) = &spec.style.background {
        if let rune_ir::view::ViewBackground::Solid { color } = bg {
            if let Some(parsed) = crate::ir_adapter::parse_color(color) {
                table.bg_color = parsed;
            }
        }
    }
    if let Some(border_width) = spec.style.border_width {
        table.border_width = border_width as f32;
    }
    if let Some(border_color) = &spec.style.border_color {
        if let Some(parsed) = crate::ir_adapter::parse_color(border_color) {
            table.border_color = parsed;
        }
    }

    // Render outer container background/border with corner radius support.
    let corner_radius = spec.style.corner_radius.unwrap_or(0.0) as f32;
    if corner_radius > 0.0 {
        let bg_brush = Brush::Solid(table.bg_color);
        let stroke_width = table.border_width;
        let stroke_brush = Brush::Solid(table.border_color);
        rune_surface::shapes::draw_rounded_rectangle(
            canvas,
            RoundedRect {
                rect,
                radii: RoundedRadii {
                    tl: corner_radius,
                    tr: corner_radius,
                    br: corner_radius,
                    bl: corner_radius,
                },
            },
            Some(bg_brush),
            Some(stroke_width),
            Some(stroke_brush),
            z,
        );
        // Avoid double-drawing square borders inside the rounded cloak.
        table.border_width = 0.0;
    } else {
        render_background_element(canvas, &spec.style.background, rect, z);
    }

    table.render(canvas, z);
}

/// Render a modal overlay with scrim, panel, and hit regions.
pub(super) fn render_modal_overlay(
    canvas: &mut rune_surface::Canvas,
    hit_registry: &mut HitRegionRegistry,
    overlay_id: &ViewNodeId,
    spec: &OverlayContainerSpec,
    title_text: &str,
    overlay_z: i32,
    viewport_width: f32,
    viewport_height: f32,
    view_doc: &rune_ir::view::ViewDocument,
    data_doc: &DataDocument,
) {
    let dismissible = spec.dismissible.unwrap_or(true);
    let show_close = spec.show_close.unwrap_or(true);

    // Resolve content and buttons from children
    let (content_text, buttons) = resolve_modal_children(view_doc, data_doc, &spec.children);

    let mut modal = Modal::new(
        viewport_width,
        viewport_height,
        title_text.to_string(),
        content_text,
        buttons,
    )
    .with_close_on_background_click(dismissible);
    modal.show_shadow = true; // Shadow enabled since visual scrim is disabled
    modal.base_z = overlay_z + 10;
    if !show_close {
        modal.close_icon_color = ColorLinPremul::from_srgba_u8([0, 0, 0, 0]);
    }

    modal.render_overlay(
        canvas,
        hit_registry,
        overlay_id,
        dismissible,
        show_close,
        viewport_width,
        viewport_height,
    );
}

/// Resolve modal content and buttons from overlay children.
fn resolve_modal_children(
    view_doc: &rune_ir::view::ViewDocument,
    data_doc: &DataDocument,
    children: &[ViewNodeId],
) -> (String, Vec<ModalButton>) {
    use rune_ir::view::ViewNodeKind;

    let mut content_lines: Vec<String> = Vec::new();
    let mut buttons: Vec<ModalButton> = Vec::new();

    for child_id in children {
        let Some(child_node) = view_doc.node(child_id) else {
            continue;
        };

        match &child_node.kind {
            ViewNodeKind::Text(_spec) => {
                // Resolve text from data document
                if let Some(node_id) = &child_node.node_id {
                    if let Some(text) = resolve_text_from_data(data_doc, node_id) {
                        content_lines.push(text);
                    }
                }
            }
            ViewNodeKind::Button(button_spec) => {
                // Resolve button label from data document
                let label = child_node
                    .node_id
                    .as_ref()
                    .and_then(|node_id| resolve_action_label_from_data(data_doc, node_id))
                    .unwrap_or_else(|| "Button".to_string());

                // Check if this is a primary button based on intent
                let is_primary = button_spec
                    .on_click_intent
                    .as_ref()
                    .map(|intent| {
                        intent.contains("confirm")
                            || intent.contains("submit")
                            || intent.contains("primary")
                            || intent.contains("ok")
                    })
                    .unwrap_or(false);

                if is_primary {
                    buttons.push(ModalButton::primary(label));
                } else {
                    buttons.push(ModalButton::new(label));
                }
            }
            _ => {}
        }
    }

    // Use default content if none found
    let content_text = if content_lines.is_empty() {
        "Are you sure you want to continue?\nThis action cannot be undone.".to_string()
    } else {
        content_lines.join("\n")
    };

    // Use default buttons if none found
    if buttons.is_empty() {
        buttons.push(ModalButton::new("Cancel"));
        buttons.push(ModalButton::primary("Continue"));
    }

    (content_text, buttons)
}

/// Render an alert overlay with action hit targets (no scrim background).
pub(super) fn render_alert_overlay(
    canvas: &mut rune_surface::Canvas,
    hit_registry: &mut HitRegionRegistry,
    overlay_id: &ViewNodeId,
    spec: &OverlayContainerSpec,
    title_text: &str,
    overlay_z: i32,
    viewport_width: f32,
    viewport_height: f32,
    view_doc: &rune_ir::view::ViewDocument,
    data_doc: &DataDocument,
) {
    let dismissible = spec.dismissible.unwrap_or(true);
    let show_close = spec.show_close.unwrap_or(false); // Alerts typically don't show close

    // Resolve content and action from children
    let (message_text, action_label) = resolve_alert_children(view_doc, data_doc, &spec.children);

    let mut alert = Alert::new(
        viewport_width,
        viewport_height,
        title_text.to_string(),
        message_text,
    );
    if let Some(label) = action_label {
        alert = alert.with_action(label);
    }
    alert.base_z = overlay_z + 10;
    alert.position = alert_position_from_overlay(&spec.position);

    alert.render_overlay(canvas, hit_registry, overlay_id, dismissible, show_close);
}

/// Resolve alert content and action label from overlay children.
fn resolve_alert_children(
    view_doc: &rune_ir::view::ViewDocument,
    data_doc: &DataDocument,
    children: &[ViewNodeId],
) -> (String, Option<String>) {
    use rune_ir::view::ViewNodeKind;

    let mut message_lines: Vec<String> = Vec::new();
    let mut action_label: Option<String> = None;

    for child_id in children {
        let Some(child_node) = view_doc.node(child_id) else {
            continue;
        };

        match &child_node.kind {
            ViewNodeKind::Text(_spec) => {
                // Resolve text from data document
                if let Some(node_id) = &child_node.node_id {
                    if let Some(text) = resolve_text_from_data(data_doc, node_id) {
                        message_lines.push(text);
                    }
                }
            }
            ViewNodeKind::Button(_button_spec) => {
                // Resolve button label from data document (action button)
                let label = child_node
                    .node_id
                    .as_ref()
                    .and_then(|node_id| resolve_action_label_from_data(data_doc, node_id))
                    .unwrap_or_else(|| "OK".to_string());
                action_label = Some(label);
            }
            _ => {}
        }
    }

    // Use default message if none found
    let message_text = message_lines.join("\n");

    // Default action if none found
    let action = action_label.or_else(|| Some("OK".to_string()));

    (message_text, action)
}

/// Render a confirm dialog overlay with scrim bands and hit regions.
pub(super) fn render_confirm_overlay(
    canvas: &mut rune_surface::Canvas,
    hit_registry: &mut HitRegionRegistry,
    overlay_id: &ViewNodeId,
    spec: &OverlayContainerSpec,
    title_text: &str,
    overlay_z: i32,
    viewport_width: f32,
    viewport_height: f32,
    view_doc: &rune_ir::view::ViewDocument,
    data_doc: &DataDocument,
) {
    let dismissible = spec.dismissible.unwrap_or(true);
    let show_close = spec.show_close.unwrap_or(true);

    // Resolve content and buttons from children
    let (message_text, primary_label, secondary_label) =
        resolve_confirm_children(view_doc, data_doc, &spec.children);

    let mut confirm = ConfirmDialog::new(
        viewport_width,
        viewport_height,
        title_text.to_string(),
        message_text,
    );
    confirm.primary_label = primary_label;
    confirm.secondary_label = secondary_label;
    confirm.show_shadow = true; // Shadow enabled since visual scrim is disabled
    confirm.base_z = overlay_z + 10;

    confirm.render_overlay(canvas, hit_registry, overlay_id, dismissible, show_close);
}

/// Resolve confirm dialog content and button labels from overlay children.
fn resolve_confirm_children(
    view_doc: &rune_ir::view::ViewDocument,
    data_doc: &DataDocument,
    children: &[ViewNodeId],
) -> (String, String, Option<String>) {
    use rune_ir::view::ViewNodeKind;

    let mut message_lines: Vec<String> = Vec::new();
    let mut primary_label: Option<String> = None;
    let mut secondary_label: Option<String> = None;

    for child_id in children {
        let Some(child_node) = view_doc.node(child_id) else {
            continue;
        };

        match &child_node.kind {
            ViewNodeKind::Text(_spec) => {
                // Resolve text from data document
                if let Some(node_id) = &child_node.node_id {
                    if let Some(text) = resolve_text_from_data(data_doc, node_id) {
                        message_lines.push(text);
                    }
                }
            }
            ViewNodeKind::Button(button_spec) => {
                // Resolve button label from data document
                let label = child_node
                    .node_id
                    .as_ref()
                    .and_then(|node_id| resolve_action_label_from_data(data_doc, node_id))
                    .unwrap_or_else(|| "Button".to_string());

                // Check if this is a primary button based on intent
                let is_primary = button_spec
                    .on_click_intent
                    .as_ref()
                    .map(|intent| {
                        intent.contains("confirm")
                            || intent.contains("submit")
                            || intent.contains("primary")
                            || intent.contains("ok")
                    })
                    .unwrap_or(false);

                if is_primary {
                    primary_label = Some(label);
                } else {
                    secondary_label = Some(label);
                }
            }
            _ => {}
        }
    }

    // Use default message if none found
    let message_text = if message_lines.is_empty() {
        "Are you sure?".to_string()
    } else {
        message_lines.join("\n")
    };

    // Use default labels if none found
    let primary = primary_label.unwrap_or_else(|| "OK".to_string());
    let secondary = secondary_label.or_else(|| Some("Cancel".to_string()));

    (message_text, primary, secondary)
}

fn alert_position_from_overlay(position: &OverlayPosition) -> AlertPosition {
    match position {
        OverlayPosition::TopLeft => AlertPosition::TopLeft,
        OverlayPosition::TopCenter | OverlayPosition::Center => AlertPosition::TopCenter,
        OverlayPosition::TopRight => AlertPosition::TopRight,
        OverlayPosition::BottomLeft => AlertPosition::BottomLeft,
        OverlayPosition::BottomCenter => AlertPosition::BottomCenter,
        OverlayPosition::BottomRight => AlertPosition::BottomRight,
        OverlayPosition::Absolute { .. } => AlertPosition::TopCenter,
    }
}

/// Resolve text content from DataDocument by node_id.
pub(super) fn resolve_text_from_data(data_doc: &DataDocument, node_id: &str) -> Option<String> {
    use rune_ir::data::document::DataNodeKind;
    let data_node = data_doc.node(node_id)?;
    match &data_node.kind {
        DataNodeKind::Text(text_data) => Some(text_data.text.clone()),
        _ => None,
    }
}

/// Resolve action label from DataDocument by node_id.
fn resolve_action_label_from_data(data_doc: &DataDocument, node_id: &str) -> Option<String> {
    use rune_ir::data::document::DataNodeKind;
    let data_node = data_doc.node(node_id)?;
    match &data_node.kind {
        DataNodeKind::Action(action_data) => Some(action_data.label.clone()),
        _ => None,
    }
}

/// Resolve action href from DataDocument by node_id.
fn resolve_action_href_from_data(data_doc: &DataDocument, node_id: &str) -> Option<String> {
    use rune_ir::data::document::DataNodeKind;
    let data_node = data_doc.node(node_id)?;
    match &data_node.kind {
        DataNodeKind::Action(action_data) => action_data.href.clone(),
        _ => None,
    }
}

/// Resolve image source from DataDocument by node_id.
fn resolve_image_source_from_data(
    data_doc: &DataDocument,
    node_id: &str,
) -> Option<std::path::PathBuf> {
    use rune_ir::data::document::DataNodeKind;
    let data_node = data_doc.node(node_id)?;
    match &data_node.kind {
        DataNodeKind::Image(image_data) => Some(std::path::PathBuf::from(&image_data.source)),
        _ => None,
    }
}

/// Resolve table payload from DataDocument by node_id.
fn resolve_table_from_data(
    data_doc: &DataDocument,
    node_id: &str,
) -> Option<rune_ir::data::document::TableNodeData> {
    use rune_ir::data::document::DataNodeKind;
    let data_node = data_doc.node(node_id)?;
    match &data_node.kind {
        DataNodeKind::Table(table_data) => Some(table_data.clone()),
        _ => None,
    }
}

/// Convert rune-ir ViewBackground to engine-core Brush.
fn brush_from_view_background(
    bg: &rune_ir::view::ViewBackground,
    rect: engine_core::Rect,
) -> engine_core::Brush {
    use rune_ir::view::ViewBackground;
    let default_color = engine_core::ColorLinPremul::from_srgba_u8([40, 45, 65, 255]);

    match bg {
        ViewBackground::Solid { color } => crate::ir_adapter::parse_color(color)
            .map(engine_core::Brush::Solid)
            .unwrap_or(engine_core::Brush::Solid(default_color)),
        ViewBackground::LinearGradient { angle, stops } => {
            let angle_rad = (*angle as f32).to_radians();
            let cx = rect.x + rect.w * 0.5;
            let cy = rect.y + rect.h * 0.5;
            let dx = angle_rad.cos();
            let dy = angle_rad.sin();
            let half_len = 0.5 * rect.w.max(rect.h).max(1.0);

            let start = [cx - dx * half_len, cy - dy * half_len];
            let end = [cx + dx * half_len, cy + dy * half_len];

            let parsed_stops: Vec<(f32, engine_core::ColorLinPremul)> = stops
                .iter()
                .filter_map(|(color, t)| {
                    crate::ir_adapter::parse_color(color).map(|c| (*t as f32, c))
                })
                .collect();

            if parsed_stops.is_empty() {
                engine_core::Brush::Solid(default_color)
            } else {
                engine_core::Brush::LinearGradient {
                    start,
                    end,
                    stops: parsed_stops,
                }
            }
        }
        ViewBackground::RadialGradient {
            cx,
            cy,
            rx,
            ry,
            stops,
        } => {
            let center = [
                rect.x + rect.w * (*cx as f32),
                rect.y + rect.h * (*cy as f32),
            ];
            let radius = (*rx as f32)
                .max(*ry as f32)
                .max(rect.w.max(rect.h) * 0.5)
                .max(1.0);

            let parsed_stops: Vec<(f32, engine_core::ColorLinPremul)> = stops
                .iter()
                .filter_map(|(color, t)| {
                    crate::ir_adapter::parse_color(color).map(|c| (*t as f32, c))
                })
                .collect();

            if parsed_stops.is_empty() {
                engine_core::Brush::Solid(default_color)
            } else {
                engine_core::Brush::RadialGradient {
                    center,
                    radius,
                    stops: parsed_stops,
                }
            }
        }
    }
}
