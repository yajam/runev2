use unicode_segmentation::UnicodeSegmentation;

use crate::font::FontFace;
use crate::layout::{line_breaker::compute_line_breaks, LineBox, PrefixSums, WrapMode};
use crate::shaping::TextShaper;

/// Complete text layout with all lines for a single font.
///
/// Phase 2.2 focuses on building `LineBox` instances with
/// correct metrics and supporting basic multi-line layout
/// driven by explicit newline characters.
#[derive(Debug)]
pub struct TextLayout {
    /// Source text.
    text: String,
    /// All line boxes in visual/top-to-bottom order.
    lines: Vec<LineBox>,
    /// Prefix sums over characters and bytes for fast lookups.
    prefix_sums: PrefixSums,
}

impl TextLayout {
    /// Layout text using a single font and font size with no wrapping
    /// beyond explicit newline characters.
    pub fn new(text: impl Into<String>, font: &FontFace, font_size: f32) -> Self {
        Self::with_wrap(text, font, font_size, None, WrapMode::NoWrap)
    }

    /// Layout text with an optional width constraint and wrapping mode.
    ///
    /// - If `max_width` is `None` or `WrapMode::NoWrap`, only explicit
    ///   newlines are honored.
    /// - For `WrapMode::BreakWord`, uses Unicode line breaking and
    ///   falls back to grapheme boundaries when a single word exceeds
    ///   `max_width`.
    /// - For `WrapMode::BreakAll`, always breaks at grapheme
    ///   boundaries using a greedy algorithm.
    pub fn with_wrap(
        text: impl Into<String>,
        font: &FontFace,
        font_size: f32,
        max_width: Option<f32>,
        wrap_mode: WrapMode,
    ) -> Self {
        let text = text.into();
        let mut lines = Vec::new();

        // Compute scaled metrics once; reused for all lines.
        let scaled = font.scaled_metrics(font_size);
        let line_height = scaled.ascent + scaled.descent + scaled.line_gap;

        let mut y = 0.0f32;

        // Split into logical paragraphs on '\n'. Wrapping decisions
        // are made per paragraph.
        let mut para_start = 0usize;
        for (idx, ch) in text.char_indices() {
            if ch == '\n' {
                Self::layout_paragraph(
                    &text,
                    para_start..idx,
                    font,
                    font_size,
                    line_height,
                    scaled.ascent,
                    scaled.descent,
                    scaled.line_gap,
                    max_width,
                    wrap_mode,
                    &mut y,
                    &mut lines,
                );
                // Preserve empty line after newline.
                Self::layout_paragraph(
                    &text,
                    idx..idx,
                    font,
                    font_size,
                    line_height,
                    scaled.ascent,
                    scaled.descent,
                    scaled.line_gap,
                    max_width,
                    wrap_mode,
                    &mut y,
                    &mut lines,
                );
                para_start = idx + ch.len_utf8();
            }
        }
        if para_start <= text.len() {
            Self::layout_paragraph(
                &text,
                para_start..text.len(),
                font,
                font_size,
                line_height,
                scaled.ascent,
                scaled.descent,
                scaled.line_gap,
                max_width,
                wrap_mode,
                &mut y,
                &mut lines,
            );
        }

        let prefix_sums = PrefixSums::new(&text, &lines);
        Self {
            text,
            lines,
            prefix_sums,
        }
    }

    /// Underlying source text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// All line boxes in this layout.
    pub fn lines(&self) -> &[LineBox] {
        &self.lines
    }

    /// Get the line containing the given character offset, if any.
    pub fn line_at_char(&self, char_offset: usize) -> Option<&LineBox> {
        self.prefix_sums
            .line_at_char(char_offset)
            .and_then(|idx| self.lines.get(idx))
    }

    /// Get the cumulative character offset at the start of a line.
    pub fn char_offset_at_line(&self, line_index: usize) -> Option<usize> {
        self.prefix_sums.char_offset_at_line(line_index)
    }
}

impl TextLayout {
    #[allow(clippy::too_many_arguments)]
    fn layout_paragraph(
        full_text: &str,
        range: core::ops::Range<usize>,
        font: &FontFace,
        font_size: f32,
        line_height: f32,
        ascent: f32,
        descent: f32,
        leading: f32,
        max_width: Option<f32>,
        wrap_mode: WrapMode,
        y: &mut f32,
        out_lines: &mut Vec<LineBox>,
    ) {
        let paragraph = &full_text[range.clone()];

        // Empty paragraph: emit a single empty line.
        if paragraph.is_empty() {
            let line = LineBox {
                text_range: range,
                width: 0.0,
                height: line_height,
                baseline_offset: ascent,
                ascent,
                descent,
                leading,
                runs: Vec::new(),
                y_offset: *y,
            };
            *y += line_height;
            out_lines.push(line);
            return;
        }

        // No wrapping requested or no width constraint: single line.
        if max_width.is_none() || matches!(wrap_mode, WrapMode::NoWrap) {
            let run = TextShaper::shape_ltr(paragraph, range.clone(), font, 0, font_size);
            let line = LineBox {
                text_range: range,
                width: run.width,
                height: line_height,
                baseline_offset: ascent,
                ascent,
                descent,
                leading,
                runs: vec![run],
                y_offset: *y,
            };
            *y += line_height;
            out_lines.push(line);
            return;
        }

        let max_width = max_width.unwrap();

        match wrap_mode {
            WrapMode::BreakWord => {
                Self::layout_with_word_wrap(
                    full_text,
                    range,
                    font,
                    font_size,
                    line_height,
                    ascent,
                    descent,
                    leading,
                    max_width,
                    y,
                    out_lines,
                );
            }
            WrapMode::BreakAll => {
                Self::layout_with_break_all(
                    full_text,
                    range,
                    font,
                    font_size,
                    line_height,
                    ascent,
                    descent,
                    leading,
                    max_width,
                    y,
                    out_lines,
                );
            }
            WrapMode::NoWrap => unreachable!(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_with_word_wrap(
        full_text: &str,
        range: core::ops::Range<usize>,
        font: &FontFace,
        font_size: f32,
        line_height: f32,
        ascent: f32,
        descent: f32,
        leading: f32,
        max_width: f32,
        y: &mut f32,
        out_lines: &mut Vec<LineBox>,
    ) {
        let paragraph = &full_text[range.clone()];
        let mut local_start = 0usize;
        let para_len = paragraph.len();

        // Precompute line break opportunities within the paragraph.
        let breaks = compute_line_breaks(paragraph);

        while local_start < para_len {
            let mut best_end = None;

            // Try all break opportunities after local_start, greedily
            // picking the last that fits.
            for br in breaks.iter().filter(|b| b.offset > local_start) {
                let local_end = br.offset.min(para_len);
                let segment = &paragraph[local_start..local_end];
                let run = TextShaper::shape_ltr(
                    segment,
                    (range.start + local_start)..(range.start + local_end),
                    font,
                    0,
                    font_size,
                );
                if run.width <= max_width {
                    best_end = Some((local_end, run));
                } else {
                    break;
                }
            }

            // If we found a suitable break at a word boundary, use it.
            if let Some((local_end, run)) = best_end {
                let line = LineBox {
                    text_range: (range.start + local_start)..(range.start + local_end),
                    width: run.width,
                    height: line_height,
                    baseline_offset: ascent,
                    ascent,
                    descent,
                    leading,
                    runs: vec![run],
                    y_offset: *y,
                };
                *y += line_height;
                out_lines.push(line);
                local_start = local_end;
                continue;
            }

            // Fall back to breaking at grapheme boundaries if a single
            // word exceeds max_width.
            let mut best_end = None;
            for (idx, g) in paragraph[local_start..].grapheme_indices(true) {
                let local_end = local_start + idx + g.len();
                let segment = &paragraph[local_start..local_end];
                let run = TextShaper::shape_ltr(
                    segment,
                    (range.start + local_start)..(range.start + local_end),
                    font,
                    0,
                    font_size,
                );
                if run.width <= max_width {
                    best_end = Some((local_end, run));
                } else {
                    break;
                }
            }

            if let Some((local_end, run)) = best_end {
                let line = LineBox {
                    text_range: (range.start + local_start)..(range.start + local_end),
                    width: run.width,
                    height: line_height,
                    baseline_offset: ascent,
                    ascent,
                    descent,
                    leading,
                    runs: vec![run],
                    y_offset: *y,
                };
                *y += line_height;
                out_lines.push(line);
                local_start = local_end;
            } else {
                // As a last resort, force at least one grapheme.
                let mut iter = paragraph[local_start..].grapheme_indices(true);
                if let Some((idx, g)) = iter.next() {
                    let local_end = local_start + idx + g.len();
                    let segment = &paragraph[local_start..local_end];
                    let run = TextShaper::shape_ltr(
                        segment,
                        (range.start + local_start)..(range.start + local_end),
                        font,
                        0,
                        font_size,
                    );
                    let line = LineBox {
                        text_range: (range.start + local_start)..(range.start + local_end),
                        width: run.width,
                        height: line_height,
                        baseline_offset: ascent,
                        ascent,
                        descent,
                        leading,
                        runs: vec![run],
                        y_offset: *y,
                    };
                    *y += line_height;
                    out_lines.push(line);
                    local_start = local_end;
                } else {
                    break;
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_with_break_all(
        full_text: &str,
        range: core::ops::Range<usize>,
        font: &FontFace,
        font_size: f32,
        line_height: f32,
        ascent: f32,
        descent: f32,
        leading: f32,
        max_width: f32,
        y: &mut f32,
        out_lines: &mut Vec<LineBox>,
    ) {
        let paragraph = &full_text[range.clone()];
        let mut local_start = 0usize;
        let para_len = paragraph.len();

        while local_start < para_len {
            let mut best_end = None;
            for (idx, g) in paragraph[local_start..].grapheme_indices(true) {
                let local_end = local_start + idx + g.len();
                let segment = &paragraph[local_start..local_end];
                let run = TextShaper::shape_ltr(
                    segment,
                    (range.start + local_start)..(range.start + local_end),
                    font,
                    0,
                    font_size,
                );
                if run.width <= max_width {
                    best_end = Some((local_end, run));
                } else {
                    break;
                }
            }

            if let Some((local_end, run)) = best_end {
                let line = LineBox {
                    text_range: (range.start + local_start)..(range.start + local_end),
                    width: run.width,
                    height: line_height,
                    baseline_offset: ascent,
                    ascent,
                    descent,
                    leading,
                    runs: vec![run],
                    y_offset: *y,
                };
                *y += line_height;
                out_lines.push(line);
                local_start = local_end;
            } else {
                // Force at least one grapheme.
                let mut iter = paragraph[local_start..].grapheme_indices(true);
                if let Some((idx, g)) = iter.next() {
                    let local_end = local_start + idx + g.len();
                    let segment = &paragraph[local_start..local_end];
                    let run = TextShaper::shape_ltr(
                        segment,
                        (range.start + local_start)..(range.start + local_end),
                        font,
                        0,
                        font_size,
                    );
                    let line = LineBox {
                        text_range: (range.start + local_start)..(range.start + local_end),
                        width: run.width,
                        height: line_height,
                        baseline_offset: ascent,
                        ascent,
                        descent,
                        leading,
                        runs: vec![run],
                        y_offset: *y,
                    };
                    *y += line_height;
                    out_lines.push(line);
                    local_start = local_end;
                } else {
                    break;
                }
            }
        }
    }
}
