use engine_core::{Brush, Color, ColorLinPremul, FillRule, Path, PathCmd, Rect, RoundedRadii, RoundedRect};
use rune_surface::Canvas;
use rune_surface::shapes;
use unicode_segmentation::UnicodeSegmentation;
use crate::elements::caret::CaretBlink;

/// InputBox with text editing support using system fonts via TextProvider.
/// 
/// Features:
/// - Cursor with blinking animation
/// - Horizontal scrolling when text exceeds width
/// - Text insertion and deletion
/// - Placeholder text support
/// - Grapheme-aware cursor movement
/// - Uses canvas TextProvider (no FontFace required)
pub struct InputBox {
    pub rect: Rect,
    pub text: String,
    pub text_size: f32,
    pub text_color: ColorLinPremul,
    pub placeholder: Option<String>,
    pub focused: bool,

    // Shared caret blink state (visibility + blink phase)
    caret: CaretBlink,
    pub cursor_position: usize, // Byte offset in text
    
    // Horizontal scrolling
    scroll_x: f32,
    
    // Padding
    padding_x: f32,
    padding_y: f32,
    
    // Cached approximate advance width for a space character at `text_size`.
    space_advance: Option<f32>,
}

impl InputBox {
    /// Create a new InputBox.
    pub fn new(
        rect: Rect,
        text: String,
        text_size: f32,
        text_color: ColorLinPremul,
        placeholder: Option<String>,
        focused: bool,
    ) -> Self {
        let initial_cursor = if focused { text.len() } else { 0 };
        Self {
            rect,
            text,
            text_size,
            text_color,
            placeholder,
            focused,
            caret: CaretBlink::new(focused),
            cursor_position: initial_cursor,
            scroll_x: 0.0,
            padding_x: 8.0,
            padding_y: 6.0,
            space_advance: None,
        }
    }

    /// Approximate advance width for a single ASCII space.
    fn space_width(&mut self, provider: &dyn engine_core::TextProvider) -> f32 {
        if let Some(w) = self.space_advance {
            return w;
        }
        // Try to estimate from provider metrics: width("a a") - width("aa").
        let est = if let (Some(with_space), Some(no_space)) = (
            crate::text::measure_run(provider, "a a", self.text_size),
            crate::text::measure_run(provider, "aa", self.text_size),
        ) {
            let diff = (with_space.width - no_space.width).max(0.0);
            if diff > 0.0 { diff } else { self.text_size * 0.5 }
        } else {
            self.text_size * 0.5
        };
        self.space_advance = Some(est);
        est
    }

    /// Compute cursor X (logical pixels) for current cursor position,
    /// approximating trailing spaces using cached space advance.
    fn cursor_x(&mut self, provider: &dyn engine_core::TextProvider) -> f32 {
        if self.text.is_empty() || self.cursor_position == 0 {
            return 0.0;
        }
        let safe_cursor = self.cursor_position.min(self.text.len());
        if self.cursor_position != safe_cursor {
            self.cursor_position = safe_cursor;
        }
        let prefix = &self.text[..safe_cursor];
        // Split into non-space prefix and trailing ASCII spaces.
        let trimmed_end = prefix.trim_end_matches(' ').len();
        let (non_space, trailing) = prefix.split_at(trimmed_end);

        let base = crate::text::measure_run(provider, non_space, self.text_size)
            .map(|m| m.width)
            .unwrap_or(0.0);

        if trailing.is_empty() {
            return base;
        }

        let spaces = trailing.chars().filter(|&ch| ch == ' ').count() as f32;
        if spaces == 0.0 {
            return base;
        }

        base + self.space_width(provider) * spaces
    }
    
    /// Update the cursor blink animation.
    pub fn update_blink(&mut self, delta_time: f32) {
        self.caret.update(delta_time, self.focused);
    }
    
    /// Reset cursor blink (make visible).
    fn reset_cursor_blink(&mut self) {
        self.caret.reset_manual();
    }
    
    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, ch: char) {
        let offset = self.cursor_position.min(self.text.len());
        
        let mut new_text = String::new();
        new_text.push_str(&self.text[..offset]);
        new_text.push(ch);
        new_text.push_str(&self.text[offset..]);
        
        self.text = new_text;
        self.cursor_position = offset + ch.len_utf8();
        self.reset_cursor_blink();
    }
    
    /// Delete the character before the cursor (backspace).
    pub fn delete_before_cursor(&mut self) {
        if self.text.is_empty() || self.cursor_position == 0 {
            return;
        }
        
        // Find the previous grapheme boundary
        let mut new_offset = 0;
        for (idx, _) in self.text.grapheme_indices(true) {
            if idx >= self.cursor_position {
                break;
            }
            new_offset = idx;
        }
        
        let mut new_text = String::new();
        new_text.push_str(&self.text[..new_offset]);
        new_text.push_str(&self.text[self.cursor_position..]);
        
        self.text = new_text;
        self.cursor_position = new_offset;
        self.reset_cursor_blink();
    }
    
    /// Delete the character after the cursor (delete key).
    pub fn delete_after_cursor(&mut self) {
        if self.text.is_empty() || self.cursor_position >= self.text.len() {
            return;
        }
        
        // Find the next grapheme boundary
        let mut next_offset = self.text.len();
        for (idx, _) in self.text.grapheme_indices(true) {
            if idx > self.cursor_position {
                next_offset = idx;
                break;
            }
        }
        
        let mut new_text = String::new();
        new_text.push_str(&self.text[..self.cursor_position]);
        new_text.push_str(&self.text[next_offset..]);
        
        self.text = new_text;
        self.reset_cursor_blink();
    }
    
    /// Move cursor left by one grapheme.
    pub fn move_cursor_left(&mut self) {
        if self.cursor_position == 0 {
            return;
        }
        
        let mut new_offset = 0;
        for (idx, _) in self.text.grapheme_indices(true) {
            if idx >= self.cursor_position {
                break;
            }
            new_offset = idx;
        }
        
        self.cursor_position = new_offset;
        self.reset_cursor_blink();
    }
    
    /// Move cursor right by one grapheme.
    pub fn move_cursor_right(&mut self) {
        if self.cursor_position >= self.text.len() {
            return;
        }
        
        for (idx, _) in self.text.grapheme_indices(true) {
            if idx > self.cursor_position {
                self.cursor_position = idx;
                self.reset_cursor_blink();
                return;
            }
        }
        
        self.cursor_position = self.text.len();
        self.reset_cursor_blink();
    }
    
    /// Move cursor to start of text.
    pub fn move_cursor_to_start(&mut self) {
        self.cursor_position = 0;
        self.scroll_x = 0.0;
        self.reset_cursor_blink();
    }
    
    /// Move cursor to end of text.
    pub fn move_cursor_to_end(&mut self) {
        self.cursor_position = self.text.len();
        self.reset_cursor_blink();
    }
    
    /// Update scroll position based on cursor and text metrics.
    /// This should be called after any operation that changes cursor position.
    pub fn update_scroll(&mut self, provider: &dyn engine_core::TextProvider) {
        if self.text.is_empty() {
            self.scroll_x = 0.0;
            return;
        }
        let cursor_x = self.cursor_x(provider);

        let content_width = self.rect.w - self.padding_x * 2.0;
        let margin = 10.0;

        // Total text width for clamping scroll range
        let total_width = if let Some(metrics) =
            crate::text::measure_run(provider, &self.text, self.text_size)
        {
            metrics.width
        } else {
            0.0
        };

        // If all text fits, ensure no horizontal scroll.
        if total_width <= content_width {
            self.scroll_x = 0.0;
            return;
        }

        // Desired scroll bounds such that the cursor stays within a margin from
        // the left/right edges, while keeping the viewport inside the text.
        let viewport_limit = (total_width - content_width).max(0.0);

        // Smallest scroll that still keeps cursor inside the right margin.
        let mut min_scroll = if cursor_x > content_width - margin {
            cursor_x - (content_width - margin)
        } else {
            0.0
        };

        // Largest scroll that still keeps cursor inside the left margin.
        let mut max_scroll = if cursor_x > margin {
            cursor_x - margin
        } else {
            0.0
        };

        min_scroll = min_scroll.clamp(0.0, viewport_limit);
        max_scroll = max_scroll.clamp(0.0, viewport_limit);

        // Clamp current scroll_x into [min_scroll, max_scroll].
        self.scroll_x = self.scroll_x.clamp(min_scroll, max_scroll);
        // If for some reason min/max collapsed, ensure non-negative.
        if self.scroll_x < 0.0 {
            self.scroll_x = 0.0;
        }
    }
    
    /// Render the input box.
    pub fn render(&mut self, canvas: &mut Canvas, z: i32, provider: &dyn engine_core::TextProvider) {
        let radius = 6.0;
        let rrect = RoundedRect {
            rect: self.rect,
            radii: RoundedRadii { tl: radius, tr: radius, br: radius, bl: radius },
        };
        
        // Background
        let bg = Color::rgba(45, 52, 71, 255);
        canvas.rounded_rect(rrect, Brush::Solid(bg), z);
        
        // Border
        let border_color = if self.focused {
            Color::rgba(63, 130, 246, 255)
        } else {
            Color::rgba(80, 90, 110, 255)
        };
        let border_width = if self.focused { 2.0 } else { 1.0 };
        shapes::draw_rounded_rectangle(
            canvas,
            rrect,
            None,
            Some(border_width),
            Some(Brush::Solid(border_color)),
            z + 1,
        );

        // Calculate cursor X position and update scroll BEFORE clipping
        let cursor_x = self.cursor_x(provider);
        // Update scroll to keep cursor visible and handle shrinking text.
        self.update_scroll(provider);
        
        // Calculate content area
        let content_x = self.rect.x + self.padding_x;
        let content_y = self.rect.y + self.padding_y;
        let content_width = self.rect.w - self.padding_x * 2.0;
        let content_height = self.rect.h - self.padding_y * 2.0;
        
        // Set up clipping for text area
        let content_rect = Rect {
            x: content_x,
            y: content_y,
            w: content_width,
            h: content_height,
        };
        canvas.push_clip_rect(content_rect);
        
        // Text position (with scroll offset)
        let text_x = content_x - self.scroll_x;
        let text_y = self.rect.y + self.rect.h * 0.5 + self.text_size * 0.35;
        
        if !self.text.is_empty() {
            // Render text using draw_text_direct which respects clipping better
            canvas.draw_text_direct(
                [text_x, text_y],
                &self.text,
                self.text_size,
                self.text_color,
                provider,
            );
            
            // Render cursor if focused and visible
            if self.focused && self.caret.visible {
                let cx = text_x + cursor_x;
                let cy0 = self.rect.y + self.padding_y;
                let cy1 = self.rect.y + self.rect.h - self.padding_y;
                
                let mut caret = Path {
                    cmds: Vec::new(),
                    fill_rule: FillRule::NonZero,
                };
                caret.cmds.push(PathCmd::MoveTo([cx, cy0]));
                caret.cmds.push(PathCmd::LineTo([cx, cy1]));
                canvas.stroke_path(caret, 1.5, Color::rgba(63, 130, 246, 255), z + 3);
            }
        } else {
            // Optional placeholder text when empty
            if let Some(ref placeholder) = self.placeholder {
                canvas.draw_text_direct(
                    [text_x, text_y],
                    placeholder,
                    self.text_size,
                    Color::rgba(120, 120, 130, 255),
                    provider,
                );
            }
            
            // Render cursor at start if focused and visible, even without placeholder
            if self.focused && self.caret.visible {
                let cx = text_x;
                let cy0 = self.rect.y + self.padding_y;
                let cy1 = self.rect.y + self.rect.h - self.padding_y;
                
                let mut caret = Path {
                    cmds: Vec::new(),
                    fill_rule: FillRule::NonZero,
                };
                caret.cmds.push(PathCmd::MoveTo([cx, cy0]));
                caret.cmds.push(PathCmd::LineTo([cx, cy1]));
                canvas.stroke_path(caret, 1.5, Color::rgba(63, 130, 246, 255), z + 3);
            }
        }
        
        canvas.pop_clip();
    }
}
