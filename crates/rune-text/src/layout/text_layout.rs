use unicode_segmentation::UnicodeSegmentation;

use crate::font::FontFace;
use crate::layout::{
    cursor::{Cursor, CursorAffinity, CursorPosition, CursorRect},
    cursor_movement::CursorMovement,
    hit_test::{HitTestPolicy, HitTestResult, Point, Position},
    line_breaker::{compute_line_breaks, compute_word_boundaries},
    selection::{Selection, SelectionRect},
    LineBox, PrefixSums, WrapMode,
};
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

    /// Calculate the visual rectangle for rendering a cursor at the given position.
    ///
    /// Returns `None` if the cursor position is invalid or outside the text bounds.
    pub fn cursor_rect(&self, cursor: &Cursor) -> Option<CursorRect> {
        self.cursor_rect_at_position(cursor.position())
    }

    /// Calculate the visual rectangle for a cursor at a specific position.
    ///
    /// Returns `None` if the position is invalid or outside the text bounds.
    pub fn cursor_rect_at_position(&self, position: CursorPosition) -> Option<CursorRect> {
        let byte_offset = position.byte_offset;

        // Handle empty text
        if self.text.is_empty() {
            if byte_offset == 0 && !self.lines.is_empty() {
                let line = &self.lines[0];
                return Some(CursorRect::new(
                    0.0,
                    line.y_offset,
                    1.0, // Default cursor width
                    line.height,
                ));
            }
            return None;
        }

        // Validate byte offset
        if byte_offset > self.text.len() {
            return None;
        }

        // Find the line containing this byte offset
        let line_idx = self.find_line_at_byte_offset(byte_offset)?;
        let line = &self.lines[line_idx];

        // Calculate X position within the line
        let x = self.calculate_x_at_byte_offset(line, byte_offset);

        Some(CursorRect::new(
            x,
            line.y_offset,
            1.0, // Default cursor width
            line.height,
        ))
    }

    /// Find the line index containing the given byte offset.
    fn find_line_at_byte_offset(&self, byte_offset: usize) -> Option<usize> {
        for (idx, line) in self.lines.iter().enumerate() {
            if line.text_range.contains(&byte_offset) || line.text_range.end == byte_offset {
                return Some(idx);
            }
        }
        None
    }

    /// Calculate the X position for a cursor at the given byte offset within a line.
    /// Uses cluster information for accurate positioning with ligatures.
    fn calculate_x_at_byte_offset(&self, line: &LineBox, byte_offset: usize) -> f32 {
        // If the offset is at or before the line start, cursor is at the beginning
        if byte_offset <= line.text_range.start {
            return 0.0;
        }

        // If the offset is at or after the line end, cursor is at the end
        if byte_offset >= line.text_range.end {
            return line.width;
        }

        // Calculate position within the line by measuring text up to the cursor
        let mut x = 0.0;
        for run in &line.runs {
            if byte_offset <= run.text_range.start {
                // Cursor is before this run
                break;
            }

            if byte_offset >= run.text_range.end {
                // Cursor is after this run, add full run width
                x += run.width;
                continue;
            }

            // Cursor is within this run - use cluster information for precise positioning
            let offset_in_run = byte_offset - run.text_range.start;
            
            // Find the cluster containing or just before this offset
            let mut current_x = 0.0;
            let mut i = 0;
            
            while i < run.clusters.len() {
                let cluster_start = run.clusters[i];
                
                // If this cluster starts at or after our target offset, we're done
                if cluster_start as usize >= offset_in_run {
                    x += current_x;
                    return x;
                }
                
                // Accumulate width of all glyphs in this cluster
                let mut cluster_width = 0.0;
                let mut j = i;
                while j < run.clusters.len() && run.clusters[j] == cluster_start {
                    cluster_width += run.advances[j];
                    j += 1;
                }
                
                // Check if our offset is within this cluster
                // Find the next cluster boundary
                let next_cluster_start = if j < run.clusters.len() {
                    run.clusters[j] as usize
                } else {
                    run.text_range.len()
                };
                
                if offset_in_run < next_cluster_start {
                    // Offset is within this cluster
                    // Position at start of cluster (ligatures are indivisible)
                    x += current_x;
                    return x;
                }
                
                current_x += cluster_width;
                i = j;
            }
            
            // Offset is after all clusters, position at end of run
            x += run.width;
            break;
        }

        x
    }

    /// Validate and snap a cursor position to the nearest grapheme boundary.
    pub fn snap_cursor_to_boundary(&self, position: CursorPosition) -> CursorPosition {
        position.snap_to_grapheme_boundary(&self.text)
    }

    /// Create a cursor at the start of the text.
    pub fn cursor_at_start(&self) -> Cursor {
        Cursor::at_position(CursorPosition::new(0))
    }

    /// Create a cursor at the end of the text.
    pub fn cursor_at_end(&self) -> Cursor {
        Cursor::at_position(CursorPosition::new(self.text.len()))
    }

    /// Perform hit testing to find the character offset at a given point.
    ///
    /// The point coordinates are in zone-local space (relative to the layout origin).
    /// This ensures hit testing works correctly regardless of where the text is
    /// positioned on screen.
    ///
    /// # Arguments
    /// * `point` - Zone-local coordinates to test
    /// * `policy` - How to handle points outside text bounds
    ///
    /// # Returns
    /// `Some(HitTestResult)` with the byte offset, affinity, and line index,
    /// or `None` if using `HitTestPolicy::Strict` and the point is outside bounds.
    pub fn hit_test(&self, point: Point, policy: HitTestPolicy) -> Option<HitTestResult> {
        // Handle empty text
        if self.text.is_empty() {
            return match policy {
                HitTestPolicy::Clamp => Some(HitTestResult::new(
                    0,
                    CursorAffinity::Downstream,
                    0,
                )),
                HitTestPolicy::Strict => None,
            };
        }

        // Find the line containing this Y coordinate
        let line_idx = self.find_line_at_y(point.y, policy)?;
        let line = &self.lines[line_idx];

        // Hit test within the line to find X position
        let byte_offset = self.hit_test_line(line, point.x, policy)?;

        // Determine affinity based on position within character
        let affinity = CursorAffinity::Downstream;

        Some(HitTestResult::new(byte_offset, affinity, line_idx))
    }

    /// Map a character offset to its visual position in zone-local coordinates.
    ///
    /// Returns the position where a cursor at this offset would be rendered.
    /// All coordinates are relative to the layout origin.
    ///
    /// # Arguments
    /// * `byte_offset` - Byte offset in the source text
    ///
    /// # Returns
    /// `Some(Position)` with zone-local coordinates, or `None` if offset is invalid.
    pub fn offset_to_position(&self, byte_offset: usize) -> Option<Position> {
        // Validate offset
        if byte_offset > self.text.len() {
            return None;
        }

        // Handle empty text
        if self.text.is_empty() && byte_offset == 0 {
            return Some(Position::new(0.0, 0.0, 0));
        }

        // Find the line containing this offset
        let line_idx = self.find_line_at_byte_offset(byte_offset)?;
        let line = &self.lines[line_idx];

        // Calculate X position within the line
        let x = self.calculate_x_at_byte_offset(line, byte_offset);

        // Y position is the top of the line
        let y = line.y_offset;

        Some(Position::new(x, y, line_idx))
    }

    /// Get the position of the cursor baseline (for IME candidate window positioning).
    ///
    /// Returns zone-local coordinates of the baseline at the given offset.
    pub fn offset_to_baseline_position(&self, byte_offset: usize) -> Option<Position> {
        let line_idx = self.find_line_at_byte_offset(byte_offset)?;
        let line = &self.lines[line_idx];
        let x = self.calculate_x_at_byte_offset(line, byte_offset);
        let y = line.baseline_y();

        Some(Position::new(x, y, line_idx))
    }

    /// Find the line index containing the given Y coordinate.
    fn find_line_at_y(&self, y: f32, policy: HitTestPolicy) -> Option<usize> {
        if self.lines.is_empty() {
            return None;
        }

        // Before first line
        if y < self.lines[0].y_offset {
            return match policy {
                HitTestPolicy::Clamp => Some(0),
                HitTestPolicy::Strict => None,
            };
        }

        // After last line
        let last_idx = self.lines.len() - 1;
        if y >= self.lines[last_idx].bottom_y() {
            return match policy {
                HitTestPolicy::Clamp => Some(last_idx),
                HitTestPolicy::Strict => None,
            };
        }

        // Find the line containing this Y
        for (idx, line) in self.lines.iter().enumerate() {
            if y >= line.y_offset && y < line.bottom_y() {
                return Some(idx);
            }
        }

        // Shouldn't reach here, but clamp to last line
        match policy {
            HitTestPolicy::Clamp => Some(last_idx),
            HitTestPolicy::Strict => None,
        }
    }

    /// Hit test within a specific line to find the byte offset at X coordinate.
    /// Handles BiDi text by considering visual order of runs.
    fn hit_test_line(&self, line: &LineBox, x: f32, policy: HitTestPolicy) -> Option<usize> {
        // Before line start
        if x <= 0.0 {
            return match policy {
                HitTestPolicy::Clamp => {
                    // For BiDi, find the visually first run
                    self.find_visual_start_offset(line)
                },
                HitTestPolicy::Strict => None,
            };
        }

        // After line end
        if x >= line.width {
            return match policy {
                HitTestPolicy::Clamp => {
                    // For BiDi, find the visually last run
                    self.find_visual_end_offset(line)
                },
                HitTestPolicy::Strict => None,
            };
        }

        // Empty line
        if line.text_range.is_empty() {
            return Some(line.text_range.start);
        }

        // Find the position within the line
        // Runs are already in visual order from BiDi reordering
        let mut current_x = 0.0;
        for run in &line.runs {
            let run_end_x = current_x + run.width;

            if x >= current_x && x < run_end_x {
                // Hit is within this run
                let x_in_run = if run.direction == crate::shaping::Direction::RightToLeft {
                    // For RTL runs, reverse the X coordinate
                    run.width - (x - current_x)
                } else {
                    x - current_x
                };
                return self.hit_test_run(run, x_in_run);
            }

            current_x = run_end_x;
        }

        // Shouldn't reach here, but clamp to line end
        Some(line.text_range.end)
    }

    /// Find the byte offset at the visual start of a line (handles BiDi).
    fn find_visual_start_offset(&self, line: &LineBox) -> Option<usize> {
        if line.runs.is_empty() {
            return Some(line.text_range.start);
        }
        
        // The first run in visual order
        let first_run = &line.runs[0];
        if first_run.direction == crate::shaping::Direction::RightToLeft {
            // RTL run: visual start is logical end
            Some(first_run.text_range.end)
        } else {
            // LTR run: visual start is logical start
            Some(first_run.text_range.start)
        }
    }

    /// Find the byte offset at the visual end of a line (handles BiDi).
    fn find_visual_end_offset(&self, line: &LineBox) -> Option<usize> {
        if line.runs.is_empty() {
            return Some(line.text_range.end);
        }
        
        // The last run in visual order
        let last_run = &line.runs[line.runs.len() - 1];
        if last_run.direction == crate::shaping::Direction::RightToLeft {
            // RTL run: visual end is logical start
            Some(last_run.text_range.start)
        } else {
            // LTR run: visual end is logical end
            Some(last_run.text_range.end)
        }
    }

    /// Hit test within a shaped run to find the byte offset.
    /// Uses cluster information from HarfBuzz for ligature-aware positioning.
    fn hit_test_run(&self, run: &crate::shaping::ShapedRun, x: f32) -> Option<usize> {
        let run_text = &self.text[run.text_range.clone()];

        // Empty run
        if run_text.is_empty() {
            return Some(run.text_range.start);
        }

        // If no glyphs, return start
        if run.glyphs.is_empty() {
            return Some(run.text_range.start);
        }

        // Build cluster boundaries with their X positions
        // This handles ligatures correctly since multiple glyphs can map to same cluster
        let mut cluster_bounds: Vec<(u32, f32, f32)> = Vec::new(); // (cluster_byte_offset, x_start, x_end)
        
        let mut current_x = 0.0;
        let mut i = 0;
        
        while i < run.clusters.len() {
            let cluster_start = run.clusters[i];
            let cluster_x_start = current_x;
            
            // Find all glyphs belonging to this cluster
            let mut cluster_width = 0.0;
            let mut j = i;
            while j < run.clusters.len() && run.clusters[j] == cluster_start {
                cluster_width += run.advances[j];
                j += 1;
            }
            
            let cluster_x_end = current_x + cluster_width;
            cluster_bounds.push((cluster_start, cluster_x_start, cluster_x_end));
            
            current_x = cluster_x_end;
            i = j;
        }

        // Find the cluster containing the X position
        for (cluster_offset, x_start, x_end) in cluster_bounds.iter() {
            if x >= *x_start && x < *x_end {
                // Check if we're closer to start or end of this cluster
                let mid_point = (x_start + x_end) / 2.0;
                if x < mid_point {
                    // Closer to start of cluster
                    return Some(run.text_range.start + *cluster_offset as usize);
                } else {
                    // Closer to end of cluster - find next cluster boundary
                    // This handles ligatures correctly
                    let run_text = &self.text[run.text_range.clone()];
                    let cluster_byte_start = *cluster_offset as usize;
                    
                    // Find the next grapheme boundary after this cluster
                    for (idx, _) in run_text.grapheme_indices(true) {
                        if idx > cluster_byte_start {
                            return Some(run.text_range.start + idx);
                        }
                    }
                    
                    // If no next boundary found, return end of run
                    return Some(run.text_range.end);
                }
            }
        }

        // Past all clusters, return end of run
        Some(run.text_range.end)
    }

    // ========================================================================
    // Cursor Movement (Phase 6.3)
    // ========================================================================

    /// Move cursor left by one grapheme cluster.
    ///
    /// Returns the new byte offset after moving left.
    pub fn move_cursor_left(&self, byte_offset: usize) -> usize {
        CursorMovement::move_left_char(&self.text, byte_offset)
    }

    /// Move cursor right by one grapheme cluster.
    ///
    /// Returns the new byte offset after moving right.
    pub fn move_cursor_right(&self, byte_offset: usize) -> usize {
        CursorMovement::move_right_char(&self.text, byte_offset)
    }

    /// Move cursor left by one word boundary.
    ///
    /// Returns the new byte offset after moving to the previous word.
    pub fn move_cursor_left_word(&self, byte_offset: usize) -> usize {
        CursorMovement::move_left_word(&self.text, byte_offset)
    }

    /// Move cursor right by one word boundary.
    ///
    /// Returns the new byte offset after moving to the next word.
    pub fn move_cursor_right_word(&self, byte_offset: usize) -> usize {
        CursorMovement::move_right_word(&self.text, byte_offset)
    }

    /// Move cursor up by one line, maintaining horizontal position.
    ///
    /// The `preferred_x` parameter is used to maintain the column position
    /// when moving vertically. Pass `None` to calculate it from the current offset.
    ///
    /// Returns `(new_byte_offset, preferred_x)`.
    pub fn move_cursor_up(&self, byte_offset: usize, preferred_x: Option<f32>) -> (usize, f32) {
        // Find current line
        let Some(current_line_idx) = self.find_line_at_byte_offset(byte_offset) else {
            return (byte_offset, preferred_x.unwrap_or(0.0));
        };

        // If we're on the first line, stay at current position
        if current_line_idx == 0 {
            let x = preferred_x.unwrap_or_else(|| {
                self.calculate_x_at_byte_offset(&self.lines[current_line_idx], byte_offset)
            });
            return (byte_offset, x);
        }

        // Calculate X position if not provided
        let x = preferred_x.unwrap_or_else(|| {
            self.calculate_x_at_byte_offset(&self.lines[current_line_idx], byte_offset)
        });

        // Move to previous line
        let prev_line = &self.lines[current_line_idx - 1];

        // Hit test at the same X position on the previous line
        let new_offset = self.hit_test_line(prev_line, x, HitTestPolicy::Clamp)
            .unwrap_or(prev_line.text_range.start);

        (new_offset, x)
    }

    /// Move cursor down by one line, maintaining horizontal position.
    ///
    /// The `preferred_x` parameter is used to maintain the column position
    /// when moving vertically. Pass `None` to calculate it from the current offset.
    ///
    /// Returns `(new_byte_offset, preferred_x)`.
    pub fn move_cursor_down(&self, byte_offset: usize, preferred_x: Option<f32>) -> (usize, f32) {
        // Find current line
        let Some(current_line_idx) = self.find_line_at_byte_offset(byte_offset) else {
            return (byte_offset, preferred_x.unwrap_or(0.0));
        };

        // If we're on the last line, stay at current position
        if current_line_idx >= self.lines.len() - 1 {
            let x = preferred_x.unwrap_or_else(|| {
                self.calculate_x_at_byte_offset(&self.lines[current_line_idx], byte_offset)
            });
            return (byte_offset, x);
        }

        // Calculate X position if not provided
        let x = preferred_x.unwrap_or_else(|| {
            self.calculate_x_at_byte_offset(&self.lines[current_line_idx], byte_offset)
        });

        // Move to next line
        let next_line = &self.lines[current_line_idx + 1];

        // Hit test at the same X position on the next line
        let new_offset = self.hit_test_line(next_line, x, HitTestPolicy::Clamp)
            .unwrap_or(next_line.text_range.start);

        (new_offset, x)
    }

    /// Move cursor to the start of the current line.
    ///
    /// Returns the byte offset at the start of the line.
    pub fn move_cursor_line_start(&self, byte_offset: usize) -> usize {
        let Some(line_idx) = self.find_line_at_byte_offset(byte_offset) else {
            return 0;
        };

        let line = &self.lines[line_idx];
        
        // For BiDi text, find the visual start of the line
        self.find_visual_start_offset(line).unwrap_or(line.text_range.start)
    }

    /// Move cursor to the end of the current line.
    ///
    /// Returns the byte offset at the end of the line.
    pub fn move_cursor_line_end(&self, byte_offset: usize) -> usize {
        let Some(line_idx) = self.find_line_at_byte_offset(byte_offset) else {
            return self.text.len();
        };

        let line = &self.lines[line_idx];
        
        // For BiDi text, find the visual end of the line
        self.find_visual_end_offset(line).unwrap_or(line.text_range.end)
    }

    /// Move cursor to the start of the document.
    ///
    /// Returns 0.
    pub fn move_cursor_document_start(&self) -> usize {
        0
    }

    /// Move cursor to the end of the document.
    ///
    /// Returns the byte length of the text.
    pub fn move_cursor_document_end(&self) -> usize {
        self.text.len()
    }

    // ========================================================================
    // Selection Management (Phase 6.4)
    // ========================================================================

    /// Calculate selection rectangles for rendering a selection.
    ///
    /// Returns a vector of rectangles, one for each line that the selection spans.
    /// Empty if the selection is collapsed.
    pub fn selection_rects(&self, selection: &Selection) -> Vec<SelectionRect> {
        if selection.is_collapsed() {
            return Vec::new();
        }

        let range = selection.range();
        let start = range.start;
        let end = range.end;

        let mut rects = Vec::new();

        // Find all lines that intersect with the selection
        for line in &self.lines {
            // Skip lines that don't intersect with selection
            if line.text_range.end <= start || line.text_range.start >= end {
                continue;
            }

            // Calculate the intersection of selection and line
            let line_sel_start = start.max(line.text_range.start);
            let line_sel_end = end.min(line.text_range.end);

            // Calculate X positions for start and end of selection on this line
            let x_start = self.calculate_x_at_byte_offset(line, line_sel_start);
            let x_end = self.calculate_x_at_byte_offset(line, line_sel_end);

            rects.push(SelectionRect::new(
                x_start,
                line.y_offset,
                x_end - x_start,
                line.height,
            ));
        }

        rects
    }

    /// Select the word at the given byte offset.
    ///
    /// Returns a Selection covering the entire word containing the offset.
    /// If the offset is in whitespace or punctuation, selects that segment.
    pub fn select_word_at(&self, byte_offset: usize) -> Selection {
        if self.text.is_empty() {
            return Selection::collapsed(0);
        }

        let offset = byte_offset.min(self.text.len());
        let boundaries = compute_word_boundaries(&self.text);

        // Find the word boundary containing this offset
        for boundary in boundaries.iter() {
            if boundary.range.contains(&offset) || boundary.range.start == offset {
                return Selection::new(boundary.range.start, boundary.range.end);
            }
        }

        // If not found, return collapsed selection
        Selection::collapsed(offset)
    }

    /// Select the line at the given byte offset.
    ///
    /// Returns a Selection covering the entire line containing the offset.
    pub fn select_line_at(&self, byte_offset: usize) -> Selection {
        if self.text.is_empty() {
            return Selection::collapsed(0);
        }

        let offset = byte_offset.min(self.text.len());

        // Find the line containing this offset
        if let Some(line_idx) = self.find_line_at_byte_offset(offset) {
            let line = &self.lines[line_idx];
            return Selection::new(line.text_range.start, line.text_range.end);
        }

        Selection::collapsed(offset)
    }

    /// Select the entire paragraph at the given byte offset.
    ///
    /// A paragraph is defined as text between newlines.
    pub fn select_paragraph_at(&self, byte_offset: usize) -> Selection {
        if self.text.is_empty() {
            return Selection::collapsed(0);
        }

        let offset = byte_offset.min(self.text.len());

        // Find paragraph boundaries (text between newlines)
        let mut para_start = 0;
        let mut para_end = self.text.len();

        // Search backward for newline or start
        for (idx, ch) in self.text[..offset].char_indices().rev() {
            if ch == '\n' {
                para_start = idx + ch.len_utf8();
                break;
            }
        }

        // Search forward for newline or end
        for (idx, ch) in self.text[offset..].char_indices() {
            if ch == '\n' {
                para_end = offset + idx;
                break;
            }
        }

        Selection::new(para_start, para_end)
    }

    /// Extend a selection using a cursor movement operation.
    ///
    /// This is used for Shift+movement operations. The anchor stays fixed
    /// and the active end moves according to the movement function.
    ///
    /// # Arguments
    /// * `selection` - The current selection to extend
    /// * `move_fn` - A function that takes a byte offset and returns a new offset
    ///
    /// # Returns
    /// A new selection with the active end moved.
    pub fn extend_selection<F>(&self, selection: &Selection, move_fn: F) -> Selection
    where
        F: FnOnce(usize) -> usize,
    {
        let new_active = move_fn(selection.active());
        Selection::new(selection.anchor(), new_active)
    }

    /// Extend a selection using a cursor movement operation that returns (offset, x).
    ///
    /// This is used for Shift+Up/Down operations where we need to track the preferred X.
    ///
    /// # Arguments
    /// * `selection` - The current selection to extend
    /// * `move_fn` - A function that takes (offset, preferred_x) and returns (new_offset, new_x)
    /// * `preferred_x` - The preferred X position for vertical movement
    ///
    /// # Returns
    /// A tuple of (new_selection, new_preferred_x).
    pub fn extend_selection_vertical<F>(
        &self,
        selection: &Selection,
        move_fn: F,
        preferred_x: Option<f32>,
    ) -> (Selection, f32)
    where
        F: FnOnce(usize, Option<f32>) -> (usize, f32),
    {
        let (new_active, new_x) = move_fn(selection.active(), preferred_x);
        (Selection::new(selection.anchor(), new_active), new_x)
    }

    /// Validate and snap a selection to grapheme boundaries.
    ///
    /// Ensures both anchor and active positions are at valid grapheme cluster boundaries.
    pub fn snap_selection_to_boundaries(&self, selection: &Selection) -> Selection {
        let anchor = CursorPosition::new(selection.anchor())
            .snap_to_grapheme_boundary(&self.text)
            .byte_offset;
        let active = CursorPosition::new(selection.active())
            .snap_to_grapheme_boundary(&self.text)
            .byte_offset;
        Selection::new(anchor, active)
    }

    // ========================================================================
    // Text Insertion (Phase 6.5)
    // ========================================================================

    /// Insert a character at the cursor position.
    ///
    /// If a selection is active, it will be replaced with the character.
    /// Returns the new cursor position after insertion.
    ///
    /// # Arguments
    /// * `cursor_offset` - Byte offset where to insert
    /// * `ch` - Character to insert
    /// * `font` - Font to use for re-layout
    /// * `font_size` - Font size to use for re-layout
    /// * `max_width` - Optional width constraint for wrapping
    /// * `wrap_mode` - Text wrapping mode
    ///
    /// # Returns
    /// New cursor position after insertion (at the end of inserted character).
    pub fn insert_char(
        &mut self,
        cursor_offset: usize,
        ch: char,
        font: &FontFace,
        font_size: f32,
        max_width: Option<f32>,
        wrap_mode: WrapMode,
    ) -> usize {
        let mut s = String::new();
        s.push(ch);
        self.insert_str(cursor_offset, &s, font, font_size, max_width, wrap_mode)
    }

    /// Insert a string at the cursor position.
    ///
    /// If a selection is active, it will be replaced with the string.
    /// Returns the new cursor position after insertion.
    ///
    /// # Arguments
    /// * `cursor_offset` - Byte offset where to insert
    /// * `text` - String to insert
    /// * `font` - Font to use for re-layout
    /// * `font_size` - Font size to use for re-layout
    /// * `max_width` - Optional width constraint for wrapping
    /// * `wrap_mode` - Text wrapping mode
    ///
    /// # Returns
    /// New cursor position after insertion (at the end of inserted text).
    pub fn insert_str(
        &mut self,
        cursor_offset: usize,
        text: &str,
        font: &FontFace,
        font_size: f32,
        max_width: Option<f32>,
        wrap_mode: WrapMode,
    ) -> usize {
        // Validate cursor offset
        let offset = cursor_offset.min(self.text.len());

        // Validate inserted text (ensure it's valid UTF-8 and grapheme clusters)
        // The text parameter is already a &str, so it's valid UTF-8
        
        // Insert the text
        self.text.insert_str(offset, text);
        
        // Calculate new cursor position (after inserted text)
        let new_cursor = offset + text.len();
        
        // Trigger layout invalidation and re-layout
        self.relayout(font, font_size, max_width, wrap_mode);
        
        new_cursor
    }

    /// Replace a selection with inserted text.
    ///
    /// Deletes the selected range and inserts the new text.
    /// Returns the new cursor position after insertion.
    ///
    /// # Arguments
    /// * `selection` - Selection range to replace
    /// * `text` - String to insert
    /// * `font` - Font to use for re-layout
    /// * `font_size` - Font size to use for re-layout
    /// * `max_width` - Optional width constraint for wrapping
    /// * `wrap_mode` - Text wrapping mode
    ///
    /// # Returns
    /// New cursor position after insertion (at the end of inserted text).
    pub fn replace_selection(
        &mut self,
        selection: &Selection,
        text: &str,
        font: &FontFace,
        font_size: f32,
        max_width: Option<f32>,
        wrap_mode: WrapMode,
    ) -> usize {
        if selection.is_collapsed() {
            // No selection, just insert at cursor
            return self.insert_str(selection.active(), text, font, font_size, max_width, wrap_mode);
        }

        let range = selection.range();
        let start = range.start.min(self.text.len());
        let end = range.end.min(self.text.len());

        // Delete the selection
        self.text.replace_range(start..end, "");
        
        // Insert the new text
        self.text.insert_str(start, text);
        
        // Calculate new cursor position
        let new_cursor = start + text.len();
        
        // Trigger layout invalidation and re-layout
        self.relayout(font, font_size, max_width, wrap_mode);
        
        new_cursor
    }

    /// Insert a newline at the cursor position.
    ///
    /// Returns the new cursor position after insertion.
    pub fn insert_newline(
        &mut self,
        cursor_offset: usize,
        font: &FontFace,
        font_size: f32,
        max_width: Option<f32>,
        wrap_mode: WrapMode,
    ) -> usize {
        self.insert_char(cursor_offset, '\n', font, font_size, max_width, wrap_mode)
    }

    /// Insert a tab at the cursor position.
    ///
    /// Returns the new cursor position after insertion.
    pub fn insert_tab(
        &mut self,
        cursor_offset: usize,
        font: &FontFace,
        font_size: f32,
        max_width: Option<f32>,
        wrap_mode: WrapMode,
    ) -> usize {
        self.insert_char(cursor_offset, '\t', font, font_size, max_width, wrap_mode)
    }

    // ========================================================================
    // Text Deletion (Phase 6.6)
    // ========================================================================

    /// Delete the character before the cursor (backspace).
    ///
    /// Respects grapheme cluster boundaries.
    /// Returns the new cursor position after deletion.
    ///
    /// # Arguments
    /// * `cursor_offset` - Current cursor position
    /// * `font` - Font to use for re-layout
    /// * `font_size` - Font size to use for re-layout
    /// * `max_width` - Optional width constraint for wrapping
    /// * `wrap_mode` - Text wrapping mode
    ///
    /// # Returns
    /// New cursor position after deletion.
    pub fn delete_backward(
        &mut self,
        cursor_offset: usize,
        font: &FontFace,
        font_size: f32,
        max_width: Option<f32>,
        wrap_mode: WrapMode,
    ) -> usize {
        if cursor_offset == 0 || self.text.is_empty() {
            return 0;
        }

        // Find the previous grapheme boundary
        let mut prev_boundary = 0;
        for (idx, _) in self.text.grapheme_indices(true) {
            if idx >= cursor_offset {
                break;
            }
            prev_boundary = idx;
        }

        // Delete from prev_boundary to cursor_offset
        self.text.replace_range(prev_boundary..cursor_offset, "");
        
        // Trigger layout invalidation and re-layout
        self.relayout(font, font_size, max_width, wrap_mode);
        
        prev_boundary
    }

    /// Delete the character after the cursor (delete key).
    ///
    /// Respects grapheme cluster boundaries.
    /// Returns the new cursor position after deletion (unchanged).
    ///
    /// # Arguments
    /// * `cursor_offset` - Current cursor position
    /// * `font` - Font to use for re-layout
    /// * `font_size` - Font size to use for re-layout
    /// * `max_width` - Optional width constraint for wrapping
    /// * `wrap_mode` - Text wrapping mode
    ///
    /// # Returns
    /// New cursor position after deletion (same as input).
    pub fn delete_forward(
        &mut self,
        cursor_offset: usize,
        font: &FontFace,
        font_size: f32,
        max_width: Option<f32>,
        wrap_mode: WrapMode,
    ) -> usize {
        if cursor_offset >= self.text.len() || self.text.is_empty() {
            return cursor_offset;
        }

        // Find the next grapheme boundary
        let mut next_boundary = self.text.len();
        for (idx, _) in self.text.grapheme_indices(true) {
            if idx > cursor_offset {
                next_boundary = idx;
                break;
            }
        }

        // Delete from cursor_offset to next_boundary
        self.text.replace_range(cursor_offset..next_boundary, "");
        
        // Trigger layout invalidation and re-layout
        self.relayout(font, font_size, max_width, wrap_mode);
        
        cursor_offset
    }

    /// Delete the word before the cursor (Ctrl+Backspace).
    ///
    /// Returns the new cursor position after deletion.
    pub fn delete_word_backward(
        &mut self,
        cursor_offset: usize,
        font: &FontFace,
        font_size: f32,
        max_width: Option<f32>,
        wrap_mode: WrapMode,
    ) -> usize {
        if cursor_offset == 0 || self.text.is_empty() {
            return 0;
        }

        // Find the previous word boundary
        let prev_word_offset = CursorMovement::move_left_word(&self.text, cursor_offset);
        
        // Delete from prev_word_offset to cursor_offset
        self.text.replace_range(prev_word_offset..cursor_offset, "");
        
        // Trigger layout invalidation and re-layout
        self.relayout(font, font_size, max_width, wrap_mode);
        
        prev_word_offset
    }

    /// Delete the word after the cursor (Ctrl+Delete).
    ///
    /// Returns the new cursor position after deletion (unchanged).
    pub fn delete_word_forward(
        &mut self,
        cursor_offset: usize,
        font: &FontFace,
        font_size: f32,
        max_width: Option<f32>,
        wrap_mode: WrapMode,
    ) -> usize {
        if cursor_offset >= self.text.len() || self.text.is_empty() {
            return cursor_offset;
        }

        // Find the next word boundary
        let next_word_offset = CursorMovement::move_right_word(&self.text, cursor_offset);
        
        // Delete from cursor_offset to next_word_offset
        self.text.replace_range(cursor_offset..next_word_offset, "");
        
        // Trigger layout invalidation and re-layout
        self.relayout(font, font_size, max_width, wrap_mode);
        
        cursor_offset
    }

    /// Delete a selection range.
    ///
    /// Returns the new cursor position (at the start of the deleted range).
    ///
    /// # Arguments
    /// * `selection` - Selection range to delete
    /// * `font` - Font to use for re-layout
    /// * `font_size` - Font size to use for re-layout
    /// * `max_width` - Optional width constraint for wrapping
    /// * `wrap_mode` - Text wrapping mode
    ///
    /// # Returns
    /// New cursor position (at the start of the deleted range).
    pub fn delete_selection(
        &mut self,
        selection: &Selection,
        font: &FontFace,
        font_size: f32,
        max_width: Option<f32>,
        wrap_mode: WrapMode,
    ) -> usize {
        if selection.is_collapsed() {
            return selection.active();
        }

        let range = selection.range();
        let start = range.start.min(self.text.len());
        let end = range.end.min(self.text.len());

        // Delete the range
        self.text.replace_range(start..end, "");
        
        // Trigger layout invalidation and re-layout
        self.relayout(font, font_size, max_width, wrap_mode);
        
        start
    }

    /// Delete the entire line containing the cursor.
    ///
    /// Returns the new cursor position.
    pub fn delete_line(
        &mut self,
        cursor_offset: usize,
        font: &FontFace,
        font_size: f32,
        max_width: Option<f32>,
        wrap_mode: WrapMode,
    ) -> usize {
        if self.text.is_empty() {
            return 0;
        }

        let offset = cursor_offset.min(self.text.len());

        // Find line boundaries (between newlines)
        let mut line_start = 0;
        let mut line_end = self.text.len();

        // Search backward for newline or start
        for (idx, ch) in self.text[..offset].char_indices().rev() {
            if ch == '\n' {
                line_start = idx + ch.len_utf8();
                break;
            }
        }

        // Search forward for newline or end
        for (idx, ch) in self.text[offset..].char_indices() {
            if ch == '\n' {
                line_end = offset + idx + ch.len_utf8(); // Include the newline
                break;
            }
        }

        // Delete the line
        self.text.replace_range(line_start..line_end, "");
        
        // Trigger layout invalidation and re-layout
        self.relayout(font, font_size, max_width, wrap_mode);
        
        line_start.min(self.text.len())
    }

    // ========================================================================
    // Helper Methods for Text Modification
    // ========================================================================

    /// Re-layout the text after modification.
    ///
    /// This is called internally after any text insertion or deletion.
    fn relayout(
        &mut self,
        font: &FontFace,
        font_size: f32,
        max_width: Option<f32>,
        wrap_mode: WrapMode,
    ) {
        // Clear existing layout
        self.lines.clear();

        // Compute scaled metrics once; reused for all lines.
        let scaled = font.scaled_metrics(font_size);
        let line_height = scaled.ascent + scaled.descent + scaled.line_gap;

        let mut y = 0.0f32;

        // Split into logical paragraphs on '\n'. Wrapping decisions
        // are made per paragraph.
        let mut para_start = 0usize;
        for (idx, ch) in self.text.char_indices() {
            if ch == '\n' {
                Self::layout_paragraph(
                    &self.text,
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
                    &mut self.lines,
                );
                // Preserve empty line after newline.
                Self::layout_paragraph(
                    &self.text,
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
                    &mut self.lines,
                );
                para_start = idx + ch.len_utf8();
            }
        }
        if para_start <= self.text.len() {
            Self::layout_paragraph(
                &self.text,
                para_start..self.text.len(),
                font,
                font_size,
                line_height,
                scaled.ascent,
                scaled.descent,
                scaled.line_gap,
                max_width,
                wrap_mode,
                &mut y,
                &mut self.lines,
            );
        }

        // Rebuild prefix sums
        self.prefix_sums = PrefixSums::new(&self.text, &self.lines);
    }

    /// Get a mutable reference to the underlying text.
    ///
    /// **Warning**: Modifying the text directly will invalidate the layout.
    /// You must call `relayout()` after making changes, or use the provided
    /// insertion/deletion methods instead.
    pub fn text_mut(&mut self) -> &mut String {
        &mut self.text
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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_font() -> FontFace {
        // Load a test font - using the Geist font from the fonts directory
        let font_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fonts/Geist/Geist-VariableFont_wght.ttf");
        
        FontFace::from_path(&font_path, 0).expect("Failed to load test font")
    }

    #[test]
    fn test_insert_char() {
        let font = create_test_font();
        let mut layout = TextLayout::new("Hello", &font, 16.0);
        
        // Insert a character in the middle
        let new_cursor = layout.insert_char(5, '!', &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "Hello!");
        assert_eq!(new_cursor, 6);
    }

    #[test]
    fn test_insert_str() {
        let font = create_test_font();
        let mut layout = TextLayout::new("Hello", &font, 16.0);
        
        // Insert a string in the middle
        let new_cursor = layout.insert_str(5, " World", &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "Hello World");
        assert_eq!(new_cursor, 11);
    }

    #[test]
    fn test_insert_at_start() {
        let font = create_test_font();
        let mut layout = TextLayout::new("World", &font, 16.0);
        
        let new_cursor = layout.insert_str(0, "Hello ", &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "Hello World");
        assert_eq!(new_cursor, 6);
    }

    #[test]
    fn test_insert_newline() {
        let font = create_test_font();
        let mut layout = TextLayout::new("HelloWorld", &font, 16.0);
        
        let new_cursor = layout.insert_newline(5, &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "Hello\nWorld");
        assert_eq!(new_cursor, 6);
        // The layout creates 3 lines: "Hello", empty line after \n, and "World"
        assert_eq!(layout.lines().len(), 3);
    }

    #[test]
    fn test_insert_tab() {
        let font = create_test_font();
        let mut layout = TextLayout::new("HelloWorld", &font, 16.0);
        
        let new_cursor = layout.insert_tab(5, &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "Hello\tWorld");
        assert_eq!(new_cursor, 6);
    }

    #[test]
    fn test_replace_selection() {
        let font = create_test_font();
        let mut layout = TextLayout::new("Hello World", &font, 16.0);
        
        // Replace "World" with "Rust"
        let selection = Selection::new(6, 11);
        let new_cursor = layout.replace_selection(&selection, "Rust", &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "Hello Rust");
        assert_eq!(new_cursor, 10);
    }

    #[test]
    fn test_replace_collapsed_selection() {
        let font = create_test_font();
        let mut layout = TextLayout::new("Hello", &font, 16.0);
        
        // Collapsed selection should just insert
        let selection = Selection::collapsed(5);
        let new_cursor = layout.replace_selection(&selection, "!", &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "Hello!");
        assert_eq!(new_cursor, 6);
    }

    #[test]
    fn test_delete_backward() {
        let font = create_test_font();
        let mut layout = TextLayout::new("Hello!", &font, 16.0);
        
        // Delete the '!'
        let new_cursor = layout.delete_backward(6, &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "Hello");
        assert_eq!(new_cursor, 5);
    }

    #[test]
    fn test_delete_backward_at_start() {
        let font = create_test_font();
        let mut layout = TextLayout::new("Hello", &font, 16.0);
        
        // Delete at start should do nothing
        let new_cursor = layout.delete_backward(0, &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "Hello");
        assert_eq!(new_cursor, 0);
    }

    #[test]
    fn test_delete_forward() {
        let font = create_test_font();
        let mut layout = TextLayout::new("Hello!", &font, 16.0);
        
        // Delete the 'H'
        let new_cursor = layout.delete_forward(0, &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "ello!");
        assert_eq!(new_cursor, 0);
    }

    #[test]
    fn test_delete_forward_at_end() {
        let font = create_test_font();
        let mut layout = TextLayout::new("Hello", &font, 16.0);
        
        // Delete at end should do nothing
        let new_cursor = layout.delete_forward(5, &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "Hello");
        assert_eq!(new_cursor, 5);
    }

    #[test]
    fn test_delete_word_backward() {
        let font = create_test_font();
        let mut layout = TextLayout::new("Hello World", &font, 16.0);
        
        // Delete "World"
        let new_cursor = layout.delete_word_backward(11, &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "Hello ");
        assert_eq!(new_cursor, 6);
    }

    #[test]
    fn test_delete_word_forward() {
        let font = create_test_font();
        let mut layout = TextLayout::new("Hello World", &font, 16.0);
        
        // Delete "Hello"
        let new_cursor = layout.delete_word_forward(0, &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), " World");
        assert_eq!(new_cursor, 0);
    }

    #[test]
    fn test_delete_selection() {
        let font = create_test_font();
        let mut layout = TextLayout::new("Hello World", &font, 16.0);
        
        // Delete "World"
        let selection = Selection::new(6, 11);
        let new_cursor = layout.delete_selection(&selection, &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "Hello ");
        assert_eq!(new_cursor, 6);
    }

    #[test]
    fn test_delete_collapsed_selection() {
        let font = create_test_font();
        let mut layout = TextLayout::new("Hello", &font, 16.0);
        
        // Collapsed selection should do nothing
        let selection = Selection::collapsed(3);
        let new_cursor = layout.delete_selection(&selection, &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "Hello");
        assert_eq!(new_cursor, 3);
    }

    #[test]
    fn test_delete_line() {
        let font = create_test_font();
        let mut layout = TextLayout::new("Line 1\nLine 2\nLine 3", &font, 16.0);
        
        // Delete the second line
        let new_cursor = layout.delete_line(10, &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "Line 1\nLine 3");
        assert_eq!(new_cursor, 7);
    }

    #[test]
    fn test_delete_line_first() {
        let font = create_test_font();
        let mut layout = TextLayout::new("Line 1\nLine 2", &font, 16.0);
        
        // Delete the first line
        let new_cursor = layout.delete_line(3, &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "Line 2");
        assert_eq!(new_cursor, 0);
    }

    #[test]
    fn test_delete_line_last() {
        let font = create_test_font();
        let mut layout = TextLayout::new("Line 1\nLine 2", &font, 16.0);
        
        // Delete the last line
        let new_cursor = layout.delete_line(10, &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "Line 1\n");
        assert_eq!(new_cursor, 7);
    }

    #[test]
    fn test_insert_with_unicode() {
        let font = create_test_font();
        let mut layout = TextLayout::new("Hello", &font, 16.0);
        
        // Insert emoji
        let new_cursor = layout.insert_str(5, " 👋", &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "Hello 👋");
        assert!(new_cursor > 5);
    }

    #[test]
    fn test_delete_backward_with_unicode() {
        let font = create_test_font();
        let mut layout = TextLayout::new("Hello 👋", &font, 16.0);
        
        let text_len = layout.text().len();
        // Delete the emoji (grapheme cluster)
        let new_cursor = layout.delete_backward(text_len, &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "Hello ");
        assert_eq!(new_cursor, 6);
    }

    #[test]
    fn test_insert_and_relayout() {
        let font = create_test_font();
        let mut layout = TextLayout::new("Short", &font, 16.0);
        
        // Insert text and verify layout is updated
        layout.insert_str(5, " text that should wrap", &font, 16.0, Some(100.0), WrapMode::BreakWord);
        
        // Should have multiple lines due to wrapping
        assert!(layout.lines().len() > 1);
    }

    #[test]
    fn test_delete_and_relayout() {
        let font = create_test_font();
        let mut layout = TextLayout::new("This is a very long text that wraps", &font, 16.0);
        
        // Delete most of the text
        let selection = Selection::new(4, 36);
        layout.delete_selection(&selection, &font, 16.0, Some(100.0), WrapMode::BreakWord);
        
        assert_eq!(layout.text(), "This");
        // Should have only one line now
        assert_eq!(layout.lines().len(), 1);
    }

    #[test]
    fn test_combining_marks() {
        let font = create_test_font();
        let mut layout = TextLayout::new("café", &font, 16.0);
        
        let initial_len = layout.text().len();
        // Delete the 'é' (which might be e + combining accent)
        let new_cursor = layout.delete_backward(initial_len, &font, 16.0, None, WrapMode::NoWrap);
        
        assert_eq!(layout.text(), "caf");
        assert!(new_cursor < initial_len);
    }
}
