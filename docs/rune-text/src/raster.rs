use crate::font::RuneFont;
use crate::run::GlyphPos;
use swash::scale::{ScaleContext, RenderMode};
use swash::text::Glyph;

#[derive(Clone)]
pub struct RasterizedGlyph {
    pub glyph_id: u16,
    pub width: u32,
    pub height: u32,
    pub bearing_x: f32,
    pub bearing_y: f32,
    pub data: Vec<u8>,
}

pub struct GlyphRasterizer {
    ctx: ScaleContext,
}

impl GlyphRasterizer {
    pub fn new() -> Self {
        Self { ctx: ScaleContext::new() }
    }

    pub fn rasterize(
        &mut self,
        font: &RuneFont,
        gp: &GlyphPos,
        px: f32,
    ) -> Option<RasterizedGlyph> {
        let mut scaler = self.ctx.builder(font.font.clone())
            .size(px)
            .hint(true)
            .render_mode(RenderMode::Grayscale)
            .build();

        let glyph = Glyph::new(gp.glyph_id);

        if let Some(img) = scaler.render(&glyph) {
            return Some(RasterizedGlyph {
                glyph_id: gp.glyph_id,
                width: img.width,
                height: img.height,
                bearing_x: img.placement.left as f32,
                bearing_y: img.placement.top as f32,
                data: img.data.to_vec(),
            });
        }
        None
    }
}
