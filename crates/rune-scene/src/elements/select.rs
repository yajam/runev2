use engine_core::{Brush, Color, ColorLinPremul, Rect, RoundedRadii, RoundedRect};
use rune_surface::Canvas;
use rune_surface::shapes::{self};

pub struct Select {
    pub rect: Rect,
    pub label: String,
    pub label_size: f32,
    pub label_color: ColorLinPremul,
    pub open: bool,
    pub focused: bool,
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
        let icon_size = 16.0;
        let icon_x = self.rect.x + self.rect.w - icon_size - 12.0;
        let icon_y = self.rect.y + (self.rect.h - icon_size) * 0.5;
        let chevron_path = if self.open {
            "images/chevron-up.svg"
        } else {
            "images/chevron-down.svg"
        };
        canvas.draw_svg(
            chevron_path,
            [icon_x, icon_y],
            [icon_size, icon_size],
            z + 3,
        );
    }
}
