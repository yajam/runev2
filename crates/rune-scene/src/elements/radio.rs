use engine_core::{Brush, Color, ColorLinPremul};
use rune_surface::Canvas;
use rune_surface::shapes;

pub struct Radio {
    pub center: [f32; 2],
    pub radius: f32,
    pub selected: bool,
    pub label: Option<String>,
    pub label_size: f32,
    pub label_color: ColorLinPremul,
    pub focused: bool,
}

impl Radio {
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        // Base circle background
        let bg = Color::rgba(45, 52, 71, 255);
        canvas.ellipse(self.center, [self.radius, self.radius], Brush::Solid(bg), z);

        // Border circle
        let border_color = Color::rgba(80, 90, 110, 255);
        shapes::draw_ellipse(
            canvas,
            self.center,
            [self.radius, self.radius],
            None,
            Some(1.0),
            Some(Brush::Solid(border_color)),
            z + 1,
        );

        // Selected inner dot
        if self.selected {
            let inner = self.radius * 0.6;
            let col = Color::rgba(63, 130, 246, 255);
            canvas.ellipse(self.center, [inner, inner], Brush::Solid(col), z + 2);
        }

        // Focus ring
        if self.focused {
            let focus_radius = self.radius + 2.0;
            shapes::draw_ellipse(
                canvas,
                self.center,
                [focus_radius, focus_radius],
                None,
                Some(2.0),
                Some(Brush::Solid(Color::rgba(63, 130, 246, 255))),
                z + 3,
            );
        }

        // Label
        if let Some(text) = &self.label {
            let pos = [
                self.center[0] + self.radius + 8.0,
                self.center[1] + self.label_size * 0.35,
            ];
            canvas.draw_text_run(pos, text.clone(), self.label_size, self.label_color, z + 3);
        }
    }
}
