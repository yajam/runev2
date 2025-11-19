use engine_core::{Brush, ColorLinPremul, Rect, RoundedRadii, RoundedRect};
use rune_surface::Canvas;
use rune_surface::shapes;

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
}

impl ConfirmDialog {
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

        // Shadow behind the panel.
        let shadow_offset = 8.0;
        canvas.fill_rect(
            panel.x,
            panel.y + shadow_offset,
            panel.w,
            panel.h,
            Brush::Solid(ColorLinPremul::from_srgba_u8([0, 0, 0, 40])),
            z - 10,
        );

        let panel_rrect = RoundedRect {
            rect: panel,
            radii: RoundedRadii {
                tl: self.panel_radius,
                tr: self.panel_radius,
                br: self.panel_radius,
                bl: self.panel_radius,
            },
        };

        // Panel background and border.
        canvas.rounded_rect(panel_rrect, Brush::Solid(self.panel_bg), z);
        shapes::draw_rounded_rectangle(
            canvas,
            panel_rrect,
            None,
            Some(1.0),
            Some(Brush::Solid(self.panel_border_color)),
            z + 1,
        );

        // Title.
        let content_left = panel.x + 24.0;
        let content_top = panel.y + 28.0;
        let title_x = content_left;
        let title_y = content_top + self.title_size;
        canvas.draw_text_run(
            [title_x, title_y],
            self.title.clone(),
            self.title_size,
            self.title_color,
            z + 2,
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
                z + 2,
            );
        }

        // Secondary button (if any).
        if let (Some(label), Some(rect)) = (&self.secondary_label, self.secondary_button_rect()) {
            self.render_button(canvas, &label, rect, self.secondary_bg, self.secondary_fg, z + 3);
        }

        // Primary button.
        let primary_rect = self.primary_button_rect();
        self.render_button(
            canvas,
            &self.primary_label,
            primary_rect,
            self.primary_bg,
            self.primary_fg,
            z + 4,
        );
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
