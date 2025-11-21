use crate::ir_renderer::HitRegionRegistry;
use engine_core::{Brush, ColorLinPremul, Rect, RoundedRadii, RoundedRect};
use rune_ir::view::ViewNodeId;
use rune_surface::Canvas;
use rune_surface::shapes;

/// Result of a confirm dialog click event
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfirmClickResult {
    /// Primary button was clicked (e.g., "OK")
    Primary,
    /// Secondary button was clicked (e.g., "Cancel")
    Secondary,
    /// Background/scrim was clicked (outside panel)
    Background,
    /// Panel was clicked but not a specific button
    Panel,
    /// Click was not handled by confirm dialog
    Ignored,
}

/// Simple center-positioned alert/confirm dialog without a fullscreen overlay.
/// Intended for short messages with one or two actions.
pub struct ConfirmDialog {
    /// Screen dimensions for centering the panel.
    pub screen_width: f32,
    pub screen_height: f32,

    /// Panel size.
    pub panel_width: f32,
    pub panel_height: f32,

    /// Title and message text.
    pub title: String,
    pub message: String,

    /// Button labels.
    pub primary_label: String,
    pub secondary_label: Option<String>,

    /// Visual styling.
    pub panel_bg: ColorLinPremul,
    pub panel_border_color: ColorLinPremul,
    pub title_color: ColorLinPremul,
    pub message_color: ColorLinPremul,
    pub primary_bg: ColorLinPremul,
    pub primary_fg: ColorLinPremul,
    pub secondary_bg: ColorLinPremul,
    pub secondary_fg: ColorLinPremul,

    /// Font sizes.
    pub title_size: f32,
    pub message_size: f32,
    pub button_label_size: f32,

    /// Border radius for panel.
    pub panel_radius: f32,

    /// Base z-index for the dialog content.
    pub base_z: i32,

    /// Optional fullscreen overlay color behind the dialog (scrim).
    /// When fully transparent, the overlay is effectively disabled.
    pub overlay_color: ColorLinPremul,
    /// Whether to draw the overlay scrim behind the panel.
    pub show_overlay: bool,
    /// Whether to draw the panel shadow.
    pub show_shadow: bool,
}

impl ConfirmDialog {
    /// Color used for the confirm dialog scrim overlay.
    pub fn scrim_color(&self) -> ColorLinPremul {
        self.overlay_color
    }

    /// Create a new confirm dialog with default styling and "Cancel" / "OK" buttons.
    pub fn new(
        screen_width: f32,
        screen_height: f32,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            screen_width,
            screen_height,
            panel_width: 420.0,
            panel_height: 180.0,
            title: title.into(),
            message: message.into(),
            primary_label: "OK".to_string(),
            secondary_label: Some("Cancel".to_string()),
            panel_bg: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
            panel_border_color: ColorLinPremul::from_srgba_u8([220, 220, 220, 255]),
            title_color: ColorLinPremul::from_srgba_u8([20, 20, 20, 255]),
            message_color: ColorLinPremul::from_srgba_u8([90, 90, 90, 255]),
            primary_bg: ColorLinPremul::from_srgba_u8([59, 130, 246, 255]),
            primary_fg: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
            secondary_bg: ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
            secondary_fg: ColorLinPremul::from_srgba_u8([60, 60, 60, 255]),
            title_size: 18.0,
            message_size: 14.0,
            button_label_size: 14.0,
            panel_radius: 10.0,
            base_z: 9400,
            overlay_color: ColorLinPremul::from_srgba_u8([0, 0, 0, 150]),
            show_overlay: true,
            show_shadow: true,
        }
    }

    /// Centered panel rect.
    pub fn panel_rect(&self) -> Rect {
        Rect {
            x: (self.screen_width - self.panel_width) * 0.5,
            y: (self.screen_height - self.panel_height) * 0.5,
            w: self.panel_width,
            h: self.panel_height,
        }
    }

    /// Button layout: primary on the right, optional secondary to its left.
    pub fn primary_button_rect(&self) -> Rect {
        let panel = self.panel_rect();
        let button_height = 36.0;
        let button_width = 96.0;
        let margin = 20.0;

        Rect {
            x: panel.x + panel.w - margin - button_width,
            y: panel.y + panel.h - margin - button_height,
            w: button_width,
            h: button_height,
        }
    }

    pub fn secondary_button_rect(&self) -> Option<Rect> {
        if self.secondary_label.is_none() {
            return None;
        }
        let panel = self.panel_rect();
        let primary = self.primary_button_rect();
        let button_height = primary.h;
        let button_width = primary.w;
        let spacing = 12.0;

        Some(Rect {
            x: primary.x - spacing - button_width,
            y: panel.y + panel.h - 20.0 - button_height,
            w: button_width,
            h: button_height,
        })
    }

    pub fn render(&self, canvas: &mut Canvas) {
        let z = self.base_z;
        let panel = self.panel_rect();

        // Shadow disabled for now - investigating corner artifacts
        let _ = self.show_shadow;

        let panel_rrect = RoundedRect {
            rect: panel,
            radii: RoundedRadii {
                tl: self.panel_radius,
                tr: self.panel_radius,
                br: self.panel_radius,
                bl: self.panel_radius,
            },
        };

        // Panel background and border (combined for consistent rounding)
        shapes::draw_rounded_rectangle(
            canvas,
            panel_rrect,
            Some(Brush::Solid(self.panel_bg)),
            Some(1.0),
            Some(Brush::Solid(self.panel_border_color)),
            z,
        );

        // Title (render above panel background).
        let content_left = panel.x + 24.0;
        let content_top = panel.y + 28.0;
        let title_x = content_left;
        let title_y = content_top + self.title_size;
        canvas.draw_text_run(
            [title_x, title_y],
            self.title.clone(),
            self.title_size,
            self.title_color,
            z + 10,
        );

        // Message with simple word-wrapping to keep text within the panel.
        let msg_x = content_left;
        let msg_y = title_y + self.title_size * 0.9;
        let max_text_width = panel.w - 48.0; // 24px padding on both sides
        let wrapped_lines = wrap_text_simple(&self.message, max_text_width, self.message_size);
        for (i, line) in wrapped_lines.iter().enumerate() {
            canvas.draw_text_run(
                [msg_x, msg_y + i as f32 * self.message_size * 1.6],
                line.clone(),
                self.message_size,
                self.message_color,
                z + 10,
            );
        }

        // Secondary button (if any).
        if let (Some(label), Some(rect)) = (&self.secondary_label, self.secondary_button_rect()) {
            self.render_button(
                canvas,
                &label,
                rect,
                self.secondary_bg,
                self.secondary_fg,
                z + 20,
            );
        }

        // Primary button.
        let primary_rect = self.primary_button_rect();
        self.render_button(
            canvas,
            &self.primary_label,
            primary_rect,
            self.primary_bg,
            self.primary_fg,
            z + 25,
        );
    }

    /// Compute scrim bands that darken everything except the panel.
    ///
    /// Coordinates are in viewport-local space (0,0 is top-left of viewport).
    pub fn scrim_bands(&self) -> Vec<Rect> {
        let panel = self.panel_rect();

        let mut bands = Vec::with_capacity(16);
        let _r = self.panel_radius;
        let _slices = 8;

        // Main bands with exact rectangular hole (no overlap with panel)
        // Top band
        if panel.y > 0.0 {
            bands.push(Rect {
                x: 0.0,
                y: 0.0,
                w: self.screen_width,
                h: panel.y,
            });
        }

        // Bottom band
        let panel_bottom = panel.y + panel.h;
        if panel_bottom < self.screen_height {
            bands.push(Rect {
                x: 0.0,
                y: panel_bottom,
                w: self.screen_width,
                h: self.screen_height - panel_bottom,
            });
        }

        // Left band
        if panel.x > 0.0 {
            bands.push(Rect {
                x: 0.0,
                y: panel.y,
                w: panel.x,
                h: panel.h,
            });
        }

        // Right band
        let panel_right = panel.x + panel.w;
        if panel_right < self.screen_width {
            bands.push(Rect {
                x: panel_right,
                y: panel.y,
                w: self.screen_width - panel_right,
                h: panel.h,
            });
        }

        bands
    }

    /// Render the complete confirm dialog overlay with scrim bands and hit regions.
    ///
    /// This is the primary method for rendering confirm dialogs in the IR renderer.
    /// It handles:
    /// - Scrim bands (darkened areas around the dialog) with optional hit regions
    /// - Panel hit region
    /// - Dialog rendering (panel, title, message, buttons)
    /// - Hit regions for primary/secondary buttons and close button
    pub fn render_overlay(
        &self,
        canvas: &mut Canvas,
        hit_registry: &mut HitRegionRegistry,
        overlay_id: &ViewNodeId,
        dismissible: bool,
        show_close: bool,
    ) {
        let panel_rect = self.panel_rect();
        let overlay_z = self.base_z - 10; // Scrim below dialog

        // 1. Render scrim bands using the depth-bypass scrim pipeline so background
        // content stays visible and the panel remains undimmed.
        let scrim_color = self.scrim_color();
        let panel_rrect = RoundedRect {
            rect: panel_rect,
            radii: RoundedRadii {
                tl: self.panel_radius,
                tr: self.panel_radius,
                br: self.panel_radius,
                bl: self.panel_radius,
            },
        };
        canvas.fill_scrim_with_cutout(panel_rrect, scrim_color);

        // Register scrim hit region for dismissal (outside panel area)
        // Use bands for hit testing to exclude the panel area
        if dismissible {
            let scrim_region_id = hit_registry.register(&format!("__scrim__{}", overlay_id));
            let full = Rect {
                x: 0.0,
                y: 0.0,
                w: self.screen_width,
                h: self.screen_height,
            };
            canvas.hit_region_rect(scrim_region_id, full, overlay_z + 1);
        }

        // 2. Register panel hit region
        let panel_region_id = hit_registry.register(overlay_id);
        canvas.hit_region_rect(panel_region_id, panel_rect, self.base_z + 50);

        // 3. Render dialog (panel, title, message, buttons)
        self.render(canvas);

        // 4. Register hit regions for buttons
        if let Some(cancel_rect) = self.secondary_button_rect() {
            let cancel_region = hit_registry.register(&format!("__cancel__{}", overlay_id));
            canvas.hit_region_rect(cancel_region, cancel_rect, self.base_z + 60);
        }

        let ok_rect = self.primary_button_rect();
        let ok_region = hit_registry.register(&format!("__ok__{}", overlay_id));
        canvas.hit_region_rect(ok_region, ok_rect, self.base_z + 70);

        // 5. Register hit region for close button if shown
        if show_close {
            self.render_close_button(canvas, hit_registry, overlay_id, panel_rect);
        }
    }

    /// Render a close button in the top-right corner of the panel.
    fn render_close_button(
        &self,
        canvas: &mut Canvas,
        hit_registry: &mut HitRegionRegistry,
        overlay_id: &ViewNodeId,
        panel_rect: Rect,
    ) {
        let close_size = 24.0;
        let close_rect = Rect {
            x: panel_rect.x + panel_rect.w - close_size - 16.0,
            y: panel_rect.y + 16.0,
            w: close_size,
            h: close_size,
        };

        let close_bg = ColorLinPremul::from_srgba_u8([240, 240, 240, 255]);
        canvas.fill_rect(
            close_rect.x,
            close_rect.y,
            close_rect.w,
            close_rect.h,
            Brush::Solid(close_bg),
            self.base_z + 30,
        );

        let x_color = ColorLinPremul::from_srgba_u8([100, 100, 100, 255]);
        canvas.draw_text_run(
            [close_rect.x + 6.0, close_rect.y + close_rect.h - 6.0],
            "✕".to_string(),
            14.0,
            x_color,
            self.base_z + 31,
        );

        let close_region_id = hit_registry.register(&format!("__close__{}", overlay_id));
        canvas.hit_region_rect(close_region_id, close_rect, self.base_z + 40);
    }

    fn render_button(
        &self,
        canvas: &mut Canvas,
        label: &str,
        rect: Rect,
        bg: ColorLinPremul,
        fg: ColorLinPremul,
        z: i32,
    ) {
        let rrect = RoundedRect {
            rect,
            radii: RoundedRadii {
                tl: 6.0,
                tr: 6.0,
                br: 6.0,
                bl: 6.0,
            },
        };
        canvas.rounded_rect(rrect, Brush::Solid(bg), z);

        shapes::draw_rounded_rectangle(
            canvas,
            rrect,
            None,
            Some(1.0),
            Some(Brush::Solid(self.panel_border_color)),
            z + 1,
        );

        let approx_text_width = label.len() as f32 * self.button_label_size * 0.5;
        let text_x = rect.x + (rect.w - approx_text_width) * 0.5;
        let text_y = rect.y + rect.h * 0.5 + self.button_label_size * 0.35;
        canvas.draw_text_run(
            [text_x, text_y],
            label.to_string(),
            self.button_label_size,
            fg,
            z + 2,
        );
    }

    // =========================================================================
    // EVENT HANDLING METHODS
    // =========================================================================

    /// Hit test the primary button
    pub fn hit_test_primary_button(&self, x: f32, y: f32) -> bool {
        let rect = self.primary_button_rect();
        x >= rect.x && x <= rect.x + rect.w && y >= rect.y && y <= rect.y + rect.h
    }

    /// Hit test the secondary button (if it exists)
    pub fn hit_test_secondary_button(&self, x: f32, y: f32) -> bool {
        if let Some(rect) = self.secondary_button_rect() {
            x >= rect.x && x <= rect.x + rect.w && y >= rect.y && y <= rect.y + rect.h
        } else {
            false
        }
    }

    /// Hit test the confirm dialog panel
    pub fn hit_test_panel(&self, x: f32, y: f32) -> bool {
        let panel_rect = self.panel_rect();
        x >= panel_rect.x
            && x <= panel_rect.x + panel_rect.w
            && y >= panel_rect.y
            && y <= panel_rect.y + panel_rect.h
    }

    /// Handle click event on the confirm dialog
    /// Returns ConfirmClickResult indicating what was clicked
    pub fn handle_click(&self, x: f32, y: f32) -> ConfirmClickResult {
        // Check if click is on panel first
        if self.hit_test_panel(x, y) {
            // Check primary button
            if self.hit_test_primary_button(x, y) {
                return ConfirmClickResult::Primary;
            }

            // Check secondary button
            if self.hit_test_secondary_button(x, y) {
                return ConfirmClickResult::Secondary;
            }

            // Click was on panel but not on any button
            return ConfirmClickResult::Panel;
        } else {
            // Click was outside panel (background/scrim)
            return ConfirmClickResult::Background;
        }
    }
}

/// Very simple word-wrapper based on an approximate character width. This is
/// sufficient for short confirm-dialog messages without pulling in full text
/// layout machinery. It keeps each line within `max_width` (in pixels).
fn wrap_text_simple(text: &str, max_width: f32, size_px: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    let approx_char_width = size_px * 0.55f32;

    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{} {}", current, word)
        };
        let width = candidate.len() as f32 * approx_char_width;
        if width > max_width && !current.is_empty() {
            lines.push(current);
            current = word.to_string();
        } else {
            current = candidate;
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}
