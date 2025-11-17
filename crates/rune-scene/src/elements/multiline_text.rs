use crate::text::{LayoutOptions, Wrap, layout_text};
use engine_core::ColorLinPremul;
use rune_surface::Canvas;

/// A multiline text element that supports automatic word wrapping.
///
/// This element renders text that can span multiple lines, with automatic
/// word wrapping at a specified maximum width. It uses the text layout
/// infrastructure to compute proper baselines and line spacing.
pub struct MultilineText {
    /// Starting position [x, y] where y is the baseline of the first line
    pub pos: [f32; 2],
    /// The text content to render
    pub text: String,
    /// Font size in pixels
    pub size: f32,
    /// Text color
    pub color: ColorLinPremul,
    /// Maximum width for wrapping. If None, no wrapping occurs.
    pub max_width: Option<f32>,
    /// Line height multiplier (e.g., 1.2 for 20% extra spacing). Defaults to 1.2 if None.
    pub line_height_factor: Option<f32>,
}

impl MultilineText {
    /// Render the multiline text with wrapping support.
    ///
    /// This method requires a text provider to measure text and compute proper
    /// line wrapping and baseline positions.
    pub fn render(
        &self,
        canvas: &mut Canvas,
        z: i32,
        provider: &dyn engine_core::TextProvider,
        scale_factor: Option<f32>,
    ) {
        // Determine wrapping mode
        let wrap = match self.max_width {
            Some(w) if w > 0.0 => Wrap::Word(w),
            _ => Wrap::NoWrap,
        };

        // Compute line spacing
        let lh_factor = self.line_height_factor.unwrap_or(1.2);
        let line_pad = ((lh_factor - 1.0).max(0.0) * self.size).max(0.0);

        // Layout the text
        let opts = LayoutOptions {
            size_px: self.size,
            wrap,
            start_baseline_y: self.pos[1],
            line_pad,
            scale_factor,
        };

        let layout = layout_text(provider, &self.text, &opts);

        // Render each line at its computed baseline
        for (i, line) in layout.lines.iter().enumerate() {
            if i >= layout.baselines.len() {
                break;
            }
            let baseline_y = layout.baselines[i];
            canvas.draw_text_run(
                [self.pos[0], baseline_y],
                line.clone(),
                self.size,
                self.color,
                z,
            );
        }
    }

    /// Fast render using character-count approximation for wrapping.
    ///
    /// This method uses engine_core's fast wrapping without caching.
    /// For better performance with repeated renders, use render_cached instead.
    pub fn render_fast(&self, canvas: &mut Canvas, z: i32) {
        let lh_factor = self.line_height_factor.unwrap_or(1.2);

        // Determine wrapping width
        let wrap_width = match self.max_width {
            Some(w) if w > 0.0 => w,
            _ => {
                // No wrapping - render as single line
                canvas.draw_text_run(self.pos, self.text.clone(), self.size, self.color, z);
                return;
            }
        };

        // Use engine_core's fast wrapping (no allocation on cache hit)
        let wrapped = engine_core::wrap_text_fast(&self.text, wrap_width, self.size, lh_factor);

        // Render lines
        for (i, line) in wrapped.lines.iter().enumerate() {
            let y = self.pos[1] + (i as f32) * wrapped.line_height;
            canvas.draw_text_run([self.pos[0], y], line.clone(), self.size, self.color, z);
        }
    }

    /// Render with caching for maximum performance on repeated frames.
    ///
    /// Uses a TextLayoutCache to avoid recomputing wrapping on every frame.
    /// This is the recommended method for UI text that doesn't change frequently.
    ///
    /// Handles explicit paragraph breaks (newlines) by wrapping each paragraph separately.
    pub fn render_cached(
        &self,
        canvas: &mut Canvas,
        z: i32,
        cache: &engine_core::TextLayoutCache,
    ) -> f32 {
        let lh_factor = self.line_height_factor.unwrap_or(1.2);
        let line_height = self.size * lh_factor;

        // Determine wrapping width
        let wrap_width = match self.max_width {
            Some(w) if w > 0.0 => w,
            _ => {
                // No wrapping - just handle explicit newlines
                let lines: Vec<&str> = self.text.lines().collect();
                for (i, line) in lines.iter().enumerate() {
                    let y = self.pos[1] + (i as f32) * line_height;
                    canvas.draw_text_run(
                        [self.pos[0], y],
                        line.to_string(),
                        self.size,
                        self.color,
                        z,
                    );
                }
                return lines.len() as f32 * line_height;
            }
        };

        // Split text into paragraphs (preserve explicit newlines)
        let paragraphs: Vec<&str> = self.text.split("\n\n").collect();
        let mut current_y = self.pos[1];

        for (para_idx, paragraph) in paragraphs.iter().enumerate() {
            if paragraph.trim().is_empty() {
                // Empty paragraph - just add spacing
                current_y += line_height * 0.5;
                continue;
            }

            // Get wrapped text from cache (or compute and cache it)
            let wrapped = cache.get_or_wrap(paragraph, wrap_width, self.size, lh_factor);

            // Render lines for this paragraph
            for (i, line) in wrapped.lines.iter().enumerate() {
                let y = current_y + (i as f32) * wrapped.line_height;
                canvas.draw_text_run([self.pos[0], y], line.clone(), self.size, self.color, z);
            }

            // Move to next paragraph position
            current_y += wrapped.total_height;

            // Add extra spacing between paragraphs (except after the last one)
            if para_idx < paragraphs.len() - 1 {
                current_y += line_height * 0.5;
            }
        }

        // Return total height
        current_y - self.pos[1]
    }

    /// Simple render without wrapping (just splits on explicit newlines).
    ///
    /// This is a fallback method that doesn't perform any word wrapping.
    /// It only handles explicit newlines in the text.
    pub fn render_simple(&self, canvas: &mut Canvas, z: i32) {
        let lh_factor = self.line_height_factor.unwrap_or(1.2);
        let line_height = self.size * lh_factor;

        let lines: Vec<&str> = self.text.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let y = self.pos[1] + (i as f32) * line_height;
            canvas.draw_text_run([self.pos[0], y], line.to_string(), self.size, self.color, z);
        }
    }
}
