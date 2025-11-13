use engine_core::{Brush, Rect, Path, PathCmd, FillRule, Color};
use rune_surface::shapes::{self, BorderStyle, BorderWidths, RectStyle};
use rune_surface::Canvas;

pub struct Select {
    pub rect: Rect,
    pub label: String,
    pub label_size: f32,
    pub open: bool,
    pub focused: bool,
}

impl Select {
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        // Background
        let bg = Color::rgba(255, 255, 255, 255);
        canvas.fill_rect(self.rect.x, self.rect.y, self.rect.w, self.rect.h, Brush::Solid(bg), z);
        let base_border = Brush::Solid(Color::rgba(200, 200, 200, 255));
        let base_style = RectStyle { fill: None, border: Some(BorderStyle { widths: BorderWidths { top: 1.0, right: 1.0, bottom: 1.0, left: 1.0 }, brush: base_border }) };
        shapes::draw_rectangle(canvas, self.rect.x, self.rect.y, self.rect.w, self.rect.h, &base_style, z + 1);
        if self.focused {
            let focus = Brush::Solid(Color::rgba(63, 130, 246, 255));
            let fo = RectStyle { fill: None, border: Some(BorderStyle { widths: BorderWidths { top: 2.0, right: 2.0, bottom: 2.0, left: 2.0 }, brush: focus }) };
            shapes::draw_rectangle(canvas, self.rect.x, self.rect.y, self.rect.w, self.rect.h, &fo, z + 2);
        }
        // Label
        let tp = [self.rect.x + 8.0, self.rect.y + self.rect.h * 0.5 + self.label_size * 0.35];
        canvas.draw_text_run(tp, self.label.clone(), self.label_size, Color::rgba(16, 16, 16, 255), z + 2);
        // Caret/arrow
        let ax = self.rect.x + self.rect.w - 14.0;
        let ay = self.rect.y + self.rect.h * 0.5;
        let mut tri = Path { cmds: Vec::new(), fill_rule: FillRule::NonZero };
        if self.open {
            tri.cmds.push(PathCmd::MoveTo([ax - 6.0, ay + 3.0]));
            tri.cmds.push(PathCmd::LineTo([ax, ay - 3.0]));
            tri.cmds.push(PathCmd::LineTo([ax + 6.0, ay + 3.0]));
        } else {
            tri.cmds.push(PathCmd::MoveTo([ax - 6.0, ay - 3.0]));
            tri.cmds.push(PathCmd::LineTo([ax, ay + 3.0]));
            tri.cmds.push(PathCmd::LineTo([ax + 6.0, ay - 3.0]));
        }
        tri.cmds.push(PathCmd::Close);
        canvas.fill_path(tri, Color::rgba(80, 80, 80, 255), z + 3);
    }
}
