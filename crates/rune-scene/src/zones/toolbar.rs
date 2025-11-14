use engine_core::{ColorLinPremul, Rect};
use super::common::ZoneStyle;

/// Toolbar configuration and state
pub struct Toolbar {
    pub style: ZoneStyle,
}

impl Toolbar {
    pub fn new() -> Self {
        Self {
            style: Self::default_style(),
        }
    }

    pub fn default_style() -> ZoneStyle {
        ZoneStyle {
            bg_color: ColorLinPremul::from_srgba_u8([18, 23, 43, 255]),
            border_color: ColorLinPremul::from_srgba_u8([60, 65, 85, 255]),
            border_width: 1.0,
        }
    }

    /// Render toolbar content (toggle button hit region and icon)
    pub fn render(&self, canvas: &mut rune_surface::Canvas, toolbar_rect: Rect) {
        // Add toggle button as a hit region in toolbar local coordinates
        const TOGGLE_BUTTON_REGION_ID: u32 = 1000;
        let toggle_size = 24.0;
        let toggle_margin = 12.0;
        let toggle_x = toggle_margin;
        let toggle_y = (toolbar_rect.h - toggle_size) / 2.0;
        
        let toggle_rect = Rect {
            x: toggle_x,
            y: toggle_y,
            w: toggle_size,
            h: toggle_size,
        };
        
        canvas.hit_region_rect(TOGGLE_BUTTON_REGION_ID, toggle_rect, 10100);
        
        // Draw the toggle button icon with white stroke and custom width
        let white = engine_core::Color::rgba(255, 255, 255, 255);
        let icon_style = engine_core::SvgStyle::new()
            .with_stroke(white)
            .with_stroke_width(1.5);
        canvas.draw_svg_styled(
            "images/panel-left.svg",
            [toggle_x, toggle_y],
            [toggle_size, toggle_size],
            icon_style,
            10200, // z-index above toolbar background
        );
    }
}

impl Default for Toolbar {
    fn default() -> Self {
        Self::new()
    }
}

/// Toggle button region ID for hit testing
pub const TOGGLE_BUTTON_REGION_ID: u32 = 1000;
