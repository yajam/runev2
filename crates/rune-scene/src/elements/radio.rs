use engine_core::{Brush, Path, PathCmd, FillRule, Color};
use rune_surface::shapes;
use rune_surface::Canvas;

pub struct Radio {
    pub center: [f32; 2],
    pub radius: f32,
    pub selected: bool,
    pub label: Option<String>,
    pub label_size: f32,
    pub focused: bool,
}

impl Radio {
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        let bg = Color::rgba(240, 240, 240, 255);
        let border = Color::rgba(160, 160, 160, 255);
        canvas.ellipse(self.center, [self.radius, self.radius], Brush::Solid(bg), z);
        // Border (approx via stroke path circle)
        let mut p = Path { cmds: Vec::new(), fill_rule: FillRule::NonZero };
        // Approximate circle with a square outline (simple placeholder)
        let x = self.center[0] - self.radius;
        let y = self.center[1] - self.radius;
        let d = self.radius * 2.0;
        p.cmds.push(PathCmd::MoveTo([x, y]));
        p.cmds.push(PathCmd::LineTo([x + d, y]));
        p.cmds.push(PathCmd::LineTo([x + d, y + d]));
        p.cmds.push(PathCmd::LineTo([x, y + d]));
        p.cmds.push(PathCmd::Close);
        canvas.stroke_path(p, 1.0, border, z + 1);
        if self.selected {
            let inner = self.radius * 0.5;
            let col = Color::rgba(40, 120, 220, 255);
            canvas.ellipse(self.center, [inner, inner], Brush::Solid(col), z + 2);
        }
        if self.focused {
            shapes::draw_ellipse(
                canvas,
                self.center,
                [self.radius, self.radius],
                None,
                Some(2.0),
                Some(Brush::Solid(Color::rgba(63, 130, 246, 255))),
                z + 3,
            );
        }
        if let Some(text) = &self.label {
            let pos = [self.center[0] + self.radius + 8.0, self.center[1] + self.label_size * 0.35];
            canvas.draw_text_run(pos, text.clone(), self.label_size, Color::rgba(20, 20, 20, 255), z + 3);
        }
    }
}
