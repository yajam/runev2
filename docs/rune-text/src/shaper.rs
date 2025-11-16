use crate::font::RuneFont;
use crate::run::{GlyphPos, GlyphRun};
use harfrust::face::Face;
use harfrust::shaper::{Shaper, Features};

pub struct TextShaper {
    face: Face<'static>,
}

impl TextShaper {
    pub fn new(font: &RuneFont) -> Self {
        let face = Face::from_slice(&font.bytes, 0)
            .expect("Failed to build harfrust Face");
        Self { face }
    }

    pub fn shape(&self, text: &str, font_index: usize, px: f32) -> GlyphRun {
        let mut shaper = Shaper::new(&self.face);
        shaper.set_size(px);
        shaper.set_direction(harfrust::Direction::LTR);
        shaper.set_script(harfrust::Script::Latin);
        shaper.set_language("en");

        let glyphs = shaper.shape_text(text, Features::default());
        let mut result = Vec::new();

        for g in glyphs {
            result.push(GlyphPos {
                glyph_id: g.id,
                x: g.x as f32,
                y: -g.y as f32,
                advance: g.advance as f32,
                cluster: g.cluster as usize,
            });
        }
        GlyphRun { font_index, glyphs: result }
    }
}
