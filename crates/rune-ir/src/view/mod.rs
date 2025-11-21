use serde::{Deserialize, Serialize};

pub type ViewNodeId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewDocument {
    pub view_id: String,
    pub root: ViewNodeId,
    #[serde(default)]
    pub nodes: Vec<ViewNode>,
}

impl ViewDocument {
    pub fn node(&self, node_id: &str) -> Option<&ViewNode> {
        self.nodes.iter().find(|node| node.id == node_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewNode {
    pub id: ViewNodeId,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widget_id: Option<String>,
    #[serde(flatten)]
    pub kind: ViewNodeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ViewNodeKind {
    FlexContainer(FlexContainerSpec),
    GridContainer(GridContainerSpec),
    FormContainer(FormContainerSpec),
    Text(TextSpec),
    Button(ButtonSpec),
    Image(ImageSpec),
    Spacer(SpacerSpec),
    Link(LinkSpec),
    InputBox(InputBoxSpec),
    TextArea(TextAreaSpec),
    Checkbox(CheckboxSpec),
    Radio(RadioSpec),
    Select(SelectSpec),
    FileInput(FileInputSpec),
    /// Date picker with calendar popup
    DatePicker(DatePickerSpec),
    /// Simple table layout rendering data-driven rows/columns.
    Table(TableSpec),
    /// A transient toast/alert overlay. Defaults to `top_center` position.
    Alert(OverlayContainerSpec),
    /// A modal dialog overlay (content-defined). Defaults to centered.
    Modal(OverlayContainerSpec),
    /// A confirm dialog overlay (content-defined). Defaults to centered.
    Confirm(OverlayContainerSpec),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormContainerSpec {
    #[serde(default)]
    pub layout: FlexLayout,
    #[serde(default)]
    pub padding: EdgeInsets,
    #[serde(default)]
    pub margin: EdgeInsets,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<ViewBackground>,
    #[serde(default)]
    pub backgrounds: Vec<ViewBackground>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_color: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub box_shadow: Option<BoxShadowSpec>,
    #[serde(default)]
    pub children: Vec<ViewNodeId>,
    // Form metadata
    pub form_id: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_action: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_method: Option<FormMethod>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_encoding: Option<FormEncoding>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onsubmit_intent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlexContainerSpec {
    #[serde(default)]
    pub layout: FlexLayout,
    #[serde(default)]
    pub padding: EdgeInsets,
    #[serde(default)]
    pub margin: EdgeInsets,
    #[serde(default)]
    pub margin_left_auto: bool,
    #[serde(default)]
    pub margin_right_auto: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<ViewBackground>,
    #[serde(default)]
    pub backgrounds: Vec<ViewBackground>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_height: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_height: Option<f64>,
    // Uniform border (back-compat)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_color: Option<String>,
    // Per-side borders (CSS parity)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_top_width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_right_width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_bottom_width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_left_width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_top_color: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_right_color: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_bottom_color: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_left_color: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub box_shadow: Option<BoxShadowSpec>,
    #[serde(default)]
    pub scroll: ScrollBehavior,
    #[serde(default)]
    pub children: Vec<ViewNodeId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridContainerSpec {
    #[serde(default)]
    pub layout: GridLayout,
    #[serde(default)]
    pub padding: EdgeInsets,
    #[serde(default)]
    pub margin: EdgeInsets,
    #[serde(default)]
    pub margin_left_auto: bool,
    #[serde(default)]
    pub margin_right_auto: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<ViewBackground>,
    #[serde(default)]
    pub backgrounds: Vec<ViewBackground>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_height: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_height: Option<f64>,
    // Uniform border (back-compat)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_color: Option<String>,
    // Per-side borders (CSS parity)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_top_width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_right_width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_bottom_width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_left_width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_top_color: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_right_color: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_bottom_color: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_left_color: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub box_shadow: Option<BoxShadowSpec>,
    #[serde(default)]
    pub children: Vec<ViewNodeId>,
    /// Per-child placement aligned with `children` length
    #[serde(default)]
    pub placements: Vec<GridItemPlacement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridLayout {
    #[serde(default)]
    pub columns: Vec<GridTrackSize>,
    #[serde(default)]
    pub rows: Vec<GridTrackSize>,
    #[serde(default)]
    pub auto_flow: GridAutoFlow,
    #[serde(default)]
    pub column_gap: f64,
    #[serde(default)]
    pub row_gap: f64,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align_items: Option<LayoutAlign>,
}

impl Default for GridLayout {
    fn default() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            auto_flow: GridAutoFlow::Row,
            column_gap: 0.0,
            row_gap: 0.0,
            align_items: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GridAutoFlow {
    Row,
    Column,
    RowDense,
    ColumnDense,
}

impl Default for GridAutoFlow {
    fn default() -> Self {
        GridAutoFlow::Row
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GridTrackSize {
    Fr { value: f64 },
    Px { value: f64 },
    Auto,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GridItemPlacement {
    #[serde(default = "default_one")]
    pub col_span: u16,
    #[serde(default = "default_one")]
    pub row_span: u16,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub col_start: Option<u16>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_start: Option<u16>,
}

fn default_one() -> u16 {
    1
}

/// Positioning for fixed overlays (alert, modal, confirm)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OverlayPosition {
    /// Center of the viewport
    Center,
    /// Top center of the viewport
    TopCenter,
    /// Bottom center of the viewport
    BottomCenter,
    /// Top left of the viewport
    TopLeft,
    /// Top right of the viewport
    TopRight,
    /// Bottom left of the viewport
    BottomLeft,
    /// Bottom right of the viewport
    BottomRight,
    /// Absolute position in viewport coordinates (logical pixels)
    Absolute { x: f64, y: f64 },
}

impl Default for OverlayPosition {
    fn default() -> Self {
        Self::Center
    }
}

/// Shared container spec for alert/modal/confirm overlays.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayContainerSpec {
    #[serde(default)]
    pub layout: FlexLayout,
    #[serde(default)]
    pub padding: EdgeInsets,
    #[serde(default)]
    pub margin: EdgeInsets,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<ViewBackground>,
    #[serde(default)]
    pub backgrounds: Vec<ViewBackground>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(default)]
    pub children: Vec<ViewNodeId>,
    /// Where to place the overlay relative to viewport. Defaults differ per type
    /// but serialize default to Center to make schema simple.
    #[serde(default)]
    pub position: OverlayPosition,
    /// Whether the overlay can be dismissed by ESC/backdrop click. If omitted,
    /// runtime chooses sensible defaults per overlay type.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dismissible: Option<bool>,
    /// Whether to show a close affordance (e.g., an X button) in the overlay
    /// chrome. Typically only used by modals.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_close: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlexLayout {
    #[serde(default)]
    pub direction: LayoutDirection,
    #[serde(default)]
    pub align: LayoutAlign,
    #[serde(default)]
    pub justify: LayoutJustify,
    #[serde(default)]
    pub wrap: bool,
    #[serde(default)]
    pub gap: f64,
}

impl Default for FlexLayout {
    fn default() -> Self {
        Self {
            direction: LayoutDirection::Column,
            align: LayoutAlign::Stretch,
            justify: LayoutJustify::Start,
            wrap: false,
            gap: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutDirection {
    Row,
    Column,
}

impl Default for LayoutDirection {
    fn default() -> Self {
        LayoutDirection::Column
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutAlign {
    Start,
    Center,
    End,
    Stretch,
}

impl Default for LayoutAlign {
    fn default() -> Self {
        LayoutAlign::Stretch
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutJustify {
    Start,
    Center,
    End,
    SpaceBetween,
}

impl Default for LayoutJustify {
    fn default() -> Self {
        LayoutJustify::Start
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EdgeInsets {
    #[serde(default)]
    pub top: f64,
    #[serde(default)]
    pub right: f64,
    #[serde(default)]
    pub bottom: f64,
    #[serde(default)]
    pub left: f64,
}

impl Default for EdgeInsets {
    fn default() -> Self {
        Self {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ViewBackground {
    Solid {
        color: String,
    },
    LinearGradient {
        #[serde(default)]
        angle: f64,
        #[serde(default)]
        stops: Vec<(String, f64)>,
    },
    RadialGradient {
        /// Center position as percentages of the painted box (0..1)
        #[serde(default)]
        cx: f64,
        #[serde(default)]
        cy: f64,
        /// Radii in pixels; ellipse radii, may be approximated
        #[serde(default)]
        rx: f64,
        #[serde(default)]
        ry: f64,
        #[serde(default)]
        stops: Vec<(String, f64)>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScrollBehavior {
    #[serde(default)]
    pub horizontal: bool,
    #[serde(default)]
    pub vertical: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSpec {
    #[serde(default)]
    pub style: TextStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TextStyle {
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_height: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<ViewBackground>,
    #[serde(default)]
    pub padding: EdgeInsets,
    #[serde(default)]
    pub margin: EdgeInsets,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_align: Option<TextAlign>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonSpec {
    #[serde(default)]
    pub style: SurfaceStyle,
    #[serde(default)]
    pub label_style: TextStyle,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_click_intent: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button_type: Option<ButtonType>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_action: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_method: Option<FormMethod>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_encoding: Option<FormEncoding>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonType {
    Button,
    Submit,
    Reset,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormMethod {
    Get,
    Post,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormEncoding {
    Json,
    FormUrlencoded,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SurfaceStyle {
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<ViewBackground>,
    #[serde(default)]
    pub backgrounds: Vec<ViewBackground>,
    #[serde(default)]
    pub padding: EdgeInsets,
    #[serde(default)]
    pub margin: EdgeInsets,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_height: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_height: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_color: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub box_shadow: Option<BoxShadowSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BoxShadowSpec {
    pub offset_x: f64,
    pub offset_y: f64,
    pub blur: f64,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSpec {
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_fit: Option<ImageContentFit>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageContentFit {
    Cover,
    Contain,
    Fill,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacerSpec {
    pub size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkSpec {
    #[serde(default)]
    pub style: SurfaceStyle,
    #[serde(default)]
    pub label_style: TextStyle,
}

/// Declarative table view element. The bound data node supplies columns/rows.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TableSpec {
    /// Outer container style (background, padding, margin, radius, width/height)
    #[serde(default)]
    pub style: SurfaceStyle,
    /// Header text styling
    #[serde(default)]
    pub header_style: TextStyle,
    /// Cell text styling
    #[serde(default)]
    pub cell_style: TextStyle,
    /// Horizontal spacing between columns
    #[serde(default)]
    pub column_gap: f64,
    /// Vertical spacing between rows (in addition to cell paddings)
    #[serde(default)]
    pub row_gap: f64,
    /// Use zebra striping for rows
    #[serde(default)]
    pub zebra: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InputBoxSpec {
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    /// HTML-like input type hint. Supported values: "text" (default),
    /// "password", "email", "number", "search", "date".
    pub input_type: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    #[serde(default)]
    pub style: SurfaceStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TextAreaSpec {
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CheckboxSpec {
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_id: Option<String>,
    /// Submitted value when checked; defaults to "on" if unspecified
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_checked: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RadioSpec {
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    /// Radio group name (controls exclusivity)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Submitted value when selected
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Whether selected by default
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_selected: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SelectSpec {
    #[serde(default)]
    pub style: SurfaceStyle,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_placeholder: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(alias = "multi")]
    pub multiple: Option<bool>,
    #[serde(default)]
    pub options: Vec<SelectOptionSpec>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_height: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SelectOptionSpec {
    pub label: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default)]
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileInputSpec {
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_id: Option<String>,
    /// Allow selecting multiple files when true
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multi: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DatePickerSpec {
    #[serde(default)]
    pub style: SurfaceStyle,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_id: Option<String>,
    /// Initial date value in YYYY-MM-DD format
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
}
