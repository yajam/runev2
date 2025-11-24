use engine_core::{ColorLinPremul, Rect};

/// Zone identifiers for hit testing and layout
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneId {
    Viewport = 1000,
    Sidebar = 2000,
    Toolbar = 3000,
    DevTools = 4000,
    Chat = 6000,
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
    pub chat: Rect,
    pub sidebar_visible: bool,
    pub chat_visible: bool,
}

impl ZoneLayout {
    /// Calculate zone layout based on window dimensions
    pub fn calculate(window_width: u32, window_height: u32, sidebar_visible: bool) -> Self {
        Self::calculate_with_chat(window_width, window_height, sidebar_visible, true, false)
    }

    /// Calculate zone layout with explicit toolbar visibility control.
    /// This is used for mode-aware layouts where toolbar may be hidden in Home/IRApp modes.
    pub fn calculate_with_toolbar(
        window_width: u32,
        window_height: u32,
        sidebar_visible: bool,
        toolbar_visible: bool,
    ) -> Self {
        Self::calculate_with_chat(window_width, window_height, sidebar_visible, toolbar_visible, false)
    }

    /// Calculate zone layout with toolbar and chat panel visibility control.
    pub fn calculate_with_chat(
        window_width: u32,
        window_height: u32,
        sidebar_visible: bool,
        toolbar_visible: bool,
        chat_visible: bool,
    ) -> Self {
        let width = window_width as f32;
        let height = window_height as f32;

        // Layout constants
        const SIDEBAR_WIDTH: f32 = 280.0;
        const TOOLBAR_HEIGHT: f32 = 48.0;
        const DEVTOOLS_HEIGHT: f32 = 200.0;
        const CHAT_WIDTH: f32 = 320.0;

        let toolbar_height = if toolbar_visible { TOOLBAR_HEIGHT } else { 0.0 };
        let chat_width = if chat_visible { CHAT_WIDTH } else { 0.0 };

        // Sidebar extends full height on the left
        let sidebar = Rect {
            x: 0.0,
            y: 0.0,
            w: if sidebar_visible { SIDEBAR_WIDTH } else { 0.0 },
            h: height,
        };

        let sidebar_offset = if sidebar_visible { SIDEBAR_WIDTH } else { 0.0 };

        // Mode-aware layout:
        // - Home mode: chat acts like a centered modal overlay that does NOT
        //   steal width from the underlying viewport; hero content stays full-width.
        // - IRApp/Browser: chat is a right sidebar that shrinks the viewport.
        let nav_mode = crate::navigation::get_navigation_mode();
        let in_home_mode = matches!(nav_mode, crate::navigation::NavigationMode::Home);

        let (chat, toolbar, viewport, devtools) = if chat_visible && in_home_mode {
            // Underlying viewport uses full width (no chat subtraction)
            let viewport = Rect {
                x: sidebar_offset,
                y: toolbar_height,
                w: (width - sidebar_offset).max(0.0),
                h: (height - toolbar_height).max(0.0),
            };

            // Centered chat "modal" at 70% of viewport width
            let available_width = (width - sidebar_offset).max(0.0);
            let chat_w = available_width * 0.7;
            let chat_x = sidebar_offset + (available_width - chat_w) * 0.5;

            let chat = Rect {
                x: chat_x,
                y: toolbar_height,
                w: chat_w,
                h: (height - toolbar_height).max(0.0),
            };

            let toolbar = Rect {
                x: sidebar_offset,
                y: 0.0,
                w: (width - sidebar_offset).max(0.0),
                h: toolbar_height,
            };

            let devtools = Rect {
                x: sidebar_offset,
                y: (height - DEVTOOLS_HEIGHT).max(toolbar_height),
                w: (width - sidebar_offset).max(0.0),
                h: DEVTOOLS_HEIGHT.min(height - toolbar_height),
            };

            (chat, toolbar, viewport, devtools)
        } else {
            // Default: chat as right sidebar and viewport/devtools adjusted.
            let chat = Rect {
                x: width - chat_width,
                y: toolbar_height,
                w: chat_width,
                h: (height - toolbar_height).max(0.0),
            };

            let toolbar = Rect {
                x: sidebar_offset,
                y: 0.0,
                w: (width - sidebar_offset).max(0.0),
                h: toolbar_height,
            };

            let viewport = Rect {
                x: sidebar_offset,
                y: toolbar_height,
                w: (width - sidebar_offset - chat_width).max(0.0),
                h: (height - toolbar_height).max(0.0),
            };

            let devtools = Rect {
                x: sidebar_offset,
                y: (height - DEVTOOLS_HEIGHT).max(toolbar_height),
                w: (width - sidebar_offset - chat_width).max(0.0),
                h: DEVTOOLS_HEIGHT.min(height - toolbar_height),
            };

            (chat, toolbar, viewport, devtools)
        };

        Self {
            viewport,
            sidebar,
            toolbar,
            devtools,
            chat,
            sidebar_visible,
            chat_visible,
        }
    }

    /// Get zone rectangle by ID
    pub fn get_zone(&self, zone_id: ZoneId) -> Rect {
        match zone_id {
            ZoneId::Viewport => self.viewport,
            ZoneId::Sidebar => self.sidebar,
            ZoneId::Toolbar => self.toolbar,
            ZoneId::DevTools => self.devtools,
            ZoneId::Chat => self.chat,
        }
    }
}
