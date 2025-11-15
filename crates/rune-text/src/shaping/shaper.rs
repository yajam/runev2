use core::ops::Range;

use rustybuzz::{Face, Script as HbScript, UnicodeBuffer};
use rustybuzz::ttf_parser::Tag;
use swash::GlyphId;

use crate::font::FontFace;

use super::{Direction, GlyphPosition, Script, ShapedRun};

/// Simple text shaper for Phase 1.3 built on rustybuzz.
///
/// For now this focuses on:
/// - Single-font runs
/// - Simple LTR text
/// - Kerning and ligatures via HarfBuzz/rustybuzz
pub struct TextShaper;

impl TextShaper {
    /// Shape a UTF-8 string using the given font and size, assuming simple
    /// left-to-right directionality and Latin script.
    pub fn shape_ltr(
        text: &str,
        text_range: Range<usize>,
        font: &FontFace,
        font_id: u32,
        font_size: f32,
    ) -> ShapedRun {
        // rustybuzz `Face` owns the font bytes; keep them alive alongside the face.
        let font_data = font_bytes(font);
        let face = Face::from_slice(&font_data, 0).expect("valid font for shaping");

        // Build Unicode buffer; this will handle complex scripts, combining
        // marks, ligatures, etc., though Phase 1.3 focuses on simple LTR usage.
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.set_direction(rustybuzz::Direction::LeftToRight);
        // Latin script for now; later phases can infer script from text.
        let latin_tag = Tag::from_bytes(b"Latn");
        if let Some(script) = HbScript::from_iso15924_tag(latin_tag) {
            buffer.set_script(script);
        }

        let glyph_buffer = rustybuzz::shape(&face, &[], buffer);
        let infos = glyph_buffer.glyph_infos();
        let positions = glyph_buffer.glyph_positions();

        let mut glyphs = Vec::with_capacity(infos.len());
        let mut glyph_positions = Vec::with_capacity(infos.len());
        let mut advances = Vec::with_capacity(infos.len());

        // rustybuzz uses 26.6 fixed-point for x_advance/y_advance.
        // Convert to pixels using the font's units-per-em and requested size.
        let metrics = font.metrics();
        let scale = if metrics.units_per_em != 0 {
            font_size / metrics.units_per_em as f32
        } else {
            1.0
        };

        let mut pen_x: f32 = 0.0;
        let mut width: f32 = 0.0;

        for (info, pos) in infos.iter().zip(positions.iter()) {
            let gid = info.glyph_id as GlyphId;
            let x_advance = fixed_to_f32(pos.x_advance) * scale;
            let x_offset = fixed_to_f32(pos.x_offset) * scale;
            let y_offset = -fixed_to_f32(pos.y_offset) * scale;

            glyphs.push(gid);
            glyph_positions.push(GlyphPosition {
                x_offset: pen_x + x_offset,
                y_offset,
            });
            advances.push(x_advance);

            pen_x += x_advance;
            width = pen_x;
        }

        ShapedRun {
            text_range,
            font_id,
            font_size,
            glyphs,
            positions: glyph_positions,
            advances,
            width,
            x_offset: 0.0,
            bidi_level: 0,
            direction: Direction::LeftToRight,
            script: Script::Latin,
        }
    }
}

fn font_bytes(font: &FontFace) -> Vec<u8> {
    // Access the underlying font data as a Vec<u8> suitable for rustybuzz::Face.
    // FontFace internally stores an Arc<[u8]>; clone into a Vec for now.
    // Future phases can optimize this to avoid copies.
    font.as_bytes().to_vec()
}

fn fixed_to_f32(v: i32) -> f32 {
    v as f32 / 64.0
}
