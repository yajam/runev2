//! IR Element State Management
//!
//! This module provides state management for interactive IR elements.
//! IR documents (ViewDocument + DataDocument) are stateless specifications,
//! but interactive elements (input boxes, text areas, buttons, etc.) need
//! mutable state for focus, selection, cursor position, and user input.
//!
//! # Architecture
//!
//! - `IrElementState` stores element instances keyed by `ViewNodeId`
//! - Elements are created on-demand during rendering
//! - State persists across frames for smooth interaction
//! - Events are dispatched to elements via `EventHandler` trait
//!
//! # Example
//!
//! ```ignore
//! let mut state = IrElementState::new();
//!
//! // During rendering:
//! let input = state.get_or_create_input_box(node_id, spec, rect);
//! input.render(canvas, z, provider);
//!
//! // During event handling:
//! state.handle_mouse_click(event, hit_result);
//! ```

use crate::elements;
use crate::event_handler::{
    EventHandler, EventResult, KeyboardEvent, MouseClickEvent, MouseMoveEvent,
};
use engine_core::{ColorLinPremul, Rect};
use rune_ir::data::document::DataDocument;
use rune_ir::view::{
    ButtonSpec, CheckboxSpec, DatePickerSpec, FileInputSpec, InputBoxSpec, RadioSpec, SelectSpec,
    TextAlign, TextAreaSpec, ViewNode, ViewNodeId,
};
#[cfg(feature = "webview-cef")]
use rune_ir::view::WebViewSpec;
use std::collections::HashMap;

/// Element type identifier for focus management
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IrElementType {
    InputBox,
    TextArea,
    Button,
    Checkbox,
    Radio,
    Select,
    DatePicker,
    FileInput,
    Link,
    Modal,
    Alert,
    Confirm,
    #[cfg(feature = "webview-cef")]
    WebView,
}

/// Represents an active overlay (modal, alert, or confirm)
#[derive(Debug, Clone)]
pub struct ActiveOverlay {
    /// The view node ID of the overlay
    pub view_node_id: ViewNodeId,
    /// Type of overlay
    pub overlay_type: OverlayType,
}

/// Type of overlay
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayType {
    Modal,
    Alert,
    Confirm,
}

/// State container for all interactive IR elements
///
/// This structure maintains the runtime state of interactive elements
/// that are defined in the IR document. Each element is keyed by its
/// ViewNodeId and created on-demand during rendering.
pub struct IrElementState {
    /// Input box elements (single-line text input)
    input_boxes: HashMap<ViewNodeId, elements::InputBox>,

    /// Text area elements (multi-line text input)
    text_areas: HashMap<ViewNodeId, elements::TextArea>,

    /// Button elements
    buttons: HashMap<ViewNodeId, elements::Button>,

    /// Checkbox elements
    checkboxes: HashMap<ViewNodeId, elements::Checkbox>,

    /// Radio button elements
    radios: HashMap<ViewNodeId, elements::Radio>,

    /// Map of radio view node -> group id
    radio_groups: HashMap<ViewNodeId, String>,

    /// Map of radio group id -> member view nodes
    radio_group_members: HashMap<String, Vec<ViewNodeId>>,

    /// Select/dropdown elements
    selects: HashMap<ViewNodeId, elements::Select>,

    /// Date picker elements
    date_pickers: HashMap<ViewNodeId, elements::DatePicker>,

    /// File input elements
    file_inputs: HashMap<ViewNodeId, elements::FileInput>,

    /// WebView elements (CEF/Chrome browser instances)
    #[cfg(feature = "webview-cef")]
    webviews: HashMap<ViewNodeId, elements::WebView>,

    /// Currently focused element (if any)
    focused_element: Option<(ViewNodeId, IrElementType)>,

    /// Track dirty state (needs redraw)
    dirty: bool,

    /// Currently active overlays (modals, alerts, confirms)
    /// Stored as a stack - last one is on top
    active_overlays: Vec<ActiveOverlay>,
}

impl IrElementState {
    /// Create a new empty element state container
    pub fn new() -> Self {
        Self {
            input_boxes: HashMap::new(),
            text_areas: HashMap::new(),
            buttons: HashMap::new(),
            checkboxes: HashMap::new(),
            radios: HashMap::new(),
            radio_groups: HashMap::new(),
            radio_group_members: HashMap::new(),
            selects: HashMap::new(),
            date_pickers: HashMap::new(),
            file_inputs: HashMap::new(),
            #[cfg(feature = "webview-cef")]
            webviews: HashMap::new(),
            focused_element: None,
            dirty: false,
            active_overlays: Vec::new(),
        }
    }

    /// Clear all element state
    pub fn clear(&mut self) {
        self.input_boxes.clear();
        self.text_areas.clear();
        self.buttons.clear();
        self.checkboxes.clear();
        self.radios.clear();
        self.radio_groups.clear();
        self.radio_group_members.clear();
        self.selects.clear();
        self.date_pickers.clear();
        self.file_inputs.clear();
        #[cfg(feature = "webview-cef")]
        self.webviews.clear();
        self.focused_element = None;
        self.dirty = false;
        self.active_overlays.clear();
    }

    /// Check if a redraw is needed
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark state as clean (after redraw)
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Mark state as dirty (needs redraw)
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    // ========================================================================
    // Element Creation & Access
    // ========================================================================

    /// Get or create an InputBox element for the given ViewNode
    ///
    /// # Arguments
    /// * `view_node_id` - Unique identifier for this element in the view document
    /// * `spec` - IR specification for this input box
    /// * `rect` - Layout rectangle (from Taffy)
    /// * `data_doc` - Data document for resolving bound values
    pub fn get_or_create_input_box(
        &mut self,
        view_node_id: &ViewNodeId,
        spec: &InputBoxSpec,
        rect: Rect,
        _data_doc: &DataDocument,
    ) -> &mut elements::InputBox {
        let entry = self
            .input_boxes
            .entry(view_node_id.clone())
            .or_insert_with(|| {
                // Use default value from spec (data binding would be resolved separately)
                let text = spec.default_value.clone().unwrap_or_default();

                // Default text styling; overridden below if IR provides text_style.
                let text_color = ColorLinPremul::from_srgba_u8([26, 32, 44, 255]);
                let text_size = 16.0;

                elements::InputBox::new(
                    rect,
                    text,
                    text_size,
                    text_color,
                    spec.placeholder.clone(),
                    false, // initial focus state
                )
            });
        entry.apply_surface_style(&spec.style);

        // CRITICAL: Update rect every frame to handle window resize and layout changes
        entry.rect = rect;
        // Apply text styling from IR
        if let Some(color) = spec
            .text_style
            .color
            .as_ref()
            .and_then(|c| crate::ir_adapter::parse_color(c))
        {
            entry.text_color = color;
        }
        if let Some(size) = spec.text_style.font_size {
            entry.text_size = size as f32;
        }
        entry.text_align = spec
            .text_style
            .text_align
            .unwrap_or(TextAlign::Start);
        entry
    }

    /// Get or create a TextArea element for the given ViewNode
    pub fn get_or_create_text_area(
        &mut self,
        view_node_id: &ViewNodeId,
        spec: &TextAreaSpec,
        rect: Rect,
        _data_doc: &DataDocument,
    ) -> &mut elements::TextArea {
        let entry = self
            .text_areas
            .entry(view_node_id.clone())
            .or_insert_with(|| {
                // Use default value from spec (data binding would be resolved separately)
                let text = spec.default_value.clone().unwrap_or_default();

                let text_color = ColorLinPremul::from_srgba_u8([26, 32, 44, 255]);
                let text_size = 16.0;

                let mut textarea = elements::TextArea::new(
                    rect,
                    text,
                    text_size,
                    text_color,
                    spec.placeholder.clone(),
                    false, // initial focus state
                );
                textarea.apply_surface_style(&spec.style);
                if spec.style.background.is_none() {
                    textarea.bg_color = ColorLinPremul::from_srgba_u8([0, 0, 0, 0]);
                }
                textarea
            });

        // CRITICAL: Update rect every frame to handle window resize and layout changes
        entry.set_rect(rect);
        entry.apply_surface_style(&spec.style);
        if spec.style.background.is_none() {
            entry.bg_color = ColorLinPremul::from_srgba_u8([0, 0, 0, 0]);
        }
        if let Some(color) = spec
            .text_style
            .color
            .as_ref()
            .and_then(|c| crate::ir_adapter::parse_color(c))
        {
            entry.text_color = color;
        }
        if let Some(size) = spec.text_style.font_size {
            entry.text_size = size as f32;
        }
        entry
    }

    /// Get or create a Button element for the given ViewNode
    ///
    /// Resolves button label from the data document using the view node's node_id.
    pub fn get_or_create_button(
        &mut self,
        view_node_id: &ViewNodeId,
        view_node: &rune_ir::view::ViewNode,
        spec: &ButtonSpec,
        rect: Rect,
        data_doc: &DataDocument,
    ) -> &mut elements::Button {
        let intent = spec.on_click_intent.clone();

        // Resolve label from data document via node_id
        let label = view_node
            .node_id
            .as_ref()
            .and_then(|node_id| {
                use rune_ir::data::document::DataNodeKind;
                let data_node = data_doc.node(node_id)?;
                match &data_node.kind {
                    DataNodeKind::Action(action_data) => Some(action_data.label.clone()),
                    _ => None,
                }
            })
            .unwrap_or_else(|| "Button".to_string());

        let button = self
            .buttons
            .entry(view_node_id.clone())
            .or_insert_with(|| elements::Button {
                rect,
                radius: spec.style.corner_radius.unwrap_or(4.0) as f32,
                bg: ColorLinPremul::from_srgba_u8([59, 130, 246, 255]),
                fg: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
                label: label.clone(),
                label_size: 14.0,
                focused: false,
                on_click_intent: intent.clone(),
            });

        // CRITICAL: Update rect every frame to handle window resize and layout changes
        button.rect = rect;
        // Also update intent in case it changed
        button.on_click_intent = intent;
        // Update label in case data changed
        button.label = label;
        button
    }

    /// Get or create a Checkbox element for the given ViewNode
    pub fn get_or_create_checkbox(
        &mut self,
        view_node_id: &ViewNodeId,
        view_node: &ViewNode,
        spec: &CheckboxSpec,
        rect: Rect,
        data_doc: &DataDocument,
    ) -> &mut elements::Checkbox {
        let label = view_node
            .node_id
            .as_ref()
            .and_then(|node_id| super::elements::resolve_text_from_data(data_doc, node_id));

        let colors = crate::ir_adapter::surface_colors(
            &spec.style,
            ColorLinPremul::from_srgba_u8([245, 247, 250, 255]),
            ColorLinPremul::from_srgba_u8([200, 208, 216, 255]),
        );

        let box_size = spec
            .size
            .unwrap_or_else(|| rect.w.min(rect.h) as f64)
            .max(0.0) as f32;

        let checkbox = self
            .checkboxes
            .entry(view_node_id.clone())
            .or_insert_with(|| {
                crate::ir_adapter::IrAdapter::checkbox_from_spec(
                    spec,
                    Rect {
                        x: rect.x,
                        y: rect.y,
                        w: box_size,
                        h: box_size,
                    },
                    label.clone(),
                )
            });

        // Keep layout in sync with Taffy output; preserve user-driven checked state.
        checkbox.rect.x = rect.x;
        checkbox.rect.y = rect.y;
        checkbox.rect.w = box_size;
        checkbox.rect.h = box_size;
        checkbox.label = label;
        checkbox.box_fill = colors.fill;
        checkbox.border_color = colors.border;
        checkbox.border_width = colors.border_width;
        // Use IR-provided label color when present
        if let Some(style) = spec.label_style.as_ref() {
            if let Some(c) = style
                .color
                .as_ref()
                .and_then(|c| crate::ir_adapter::parse_color(c))
            {
                checkbox.label_color = c;
            }
            if let Some(size) = style.font_size {
                checkbox.label_size = size as f32;
            }
        } else if let Some(c) = spec
            .label_color
            .as_ref()
            .and_then(|c| crate::ir_adapter::parse_color(c))
        {
            checkbox.label_color = c;
        } else {
            checkbox.label_color = ColorLinPremul::from_srgba_u8([51, 65, 85, 255]);
        }
        checkbox.check_color = ColorLinPremul::from_srgba_u8([63, 130, 246, 255]);
        checkbox
    }

    /// Get or create a Radio button element for the given ViewNode
    pub fn get_or_create_radio(
        &mut self,
        view_node_id: &ViewNodeId,
        view_node: &ViewNode,
        spec: &RadioSpec,
        rect: Rect,
        data_doc: &DataDocument,
    ) -> &mut elements::Radio {
        let label = view_node
            .node_id
            .as_ref()
            .and_then(|node_id| super::elements::resolve_text_from_data(data_doc, node_id));

        let colors = crate::ir_adapter::surface_colors(
            &spec.style,
            ColorLinPremul::from_srgba_u8([245, 247, 250, 255]),
            ColorLinPremul::from_srgba_u8([200, 208, 216, 255]),
        );

        let diameter = spec
            .size
            .unwrap_or_else(|| rect.w.min(rect.h).max(1.0) as f64)
            .max(1.0) as f32;
        let radius = diameter * 0.5;

        let selected_after_creation = {
            let entry = self.radios.entry(view_node_id.clone()).or_insert_with(|| {
                crate::ir_adapter::IrAdapter::radio_from_spec(
                    spec,
                    Rect {
                        x: rect.x,
                        y: rect.y,
                        w: diameter,
                        h: diameter,
                    },
                    label.clone(),
                )
            });

            // Keep layout and metadata in sync.
            entry.center = [rect.x + rect.w * 0.5, rect.y + rect.h * 0.5];
            entry.radius = radius;
            entry.label = label;
            entry.bg = colors.fill;
            entry.border_color = colors.border;
            entry.border_width = colors.border_width;
            // Use IR-provided label color when present
            if let Some(c) = spec
                .label_color
                .as_ref()
                .and_then(|c| crate::ir_adapter::parse_color(c))
            {
                entry.label_color = c;
            } else if let Some(style) = spec.label_style.as_ref() {
                if let Some(c) = style.color.as_ref().and_then(|c| crate::ir_adapter::parse_color(c))
                {
                    entry.label_color = c;
                }
                if let Some(size) = style.font_size {
                    entry.label_size = size as f32;
                }
            } else {
                entry.label_color = ColorLinPremul::from_srgba_u8([51, 65, 85, 255]);
            }
            if let Some(style) = spec.label_style.as_ref() {
                if let Some(size) = style.font_size {
                    entry.label_size = size as f32;
                }
            }
            entry.dot_color = ColorLinPremul::from_srgba_u8([63, 130, 246, 255]);
            entry.selected
        };

        // Track radio group membership for mutual exclusivity.
        if let Some(group) = &spec.group {
            self.radio_groups
                .insert(view_node_id.clone(), group.clone());
            let members = self
                .radio_group_members
                .entry(group.clone())
                .or_insert_with(Vec::new);
            if !members.contains(view_node_id) {
                members.push(view_node_id.clone());
            }
        }

        if selected_after_creation {
            self.deselect_other_radios(view_node_id);
        }

        self.radios
            .get_mut(view_node_id)
            .expect("radio should exist after insertion")
    }

    fn deselect_other_radios(&mut self, view_node_id: &ViewNodeId) {
        let Some(group_id) = self.radio_groups.get(view_node_id) else {
            return;
        };

        if let Some(members) = self.radio_group_members.get(group_id) {
            for other in members {
                if other != view_node_id {
                    if let Some(radio) = self.radios.get_mut(other) {
                        radio.selected = false;
                    }
                }
            }
        }
    }

    /// Get or create a Select element for the given ViewNode
    pub fn get_or_create_select(
        &mut self,
        view_node_id: &ViewNodeId,
        spec: &SelectSpec,
        rect: Rect,
    ) -> &mut elements::Select {
        let entry = self.selects.entry(view_node_id.clone()).or_insert_with(|| {
            let options: Vec<String> = spec.options.iter().map(|opt| opt.label.clone()).collect();

            let selected_index = spec.options.iter().position(|opt| opt.selected);
            let placeholder = spec
                .placeholder
                .clone()
                .unwrap_or_else(|| "Select an option".to_string());
            let (label, is_placeholder) = match selected_index {
                Some(idx) if idx < options.len() => (options[idx].clone(), false),
                _ => (placeholder.clone(), true),
            };
            let label_color = crate::ir_adapter::IrAdapter::color_from_text_style(&spec.label_style);
            let placeholder_color = engine_core::ColorLinPremul {
                r: label_color.r,
                g: label_color.g,
                b: label_color.b,
                a: label_color.a * 0.6,
            };
            let label_size =
                crate::ir_adapter::IrAdapter::font_size_from_text_style(&spec.label_style);

            elements::Select {
                rect,
                label,
                placeholder,
                label_size,
                label_color,
                placeholder_color,
                is_placeholder,
                open: false,
                focused: false,
                options,
                selected_index,
                padding_left: 14.0,
                padding_right: 14.0,
                padding_top: 10.0,
                padding_bottom: 10.0,
                bg_color: ColorLinPremul::from_srgba_u8([245, 247, 250, 255]),
                border_color: ColorLinPremul::from_srgba_u8([200, 208, 216, 255]),
                border_width: 1.0,
                radius: 8.0,
            }
        });

        // Keep layout in sync with Taffy output each frame.
        entry.rect = rect;
        entry.apply_surface_style(&spec.style);
        // Update text styling each frame in case IR changed
        entry.label_size =
            crate::ir_adapter::IrAdapter::font_size_from_text_style(&spec.label_style);
        entry.label_color = crate::ir_adapter::IrAdapter::color_from_text_style(&spec.label_style);
        entry.placeholder_color = engine_core::ColorLinPremul {
            r: entry.label_color.r,
            g: entry.label_color.g,
            b: entry.label_color.b,
            a: entry.label_color.a * 0.6,
        };
        entry
    }

    /// Get or create a FileInput element for the given ViewNode
    pub fn get_or_create_file_input(
        &mut self,
        view_node_id: &ViewNodeId,
        spec: &FileInputSpec,
        rect: Rect,
    ) -> &mut elements::FileInput {
        let multi = spec.multi.unwrap_or(false);
        let placeholder = spec.placeholder.clone();

        let entry = self
            .file_inputs
            .entry(view_node_id.clone())
            .or_insert_with(|| {
                let mut file_input = elements::FileInput::new(rect, multi);

                if let Some(ph) = &placeholder {
                    file_input.placeholder = ph.clone();
                }

                file_input
            });

        // Keep layout and config in sync with the latest spec.
        entry.rect = rect;
        entry.multi = multi;
        entry.placeholder = placeholder.unwrap_or_else(|| {
            if entry.multi {
                "Choose files".to_string()
            } else {
                "Choose File".to_string()
            }
        });
        // Apply label style overrides
        let label_style = &spec.label_style;
        entry.label_size = label_style.font_size.unwrap_or(entry.label_size as f64) as f32;
        entry.label_color = crate::ir_adapter::IrAdapter::color_from_text_style(label_style);
        // Use same color for button/file text to keep legibility aligned
        entry.button_text_color = entry.label_color;
        entry.file_text_color = entry.label_color;

        entry
    }

    /// Get or create a DatePicker element for the given ViewNode
    pub fn get_or_create_date_picker(
        &mut self,
        view_node_id: &ViewNodeId,
        spec: &DatePickerSpec,
        rect: Rect,
        viewport_height: f32,
    ) -> &mut elements::DatePicker {
        let entry = self
            .date_pickers
            .entry(view_node_id.clone())
            .or_insert_with(|| {
                let label_size = spec
                    .label_style
                    .font_size
                    .unwrap_or(16.0) as f32;
                let label_color =
                    crate::ir_adapter::IrAdapter::color_from_text_style(&spec.label_style);
                // Parse initial date from spec if provided (YYYY-MM-DD format)
                let initial_date = spec.default_value.as_ref().and_then(|s| {
                    let parts: Vec<&str> = s.split('-').collect();
                    if parts.len() == 3 {
                        let year = parts[0].parse::<u32>().ok()?;
                        let month = parts[1].parse::<u32>().ok()?;
                        let day = parts[2].parse::<u32>().ok()?;
                        Some((year, month, day))
                    } else {
                        None
                    }
                });

                elements::DatePicker::new(
                    rect,
                    label_size,
                    label_color,
                    initial_date,
                )
            });

        // Keep layout in sync with Taffy output each frame
        entry.rect = rect;
        // Set viewport height for smart popup positioning
        entry.set_viewport_height(viewport_height);
        // Apply surface styling
        entry.apply_surface_style(&spec.style);
        // Apply label style overrides
        entry.label_size = spec.label_style.font_size.unwrap_or(entry.label_size as f64) as f32;
        entry.label_color = crate::ir_adapter::IrAdapter::color_from_text_style(&spec.label_style);
        entry
    }

    /// Get or create a WebView element for the given ViewNode
    #[cfg(feature = "webview-cef")]
    pub fn get_or_create_webview(
        &mut self,
        view_node_id: &ViewNodeId,
        spec: &WebViewSpec,
        rect: Rect,
    ) -> &mut elements::WebView {
        let entry = self
            .webviews
            .entry(view_node_id.clone())
            .or_insert_with(|| {
                let mut webview =
                    elements::WebView::new(rect, spec.url.clone(), spec.html.clone());

                // Apply configuration from spec
                if let Some(base_url) = &spec.base_url {
                    webview.base_url = Some(base_url.clone());
                }
                if let Some(scale) = spec.scale_factor {
                    webview.scale_factor = scale as f32;
                }
                if let Some(js_enabled) = spec.javascript_enabled {
                    webview.javascript_enabled = js_enabled;
                }
                if let Some(ua) = &spec.user_agent {
                    webview.user_agent = Some(ua.clone());
                }

                webview
            });

        // CRITICAL: Update rect every frame to handle window resize and layout changes
        entry.rect = rect;

        // Apply surface styling
        if let Some(bg) = &spec.style.background {
            if let Some(color) = crate::ir_adapter::view_background_to_color(bg) {
                entry.bg_color = color;
            }
        }
        if let Some(border_color) = spec
            .style
            .border_color
            .as_ref()
            .and_then(|c| crate::ir_adapter::parse_color(c))
        {
            entry.border_color = border_color;
        }
        if let Some(border_width) = spec.style.border_width {
            entry.border_width = border_width as f32;
        }
        if let Some(radius) = spec.style.corner_radius {
            entry.corner_radius = radius as f32;
        }

        entry
    }

    /// Get a mutable reference to a WebView element
    #[cfg(feature = "webview-cef")]
    pub fn get_webview_mut(&mut self, view_node_id: &ViewNodeId) -> Option<&mut elements::WebView> {
        self.webviews.get_mut(view_node_id)
    }

    /// Get all WebView elements for frame updates
    #[cfg(feature = "webview-cef")]
    pub fn webviews_mut(&mut self) -> impl Iterator<Item = (&ViewNodeId, &mut elements::WebView)> {
        self.webviews.iter_mut()
    }

    // ========================================================================
    // Focus Management
    // ========================================================================

    /// Set focus to a specific element
    pub fn set_focus(&mut self, view_node_id: ViewNodeId, element_type: IrElementType) {
        // Clear focus from all elements
        self.clear_all_focus();

        // Set focus on the target element
        match element_type {
            IrElementType::InputBox => {
                if let Some(input) = self.input_boxes.get_mut(&view_node_id) {
                    input.set_focused(true);
                }
            }
            IrElementType::TextArea => {
                if let Some(textarea) = self.text_areas.get_mut(&view_node_id) {
                    textarea.set_focused(true);
                }
            }
            IrElementType::Button => {
                if let Some(button) = self.buttons.get_mut(&view_node_id) {
                    button.set_focused(true);
                }
            }
            IrElementType::Checkbox => {
                if let Some(checkbox) = self.checkboxes.get_mut(&view_node_id) {
                    checkbox.set_focused(true);
                }
            }
            IrElementType::Radio => {
                if let Some(radio) = self.radios.get_mut(&view_node_id) {
                    radio.set_focused(true);
                }
            }
            IrElementType::Select => {
                if let Some(select) = self.selects.get_mut(&view_node_id) {
                    select.set_focused(true);
                }
            }
            IrElementType::DatePicker => {
                if let Some(picker) = self.date_pickers.get_mut(&view_node_id) {
                    picker.set_focused(true);
                }
            }
            IrElementType::FileInput => {
                if let Some(file_input) = self.file_inputs.get_mut(&view_node_id) {
                    file_input.set_focused(true);
                }
            }
            #[cfg(feature = "webview-cef")]
            IrElementType::WebView => {
                if let Some(webview) = self.webviews.get_mut(&view_node_id) {
                    webview.set_focused(true);
                }
            }
            _ => {}
        }

        self.focused_element = Some((view_node_id, element_type));
        self.dirty = true;
    }

    /// Clear focus from all elements
    pub fn clear_all_focus(&mut self) {
        for input in self.input_boxes.values_mut() {
            input.set_focused(false);
        }
        for textarea in self.text_areas.values_mut() {
            textarea.set_focused(false);
        }
        for button in self.buttons.values_mut() {
            button.set_focused(false);
        }
        for checkbox in self.checkboxes.values_mut() {
            checkbox.set_focused(false);
        }
        for radio in self.radios.values_mut() {
            radio.set_focused(false);
        }
        for select in self.selects.values_mut() {
            select.set_focused(false);
        }
        for picker in self.date_pickers.values_mut() {
            picker.set_focused(false);
        }
        for file_input in self.file_inputs.values_mut() {
            file_input.set_focused(false);
        }
        #[cfg(feature = "webview-cef")]
        for webview in self.webviews.values_mut() {
            webview.set_focused(false);
        }

        self.focused_element = None;
        self.dirty = true;
    }

    /// Close all open select dropdowns
    pub fn close_all_selects(&mut self) {
        for select in self.selects.values_mut() {
            if select.is_open() {
                select.close();
                self.dirty = true;
            }
        }
    }

    /// Check if a point is inside any open select's overlay
    pub fn is_point_in_select_overlay(&self, x: f32, y: f32) -> bool {
        for select in self.selects.values() {
            if select.is_open() {
                if let Some(overlay_bounds) = select.get_overlay_bounds() {
                    if x >= overlay_bounds.x
                        && x <= overlay_bounds.x + overlay_bounds.w
                        && y >= overlay_bounds.y
                        && y <= overlay_bounds.y + overlay_bounds.h
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Close all open selects except those whose overlay contains the given point
    pub fn close_selects_except_at_point(&mut self, x: f32, y: f32) {
        for select in self.selects.values_mut() {
            if select.is_open() {
                // Don't close if click is on the overlay
                if let Some(overlay_bounds) = select.get_overlay_bounds() {
                    if x >= overlay_bounds.x
                        && x <= overlay_bounds.x + overlay_bounds.w
                        && y >= overlay_bounds.y
                        && y <= overlay_bounds.y + overlay_bounds.h
                    {
                        continue;
                    }
                }
                // Don't close if click is on the select field itself
                if select.contains_point(x, y) {
                    continue;
                }
                select.close();
                self.dirty = true;
            }
        }
    }

    /// Close all open date picker popups
    pub fn close_all_date_pickers(&mut self) {
        for picker in self.date_pickers.values_mut() {
            if picker.is_open() {
                picker.close_popup();
                self.dirty = true;
            }
        }
    }

    /// Close all open date pickers except those whose popup contains the given point
    pub fn close_date_pickers_except_at_point(&mut self, x: f32, y: f32) {
        for picker in self.date_pickers.values_mut() {
            if picker.is_open() {
                // Don't close if click is on the popup
                if let Some(popup_bounds) = picker.get_popup_bounds() {
                    if x >= popup_bounds.x
                        && x <= popup_bounds.x + popup_bounds.w
                        && y >= popup_bounds.y
                        && y <= popup_bounds.y + popup_bounds.h
                    {
                        continue;
                    }
                }
                // Don't close if click is on the date picker field itself
                if picker.contains_point(x, y) {
                    continue;
                }
                picker.close_popup();
                self.dirty = true;
            }
        }
    }

    /// Get the currently focused element
    pub fn get_focused_element(&self) -> Option<(ViewNodeId, IrElementType)> {
        self.focused_element.clone()
    }

    // ========================================================================
    // Overlay Management
    // ========================================================================

    /// Show an overlay (modal, alert, or confirm)
    pub fn show_overlay(&mut self, view_node_id: ViewNodeId, overlay_type: OverlayType) {
        // Check if already shown
        if self
            .active_overlays
            .iter()
            .any(|o| o.view_node_id == view_node_id)
        {
            return;
        }

        self.active_overlays.push(ActiveOverlay {
            view_node_id,
            overlay_type,
        });
        self.dirty = true;
    }

    /// Hide an overlay by view node ID
    pub fn hide_overlay(&mut self, view_node_id: &ViewNodeId) {
        if let Some(pos) = self
            .active_overlays
            .iter()
            .position(|o| &o.view_node_id == view_node_id)
        {
            self.active_overlays.remove(pos);
            self.dirty = true;
        }
    }

    /// Hide all overlays
    pub fn hide_all_overlays(&mut self) {
        if !self.active_overlays.is_empty() {
            self.active_overlays.clear();
            self.dirty = true;
        }
    }

    /// Hide the topmost overlay
    pub fn hide_topmost_overlay(&mut self) {
        if self.active_overlays.pop().is_some() {
            self.dirty = true;
        }
    }

    /// Get active overlays (for rendering)
    pub fn get_active_overlays(&self) -> &[ActiveOverlay] {
        &self.active_overlays
    }

    /// Check if any overlay is active
    pub fn has_active_overlay(&self) -> bool {
        !self.active_overlays.is_empty()
    }

    /// Check if a specific overlay is active
    pub fn is_overlay_active(&self, view_node_id: &ViewNodeId) -> bool {
        self.active_overlays
            .iter()
            .any(|o| &o.view_node_id == view_node_id)
    }

    /// Handle an intent string from button click
    /// Returns true if the intent was handled
    pub fn handle_intent(&mut self, intent: &str) -> bool {
        // Parse intent format: "action:target" (e.g., "show:modal_id", "hide:alert_id")
        let parts: Vec<&str> = intent.splitn(2, ':').collect();
        if parts.len() != 2 {
            return false;
        }

        let action = parts[0];
        let target = parts[1];

        match action {
            "show_modal" => {
                self.show_overlay(target.to_string(), OverlayType::Modal);
                true
            }
            "show_alert" => {
                self.show_overlay(target.to_string(), OverlayType::Alert);
                true
            }
            "show_confirm" => {
                self.show_overlay(target.to_string(), OverlayType::Confirm);
                true
            }
            "hide" => {
                self.hide_overlay(&target.to_string());
                true
            }
            "dismiss" => {
                // Dismiss topmost overlay
                self.hide_topmost_overlay();
                true
            }
            _ => false,
        }
    }

    // ========================================================================
    // Event Handling
    // ========================================================================

    /// Handle a mouse click event
    ///
    /// Returns true if the event was handled by an element
    pub fn handle_mouse_click(
        &mut self,
        view_node_id: &ViewNodeId,
        event: MouseClickEvent,
    ) -> EventResult {
        // Try each element type
        if let Some(input) = self.input_boxes.get_mut(view_node_id) {
            let result = input.handle_mouse_click(event);
            if result.is_handled() {
                self.set_focus(view_node_id.clone(), IrElementType::InputBox);
                return EventResult::Handled;
            }
        }

        if let Some(textarea) = self.text_areas.get_mut(view_node_id) {
            let result = textarea.handle_mouse_click(event);
            if result.is_handled() {
                self.set_focus(view_node_id.clone(), IrElementType::TextArea);
                return EventResult::Handled;
            }
        }

        if let Some(button) = self.buttons.get_mut(view_node_id) {
            let result = button.handle_mouse_click(event);
            if result.is_handled() {
                // Extract intent before setting focus (avoids borrow issues)
                let intent = button.on_click_intent.clone();
                self.set_focus(view_node_id.clone(), IrElementType::Button);

                // Process intent if present
                if let Some(intent_str) = intent {
                    self.handle_intent(&intent_str);
                }
                return EventResult::Handled;
            }
        }

        if let Some(checkbox) = self.checkboxes.get_mut(view_node_id) {
            let result = checkbox.handle_mouse_click(event);
            if result.is_handled() {
                self.set_focus(view_node_id.clone(), IrElementType::Checkbox);
                return EventResult::Handled;
            }
        }

        if let Some(radio) = self.radios.get_mut(view_node_id) {
            let result = radio.handle_mouse_click(event);
            if result.is_handled() {
                self.set_focus(view_node_id.clone(), IrElementType::Radio);
                self.deselect_other_radios(view_node_id);
                self.dirty = true;
                return EventResult::Handled;
            }
        }

        if let Some(select) = self.selects.get_mut(view_node_id) {
            let result = select.handle_mouse_click(event);
            if result.is_handled() {
                self.set_focus(view_node_id.clone(), IrElementType::Select);
                return EventResult::Handled;
            }
        }

        if let Some(picker) = self.date_pickers.get_mut(view_node_id) {
            let result = picker.handle_mouse_click(event);
            if result.is_handled() {
                self.set_focus(view_node_id.clone(), IrElementType::DatePicker);
                return EventResult::Handled;
            }
        }

        if let Some(file_input) = self.file_inputs.get_mut(view_node_id) {
            let result = file_input.handle_mouse_click(event);
            if result.is_handled() {
                self.set_focus(view_node_id.clone(), IrElementType::FileInput);
                self.dirty = true;
                return EventResult::Handled;
            }
        }

        #[cfg(feature = "webview-cef")]
        if let Some(webview) = self.webviews.get_mut(view_node_id) {
            let result = webview.handle_mouse_click(event);
            if result.is_handled() {
                self.set_focus(view_node_id.clone(), IrElementType::WebView);
                self.dirty = true;
                return EventResult::Handled;
            }
        }

        EventResult::Ignored
    }

    /// Handle a keyboard event
    ///
    /// Dispatches to the currently focused element
    pub fn handle_keyboard(&mut self, event: KeyboardEvent) -> EventResult {
        if let Some((view_node_id, element_type)) = &self.focused_element {
            match element_type {
                IrElementType::InputBox => {
                    if let Some(input) = self.input_boxes.get_mut(view_node_id) {
                        let result = input.handle_keyboard(event);
                        if result.is_handled() {
                            self.dirty = true;
                        }
                        return result;
                    }
                }
                IrElementType::TextArea => {
                    if let Some(textarea) = self.text_areas.get_mut(view_node_id) {
                        let result = textarea.handle_keyboard(event);
                        if result.is_handled() {
                            self.dirty = true;
                        }
                        return result;
                    }
                }
                IrElementType::Button => {
                    if let Some(button) = self.buttons.get_mut(view_node_id) {
                        return button.handle_keyboard(event);
                    }
                }
                IrElementType::Checkbox => {
                    if let Some(checkbox) = self.checkboxes.get_mut(view_node_id) {
                        let result = checkbox.handle_keyboard(event);
                        if result.is_handled() {
                            self.dirty = true;
                        }
                        return result;
                    }
                }
                IrElementType::Radio => {
                    let focused_id = view_node_id.clone();
                    if let Some(radio) = self.radios.get_mut(&focused_id) {
                        let result = radio.handle_keyboard(event);
                        if result.is_handled() {
                            self.deselect_other_radios(&focused_id);
                            self.dirty = true;
                        }
                        return result;
                    }
                }
                IrElementType::Select => {
                    // TODO: Select has a different keyboard interface (SelectKey vs KeyboardEvent)
                    // For now, we don't handle keyboard events for selects in IR mode
                    return EventResult::Ignored;
                }
                IrElementType::FileInput => {
                    if let Some(file_input) = self.file_inputs.get_mut(view_node_id) {
                        let result = file_input.handle_keyboard(event);
                        if result.is_handled() {
                            self.dirty = true;
                        }
                        return result;
                    }
                }
                #[cfg(feature = "webview-cef")]
                IrElementType::WebView => {
                    if let Some(webview) = self.webviews.get_mut(view_node_id) {
                        let result = webview.handle_keyboard(event);
                        if result.is_handled() {
                            self.dirty = true;
                        }
                        return result;
                    }
                }
                _ => {}
            }
        }

        EventResult::Ignored
    }

    /// Handle textual character input (e.g. winit's `text` field).
    ///
    /// This is separate from `handle_keyboard` so we can process graphemes that
    /// don't map cleanly to `KeyCode` (IME, punctuation, etc.).
    pub fn handle_text_input(&mut self, text: &str) -> EventResult {
        if text.is_empty() {
            return EventResult::Ignored;
        }

        let Some((view_node_id, element_type)) = &self.focused_element else {
            return EventResult::Ignored;
        };

        match element_type {
            IrElementType::InputBox => {
                if let Some(input) = self.input_boxes.get_mut(view_node_id) {
                    let mut handled = false;
                    for mut ch in text.chars() {
                        if ch == '\r' {
                            ch = '\n';
                        }
                        // Skip control characters except space; drop newlines for single-line.
                        if ch == '\n' || ch == '\r' {
                            continue;
                        }
                        if ch.is_control() && ch != ' ' {
                            continue;
                        }
                        input.insert_char(ch);
                        handled = true;
                    }
                    if handled {
                        self.dirty = true;
                        return EventResult::Handled;
                    }
                }
            }
            IrElementType::TextArea => {
                if let Some(textarea) = self.text_areas.get_mut(view_node_id) {
                    let mut handled = false;
                    for mut ch in text.chars() {
                        if ch == '\r' {
                            ch = '\n';
                        }
                        if ch.is_control() && ch != '\n' && ch != '\t' && ch != ' ' {
                            continue;
                        }
                        textarea.insert_char(ch);
                        handled = true;
                    }
                    if handled {
                        self.dirty = true;
                        return EventResult::Handled;
                    }
                }
            }
            _ => {}
        }

        EventResult::Ignored
    }

    /// Handle a mouse move event
    ///
    /// Dispatches to the currently focused element for drag operations
    pub fn handle_mouse_move(&mut self, event: MouseMoveEvent) -> EventResult {
        if let Some((view_node_id, element_type)) = &self.focused_element {
            match element_type {
                IrElementType::InputBox => {
                    if let Some(input) = self.input_boxes.get_mut(view_node_id) {
                        let result = input.handle_mouse_move(event);
                        if result.is_handled() {
                            self.dirty = true;
                        }
                        return result;
                    }
                }
                IrElementType::TextArea => {
                    if let Some(textarea) = self.text_areas.get_mut(view_node_id) {
                        let result = textarea.handle_mouse_move(event);
                        if result.is_handled() {
                            self.dirty = true;
                        }
                        return result;
                    }
                }
                #[cfg(feature = "webview-cef")]
                IrElementType::WebView => {
                    if let Some(webview) = self.webviews.get_mut(view_node_id) {
                        let result = webview.handle_mouse_move(event);
                        if result.is_handled() {
                            self.dirty = true;
                        }
                        return result;
                    }
                }
                _ => {}
            }
        }

        EventResult::Ignored
    }

    // ========================================================================
    // Animation Updates
    // ========================================================================

    /// Update cursor blink animation for all text editing elements
    ///
    /// Call this every frame to keep cursor blinking
    pub fn update_blink_animation(&mut self, delta_time: f32) {
        for input in self.input_boxes.values_mut() {
            input.update_blink(delta_time);
        }
        for textarea in self.text_areas.values_mut() {
            textarea.update_blink(delta_time);
        }
    }

    // ========================================================================
    // Data Synchronization
    // ========================================================================

    /// Synchronize element state back to data document
    ///
    /// Updates text content in the data document when user edits input fields
    pub fn sync_to_data_document(&self, _data_doc: &mut DataDocument) {
        // TODO: Implement bidirectional data binding
        // For now, elements maintain their own state
        // Future: Update DataDocument text nodes when inputs change
    }
}

impl Default for IrElementState {
    fn default() -> Self {
        Self::new()
    }
}
