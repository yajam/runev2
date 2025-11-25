use super::common::ZoneStyle;
use engine_core::{ColorLinPremul, Rect};

/// Console log level for DevTools console.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleLevel {
    Log,
    Warn,
    Error,
}

/// Single console entry shown in the DevTools console tab.
#[derive(Debug, Clone)]
pub struct ConsoleEntry {
    pub level: ConsoleLevel,
    pub message: String,
}

/// DevTools tab types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevToolsTab {
    Elements,
    Console,
}

/// DevTools configuration and state
pub struct DevTools {
    pub style: ZoneStyle,
    pub visible: bool,
    pub active_tab: DevToolsTab,
    pub console_entries: Vec<ConsoleEntry>,
    pub max_console_entries: usize,
}

impl DevTools {
    pub fn new() -> Self {
        Self {
            style: Self::default_style(),
            visible: false,
            active_tab: DevToolsTab::Elements,
            console_entries: Vec::new(),
            max_console_entries: 256,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_active_tab(&mut self, tab: DevToolsTab) {
        self.active_tab = tab;
    }

    pub fn get_active_tab(&self) -> DevToolsTab {
        self.active_tab
    }

    pub fn log_console(&mut self, level: ConsoleLevel, message: String) {
        self.console_entries.push(ConsoleEntry { level, message });
        if self.console_entries.len() > self.max_console_entries {
            let overflow = self.console_entries.len() - self.max_console_entries;
            self.console_entries.drain(0..overflow);
        }
    }

    /// Clear all console entries.
    pub fn clear_console(&mut self) {
        self.console_entries.clear();
    }

    pub fn default_style() -> ZoneStyle {
        ZoneStyle {
            bg_color: ColorLinPremul::from_srgba_u8([40, 50, 70, 255]), // Much brighter to be clearly visible
            border_color: ColorLinPremul::from_srgba_u8([100, 120, 150, 255]), // Very bright border
            border_width: 1.0,
        }
    }

    /// Render devtools content
    pub fn render(&self, canvas: &mut rune_surface::Canvas, devtools_rect: Rect) {
        // Fill entire devtools area with a semi-transparent overlay to make it obvious
        let overlay_color = engine_core::Color::rgba(60, 80, 120, 200);
        canvas.fill_rect(
            0.0,
            0.0,
            devtools_rect.w,
            devtools_rect.h,
            engine_core::Brush::Solid(overlay_color),
            10150,
        );

        // Add a prominent label
        let label_color = engine_core::Color::rgba(220, 230, 240, 255);
        canvas.draw_text_run(
            [20.0, 20.0],
            "DevTools Panel (Bottom Overlay)".to_string(),
            18.0,
            label_color,
            10200, // z-index above devtools background
        );
    }
}

impl Default for DevTools {
    fn default() -> Self {
        Self::new()
    }
}

/// Close button region ID for hit testing
/// Use IDs distinct from toolbar controls to avoid collisions.
pub const DEVTOOLS_CLOSE_BUTTON_REGION_ID: u32 = 1100;

/// Elements tab region ID for hit testing
pub const DEVTOOLS_ELEMENTS_TAB_REGION_ID: u32 = 1101;

/// Console tab region ID for hit testing
pub const DEVTOOLS_CONSOLE_TAB_REGION_ID: u32 = 1102;
