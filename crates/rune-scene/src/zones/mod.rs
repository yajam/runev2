pub mod chat;
pub mod common;
pub mod devtools;
pub mod dock;
pub mod render;
pub mod sidebar;
pub mod toolbar;
pub mod viewport;

// Re-export commonly used types
pub use common::{ZoneId, ZoneLayout, ZoneStyle};
pub use chat::{
    ChatPanel, ChatPanelState, render_chat_fab, CHAT_CLOSE_BUTTON_REGION_ID,
    CHAT_FAB_REGION_ID, CHAT_INPUT_REGION_ID, CHAT_MESSAGE_REGION_BASE, CHAT_SEND_BUTTON_REGION_ID,
};
pub use devtools::{
    ConsoleLevel, DEVTOOLS_CLOSE_BUTTON_REGION_ID, DEVTOOLS_CONSOLE_TAB_REGION_ID,
    DEVTOOLS_ELEMENTS_TAB_REGION_ID, DevTools, DevToolsTab,
};
pub use dock::{
    Dock, DockApp, DockState, DOCK_PANEL_REGION_ID, DOCK_PINNED_APP_REGION_BASE,
    DOCK_RECENT_ITEM_REGION_BASE, DOCK_SCRIM_REGION_ID,
};
pub use sidebar::{
    Sidebar, SidebarHit, SIDEBAR_ADD_BOOKMARK_REGION_ID, SIDEBAR_BOOKMARK_DELETE_REGION_BASE,
    SIDEBAR_BOOKMARK_REGION_BASE, SIDEBAR_TAB_CLOSE_REGION_BASE, SIDEBAR_TAB_REGION_BASE,
};
pub use toolbar::{
    ADDRESS_BAR_REGION_ID, BACK_BUTTON_REGION_ID, CHAT_BUTTON_REGION_ID, DEVTOOLS_BUTTON_REGION_ID,
    FORWARD_BUTTON_REGION_ID, HOME_BUTTON_REGION_ID, REFRESH_BUTTON_REGION_ID,
    TOGGLE_BUTTON_REGION_ID, Toolbar,
};
pub use render::render_zones;
pub use viewport::Viewport;

/// Zone manager for rendering and interaction
pub struct ZoneManager {
    pub layout: ZoneLayout,
    pub viewport: Viewport,
    pub sidebar: Sidebar,
    pub toolbar: Toolbar,
    pub devtools: DevTools,
    pub dock: Dock,
    pub chat: ChatPanel,
}

impl ZoneManager {
    pub fn new(window_width: u32, window_height: u32) -> Self {
        let sidebar = Sidebar::new();
        let chat = ChatPanel::new();
        let toolbar_visible = crate::navigation::should_show_toolbar();
        Self {
            layout: ZoneLayout::calculate_with_chat(
                window_width,
                window_height,
                sidebar.visible,
                toolbar_visible,
                chat.is_visible(),
            ),
            viewport: Viewport::new(),
            sidebar,
            toolbar: Toolbar::new(),
            devtools: DevTools::new(),
            dock: Dock::new(),
            chat,
        }
    }

    pub fn resize(&mut self, window_width: u32, window_height: u32) {
        let toolbar_visible = crate::navigation::should_show_toolbar();
        self.layout = ZoneLayout::calculate_with_chat(
            window_width,
            window_height,
            self.sidebar.visible,
            toolbar_visible,
            self.chat.is_visible(),
        );
    }

    /// Recalculate layout based on current navigation mode.
    /// Call this when navigation mode changes to update toolbar visibility.
    pub fn update_for_navigation_mode(&mut self, window_width: u32, window_height: u32) {
        let toolbar_visible = crate::navigation::should_show_toolbar();

        // When entering Home mode the toolbar is hidden, which removes the
        // sidebar toggle button. To avoid leaving the sidebar stuck open with
        // no way to close it, force the sidebar closed whenever the toolbar
        // is not visible (Home navigation mode).
        if !toolbar_visible {
            self.sidebar.visible = false;
        }

        self.layout = ZoneLayout::calculate_with_chat(
            window_width,
            window_height,
            self.sidebar.visible,
            toolbar_visible,
            self.chat.is_visible(),
        );
    }

    pub fn toggle_sidebar(&mut self, window_width: u32, window_height: u32) {
        self.sidebar.toggle();
        let toolbar_visible = crate::navigation::should_show_toolbar();
        self.layout = ZoneLayout::calculate_with_chat(
            window_width,
            window_height,
            self.sidebar.visible,
            toolbar_visible,
            self.chat.is_visible(),
        );
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

    /// Toggle the dock overlay
    pub fn toggle_dock(&mut self) {
        self.dock.toggle();
    }

    /// Show the dock overlay
    pub fn show_dock(&mut self) {
        self.dock.show();
    }

    /// Hide the dock overlay
    pub fn hide_dock(&mut self) {
        self.dock.hide();
    }

    /// Check if dock is visible
    pub fn is_dock_visible(&self) -> bool {
        self.dock.is_visible()
    }

    /// Toggle the chat panel
    pub fn toggle_chat(&mut self, window_width: u32, window_height: u32) {
        self.chat.toggle();
        let toolbar_visible = crate::navigation::should_show_toolbar();
        self.layout = ZoneLayout::calculate_with_chat(
            window_width,
            window_height,
            self.sidebar.visible,
            toolbar_visible,
            self.chat.is_visible(),
        );
    }

    /// Show the chat panel
    pub fn show_chat(&mut self, window_width: u32, window_height: u32) {
        self.chat.show();
        let toolbar_visible = crate::navigation::should_show_toolbar();
        self.layout = ZoneLayout::calculate_with_chat(
            window_width,
            window_height,
            self.sidebar.visible,
            toolbar_visible,
            self.chat.is_visible(),
        );
    }

    /// Hide the chat panel
    pub fn hide_chat(&mut self, window_width: u32, window_height: u32) {
        self.chat.hide();
        let toolbar_visible = crate::navigation::should_show_toolbar();
        self.layout = ZoneLayout::calculate_with_chat(
            window_width,
            window_height,
            self.sidebar.visible,
            toolbar_visible,
            self.chat.is_visible(),
        );
    }

    /// Check if chat panel is visible
    pub fn is_chat_visible(&self) -> bool {
        self.chat.is_visible()
    }

    pub fn get_style(&self, zone_id: ZoneId) -> &ZoneStyle {
        match zone_id {
            ZoneId::Viewport => &self.viewport.style,
            ZoneId::Sidebar => &self.sidebar.style,
            ZoneId::Toolbar => &self.toolbar.style,
            ZoneId::DevTools => &self.devtools.style,
            ZoneId::Chat => &self.chat.style,
        }
    }
}
