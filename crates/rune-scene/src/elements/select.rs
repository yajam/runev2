use engine_core::{Brush, Color, ColorLinPremul, Rect, RoundedRadii, RoundedRect, SvgStyle};
use rune_surface::Canvas;
use rune_surface::shapes::{self};

pub struct Select {
    pub rect: Rect,
    pub label: String,
    pub label_size: f32,
    pub label_color: ColorLinPremul,
    pub open: bool,
    pub focused: bool,
    pub options: Vec<String>,
    pub selected_index: Option<usize>,
}

impl Select {
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        let radius = 6.0;
        let rrect = RoundedRect {
            rect: self.rect,
            radii: RoundedRadii {
                tl: radius,
                tr: radius,
                br: radius,
                bl: radius,
            },
        };

        // Background
        let bg = Color::rgba(45, 52, 71, 255);
        canvas.rounded_rect(rrect, Brush::Solid(bg), z);

        // Border
        let border_color = if self.focused {
            Color::rgba(63, 130, 246, 255)
        } else {
            Color::rgba(80, 90, 110, 255)
        };
        let border_width = if self.focused { 2.0 } else { 1.0 };
        shapes::draw_rounded_rectangle(
            canvas,
            rrect,
            None,
            Some(border_width),
            Some(Brush::Solid(border_color)),
            z + 1,
        );

        // Label
        let tp = [
            self.rect.x + 12.0,
            self.rect.y + self.rect.h * 0.5 + self.label_size * 0.35,
        ];
        canvas.draw_text_run(
            tp,
            self.label.clone(),
            self.label_size,
            self.label_color,
            z + 2,
        );

        // Chevron icon (SVG)
        let icon_size = 20.0;
        let icon_x = self.rect.x + self.rect.w - icon_size - 10.0;
        let icon_y = self.rect.y + (self.rect.h - icon_size) * 0.5;
        let chevron_path = if self.open {
            "images/chevron-up.svg"
        } else {
            "images/chevron-down.svg"
        };

        // Style the chevron icon with white stroke for maximum visibility
        let icon_style = SvgStyle::new()
            .with_stroke(Color::rgba(255, 255, 255, 255))
            .with_stroke_width(2.5);

        canvas.draw_svg_styled(
            chevron_path,
            [icon_x, icon_y],
            [icon_size, icon_size],
            icon_style,
            z + 3,
        );

        // Render dropdown overlay when open
        if self.open && !self.options.is_empty() {
            self.render_dropdown_overlay(canvas, z + 1000);
        }
    }

    fn render_dropdown_overlay(&self, canvas: &mut Canvas, z: i32) {
        let option_height = 36.0;
        let overlay_padding = 4.0;
        let overlay_height = (self.options.len() as f32 * option_height) + (overlay_padding * 2.0);

        // Position overlay below the select box
        let overlay_rect = Rect {
            x: self.rect.x,
            y: self.rect.y + self.rect.h + 4.0,
            w: self.rect.w,
            h: overlay_height,
        };

        let radius = 6.0;
        let overlay_rrect = RoundedRect {
            rect: overlay_rect,
            radii: RoundedRadii {
                tl: radius,
                tr: radius,
                br: radius,
                bl: radius,
            },
        };

        // Overlay background - solid, opaque background for better visibility
        let overlay_bg = Color::rgba(30, 35, 50, 255);
        canvas.rounded_rect(overlay_rrect, Brush::Solid(overlay_bg), z);

        // Add a subtle shadow/border effect for depth
        let shadow_color = Color::rgba(0, 0, 0, 100);
        shapes::draw_rounded_rectangle(
            canvas,
            overlay_rrect,
            None,
            Some(4.0),
            Some(Brush::Solid(shadow_color)),
            z - 1,
        );

        // Overlay border
        let overlay_border = Color::rgba(80, 90, 110, 255);
        shapes::draw_rounded_rectangle(
            canvas,
            overlay_rrect,
            None,
            Some(1.0),
            Some(Brush::Solid(overlay_border)),
            z + 1,
        );

        // Render each option
        for (idx, option) in self.options.iter().enumerate() {
            let option_y = overlay_rect.y + overlay_padding + (idx as f32 * option_height);
            let option_rect = Rect {
                x: overlay_rect.x + overlay_padding,
                y: option_y,
                w: overlay_rect.w - (overlay_padding * 2.0),
                h: option_height,
            };

            // Highlight selected option
            let is_selected = self.selected_index == Some(idx);
            if is_selected {
                let highlight_rrect = RoundedRect {
                    rect: option_rect,
                    radii: RoundedRadii {
                        tl: 4.0,
                        tr: 4.0,
                        br: 4.0,
                        bl: 4.0,
                    },
                };
                let highlight_bg = Color::rgba(63, 130, 246, 200);
                canvas.rounded_rect(highlight_rrect, Brush::Solid(highlight_bg), z + 2);
            }

            // Option text
            let text_x = option_rect.x + 12.0;
            let text_y = option_rect.y + option_rect.h * 0.5 + self.label_size * 0.35;
            let text_color = if is_selected {
                Color::rgba(255, 255, 255, 255)
            } else {
                Color::rgba(220, 220, 220, 255)
            };

            canvas.draw_text_run(
                [text_x, text_y],
                option.clone(),
                self.label_size,
                text_color,
                z + 3,
            );
        }
    }
}
