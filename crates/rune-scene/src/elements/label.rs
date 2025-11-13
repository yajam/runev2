use engine_core::Color;
use rune_surface::Canvas;

pub struct Label {
    pub text: String,
    pub pos: [f32; 2],
    pub size: f32,
    pub color: Color,
}

impl Label {
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        canvas.draw_text_run(self.pos, self.text.clone(), self.size, self.color, z);
    }
}
