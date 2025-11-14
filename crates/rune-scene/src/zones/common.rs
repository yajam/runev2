use engine_core::{Rect, ColorLinPremul};

/// Zone identifiers for hit testing and layout
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneId {
    Viewport = 1000,
    Sidebar = 2000,
    Toolbar = 3000,
    DevTools = 4000,
}

impl ZoneId {
    pub fn as_u32(self) -> u32 {
        self as u32
    }
}

/// Zone styling configuration
#[derive(Debug, Clone)]
pub struct ZoneStyle {
    pub bg_color: ColorLinPremul,
    pub border_color: ColorLinPremul,
    pub border_width: f32,
}

/// Zone layout configuration
#[derive(Debug, Clone)]
pub struct ZoneLayout {
    pub viewport: Rect,
    pub sidebar: Rect,
    pub toolbar: Rect,
    pub devtools: Rect,
    pub sidebar_visible: bool,
}

impl ZoneLayout {
    /// Calculate zone layout based on window dimensions
    pub fn calculate(window_width: u32, window_height: u32, sidebar_visible: bool) -> Self {
        let width = window_width as f32;
        let height = window_height as f32;

        // Layout constants
        const SIDEBAR_WIDTH: f32 = 280.0;
        const TOOLBAR_HEIGHT: f32 = 48.0;
        const DEVTOOLS_HEIGHT: f32 = 200.0;

        // Sidebar extends full height on the left
        let sidebar = Rect {
            x: 0.0,
            y: 0.0,
            w: if sidebar_visible { SIDEBAR_WIDTH } else { 0.0 },
            h: height,
        };

        let sidebar_offset = if sidebar_visible { SIDEBAR_WIDTH } else { 0.0 };

        // Toolbar only spans the viewport area (to the right of sidebar)
        let toolbar = Rect {
            x: sidebar_offset,
            y: 0.0,
            w: (width - sidebar_offset).max(0.0),
            h: TOOLBAR_HEIGHT,
        };

        // Viewport fills the full space (right of sidebar, below toolbar)
        // DevTools will overlay on top at high z-index
        let viewport = Rect {
            x: sidebar_offset,
            y: TOOLBAR_HEIGHT,
            w: (width - sidebar_offset).max(0.0),
            h: (height - TOOLBAR_HEIGHT).max(0.0),
        };

        // DevTools overlays at the bottom (rendered at high z-index)
        let devtools = Rect {
            x: sidebar_offset,
            y: (height - DEVTOOLS_HEIGHT).max(TOOLBAR_HEIGHT),
            w: (width - sidebar_offset).max(0.0),
            h: DEVTOOLS_HEIGHT.min(height - TOOLBAR_HEIGHT),
        };

        Self {
            viewport,
            sidebar,
            toolbar,
            devtools,
            sidebar_visible,
        }
    }

    /// Get zone rectangle by ID
    pub fn get_zone(&self, zone_id: ZoneId) -> Rect {
        match zone_id {
            ZoneId::Viewport => self.viewport,
            ZoneId::Sidebar => self.sidebar,
            ZoneId::Toolbar => self.toolbar,
            ZoneId::DevTools => self.devtools,
        }
    }
}
