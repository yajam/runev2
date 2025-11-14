pub mod common;
pub mod sidebar;
pub mod toolbar;
pub mod viewport;
pub mod devtools;

// Re-export commonly used types
pub use common::{ZoneId, ZoneStyle, ZoneLayout};
pub use sidebar::Sidebar;
pub use toolbar::{Toolbar, TOGGLE_BUTTON_REGION_ID};
pub use viewport::Viewport;
pub use devtools::DevTools;

/// Zone manager for rendering and interaction
pub struct ZoneManager {
    pub layout: ZoneLayout,
    pub viewport: Viewport,
    pub sidebar: Sidebar,
    pub toolbar: Toolbar,
    pub devtools: DevTools,
}

impl ZoneManager {
    pub fn new(window_width: u32, window_height: u32) -> Self {
        let sidebar = Sidebar::new();
        Self {
            layout: ZoneLayout::calculate(window_width, window_height, sidebar.visible),
            viewport: Viewport::new(),
            sidebar,
            toolbar: Toolbar::new(),
            devtools: DevTools::new(),
        }
    }

    pub fn resize(&mut self, window_width: u32, window_height: u32) {
        self.layout = ZoneLayout::calculate(window_width, window_height, self.sidebar.visible);
    }

    pub fn toggle_sidebar(&mut self, window_width: u32, window_height: u32) {
        self.sidebar.toggle();
        self.layout = ZoneLayout::calculate(window_width, window_height, self.sidebar.visible);
    }

    pub fn is_sidebar_visible(&self) -> bool {
        self.sidebar.is_visible()
    }

    pub fn get_style(&self, zone_id: ZoneId) -> &ZoneStyle {
        match zone_id {
            ZoneId::Viewport => &self.viewport.style,
            ZoneId::Sidebar => &self.sidebar.style,
            ZoneId::Toolbar => &self.toolbar.style,
            ZoneId::DevTools => &self.devtools.style,
        }
    }
}
