use engine_core::ColorLinPremul;
use super::common::ZoneStyle;

/// Viewport configuration and state
pub struct Viewport {
    pub style: ZoneStyle,
}

impl Viewport {
    pub fn new() -> Self {
        Self {
            style: Self::default_style(),
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
