use super::common::ZoneStyle;
use engine_core::ColorLinPremul;

/// Sidebar configuration and state
pub struct Sidebar {
    pub style: ZoneStyle,
    pub visible: bool,
}

impl Sidebar {
    pub fn new() -> Self {
        Self {
            style: Self::default_style(),
            visible: true,
        }
    }

    pub fn default_style() -> ZoneStyle {
        ZoneStyle {
            bg_color: ColorLinPremul::from_srgba_u8([22, 27, 47, 255]),
            border_color: ColorLinPremul::from_srgba_u8([60, 65, 85, 255]),
            border_width: 1.0,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self) {
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }
}

impl Default for Sidebar {
    fn default() -> Self {
        Self::new()
    }
}
