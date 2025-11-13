use engine_core::{Brush, Color, Rect};
use rune_surface::Canvas;

/// Placeholder image box. Draws a colored rect. Integrate PassManager for true image rendering.
pub struct ImageBox {
    pub rect: Rect,
    pub tint: Color,
}

impl ImageBox {
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        canvas.fill_rect(self.rect.x, self.rect.y, self.rect.w, self.rect.h, Brush::Solid(self.tint), z);
    }
}
