use engine_core::{Brush, Color, ColorLinPremul, FillRule, Path, PathCmd, Rect, RoundedRadii, RoundedRect};
use rune_surface::Canvas;
use rune_surface::shapes;
use crate::elements::caret::CaretBlink;
use crate::elements::selection_renderer::{self, SelectionRenderConfig};
use crate::elements::caret_renderer::{self, CaretRenderConfig};
use rune_text::layout::{
    Selection as RtSelection, TextLayout as RtTextLayout, WrapMode as RtWrapMode,
    Point as RtPoint, HitTestPolicy, CursorPosition,
};
use rune_text::font::load_system_default_font;

/// Single-line text input widget with rich editing capabilities.
///
/// # Architecture
///
/// `InputBox` is a thin visual wrapper over `rune-text`'s `TextLayout`. All text editing,
/// cursor movement, selection, undo/redo, and clipboard operations are delegated to
/// `TextLayout`, ensuring a single source of truth for editing behavior.
///
/// # Responsibilities
///
/// - **Visual rendering**: Background, border, text, selection highlight, and caret
/// - **Layout management**: Horizontal scrolling, padding, and viewport clipping
/// - **Event routing**: Mouse and keyboard events are translated to `TextLayout` operations
/// - **State synchronization**: Keeps `text`, `cursor_position`, and `rt_selection` in sync with `TextLayout`
///
/// # Features
///
/// - Grapheme-aware cursor movement and selection (via `TextLayout`)
/// - Horizontal scrolling when text exceeds width
/// - Blinking caret animation
/// - Mouse selection (single-click, double-click word selection, triple-click line selection)
/// - Clipboard operations (copy, cut, paste)
/// - Undo/redo support
/// - Placeholder text when empty
///
/// # Note
///
/// For multi-line text editing, consider using `TextArea` (future) which will share
/// the same `TextLayout` + `Selection` + `CaretBlink` pattern.
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
    

    // rune-text TextLayout backend for all editing operations.
    rt_layout: Option<RtTextLayout>,

    // rune-text selection model; kept in sync with `cursor_position`.
    // Phase 2: editing respects this selection when non-collapsed.
    rt_selection: RtSelection,

    // Mouse selection state (Phase 3)
    mouse_selecting: bool,
    last_mouse_pos: Option<(f32, f32)>,
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

        // Build a rune-text TextLayout using a system font.
        // Falls back to None if system font loading fails (shouldn't happen in normal use).
        let rt_layout = load_system_default_font()
            .ok()
            .map(|font| RtTextLayout::with_wrap(
                text.clone(),
                &font,
                text_size,
                None,
                RtWrapMode::NoWrap,
            ));

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
            rt_layout,
            rt_selection: RtSelection::collapsed(initial_cursor),
            mouse_selecting: false,
            last_mouse_pos: None,
        }
    }

    /// Clamp selection + cursor so they stay within the current layout/text.
    fn clamp_selection_to_layout(&mut self) {
        let max = if let Some(layout) = self.rt_layout.as_ref() {
            layout.text().len()
        } else {
            self.text.len()
        };

        let anchor = self.rt_selection.anchor().min(max);
        let active = self.rt_selection.active().min(max);
        self.rt_selection = RtSelection::new(anchor, active);
        self.cursor_position = active;
    }

    /// Helper to run a TextLayout edit and synchronize text, cursor, and selection.
    fn with_layout_edit<F>(&mut self, f: F)
    where
        F: FnOnce(
            &mut RtTextLayout,
            &rune_text::FontFace,
            &RtSelection,
            f32,
        ) -> (usize, RtSelection),
    {
        // Capture current selection and size before borrowing layout mutably.
        self.clamp_selection_to_layout();
        let selection_before = self.rt_selection.clone();
        let size = self.text_size;

        let (new_cursor, new_selection, new_text) = {
            let layout = match self.rt_layout.as_mut() {
                Some(layout) => layout,
                None => return, // Should not happen in normal use
            };

            let font = match load_system_default_font() {
                Ok(font) => font,
                Err(_) => return, // Should not happen in normal use
            };

            let (new_cursor, new_selection) = f(layout, &font, &selection_before, size);
            let new_text = layout.text().to_string();
            (new_cursor, new_selection, new_text)
        };

        // Sync authoritative text/cursor/selection state from layout.
        self.text = new_text;
        let max = self.text.len();
        let anchor = new_selection.anchor().min(max);
        let active = new_selection.active().min(max);
        self.rt_selection = RtSelection::new(anchor, active);
        self.cursor_position = new_cursor.min(self.text.len());
        self.reset_cursor_blink();
    }


    /// Compute cursor X (logical pixels) for current cursor position.
    fn cursor_x(&self) -> f32 {
        if let Some(layout) = self.rt_layout.as_ref() {
            let safe_cursor = self.cursor_position.min(layout.text().len());
            let cursor_pos = CursorPosition::new(safe_cursor);
            if let Some(cursor_rect) = layout.cursor_rect_at_position(cursor_pos) {
                return cursor_rect.x;
            }
        }
        // Return 0.0 if layout unavailable (shouldn't happen in normal use)
        0.0
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
        self.with_layout_edit(|layout, font, selection, size| {
            let text = ch.to_string();
            let new_cursor = if selection.is_collapsed() {
                layout.insert_char(
                    selection.active().min(layout.text().len()),
                    ch,
                    font,
                    size,
                    None,
                    RtWrapMode::NoWrap,
                )
            } else {
                layout.replace_selection(
                    selection,
                    &text,
                    font,
                    size,
                    None,
                    RtWrapMode::NoWrap,
                )
            };
            let new_selection = RtSelection::collapsed(new_cursor);
            (new_cursor, new_selection)
        });
    }
    
    /// Delete the character before the cursor (backspace).
    pub fn delete_before_cursor(&mut self) {
        if self.text.is_empty() || self.cursor_position == 0 {
            return;
        }

        self.with_layout_edit(|layout, font, selection, size| {
            let new_cursor = if selection.is_collapsed() {
                layout.delete_backward(
                    selection.active(),
                    font,
                    size,
                    None,
                    RtWrapMode::NoWrap,
                )
            } else {
                layout.delete_selection(
                    selection,
                    font,
                    size,
                    None,
                    RtWrapMode::NoWrap,
                )
            };
            let new_selection = RtSelection::collapsed(new_cursor);
            (new_cursor, new_selection)
        });
    }
    
    /// Delete the character after the cursor (delete key).
    pub fn delete_after_cursor(&mut self) {
        if self.text.is_empty() || self.cursor_position >= self.text.len() {
            return;
        }

        self.with_layout_edit(|layout, font, selection, size| {
            let new_cursor = if selection.is_collapsed() {
                layout.delete_forward(
                    selection.active(),
                    font,
                    size,
                    None,
                    RtWrapMode::NoWrap,
                )
            } else {
                layout.delete_selection(
                    selection,
                    font,
                    size,
                    None,
                    RtWrapMode::NoWrap,
                )
            };
            let new_selection = RtSelection::collapsed(new_cursor);
            (new_cursor, new_selection)
        });
    }
    
    /// Move cursor left by one grapheme.
    /// If there's a selection, collapses it to the start instead of moving.
    pub fn move_cursor_left(&mut self) {
        // If there's a selection, collapse it to the start (anchor or active, whichever is smaller)
        if !self.rt_selection.is_collapsed() {
            let range = self.rt_selection.range();
            self.cursor_position = range.start;
            self.rt_selection = RtSelection::collapsed(self.cursor_position);
            self.reset_cursor_blink();
            return;
        }

        if self.cursor_position == 0 {
            return;
        }

        let new_cursor = {
            let layout = match self.rt_layout.as_ref() {
                Some(layout) => layout,
                None => return,
            };
            let mut pos = self.cursor_position.min(layout.text().len());
            pos = layout.move_cursor_left(pos);
            pos.min(layout.text().len())
        };
        self.cursor_position = new_cursor;
        self.rt_selection = RtSelection::collapsed(self.cursor_position);
        self.reset_cursor_blink();
    }
    
    /// Move cursor right by one grapheme.
    /// If there's a selection, collapses it to the end instead of moving.
    pub fn move_cursor_right(&mut self) {
        // If there's a selection, collapse it to the end (anchor or active, whichever is larger)
        if !self.rt_selection.is_collapsed() {
            let range = self.rt_selection.range();
            self.cursor_position = range.end;
            self.rt_selection = RtSelection::collapsed(self.cursor_position);
            self.reset_cursor_blink();
            return;
        }

        if self.cursor_position >= self.text.len() {
            return;
        }

        let new_cursor = {
            let layout = match self.rt_layout.as_ref() {
                Some(layout) => layout,
                None => return,
            };
            let mut pos = self.cursor_position.min(layout.text().len());
            pos = layout.move_cursor_right(pos);
            pos.min(layout.text().len())
        };
        self.cursor_position = new_cursor;
        self.rt_selection = RtSelection::collapsed(self.cursor_position);
        self.reset_cursor_blink();
    }
    
    /// Move cursor to start of the current line (or text for single-line).
    pub fn move_cursor_line_start(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let active = self.rt_selection.active().min(layout.text().len());
            let new = layout.move_cursor_line_start(active);
            self.cursor_position = new;
            self.rt_selection = RtSelection::collapsed(self.cursor_position);
            self.reset_cursor_blink();
        }
    }

    /// Move cursor to end of the current line (or text for single-line).
    pub fn move_cursor_line_end(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let active = self.rt_selection.active().min(layout.text().len());
            let new = layout.move_cursor_line_end(active);
            self.cursor_position = new;
            self.rt_selection = RtSelection::collapsed(self.cursor_position);
            self.reset_cursor_blink();
        }
    }

    /// Move cursor left by one word boundary.
    pub fn move_cursor_left_word(&mut self) {
        if self.cursor_position == 0 {
            return;
        }

        let new_cursor = {
            let layout = match self.rt_layout.as_ref() {
                Some(layout) => layout,
                None => return,
            };
            let mut pos = self.cursor_position.min(layout.text().len());
            pos = layout.move_cursor_left_word(pos);
            pos.min(layout.text().len())
        };
        self.cursor_position = new_cursor;
        self.rt_selection = RtSelection::collapsed(self.cursor_position);
        self.reset_cursor_blink();
    }

    /// Move cursor right by one word boundary.
    pub fn move_cursor_right_word(&mut self) {
        if self.cursor_position >= self.text.len() {
            return;
        }

        let new_cursor = {
            let layout = match self.rt_layout.as_ref() {
                Some(layout) => layout,
                None => return,
            };
            let mut pos = self.cursor_position.min(layout.text().len());
            pos = layout.move_cursor_right_word(pos);
            pos.min(layout.text().len())
        };
        self.cursor_position = new_cursor;
        self.rt_selection = RtSelection::collapsed(self.cursor_position);
        self.reset_cursor_blink();
    }

    /// Move cursor to start of text (document).
    pub fn move_cursor_to_start(&mut self) {
        self.cursor_position = 0;
        self.scroll_x = 0.0;
        self.rt_selection = RtSelection::collapsed(self.cursor_position);
        self.reset_cursor_blink();
    }
    
    /// Move cursor to end of text (document).
    pub fn move_cursor_to_end(&mut self) {
        self.cursor_position = self.text.len();
        self.rt_selection = RtSelection::collapsed(self.cursor_position);
        self.reset_cursor_blink();
    }

    /// Select all text in the input box.
    pub fn select_all(&mut self) {
        let max = if let Some(layout) = self.rt_layout.as_ref() {
            layout.text().len()
        } else {
            self.text.len()
        };
        self.rt_selection = RtSelection::new(0, max);
        self.cursor_position = max;
        self.reset_cursor_blink();
    }

    /// Extend selection from current anchor to start of the current line.
    pub fn extend_selection_to_line_start(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let active = self.rt_selection.active().min(layout.text().len());
            let new_active = layout.move_cursor_line_start(active);
            
            let max = layout.text().len();
            let anchor = self.rt_selection.anchor().min(max);
            let active = new_active.min(max);

            self.rt_selection = RtSelection::new(anchor, active);
            self.cursor_position = active;
            self.reset_cursor_blink();
        }
    }

    /// Extend selection from current anchor to end of the current line.
    pub fn extend_selection_to_line_end(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let active = self.rt_selection.active().min(layout.text().len());
            let new_active = layout.move_cursor_line_end(active);
            
            let max = layout.text().len();
            let anchor = self.rt_selection.anchor().min(max);
            let active = new_active.min(max);

            self.rt_selection = RtSelection::new(anchor, active);
            self.cursor_position = active;
            self.reset_cursor_blink();
        }
    }

    /// Extend selection left by one grapheme (Shift+Left).
    pub fn extend_selection_left(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let new_selection = layout.extend_selection(&self.rt_selection, |offset| {
                layout.move_cursor_left(offset)
            });
            let max = layout.text().len();
            let anchor = new_selection.anchor().min(max);
            let active = new_selection.active().min(max);
            self.rt_selection = RtSelection::new(anchor, active);
            self.cursor_position = active;
            self.reset_cursor_blink();
        }
    }

    /// Extend selection right by one grapheme (Shift+Right).
    pub fn extend_selection_right(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let new_selection = layout.extend_selection(&self.rt_selection, |offset| {
                layout.move_cursor_right(offset)
            });
            let max = layout.text().len();
            let anchor = new_selection.anchor().min(max);
            let active = new_selection.active().min(max);
            self.rt_selection = RtSelection::new(anchor, active);
            self.cursor_position = active;
            self.reset_cursor_blink();
        }
    }

    /// Extend selection left by one word (Option+Shift+Left on macOS, Alt+Shift+Left on Windows/Linux).
    pub fn extend_selection_left_word(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let new_selection = layout.extend_selection(&self.rt_selection, |offset| {
                layout.move_cursor_left_word(offset)
            });
            let max = layout.text().len();
            let anchor = new_selection.anchor().min(max);
            let active = new_selection.active().min(max);
            self.rt_selection = RtSelection::new(anchor, active);
            self.cursor_position = active;
            self.reset_cursor_blink();
        }
    }

    /// Extend selection right by one word (Option+Shift+Right on macOS, Alt+Shift+Right on Windows/Linux).
    pub fn extend_selection_right_word(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let new_selection = layout.extend_selection(&self.rt_selection, |offset| {
                layout.move_cursor_right_word(offset)
            });
            let max = layout.text().len();
            let anchor = new_selection.anchor().min(max);
            let active = new_selection.active().min(max);
            self.rt_selection = RtSelection::new(anchor, active);
            self.cursor_position = active;
            self.reset_cursor_blink();
        }
    }

    /// Extend selection to start of text (Shift+Home).
    pub fn extend_selection_to_start(&mut self) {
        let max = if let Some(layout) = self.rt_layout.as_ref() {
            layout.text().len()
        } else {
            self.text.len()
        };
        let anchor = self.rt_selection.anchor().min(max);
        let active = 0;

        self.rt_selection = RtSelection::new(anchor, active);
        self.cursor_position = active;
        self.reset_cursor_blink();
    }

    /// Extend selection to end of text (Shift+End).
    pub fn extend_selection_to_end(&mut self) {
        let max = if let Some(layout) = self.rt_layout.as_ref() {
            layout.text().len()
        } else {
            self.text.len()
        };
        let anchor = self.rt_selection.anchor().min(max);
        let active = max;

        self.rt_selection = RtSelection::new(anchor, active);
        self.cursor_position = active;
        self.reset_cursor_blink();
    }

    /// Start a mouse selection at the given screen coordinates.
    ///
    /// This should be called on mouse down inside the input box.
    /// The point should be in screen coordinates; this method will convert
    /// to local text coordinates accounting for viewport transform, scroll, and padding.
    ///
    /// # Arguments
    /// * `screen_x` - Mouse X position in screen coordinates
    /// * `screen_y` - Mouse Y position in screen coordinates
    pub fn start_mouse_selection(&mut self, screen_x: f32, screen_y: f32) {
        // Convert screen coordinates to local text coordinates
        let local_x = screen_x - self.rect.x - self.padding_x + self.scroll_x;
        let _local_y = screen_y - self.rect.y - self.padding_y;

        if let Some(layout) = self.rt_layout.as_ref() {
            // Use TextLayout's hit testing to find the byte offset at this position
            let point = RtPoint::new(local_x, 0.0);
            let byte_offset = layout.hit_test(point, HitTestPolicy::Clamp)
                .map(|hit| hit.byte_offset)
                .unwrap_or(0);
            
            self.rt_selection = RtSelection::collapsed(byte_offset);
            self.cursor_position = byte_offset;
            self.mouse_selecting = true;
            self.last_mouse_pos = Some((screen_x, screen_y));
            self.reset_cursor_blink();
        }
    }

    /// Extend the current mouse selection to the given screen coordinates.
    ///
    /// This should be called on mouse move with button held.
    ///
    /// # Arguments
    /// * `screen_x` - Mouse X position in screen coordinates
    /// * `screen_y` - Mouse Y position in screen coordinates
    pub fn extend_mouse_selection(&mut self, screen_x: f32, screen_y: f32) {
        if !self.mouse_selecting {
            return;
        }

        // Convert screen coordinates to local text coordinates
        let local_x = screen_x - self.rect.x - self.padding_x + self.scroll_x;
        let _local_y = screen_y - self.rect.y - self.padding_y;

        if let Some(layout) = self.rt_layout.as_ref() {
            // Use TextLayout's hit testing to find the byte offset at this position
            let point = RtPoint::new(local_x, 0.0);
            let byte_offset = layout.hit_test(point, HitTestPolicy::Clamp)
                .map(|hit| hit.byte_offset)
                .unwrap_or(0);
            
            // Extend selection from anchor to new active position
            let anchor = self.rt_selection.anchor();
            self.rt_selection = RtSelection::new(anchor, byte_offset);
            self.cursor_position = byte_offset;
            self.last_mouse_pos = Some((screen_x, screen_y));
            self.reset_cursor_blink();
        }
    }

    /// End the current mouse selection.
    ///
    /// This should be called on mouse up.
    pub fn end_mouse_selection(&mut self) {
        self.mouse_selecting = false;
    }

    /// Check if a point is inside the input box.
    ///
    /// # Arguments
    /// * `screen_x` - X position in screen coordinates
    /// * `screen_y` - Y position in screen coordinates
    ///
    /// # Returns
    /// `true` if the point is inside the input box bounds.
    pub fn contains_point(&self, screen_x: f32, screen_y: f32) -> bool {
        screen_x >= self.rect.x
            && screen_x <= self.rect.x + self.rect.w
            && screen_y >= self.rect.y
            && screen_y <= self.rect.y + self.rect.h
    }

    // ========================================================================
    // Phase 5: Clipboard Operations
    // ========================================================================

    /// Copy the selected text to the system clipboard.
    ///
    /// Returns `Ok(())` if successful, or an error if clipboard access fails.
    /// If no text is selected (collapsed selection), this does nothing but succeeds.
    pub fn copy_to_clipboard(&self) -> Result<(), String> {
        let layout = match self.rt_layout.as_ref() {
            Some(layout) => layout,
            None => return Err("TextLayout not available".to_string()),
        };

        layout.copy_to_clipboard(&self.rt_selection)
    }

    /// Cut the selected text to the system clipboard.
    ///
    /// Copies the selection to clipboard and then deletes it.
    /// Returns `Ok(())` if successful, or an error if clipboard access fails.
    pub fn cut_to_clipboard(&mut self) -> Result<(), String> {
        if self.rt_selection.is_collapsed() {
            // Nothing to cut
            return Ok(());
        }

        // Use with_layout_edit to perform the cut operation
        let result = {
            let selection = self.rt_selection.clone();
            let size = self.text_size;

            let layout = match self.rt_layout.as_mut() {
                Some(layout) => layout,
                None => return Err("TextLayout not available".to_string()),
            };

            let font = match load_system_default_font() {
                Ok(font) => font,
                Err(_) => return Err("Failed to load system font".to_string()),
            };

            layout.cut_to_clipboard(&selection, &font, size, None, RtWrapMode::NoWrap)
        };

        match result {
            Ok(new_cursor) => {
                // Sync text and selection after cut
                if let Some(layout) = self.rt_layout.as_ref() {
                    self.text = layout.text().to_string();
                }
                self.cursor_position = new_cursor.min(self.text.len());
                self.rt_selection = RtSelection::collapsed(self.cursor_position);
                self.reset_cursor_blink();
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Paste text from the system clipboard at the cursor position.
    ///
    /// If a selection is active, it will be replaced with the pasted text.
    /// Returns `Ok(())` if successful, or an error if clipboard access fails.
    pub fn paste_from_clipboard(&mut self) -> Result<(), String> {
        let result = {
            let selection = self.rt_selection.clone();
            let size = self.text_size;

            let layout = match self.rt_layout.as_mut() {
                Some(layout) => layout,
                None => return Err("TextLayout not available".to_string()),
            };

            let font = match load_system_default_font() {
                Ok(font) => font,
                Err(_) => return Err("Failed to load system font".to_string()),
            };

            if selection.is_collapsed() {
                layout.paste_from_clipboard(selection.active(), &font, size, None, RtWrapMode::NoWrap)
            } else {
                layout.paste_replace_selection(&selection, &font, size, None, RtWrapMode::NoWrap)
            }
        };

        match result {
            Ok(new_cursor) => {
                // Sync text and selection after paste
                if let Some(layout) = self.rt_layout.as_ref() {
                    self.text = layout.text().to_string();
                }
                self.cursor_position = new_cursor.min(self.text.len());
                self.rt_selection = RtSelection::collapsed(self.cursor_position);
                self.reset_cursor_blink();
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    // ========================================================================
    // Phase 5: Undo/Redo Operations
    // ========================================================================

    /// Undo the last text editing operation.
    ///
    /// Returns `true` if an operation was undone, `false` if there's nothing to undo.
    pub fn undo(&mut self) -> bool {
        let result = {
            let selection = self.rt_selection.clone();
            let size = self.text_size;

            let layout = match self.rt_layout.as_mut() {
                Some(layout) => layout,
                None => return false,
            };

            let font = match load_system_default_font() {
                Ok(font) => font,
                Err(_) => return false,
            };

            layout.undo(&selection, &font, size, None, RtWrapMode::NoWrap)
        };

        if let Some((new_cursor, new_selection)) = result {
            // Sync text, cursor, and selection after undo
            if let Some(layout) = self.rt_layout.as_ref() {
                self.text = layout.text().to_string();
            }
            let max = self.text.len();
            let anchor = new_selection.anchor().min(max);
            let active = new_selection.active().min(max);
            self.rt_selection = RtSelection::new(anchor, active);
            self.cursor_position = new_cursor.min(max);
            self.reset_cursor_blink();
            true
        } else {
            false
        }
    }

    /// Redo the last undone operation.
    ///
    /// Returns `true` if an operation was redone, `false` if there's nothing to redo.
    pub fn redo(&mut self) -> bool {
        let result = {
            let selection = self.rt_selection.clone();
            let size = self.text_size;

            let layout = match self.rt_layout.as_mut() {
                Some(layout) => layout,
                None => return false,
            };

            let font = match load_system_default_font() {
                Ok(font) => font,
                Err(_) => return false,
            };

            layout.redo(&selection, &font, size, None, RtWrapMode::NoWrap)
        };

        if let Some((new_cursor, new_selection)) = result {
            // Sync text, cursor, and selection after redo
            if let Some(layout) = self.rt_layout.as_ref() {
                self.text = layout.text().to_string();
            }
            let max = self.text.len();
            let anchor = new_selection.anchor().min(max);
            let active = new_selection.active().min(max);
            self.rt_selection = RtSelection::new(anchor, active);
            self.cursor_position = new_cursor.min(max);
            self.reset_cursor_blink();
            true
        } else {
            false
        }
    }

    // ========================================================================
    // Phase 5: Word/Line Selection (Mouse Gestures)
    // ========================================================================

    /// Start a word selection at the given screen coordinates (double-click).
    ///
    /// This should be called on double-click inside the input box.
    ///
    /// # Arguments
    /// * `screen_x` - Mouse X position in screen coordinates
    /// * `screen_y` - Mouse Y position in screen coordinates
    pub fn start_word_selection(&mut self, screen_x: f32, screen_y: f32) {
        // Convert screen coordinates to local text coordinates
        let local_x = screen_x - self.rect.x - self.padding_x + self.scroll_x;
        let _local_y = screen_y - self.rect.y - self.padding_y;

        if let Some(layout) = self.rt_layout.as_ref() {
            let point = RtPoint::new(local_x, 0.0);
            if let Some(selection) = layout.start_word_selection(point) {
                self.rt_selection = selection;
                self.cursor_position = selection.active();
                self.mouse_selecting = true;
                self.last_mouse_pos = Some((screen_x, screen_y));
                self.reset_cursor_blink();
            }
        }
    }

    /// Extend the current word selection to the given screen coordinates.
    ///
    /// This should be called on mouse move after double-click with button held.
    ///
    /// # Arguments
    /// * `screen_x` - Mouse X position in screen coordinates
    /// * `screen_y` - Mouse Y position in screen coordinates
    pub fn extend_word_selection(&mut self, screen_x: f32, screen_y: f32) {
        if !self.mouse_selecting {
            return;
        }

        // Convert screen coordinates to local text coordinates
        let local_x = screen_x - self.rect.x - self.padding_x + self.scroll_x;
        let _local_y = screen_y - self.rect.y - self.padding_y;

        if let Some(layout) = self.rt_layout.as_ref() {
            let point = RtPoint::new(local_x, 0.0);
            let new_selection = layout.extend_word_selection(&self.rt_selection, point);
            self.rt_selection = new_selection;
            self.cursor_position = new_selection.active();
            self.last_mouse_pos = Some((screen_x, screen_y));
            self.reset_cursor_blink();
        }
    }

    /// Start a line selection at the given screen coordinates (triple-click).
    ///
    /// This should be called on triple-click inside the input box.
    ///
    /// # Arguments
    /// * `screen_x` - Mouse X position in screen coordinates
    /// * `screen_y` - Mouse Y position in screen coordinates
    pub fn start_line_selection(&mut self, screen_x: f32, screen_y: f32) {
        // Convert screen coordinates to local text coordinates
        let local_x = screen_x - self.rect.x - self.padding_x + self.scroll_x;
        let _local_y = screen_y - self.rect.y - self.padding_y;

        if let Some(layout) = self.rt_layout.as_ref() {
            let point = RtPoint::new(local_x, 0.0);
            if let Some(selection) = layout.start_line_selection(point) {
                self.rt_selection = selection;
                self.cursor_position = selection.active();
                self.mouse_selecting = true;
                self.last_mouse_pos = Some((screen_x, screen_y));
                self.reset_cursor_blink();
            }
        }
    }

    /// Extend the current line selection to the given screen coordinates.
    ///
    /// This should be called on mouse move after triple-click with button held.
    ///
    /// # Arguments
    /// * `screen_x` - Mouse X position in screen coordinates
    /// * `screen_y` - Mouse Y position in screen coordinates
    pub fn extend_line_selection(&mut self, screen_x: f32, screen_y: f32) {
        if !self.mouse_selecting {
            return;
        }

        // Convert screen coordinates to local text coordinates
        let local_x = screen_x - self.rect.x - self.padding_x + self.scroll_x;
        let _local_y = screen_y - self.rect.y - self.padding_y;

        if let Some(layout) = self.rt_layout.as_ref() {
            let point = RtPoint::new(local_x, 0.0);
            let new_selection = layout.extend_line_selection(&self.rt_selection, point);
            self.rt_selection = new_selection;
            self.cursor_position = new_selection.active();
            self.last_mouse_pos = Some((screen_x, screen_y));
            self.reset_cursor_blink();
        }
    }
    
    /// Update scroll position based on cursor and text metrics.
    pub fn update_scroll(&mut self) {
        if self.text.is_empty() {
            self.scroll_x = 0.0;
            return;
        }
        let cursor_x = self.cursor_x();

        let content_width = self.rect.w - self.padding_x * 2.0;
        let margin = 10.0;

        // Get total text width from TextLayout
        let total_width = if let Some(layout) = self.rt_layout.as_ref() {
            layout.max_line_width()
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

        // Update scroll to keep cursor visible and handle shrinking text.
        self.update_scroll();
        
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
            // Draw selection highlight before text using shared renderer
            if self.focused && !self.rt_selection.is_collapsed() {
                if let Some(layout) = self.rt_layout.as_ref() {
                    let selection_config = SelectionRenderConfig {
                        content_rect,
                        text_baseline_y: text_y,
                        scroll_x: self.scroll_x,
                        scroll_y: 0.0, // No vertical scroll for InputBox
                        color: Color::rgba(63, 130, 246, 80),
                        z: z + 2,
                    };
                    
                    selection_renderer::render_selection(
                        canvas,
                        layout,
                        &self.rt_selection,
                        &selection_config,
                    );
                }
            }

            // Render text using draw_text_direct which respects clipping better
            canvas.draw_text_direct(
                [text_x, text_y],
                &self.text,
                self.text_size,
                self.text_color,
                provider,
            );
            
            // Render cursor using shared caret renderer
            if self.focused {
                if let Some(layout) = self.rt_layout.as_ref() {
                    let caret_config = CaretRenderConfig {
                        content_rect,
                        scroll_x: self.scroll_x,
                        scroll_y: 0.0, // No vertical scroll for InputBox
                        color: Color::rgba(63, 130, 246, 255),
                        width: 1.5,
                        z: z + 3,
                    };
                    
                    caret_renderer::render_caret(
                        canvas,
                        layout,
                        self.cursor_position,
                        &self.caret,
                        &caret_config,
                    );
                }
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
            
            // Render cursor at start if focused, even without placeholder
            // For empty text, we need to manually render the caret since there's no layout
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
