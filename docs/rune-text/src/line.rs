use crate::run::{GlyphPos, GlyphRun};

pub struct Line {
    pub glyphs: Vec<GlyphPos>,
    pub width: f32,
}

pub fn break_lines(run: &GlyphRun, max_width: f32) -> Vec<Line> {
    let mut lines = vec![];
    let mut current = Vec::new();
    let mut width = 0.0;

    for g in &run.glyphs {
        if width + g.advance > max_width && !current.is_empty() {
            lines.push(Line { glyphs: current, width });
            current = vec![];
            width = 0.0;
        }
        width += g.advance;
        current.push(g.clone());
    }

    if !current.is_empty() {
        lines.push(Line { glyphs: current, width });
    }
    lines
}
