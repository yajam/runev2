use engine_core::ColorLinPremul;
use rune_surface::Canvas;
use crate::text::{layout_text, LayoutOptions, Wrap};

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
    /// This method uses a simple character-based approximation instead of expensive
    /// glyph rasterization, making it much faster while still providing good results.
    /// Recommended for UI text where performance is critical.
    pub fn render_fast(&self, canvas: &mut Canvas, z: i32) {
        let lh_factor = self.line_height_factor.unwrap_or(1.2);
        let line_height = self.size * lh_factor;
        
        // Determine wrapping width
        let wrap_width = match self.max_width {
            Some(w) if w > 0.0 => w,
            _ => {
                // No wrapping - render as single line
                canvas.draw_text_run(self.pos, self.text.clone(), self.size, self.color, z);
                return;
            }
        };
        
        // Fast character-count approximation
        let avg_char_width = self.size * 0.55;
        let max_chars = (wrap_width / avg_char_width).floor() as usize;
        if max_chars == 0 {
            return;
        }
        
        // Word-wrap using character count
        let words: Vec<&str> = self.text.split_whitespace().collect();
        let mut lines: Vec<String> = Vec::new();
        let mut current_line = String::new();
        
        for word in words {
            let test = if current_line.is_empty() {
                word.to_string()
            } else {
                format!("{} {}", current_line, word)
            };
            
            if test.len() <= max_chars {
                current_line = test;
            } else {
                if !current_line.is_empty() {
                    lines.push(current_line);
                }
                // Handle very long words
                if word.len() > max_chars {
                    let mut remaining = word;
                    while remaining.len() > max_chars {
                        let (chunk, rest) = remaining.split_at(max_chars);
                        lines.push(chunk.to_string());
                        remaining = rest;
                    }
                    current_line = remaining.to_string();
                } else {
                    current_line = word.to_string();
                }
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
        
        // Render lines
        for (i, line) in lines.iter().enumerate() {
            let y = self.pos[1] + (i as f32) * line_height;
            canvas.draw_text_run(
                [self.pos[0], y],
                line.clone(),
                self.size,
                self.color,
                z,
            );
        }
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
            canvas.draw_text_run(
                [self.pos[0], y],
                line.to_string(),
                self.size,
                self.color,
                z,
            );
        }
    }
}
