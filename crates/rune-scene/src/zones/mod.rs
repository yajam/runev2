pub mod common;
pub mod devtools;
pub mod sidebar;
pub mod toolbar;
pub mod viewport;

// Re-export commonly used types
pub use common::{ZoneId, ZoneLayout, ZoneStyle};
pub use devtools::{
    DEVTOOLS_CLOSE_BUTTON_REGION_ID, DEVTOOLS_CONSOLE_TAB_REGION_ID,
    DEVTOOLS_ELEMENTS_TAB_REGION_ID, DevTools, DevToolsTab,
};
pub use sidebar::Sidebar;
pub use toolbar::{DEVTOOLS_BUTTON_REGION_ID, TOGGLE_BUTTON_REGION_ID, Toolbar};
pub use viewport::Viewport;

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

    pub fn toggle_devtools(&mut self) {
        self.devtools.toggle();
    }

    pub fn is_devtools_visible(&self) -> bool {
        self.devtools.is_visible()
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
