use engine_core::{Brush, Color, ColorLinPremul, FillRule, Path, PathCmd, Rect, RoundedRadii, RoundedRect};
use rune_surface::Canvas;
use rune_surface::shapes;
use rune_text::layout::{Cursor, Selection, TextLayout};
use rune_text::FontFace;
use unicode_segmentation::UnicodeSegmentation;

/// InputBox with full text editing support using rune-text.
/// 
/// Features:
/// - Cursor with blinking animation
/// - Text selection support
/// - Horizontal scrolling when text exceeds width
/// - Text insertion and deletion
/// - Placeholder text support
/// - Grapheme-aware cursor movement
pub struct InputBox {
    pub rect: Rect,
    pub text_size: f32,
    pub text_color: ColorLinPremul,
    pub placeholder: Option<String>,
    pub focused: bool,
    
    // Text editing state (using rune-text)
    layout: Option<TextLayout>,
    cursor: Cursor,
    selection: Selection,
    
    // Horizontal scrolling
    scroll_x: f32,
    
    // Padding
    padding_x: f32,
    padding_y: f32,
}

impl InputBox {
    /// Create a new InputBox with the given text and font.
    pub fn new(
        rect: Rect,
        text: String,
        text_size: f32,
        text_color: ColorLinPremul,
        placeholder: Option<String>,
        focused: bool,
        font: &FontFace,
    ) -> Self {
        let layout = if !text.is_empty() {
            Some(TextLayout::new(&text, font, text_size))
        } else {
            None
        };
        
        let mut cursor = Cursor::new();
        cursor.set_blink_interval(0.5); // 500ms blink interval
        
        Self {
            rect,
            text_size,
            text_color,
            placeholder,
            focused,
            layout,
            cursor,
            selection: Selection::collapsed(0),
            scroll_x: 0.0,
            padding_x: 12.0,
            padding_y: 8.0,
        }
    }
    
    /// Get the current text content.
    pub fn text(&self) -> &str {
        self.layout.as_ref().map(|l| l.text()).unwrap_or("")
    }
    
    /// Get the current horizontal scroll offset.
    pub fn scroll_offset(&self) -> f32 {
        self.scroll_x
    }
    
    /// Check if the cursor is currently visible (for blinking).
    pub fn is_cursor_visible(&self) -> bool {
        self.cursor.is_visible()
    }
    
    /// Set the text content and update layout.
    pub fn set_text(&mut self, text: String, font: &FontFace) {
        if text.is_empty() {
            self.layout = None;
            self.cursor = Cursor::new();
            self.cursor.set_blink_interval(0.5);
            self.selection = Selection::collapsed(0);
            self.scroll_x = 0.0;
        } else {
            self.layout = Some(TextLayout::new(&text, font, self.text_size));
            // Clamp cursor to new text length
            let max_offset = text.len();
            if self.cursor.byte_offset() > max_offset {
                self.cursor.set_byte_offset(max_offset);
            }
            self.selection = Selection::collapsed(self.cursor.byte_offset());
        }
    }
    
    /// Update the cursor blink animation.
    pub fn update_blink(&mut self, delta_time: f32) {
        if self.focused {
            self.cursor.update_blink(delta_time);
        }
    }
    
    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, ch: char, font: &FontFace) {
        let text = self.text().to_string();
        let offset = self.cursor.byte_offset().min(text.len());
        
        let mut new_text = String::new();
        new_text.push_str(&text[..offset]);
        new_text.push(ch);
        new_text.push_str(&text[offset..]);
        
        self.layout = Some(TextLayout::new(&new_text, font, self.text_size));
        let new_offset = offset + ch.len_utf8();
        self.cursor.set_byte_offset(new_offset);
        self.selection = Selection::collapsed(new_offset);
        
        // Update scroll to show cursor
        self.update_scroll_to_cursor();
    }
    
    /// Delete the character before the cursor (backspace).
    pub fn delete_before_cursor(&mut self, font: &FontFace) {
        let text = self.text().to_string();
        if text.is_empty() {
            return;
        }
        
        let offset = self.cursor.byte_offset();
        if offset == 0 {
            return;
        }
        
        // Find the previous grapheme boundary
        let mut new_offset = 0;
        for (idx, _) in text.grapheme_indices(true) {
            if idx >= offset {
                break;
            }
            new_offset = idx;
        }
        
        let mut new_text = String::new();
        new_text.push_str(&text[..new_offset]);
        new_text.push_str(&text[offset..]);
        
        if new_text.is_empty() {
            self.layout = None;
            self.cursor = Cursor::new();
            self.cursor.set_blink_interval(0.5);
            self.selection = Selection::collapsed(0);
            self.scroll_x = 0.0;
        } else {
            self.layout = Some(TextLayout::new(&new_text, font, self.text_size));
            self.cursor.set_byte_offset(new_offset);
            self.selection = Selection::collapsed(new_offset);
            self.update_scroll_to_cursor();
        }
    }
    
    /// Delete the character after the cursor (delete key).
    pub fn delete_after_cursor(&mut self, font: &FontFace) {
        let text = self.text().to_string();
        if text.is_empty() {
            return;
        }
        
        let offset = self.cursor.byte_offset();
        if offset >= text.len() {
            return;
        }
        
        // Find the next grapheme boundary
        let mut found_next = false;
        let mut next_offset = text.len();
        for (idx, _) in text.grapheme_indices(true) {
            if idx > offset {
                next_offset = idx;
                found_next = true;
                break;
            }
        }
        
        if !found_next {
            return;
        }
        
        let mut new_text = String::new();
        new_text.push_str(&text[..offset]);
        new_text.push_str(&text[next_offset..]);
        
        if new_text.is_empty() {
            self.layout = None;
            self.cursor = Cursor::new();
            self.cursor.set_blink_interval(0.5);
            self.selection = Selection::collapsed(0);
            self.scroll_x = 0.0;
        } else {
            self.layout = Some(TextLayout::new(&new_text, font, self.text_size));
            self.cursor.set_byte_offset(offset);
            self.selection = Selection::collapsed(offset);
        }
    }
    
    /// Move cursor left by one grapheme.
    pub fn move_cursor_left(&mut self) {
        if let Some(layout) = &self.layout {
            let new_offset = layout.move_cursor_left(self.cursor.byte_offset());
            self.cursor.set_byte_offset(new_offset);
            self.selection = Selection::collapsed(new_offset);
            self.update_scroll_to_cursor();
        }
    }
    
    /// Move cursor right by one grapheme.
    pub fn move_cursor_right(&mut self) {
        if let Some(layout) = &self.layout {
            let new_offset = layout.move_cursor_right(self.cursor.byte_offset());
            self.cursor.set_byte_offset(new_offset);
            self.selection = Selection::collapsed(new_offset);
            self.update_scroll_to_cursor();
        }
    }
    
    /// Move cursor to start of text.
    pub fn move_cursor_to_start(&mut self) {
        self.cursor.set_byte_offset(0);
        self.selection = Selection::collapsed(0);
        self.scroll_x = 0.0;
    }
    
    /// Move cursor to end of text.
    pub fn move_cursor_to_end(&mut self) {
        let end = self.text().len();
        self.cursor.set_byte_offset(end);
        self.selection = Selection::collapsed(end);
        self.update_scroll_to_cursor();
    }
    
    /// Update horizontal scroll to ensure cursor is visible.
    fn update_scroll_to_cursor(&mut self) {
        if let Some(layout) = &self.layout {
            if let Some(cursor_rect) = layout.cursor_rect(&self.cursor) {
                let viewport_width = self.rect.w - self.padding_x * 2.0;
                let cursor_x = cursor_rect.x;
                
                // Add a small margin for better UX
                let margin = 10.0;
                
                // Scroll left if cursor is before viewport
                if cursor_x < self.scroll_x + margin {
                    self.scroll_x = (cursor_x - margin).max(0.0);
                }
                // Scroll right if cursor is after viewport
                else if cursor_x > self.scroll_x + viewport_width - margin {
                    self.scroll_x = cursor_x - viewport_width + margin;
                }
            }
        }
    }
    
    /// Render the input box.
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
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

        // Set up clipping for text area
        let content_rect = Rect {
            x: self.rect.x + self.padding_x,
            y: self.rect.y + self.padding_y,
            w: self.rect.w - self.padding_x * 2.0,
            h: self.rect.h - self.padding_y * 2.0,
        };
        canvas.push_clip_rect(content_rect);
        
        // Text position (with scroll offset)
        let text_x = content_rect.x - self.scroll_x;
        let text_y = self.rect.y + self.rect.h * 0.5 + self.text_size * 0.35;
        
        if let Some(layout) = &self.layout {
            // Render text using layout
            if !self.text().is_empty() {
                canvas.draw_text_run(
                    [text_x, text_y],
                    self.text().to_string(),
                    self.text_size,
                    self.text_color,
                    z + 2,
                );
            }
            
            // Render cursor if focused and visible
            if self.focused && self.cursor.is_visible() {
                if let Some(cursor_rect) = layout.cursor_rect(&self.cursor) {
                    let cx = text_x + cursor_rect.x;
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
        } else if let Some(ref placeholder) = self.placeholder {
            // Render placeholder text
            canvas.draw_text_run(
                [text_x, text_y],
                placeholder.clone(),
                self.text_size,
                Color::rgba(120, 120, 130, 255),
                z + 2,
            );
        }
        
        canvas.pop_clip();
    }
}
