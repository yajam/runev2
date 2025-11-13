use engine_core::Color;
use rune_surface::Canvas;

pub struct Text {
    pub content: String,
    pub pos: [f32; 2],
    pub size: f32,
    pub color: Color,
}

impl Text {
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        canvas.draw_text_run(self.pos, self.content.clone(), self.size, self.color, z);
    }
}
