use engine_core::ColorLinPremul;
use super::common::ZoneStyle;

/// Viewport configuration and state
pub struct Viewport {
    pub style: ZoneStyle,
    pub scroll_offset: f32,
    pub content_height: f32,
}

impl Viewport {
    pub fn new() -> Self {
        Self {
            style: Self::default_style(),
            scroll_offset: 0.0,
            content_height: 0.0,
        }
    }

    /// Scroll by delta amount (positive = scroll down, negative = scroll up)
    pub fn scroll(&mut self, delta: f32, viewport_height: f32) {
        self.scroll_offset += delta;
        // Clamp scroll offset to valid range
        let max_scroll = (self.content_height - viewport_height).max(0.0);
        self.scroll_offset = self.scroll_offset.clamp(0.0, max_scroll);
    }

    /// Set content height and adjust scroll if needed
    pub fn set_content_height(&mut self, height: f32, viewport_height: f32) {
        self.content_height = height;
        // Clamp scroll offset if content height changed
        let max_scroll = (self.content_height - viewport_height).max(0.0);
        self.scroll_offset = self.scroll_offset.clamp(0.0, max_scroll);
    }

    pub fn default_style() -> ZoneStyle {
        ZoneStyle {
            bg_color: ColorLinPremul::from_srgba_u8([30, 35, 55, 255]),
            border_color: ColorLinPremul::from_srgba_u8([60, 65, 85, 255]),
            border_width: 1.0,
        }
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new()
    }
}
