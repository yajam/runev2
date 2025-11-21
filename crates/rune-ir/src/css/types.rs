use std::sync::OnceLock;

/// Limits and timing budgets for CSS processing.
#[derive(Debug, Clone, Copy)]
pub struct CssBudgets {
    pub max_rules: usize,
    pub max_selector_depth: usize,
    pub timeout_ms: u64,
}

impl Default for CssBudgets {
    fn default() -> Self {
        Self {
            max_rules: 10_000,
            max_selector_depth: 3,
            timeout_ms: 50,
        }
    }
}

/// Read budgets from environment variables.
pub fn budgets_from_env() -> &'static CssBudgets {
    static BUDGETS: OnceLock<CssBudgets> = OnceLock::new();
    BUDGETS.get_or_init(|| CssBudgets {
        max_rules: read_env_usize("RUNE_CSS_MAX_RULES")
            .unwrap_or_else(|| CssBudgets::default().max_rules),
        max_selector_depth: read_env_usize("RUNE_CSS_MAX_SELECTOR_DEPTH")
            .unwrap_or_else(|| CssBudgets::default().max_selector_depth),
        timeout_ms: read_env_u64("RUNE_CSS_TIMEOUT_MS")
            .unwrap_or_else(|| CssBudgets::default().timeout_ms),
    })
}

fn read_env_usize(key: &str) -> Option<usize> {
    std::env::var(key).ok()?.trim().parse().ok()
}
fn read_env_u64(key: &str) -> Option<u64> {
    std::env::var(key).ok()?.trim().parse().ok()
}

/// Minimal display set to mirror the current translator expectations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display2 {
    Block,
    Flex,
    Grid,
    Inline,
    None,
}

/// Minimal horizontal text alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign2 {
    Start,
    Center,
    End,
}

/// Edges container for margin/padding.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Edge2 {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

/// Canonical, typed style used by the experimental resolver.
#[derive(Debug, Clone, Default)]
pub struct ComputedStyle2 {
    pub display: Option<Display2>,
    pub flex_direction: Option<crate::view::LayoutDirection>,
    pub justify_content: Option<crate::view::LayoutJustify>,
    pub align_items: Option<crate::view::LayoutAlign>,
    pub text_align: Option<TextAlign2>,
    pub wrap: Option<bool>,
    pub gap: Option<f64>,
    // Grid-specific
    pub grid_template_columns: Option<GridTemplate2>,
    pub grid_template_rows: Option<GridTemplate2>,
    pub grid_auto_flow: Option<GridAutoFlow2>,
    pub column_gap: Option<f64>,
    pub row_gap: Option<f64>,
    // Child placement (span-only minimal subset)
    pub grid_column_span: Option<u16>,
    pub grid_row_span: Option<u16>,

    pub color: Option<String>,
    pub font_family: Option<String>,
    pub font_size: Option<f64>,
    pub line_height: Option<f64>,
    pub font_weight: Option<f64>,

    pub background_color: Option<String>,
    pub background_gradient: Option<LinearGradient2>,
    pub background_radial: Option<RadialGradient2>,
    pub background_layers: Vec<BackgroundLayer2>,

    pub margin: Edge2,
    // Track auto margins for used-value alignment when possible
    pub margin_left_auto: bool,
    pub margin_right_auto: bool,
    pub padding: Edge2,

    pub width: Option<f64>,
    pub height: Option<f64>,

    // Min/Max constraints
    pub min_width: Option<f64>,
    pub min_height: Option<f64>,
    pub max_width: Option<f64>,
    pub max_height: Option<f64>,

    pub object_fit: Option<crate::view::ImageContentFit>,

    pub corner_radius: Option<f64>,

    // Phase 3 extras (uniform)
    pub border_width: Option<f64>,
    pub border_color: Option<String>,
    // Per-side borders for CSS parity; renderer may approximate until full support.
    pub border_top_width: Option<f64>,
    pub border_right_width: Option<f64>,
    pub border_bottom_width: Option<f64>,
    pub border_left_width: Option<f64>,
    pub border_top_color: Option<String>,
    pub border_right_color: Option<String>,
    pub border_bottom_color: Option<String>,
    pub border_left_color: Option<String>,
    pub box_shadow: Option<BoxShadow2>,
}

impl Edge2 {
    pub fn apply(&mut self, v: [f64; 4]) {
        self.top = v[0];
        self.right = v[1];
        self.bottom = v[2];
        self.left = v[3];
    }
}

#[derive(Debug, Clone, Default)]
pub struct LinearGradient2 {
    pub angle: f64,
    pub stops: Vec<ColorStop>,
}

#[derive(Debug, Clone, Default)]
pub struct GridTemplate2 {
    pub tracks: Vec<GridTrackSize2>,
}

#[derive(Debug, Clone)]
pub enum GridTrackSize2 {
    Fr(f64),
    Px(f64),
    Auto,
}

#[derive(Debug, Clone, Copy)]
pub enum GridAutoFlow2 {
    Row,
    Column,
    RowDense,
    ColumnDense,
}

#[derive(Debug, Clone)]
pub struct RadialGradient2 {
    /// Center (normalized 0.0..1.0 range of the box)
    pub cx: f64,
    pub cy: f64,
    /// Radii in pixels (ellipse)
    pub rx: f64,
    pub ry: f64,
    /// Color stops in order (0.0..1.0)
    pub stops: Vec<ColorStop>,
}

#[derive(Debug, Clone)]
pub struct ColorStop {
    pub color: String,
    /// Offset in normalized [0.0, 1.0]
    pub offset: f64,
}

#[derive(Debug, Clone, Default)]
pub struct BoxShadow2 {
    pub offset_x: f64,
    pub offset_y: f64,
    pub blur: f64,
    pub color: String,
}

#[derive(Debug, Clone)]
pub enum BackgroundLayer2 {
    Solid(String),
    Linear(LinearGradient2),
    Radial(RadialGradient2),
}
