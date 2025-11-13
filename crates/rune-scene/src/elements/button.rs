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
        canvas.fill_rect(self.rect.x, self.rect.y, self.rect.w, self.rect.h, Brush::Solid(self.bg), z);
        // Simple shadow highlight (optional): draw rounded rect overlay
        canvas.fill_rect(self.rect.x, self.rect.y, self.rect.w, 1.0, Brush::Solid(Color::rgba(255, 255, 255, 15)), z + 1);
        // Label (left padded)
        let text_pos = [self.rect.x + 8.0, self.rect.y + self.rect.h * 0.5 + self.label_size * 0.35];
        canvas.draw_text_run(text_pos, self.label.clone(), self.label_size, self.fg, z + 2);
        // Border
        canvas.stroke_path(
            {
                let mut p = Path { cmds: Vec::new(), fill_rule: FillRule::NonZero };
                // Approximate rounded rect border with 4 lines (simple)
                p.cmds.push(PathCmd::MoveTo([self.rect.x, self.rect.y]));
                p.cmds.push(PathCmd::LineTo([self.rect.x + self.rect.w, self.rect.y]));
                p.cmds.push(PathCmd::LineTo([self.rect.x + self.rect.w, self.rect.y + self.rect.h]));
                p.cmds.push(PathCmd::LineTo([self.rect.x, self.rect.y + self.rect.h]));
                p.cmds.push(PathCmd::Close);
                p
            },
            1.0,
            Color::rgba(255, 255, 255, 26),
            z + 3,
        );
        // Focus outline
        if self.focused {
            let rr = RoundedRect { rect: self.rect, radii: RoundedRadii { tl: self.radius, tr: self.radius, br: self.radius, bl: self.radius } };
            shapes::draw_rounded_rectangle(
                canvas,
                rr,
                None,
                Some(2.0),
                Some(Brush::Solid(Color::rgba(63, 130, 246, 255))),
                z + 4,
            );
        }
        // Better: use rounded rect stroke when available via painter.stroke_rounded_rect in a future Canvas API.
        let _ = rrect; // keep to avoid unused warning when stroke is simplified
    }
}
