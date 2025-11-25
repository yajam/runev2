//! Zone rendering functions.
//!
//! These functions render the zone backgrounds and borders (toolbar, sidebar, viewport).

use super::{ZoneId, ZoneManager, HOME_BUTTON_REGION_ID};
use crate::navigation;
use engine_core::{Brush, Color, ColorLinPremul, Rect, RoundedRadii, RoundedRect, SvgStyle};

/// Render zone backgrounds and borders (toolbar, sidebar, viewport).
///
/// This function draws the chrome UI elements that frame the content viewport.
/// The toolbar is only rendered in Browser mode (based on NavigationMode).
pub fn render_zones(
    canvas: &mut rune_surface::Canvas,
    zone_manager: &mut ZoneManager,
    _provider: &dyn engine_core::TextProvider,
) {
    let show_toolbar = navigation::should_show_toolbar();

    for zone_id in [ZoneId::Viewport, ZoneId::Toolbar, ZoneId::Sidebar] {
        // Skip toolbar rendering when not in Browser mode
        if zone_id == ZoneId::Toolbar && !show_toolbar {
            continue;
        }
        let z = match zone_id {
            ZoneId::Toolbar => 9000,
            ZoneId::Sidebar => 8000, // Sidebar should sit above viewport content
            ZoneId::Viewport => 0,
            ZoneId::DevTools => 9500, // not rendered here, but keep ordering explicit
            ZoneId::Chat => 8500,     // Chat panel sits between sidebar and toolbar
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

    // Render persistent Home Button (always visible, even without toolbar)
    // Per user-flow spec section 4: "A persistent control located bottom-left or bottom-center"
    // - Tap: Returns to Home Tab
    // - Double-click: Opens App Dock overlay
    if !show_toolbar {
        render_persistent_home_button(canvas, zone_manager);
    }
}

/// Render the persistent Home Button (visible when toolbar is hidden).
/// This button appears in the bottom-left corner and provides access to Home and Dock.
fn render_persistent_home_button(
    canvas: &mut rune_surface::Canvas,
    zone_manager: &ZoneManager,
) {
    const BUTTON_SIZE: f32 = 48.0;
    const BUTTON_MARGIN: f32 = 16.0;
    const ICON_SIZE: f32 = 24.0;
    const CORNER_RADIUS: f32 = 12.0;

    // Position: bottom-left of the viewport
    let viewport = zone_manager.layout.viewport;
    let x = viewport.x + BUTTON_MARGIN;
    let y = viewport.y + viewport.h - BUTTON_SIZE - BUTTON_MARGIN;

    // Button background (semi-transparent dark)
    let bg_color = ColorLinPremul::from_srgba_u8([40, 45, 60, 220]);
    let border_color = ColorLinPremul::from_srgba_u8([80, 85, 105, 255]);

    let rrect = RoundedRect {
        rect: Rect { x, y, w: BUTTON_SIZE, h: BUTTON_SIZE },
        radii: RoundedRadii { tl: CORNER_RADIUS, tr: CORNER_RADIUS, br: CORNER_RADIUS, bl: CORNER_RADIUS },
    };

    // Background
    canvas.rounded_rect(rrect, Brush::Solid(bg_color), 9800);

    // Border
    canvas.stroke_rounded_rect(rrect, 1.0, Brush::Solid(border_color), 9801);

    // Hit region
    canvas.hit_region_rect(
        HOME_BUTTON_REGION_ID,
        Rect { x, y, w: BUTTON_SIZE, h: BUTTON_SIZE },
        9850,
    );

    // Home icon (centered in button)
    let icon_x = x + (BUTTON_SIZE - ICON_SIZE) * 0.5;
    let icon_y = y + (BUTTON_SIZE - ICON_SIZE) * 0.5;

    let icon_style = SvgStyle::new()
        .with_stroke(Color::rgba(255, 255, 255, 255))
        .with_stroke_width(2.0);

    canvas.draw_svg_styled(
        "images/layout-grid.svg",
        [icon_x, icon_y],
        [ICON_SIZE, ICON_SIZE],
        icon_style,
        9860,
    );
}
