use engine_core::{Brush, ColorLinPremul, Rect, Color};
use rune_surface::shapes::{self, BorderStyle, BorderWidths, RectStyle};
use rune_surface::Canvas;

pub struct Checkbox {
    pub rect: Rect,
    pub checked: bool,
    pub focused: bool,
    pub label: Option<String>,
    pub label_size: f32,
    pub color: ColorLinPremul,
}

impl Checkbox {
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        // Box
        let bg = Brush::Solid(Color::rgba(240, 240, 240, 255));
        let border_col = Brush::Solid(Color::rgba(160, 160, 160, 255));
        let style = RectStyle {
            fill: Some(bg),
            border: Some(BorderStyle { widths: BorderWidths { top: 1.0, right: 1.0, bottom: 1.0, left: 1.0 }, brush: border_col }),
        };
        shapes::draw_rectangle(canvas, self.rect.x, self.rect.y, self.rect.w, self.rect.h, &style, z);
        // Focus outline (inside border)
        if self.focused {
            let focus = Brush::Solid(Color::rgba(63, 130, 246, 255));
            let fo = rune_surface::shapes::RectStyle {
                fill: None,
                border: Some(rune_surface::shapes::BorderStyle { widths: rune_surface::shapes::BorderWidths { top: 2.0, right: 2.0, bottom: 2.0, left: 2.0 }, brush: focus }),
            };
            shapes::draw_rectangle(canvas, self.rect.x, self.rect.y, self.rect.w, self.rect.h, &fo, z + 1);
        }
        // Check icon via SVG
        if self.checked {
            let pad = 3.0f32;
            let origin = [self.rect.x + pad, self.rect.y + pad];
            let max_size = [self.rect.w.max(0.0) - 2.0 * pad, self.rect.h.max(0.0) - 2.0 * pad];
            canvas.draw_svg("images/check.svg", origin, max_size, z + 2);
        }
        // Label
        if let Some(text) = &self.label {
            let tx = self.rect.x + self.rect.w + 8.0;
            let ty = self.rect.y + self.rect.h * 0.5 + self.label_size * 0.35;
            canvas.draw_text_run([tx, ty], text.clone(), self.label_size, Color::rgba(20, 20, 20, 255), z + 3);
        }
    }
}
