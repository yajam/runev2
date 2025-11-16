#[derive(Debug, Clone)]
pub struct GlyphPos {
    pub glyph_id: u16,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
    pub cluster: usize,
}

#[derive(Debug, Clone)]
pub struct GlyphRun {
    pub font_index: usize,
    pub glyphs: Vec<GlyphPos>,
}
