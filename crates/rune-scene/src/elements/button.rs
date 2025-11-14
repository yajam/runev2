use engine_core::{Brush, ColorLinPremul, Rect, RoundedRadii, RoundedRect, Path, PathCmd, FillRule, Color};
use rune_surface::shapes;
use rune_surface::Canvas;

pub struct Button {
    pub rect: Rect,
    pub radius: f32,
    pub bg: ColorLinPremul,
    pub fg: ColorLinPremul,
    pub label: String,
    pub label_size: f32,
    pub focused: bool,
}

impl Button {
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        let rrect = RoundedRect { rect: self.rect, radii: RoundedRadii { tl: self.radius, tr: self.radius, br: self.radius, bl: self.radius } };
        // Draw rounded background
        canvas.rounded_rect(rrect, Brush::Solid(self.bg), z);
        // Simple shadow highlight (optional): draw rounded rect overlay at top edge
        canvas.fill_rect(self.rect.x, self.rect.y, self.rect.w, 1.0, Brush::Solid(Color::rgba(255, 255, 255, 15)), z + 1);
        // Label (centered)
        // Approximate text width for centering (rough heuristic: 0.5 * font_size per char)
        let approx_text_width = self.label.len() as f32 * self.label_size * 0.5;
        let text_x = self.rect.x + (self.rect.w - approx_text_width) * 0.5;
        let text_y = self.rect.y + self.rect.h * 0.5 + self.label_size * 0.35;
        canvas.draw_text_run([text_x, text_y], self.label.clone(), self.label_size, self.fg, z + 2);
        // Rounded border
        shapes::draw_rounded_rectangle(
            canvas,
            rrect,
            None,
            Some(1.0),
            Some(Brush::Solid(Color::rgba(255, 255, 255, 26))),
            z + 3,
        );
        // Focus outline
        if self.focused {
            shapes::draw_rounded_rectangle(
                canvas,
                rrect,
                None,
                Some(2.0),
                Some(Brush::Solid(Color::rgba(63, 130, 246, 255))),
                z + 4,
            );
        }
    }
}
