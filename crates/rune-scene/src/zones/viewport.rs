use super::common::ZoneStyle;
use engine_core::ColorLinPremul;

/// Viewport configuration and state
pub struct Viewport {
    pub style: ZoneStyle,
    pub scroll_offset_x: f32,
    pub scroll_offset_y: f32,
    pub content_width: f32,
    pub content_height: f32,
}

impl Viewport {
    pub fn new() -> Self {
        Self {
            style: Self::default_style(),
            scroll_offset_x: 0.0,
            scroll_offset_y: 0.0,
            content_width: 0.0,
            content_height: 0.0,
        }
    }

    /// Scroll by delta amounts (positive = scroll down/right, negative = scroll up/left)
    pub fn scroll(&mut self, delta_x: f32, delta_y: f32, viewport_width: f32, viewport_height: f32) {
        let max_scroll_y = (self.content_height - viewport_height).max(0.0);
        let max_scroll_x = (self.content_width - viewport_width).max(0.0);

        // Only apply horizontal scroll when there's horizontal overflow.
        if max_scroll_x > 0.0 {
            self.scroll_offset_x = (self.scroll_offset_x + delta_x).clamp(0.0, max_scroll_x);
        } else {
            self.scroll_offset_x = 0.0;
        }

        self.scroll_offset_y = (self.scroll_offset_y + delta_y).clamp(0.0, max_scroll_y);
    }

    /// Set content size and adjust scroll if needed
    pub fn set_content_size(&mut self, width: f32, height: f32, viewport_width: f32, viewport_height: f32) {
        self.content_width = width;
        self.content_height = height;
        // Clamp scroll offset if content height changed
        let max_scroll_y = (self.content_height - viewport_height).max(0.0);
        let max_scroll_x = (self.content_width - viewport_width).max(0.0);
        self.scroll_offset_y = self.scroll_offset_y.clamp(0.0, max_scroll_y);
        self.scroll_offset_x = self.scroll_offset_x.clamp(0.0, max_scroll_x);
        if max_scroll_x == 0.0 {
            self.scroll_offset_x = 0.0;
        }
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
