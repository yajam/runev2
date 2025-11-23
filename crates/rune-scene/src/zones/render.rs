//! Zone rendering functions.
//!
//! These functions render the zone backgrounds and borders (toolbar, sidebar, viewport).

use super::{ZoneId, ZoneManager};
use engine_core::Brush;

/// Render zone backgrounds and borders (toolbar, sidebar, viewport).
///
/// This function draws the chrome UI elements that frame the content viewport.
pub fn render_zones(
    canvas: &mut rune_surface::Canvas,
    zone_manager: &mut ZoneManager,
    _provider: &dyn engine_core::TextProvider,
) {
    for zone_id in [ZoneId::Viewport, ZoneId::Toolbar, ZoneId::Sidebar] {
        let z = match zone_id {
            ZoneId::Toolbar => 9000,
            ZoneId::Sidebar => 8000, // Sidebar should sit above viewport content
            ZoneId::Viewport => 0,
            ZoneId::DevTools => 9500, // not rendered here, but keep ordering explicit
        };
        // Draw borders slightly above backgrounds so content can sit
        // between them in z-order (e.g. IR backgrounds inside viewport).
        let border_z = z + 1;

        let rect = zone_manager.layout.get_zone(zone_id);
        let style = zone_manager.get_style(zone_id);

        // Background
        canvas.fill_rect(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            Brush::Solid(style.bg_color),
            z,
        );

        // Border (draw as four rectangles)
        let bw = style.border_width;
        let border_brush = Brush::Solid(style.border_color);

        // Top border
        canvas.fill_rect(rect.x, rect.y, rect.w, bw, border_brush.clone(), border_z);
        // Bottom border
        canvas.fill_rect(
            rect.x,
            rect.y + rect.h - bw,
            rect.w,
            bw,
            border_brush.clone(),
            border_z,
        );
        // Left border
        canvas.fill_rect(rect.x, rect.y, bw, rect.h, border_brush.clone(), border_z);
        // Right border
        canvas.fill_rect(
            rect.x + rect.w - bw,
            rect.y,
            bw,
            rect.h,
            border_brush,
            border_z,
        );
    }
}
