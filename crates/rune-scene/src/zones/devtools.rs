use engine_core::{ColorLinPremul, Rect};
use super::common::ZoneStyle;

/// DevTools configuration and state
pub struct DevTools {
    pub style: ZoneStyle,
}

impl DevTools {
    pub fn new() -> Self {
        Self {
            style: Self::default_style(),
        }
    }

    pub fn default_style() -> ZoneStyle {
        ZoneStyle {
            bg_color: ColorLinPremul::from_srgba_u8([40, 50, 70, 255]), // Much brighter to be clearly visible
            border_color: ColorLinPremul::from_srgba_u8([100, 120, 150, 255]), // Very bright border
            border_width: 3.0, // Even thicker border
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
