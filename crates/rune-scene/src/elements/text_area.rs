use crate::elements::caret::CaretBlink;
use crate::elements::caret_renderer::{self, CaretRenderConfig};
use crate::elements::selection_renderer::{self, SelectionRenderConfig};
use engine_core::{
    Brush, Color, ColorLinPremul, FillRule, Path, PathCmd, Rect, RoundedRadii, RoundedRect,
};
use rune_surface::Canvas;
use rune_surface::shapes;
use rune_text::layout::{
    CursorPosition, HitTestPolicy, Point as RtPoint, Selection as RtSelection,
    TextLayout as RtTextLayout, WrapMode as RtWrapMode,
};
// NOTE: Line height appears to be doubled somewhere in the rendering pipeline,
// so we use 0.7 to compensate and achieve normal single-line spacing.
// TODO: Investigate why line height is being applied 2x (possibly DPI-related)
const DEFAULT_LINE_HEIGHT_FACTOR: f32 = 0.7;
const MIN_LINE_HEIGHT_FACTOR: f32 = 0.25;
use rune_text::font::load_system_default_font;

/// Multi-line text area widget with rich editing capabilities.
pub struct TextArea {
    pub rect: Rect,
    pub text: String,
    pub text_size: f32,
    pub text_color: ColorLinPremul,
    pub placeholder: Option<String>,
    pub focused: bool,
    caret: CaretBlink,
    pub cursor_position: usize,
    scroll_y: f32,
    padding_x: f32,
    padding_y: f32,
    min_height: Option<f32>,
    max_height: Option<f32>,
    rt_layout: Option<RtTextLayout>,
    rt_selection: RtSelection,
    mouse_selecting: bool,
    last_mouse_pos: Option<(f32, f32)>,
    line_height_factor: f32,
    wrap_width: Option<f32>,
    preferred_x: Option<f32>,
}

impl TextArea {
    pub fn new(
        rect: Rect,
        text: String,
        text_size: f32,
        text_color: ColorLinPremul,
        placeholder: Option<String>,
        focused: bool,
    ) -> Self {
        let initial_cursor = if focused { text.len() } else { 0 };
        let padding_x = 8.0;
        let wrap_width = (rect.w - padding_x * 2.0).max(10.0);
        // Build TextLayout with word wrap
        let line_height_factor = DEFAULT_LINE_HEIGHT_FACTOR;
        // Use font size directly as line height for tight spacing
        let desired_line_height = text_size * line_height_factor;
        let rt_layout = load_system_default_font().ok().map(|font| {
            RtTextLayout::with_wrap_and_line_height(
                text.clone(),
                &font,
                text_size,
                Some(wrap_width),
                RtWrapMode::BreakWord,
                desired_line_height,
            )
        });

        Self {
            rect,
            text,
            text_size,
            text_color,
            placeholder,
            focused,
            caret: CaretBlink::new(focused),
            cursor_position: initial_cursor,
            scroll_y: 0.0,
            padding_x,
            padding_y: 8.0,
            min_height: Some(60.0),
            max_height: None,
            rt_layout,
            rt_selection: RtSelection::collapsed(initial_cursor),
            mouse_selecting: false,
            last_mouse_pos: None,
            line_height_factor,
            wrap_width: Some(wrap_width),
            preferred_x: None,
        }
    }

    pub fn set_rect(&mut self, rect: Rect) {
        let old_wrap_width = self.wrap_width;
        let new_wrap_width = (rect.w - self.padding_x * 2.0).max(10.0);
        self.rect = rect;
        self.wrap_width = Some(new_wrap_width);
        if old_wrap_width != Some(new_wrap_width) {
            self.rewrap_layout();
        }
    }

    fn rewrap_layout(&mut self) {
        let desired_line_height = self.desired_line_height();
        if let (Some(layout), Some(wrap_width)) = (self.rt_layout.as_mut(), self.wrap_width) {
            if let Ok(font) = load_system_default_font() {
                *layout = RtTextLayout::with_wrap_and_line_height(
                    self.text.clone(),
                    &font,
                    self.text_size,
                    Some(wrap_width),
                    RtWrapMode::BreakWord,
                    desired_line_height,
                );
            }
        }
    }

    fn desired_line_height(&self) -> f32 {
        self.text_size * self.line_height_factor
    }

    pub fn set_line_height_factor(&mut self, factor: f32) {
        let clamped = factor.max(MIN_LINE_HEIGHT_FACTOR);
        if (self.line_height_factor - clamped).abs() < 1e-4 {
            return;
        }
        self.line_height_factor = clamped;
        self.rewrap_layout();
    }

    pub fn calculate_content_height(&self) -> f32 {
        let content_height = if let Some(layout) = self.rt_layout.as_ref() {
            layout.total_height()
        } else {
            self.text_size * self.line_height_factor
        };
        let mut height = content_height + self.padding_y * 2.0;
        if let Some(min) = self.min_height {
            height = height.max(min);
        }
        if let Some(max) = self.max_height {
            height = height.min(max);
        }
        height
    }

    pub fn update_scroll(&mut self) {
        let layout = match self.rt_layout.as_ref() {
            Some(l) => l,
            None => return,
        };

        // If the content fits entirely, no vertical scroll is needed.
        let content_height = self.rect.h - self.padding_y * 2.0;
        if content_height <= 0.0 {
            self.scroll_y = 0.0;
            return;
        }

        let max_scroll = (layout.total_height() - content_height).max(0.0);

        let cursor_pos = CursorPosition::new(self.cursor_position.min(layout.text().len()));
        let cursor_rect = match layout.cursor_rect_at_position(cursor_pos) {
            Some(r) => r,
            None => return,
        };

        let margin = 10.0;
        let cursor_top = cursor_rect.y;
        let cursor_bottom = cursor_rect.y + cursor_rect.height;

        // Current visible viewport in layout coordinates.
        let viewport_top = self.scroll_y;
        let viewport_bottom = self.scroll_y + content_height;

        let mut new_scroll = self.scroll_y;

        // Scroll down if the caret goes below the bottom margin.
        if cursor_bottom + margin > viewport_bottom {
            new_scroll = (cursor_bottom + margin - content_height).min(max_scroll);
        }
        // Scroll up if the caret goes above the top margin.
        else if cursor_top - margin < viewport_top {
            new_scroll = (cursor_top - margin).max(0.0);
        }

        self.scroll_y = new_scroll.clamp(0.0, max_scroll);
    }

    pub fn update_blink(&mut self, delta_time: f32) {
        self.caret.update(delta_time, self.focused);
    }

    fn reset_cursor_blink(&mut self) {
        self.caret.reset_manual();
    }

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

    fn with_layout_edit<F>(&mut self, f: F)
    where
        F: FnOnce(
            &mut RtTextLayout,
            &rune_text::FontFace,
            &RtSelection,
            f32,
        ) -> (usize, RtSelection),
    {
        self.clamp_selection_to_layout();
        let selection_before = self.rt_selection.clone();
        let size = self.text_size;
        let (new_cursor, new_selection, new_text) = {
            let layout = match self.rt_layout.as_mut() {
                Some(l) => l,
                None => return,
            };
            let font = match load_system_default_font() {
                Ok(f) => f,
                Err(_) => return,
            };
            let (new_cursor, new_selection) = f(layout, &font, &selection_before, size);
            let new_text = layout.text().to_string();
            (new_cursor, new_selection, new_text)
        };
        self.text = new_text;
        // Recreate layout with new text to ensure proper wrapping
        self.rewrap_layout();
        let max = self.text.len();
        let anchor = new_selection.anchor().min(max);
        let active = new_selection.active().min(max);
        self.rt_selection = RtSelection::new(anchor, active);
        self.cursor_position = new_cursor.min(self.text.len());
        self.reset_cursor_blink();
    }

    pub fn insert_char(&mut self, ch: char) {
        self.with_layout_edit(|layout, font, selection, size| {
            let wrap_width = layout.max_line_width().max(100.0);
            let text = ch.to_string();
            let new_cursor = if selection.is_collapsed() {
                layout.insert_char(
                    selection.active().min(layout.text().len()),
                    ch,
                    font,
                    size,
                    Some(wrap_width),
                    RtWrapMode::BreakWord,
                )
            } else {
                layout.replace_selection(
                    selection,
                    &text,
                    font,
                    size,
                    Some(wrap_width),
                    RtWrapMode::BreakWord,
                )
            };
            (new_cursor, RtSelection::collapsed(new_cursor))
        });
        // Text edits change the visual X position; reset preferred column
        // so the next vertical movement recomputes it from the new offset.
        self.preferred_x = None;
    }

    pub fn delete_before_cursor(&mut self) {
        if self.text.is_empty() || self.cursor_position == 0 {
            return;
        }
        self.with_layout_edit(|layout, font, selection, size| {
            let wrap_width = layout.max_line_width().max(100.0);
            let new_cursor = if selection.is_collapsed() {
                layout.delete_backward(
                    selection.active(),
                    font,
                    size,
                    Some(wrap_width),
                    RtWrapMode::BreakWord,
                )
            } else {
                layout.delete_selection(
                    selection,
                    font,
                    size,
                    Some(wrap_width),
                    RtWrapMode::BreakWord,
                )
            };
            (new_cursor, RtSelection::collapsed(new_cursor))
        });
        // Horizontal position may have changed after delete; recompute on next up/down.
        self.preferred_x = None;
    }

    pub fn delete_after_cursor(&mut self) {
        if self.text.is_empty() || self.cursor_position >= self.text.len() {
            return;
        }
        self.with_layout_edit(|layout, font, selection, size| {
            let wrap_width = layout.max_line_width().max(100.0);
            let new_cursor = if selection.is_collapsed() {
                layout.delete_forward(
                    selection.active(),
                    font,
                    size,
                    Some(wrap_width),
                    RtWrapMode::BreakWord,
                )
            } else {
                layout.delete_selection(
                    selection,
                    font,
                    size,
                    Some(wrap_width),
                    RtWrapMode::BreakWord,
                )
            };
            (new_cursor, RtSelection::collapsed(new_cursor))
        });
        // Horizontal position may have changed after delete; recompute on next up/down.
        self.preferred_x = None;
    }

    pub fn move_cursor_left(&mut self) {
        // If there's a selection, collapse it to the start instead of moving
        if !self.rt_selection.is_collapsed() {
            let range = self.rt_selection.range();
            self.cursor_position = range.start;
            self.rt_selection = RtSelection::collapsed(self.cursor_position);
            self.preferred_x = None; // Reset preferred X on horizontal movement
            self.reset_cursor_blink();
            return;
        }
        if self.cursor_position == 0 {
            return;
        }
        let new_cursor = {
            let layout = match self.rt_layout.as_ref() {
                Some(l) => l,
                None => return,
            };
            let mut pos = self.cursor_position.min(layout.text().len());
            pos = layout.move_cursor_left(pos);
            pos.min(layout.text().len())
        };
        self.cursor_position = new_cursor;
        self.preferred_x = None; // Reset preferred X on horizontal movement
        self.rt_selection = RtSelection::collapsed(self.cursor_position);
        self.reset_cursor_blink();
    }

    pub fn move_cursor_right(&mut self) {
        // If there's a selection, collapse it to the end instead of moving
        if !self.rt_selection.is_collapsed() {
            let range = self.rt_selection.range();
            self.cursor_position = range.end;
            self.rt_selection = RtSelection::collapsed(self.cursor_position);
            self.preferred_x = None; // Reset preferred X on horizontal movement
            self.reset_cursor_blink();
            return;
        }
        if self.cursor_position >= self.text.len() {
            return;
        }
        let new_cursor = {
            let layout = match self.rt_layout.as_ref() {
                Some(l) => l,
                None => return,
            };
            let mut pos = self.cursor_position.min(layout.text().len());
            pos = layout.move_cursor_right(pos);
            pos.min(layout.text().len())
        };
        self.cursor_position = new_cursor;
        self.preferred_x = None; // Reset preferred X on horizontal movement
        self.rt_selection = RtSelection::collapsed(self.cursor_position);
        self.reset_cursor_blink();
    }

    pub fn move_cursor_up(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let pos = self.cursor_position.min(layout.text().len());
            let (new_pos, new_x) = layout.move_cursor_up(pos, self.preferred_x);
            self.cursor_position = new_pos;
            self.preferred_x = Some(new_x);
            self.rt_selection = RtSelection::collapsed(self.cursor_position);
            self.reset_cursor_blink();
        }
    }

    pub fn move_cursor_down(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let pos = self.cursor_position.min(layout.text().len());
            let (new_pos, new_x) = layout.move_cursor_down(pos, self.preferred_x);
            self.cursor_position = new_pos;
            self.preferred_x = Some(new_x);
            self.rt_selection = RtSelection::collapsed(self.cursor_position);
            self.reset_cursor_blink();
        }
    }

    pub fn move_cursor_line_start(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let active = self.rt_selection.active().min(layout.text().len());
            let new = layout.move_cursor_line_start(active);
            self.cursor_position = new;
            self.preferred_x = None;
            self.rt_selection = RtSelection::collapsed(self.cursor_position);
            self.reset_cursor_blink();
        }
    }

    pub fn move_cursor_line_end(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let active = self.rt_selection.active().min(layout.text().len());
            let new = layout.move_cursor_line_end(active);
            self.cursor_position = new;
            self.preferred_x = None;
            self.rt_selection = RtSelection::collapsed(self.cursor_position);
            self.reset_cursor_blink();
        }
    }

    pub fn move_cursor_to_document_start(&mut self) {
        self.cursor_position = 0;
        self.scroll_y = 0.0;
        self.preferred_x = None;
        self.rt_selection = RtSelection::collapsed(0);
        self.reset_cursor_blink();
    }

    pub fn move_cursor_to_document_end(&mut self) {
        self.cursor_position = self.text.len();
        self.preferred_x = None;
        self.rt_selection = RtSelection::collapsed(self.cursor_position);
        self.reset_cursor_blink();
        self.update_scroll();
    }

    pub fn move_cursor_left_word(&mut self) {
        if self.cursor_position == 0 {
            return;
        }
        let new_cursor = {
            let layout = match self.rt_layout.as_ref() {
                Some(l) => l,
                None => return,
            };
            let mut pos = self.cursor_position.min(layout.text().len());
            pos = layout.move_cursor_left_word(pos);
            pos.min(layout.text().len())
        };
        self.cursor_position = new_cursor;
        self.preferred_x = None;
        self.rt_selection = RtSelection::collapsed(self.cursor_position);
        self.reset_cursor_blink();
    }

    pub fn move_cursor_right_word(&mut self) {
        if self.cursor_position >= self.text.len() {
            return;
        }
        let new_cursor = {
            let layout = match self.rt_layout.as_ref() {
                Some(l) => l,
                None => return,
            };
            let mut pos = self.cursor_position.min(layout.text().len());
            pos = layout.move_cursor_right_word(pos);
            pos.min(layout.text().len())
        };
        self.cursor_position = new_cursor;
        self.preferred_x = None;
        self.rt_selection = RtSelection::collapsed(self.cursor_position);
        self.reset_cursor_blink();
    }

    pub fn select_all(&mut self) {
        let max = if let Some(layout) = self.rt_layout.as_ref() {
            layout.text().len()
        } else {
            self.text.len()
        };
        self.rt_selection = RtSelection::new(0, max);
        self.cursor_position = max;
        self.preferred_x = None;
        self.reset_cursor_blink();
    }

    pub fn extend_selection_left(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let new_selection = layout
                .extend_selection(&self.rt_selection, |offset| layout.move_cursor_left(offset));
            let max = layout.text().len();
            let anchor = new_selection.anchor().min(max);
            let active = new_selection.active().min(max);
            self.rt_selection = RtSelection::new(anchor, active);
            self.cursor_position = active;
            // Horizontal selection movement changes the active column.
            self.preferred_x = None;
            self.reset_cursor_blink();
        }
    }

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
            // Horizontal selection movement changes the active column.
            self.preferred_x = None;
            self.reset_cursor_blink();
        }
    }

    pub fn extend_selection_up(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            // Use vertical selection helper so we track a stable X column
            // while extending the selection across multiple lines.
            let (new_selection, new_x) = layout.extend_selection_vertical(
                &self.rt_selection,
                |offset, x| layout.move_cursor_up(offset, x),
                self.preferred_x,
            );
            self.rt_selection = new_selection;
            self.cursor_position = new_selection.active();
            self.preferred_x = Some(new_x);
            self.reset_cursor_blink();
        }
    }

    pub fn extend_selection_down(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            // Use vertical selection helper so we track a stable X column
            // while extending the selection across multiple lines.
            let (new_selection, new_x) = layout.extend_selection_vertical(
                &self.rt_selection,
                |offset, x| layout.move_cursor_down(offset, x),
                self.preferred_x,
            );
            self.rt_selection = new_selection;
            self.cursor_position = new_selection.active();
            self.preferred_x = Some(new_x);
            self.reset_cursor_blink();
        }
    }

    pub fn extend_selection_to_line_start(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let active = self.rt_selection.active().min(layout.text().len());
            let new_active = layout.move_cursor_line_start(active);
            let max = layout.text().len();
            let anchor = self.rt_selection.anchor().min(max);
            let active = new_active.min(max);
            self.rt_selection = RtSelection::new(anchor, active);
            self.cursor_position = active;
            self.preferred_x = None;
            self.reset_cursor_blink();
        }
    }

    pub fn extend_selection_to_line_end(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let active = self.rt_selection.active().min(layout.text().len());
            let new_active = layout.move_cursor_line_end(active);
            let max = layout.text().len();
            let anchor = self.rt_selection.anchor().min(max);
            let active = new_active.min(max);
            self.rt_selection = RtSelection::new(anchor, active);
            self.cursor_position = active;
            self.preferred_x = None;
            self.reset_cursor_blink();
        }
    }

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

    pub fn extend_selection_to_document_start(&mut self) {
        let max = if let Some(layout) = self.rt_layout.as_ref() {
            layout.text().len()
        } else {
            self.text.len()
        };
        let anchor = self.rt_selection.anchor().min(max);
        self.rt_selection = RtSelection::new(anchor, 0);
        self.cursor_position = 0;
        self.preferred_x = None;
        self.reset_cursor_blink();
    }

    pub fn extend_selection_to_document_end(&mut self) {
        let max = if let Some(layout) = self.rt_layout.as_ref() {
            layout.text().len()
        } else {
            self.text.len()
        };
        let anchor = self.rt_selection.anchor().min(max);
        self.rt_selection = RtSelection::new(anchor, max);
        self.cursor_position = max;
        self.preferred_x = None;
        self.reset_cursor_blink();
    }

    pub fn start_mouse_selection(&mut self, screen_x: f32, screen_y: f32) {
        let local_x = screen_x - self.rect.x - self.padding_x;
        let local_y = screen_y - self.rect.y - self.padding_y + self.scroll_y;
        if let Some(layout) = self.rt_layout.as_ref() {
            let point = RtPoint::new(local_x, local_y);
            let byte_offset = layout
                .hit_test(point, HitTestPolicy::Clamp)
                .map(|hit| hit.byte_offset)
                .unwrap_or(0);
            self.rt_selection = RtSelection::collapsed(byte_offset);
            self.cursor_position = byte_offset;
            self.preferred_x = None;
            self.mouse_selecting = true;
            self.last_mouse_pos = Some((screen_x, screen_y));
            self.reset_cursor_blink();
        }
    }

    pub fn extend_mouse_selection(&mut self, screen_x: f32, screen_y: f32) {
        if !self.mouse_selecting {
            return;
        }
        let local_x = screen_x - self.rect.x - self.padding_x;
        let local_y = screen_y - self.rect.y - self.padding_y + self.scroll_y;
        if let Some(layout) = self.rt_layout.as_ref() {
            let point = RtPoint::new(local_x, local_y);
            let byte_offset = layout
                .hit_test(point, HitTestPolicy::Clamp)
                .map(|hit| hit.byte_offset)
                .unwrap_or(0);
            let anchor = self.rt_selection.anchor();
            self.rt_selection = RtSelection::new(anchor, byte_offset);
            self.cursor_position = byte_offset;
            self.preferred_x = None;
            self.last_mouse_pos = Some((screen_x, screen_y));
            self.reset_cursor_blink();
        }
    }

    pub fn end_mouse_selection(&mut self) {
        self.mouse_selecting = false;
    }

    pub fn contains_point(&self, screen_x: f32, screen_y: f32) -> bool {
        screen_x >= self.rect.x
            && screen_x <= self.rect.x + self.rect.w
            && screen_y >= self.rect.y
            && screen_y <= self.rect.y + self.rect.h
    }

    pub fn start_word_selection(&mut self, screen_x: f32, screen_y: f32) {
        let local_x = screen_x - self.rect.x - self.padding_x;
        let local_y = screen_y - self.rect.y - self.padding_y + self.scroll_y;
        if let Some(layout) = self.rt_layout.as_ref() {
            let point = RtPoint::new(local_x, local_y);
            if let Some(selection) = layout.start_word_selection(point) {
                self.rt_selection = selection;
                self.cursor_position = selection.active();
                self.preferred_x = None;
                self.mouse_selecting = true;
                self.last_mouse_pos = Some((screen_x, screen_y));
                self.reset_cursor_blink();
            }
        }
    }

    pub fn extend_word_selection(&mut self, screen_x: f32, screen_y: f32) {
        if !self.mouse_selecting {
            return;
        }
        let local_x = screen_x - self.rect.x - self.padding_x;
        let local_y = screen_y - self.rect.y - self.padding_y + self.scroll_y;
        if let Some(layout) = self.rt_layout.as_ref() {
            let point = RtPoint::new(local_x, local_y);
            let new_selection = layout.extend_word_selection(&self.rt_selection, point);
            self.rt_selection = new_selection;
            self.cursor_position = new_selection.active();
            self.preferred_x = None;
            self.last_mouse_pos = Some((screen_x, screen_y));
            self.reset_cursor_blink();
        }
    }

    pub fn start_line_selection(&mut self, screen_x: f32, screen_y: f32) {
        let local_x = screen_x - self.rect.x - self.padding_x;
        let local_y = screen_y - self.rect.y - self.padding_y + self.scroll_y;
        if let Some(layout) = self.rt_layout.as_ref() {
            let point = RtPoint::new(local_x, local_y);
            if let Some(selection) = layout.start_line_selection(point) {
                self.rt_selection = selection;
                self.cursor_position = selection.active();
                self.preferred_x = None;
                self.mouse_selecting = true;
                self.last_mouse_pos = Some((screen_x, screen_y));
                self.reset_cursor_blink();
            }
        }
    }

    pub fn extend_line_selection(&mut self, screen_x: f32, screen_y: f32) {
        if !self.mouse_selecting {
            return;
        }
        let local_x = screen_x - self.rect.x - self.padding_x;
        let local_y = screen_y - self.rect.y - self.padding_y + self.scroll_y;
        if let Some(layout) = self.rt_layout.as_ref() {
            let point = RtPoint::new(local_x, local_y);
            let new_selection = layout.extend_line_selection(&self.rt_selection, point);
            self.rt_selection = new_selection;
            self.cursor_position = new_selection.active();
            self.preferred_x = None;
            self.last_mouse_pos = Some((screen_x, screen_y));
            self.reset_cursor_blink();
        }
    }

    pub fn copy_to_clipboard(&self) -> Result<(), String> {
        let layout = match self.rt_layout.as_ref() {
            Some(layout) => layout,
            None => return Err("TextLayout not available".to_string()),
        };
        layout.copy_to_clipboard(&self.rt_selection)
    }

    pub fn cut_to_clipboard(&mut self) -> Result<(), String> {
        if self.rt_selection.is_collapsed() {
            return Ok(());
        }
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
            let wrap_width = self.wrap_width.unwrap_or(100.0);
            layout.cut_to_clipboard(
                &selection,
                &font,
                size,
                Some(wrap_width),
                RtWrapMode::BreakWord,
            )
        };
        match result {
            Ok(new_cursor) => {
                if let Some(layout) = self.rt_layout.as_ref() {
                    self.text = layout.text().to_string();
                }
                self.cursor_position = new_cursor.min(self.text.len());
                self.rt_selection = RtSelection::collapsed(self.cursor_position);
                self.preferred_x = None;
                self.reset_cursor_blink();
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

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
            let wrap_width = self.wrap_width.unwrap_or(100.0);
            if selection.is_collapsed() {
                layout.paste_from_clipboard(
                    selection.active(),
                    &font,
                    size,
                    Some(wrap_width),
                    RtWrapMode::BreakWord,
                )
            } else {
                layout.paste_replace_selection(
                    &selection,
                    &font,
                    size,
                    Some(wrap_width),
                    RtWrapMode::BreakWord,
                )
            }
        };
        match result {
            Ok(new_cursor) => {
                if let Some(layout) = self.rt_layout.as_ref() {
                    self.text = layout.text().to_string();
                }
                self.cursor_position = new_cursor.min(self.text.len());
                self.rt_selection = RtSelection::collapsed(self.cursor_position);
                self.preferred_x = None;
                self.reset_cursor_blink();
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

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
            let wrap_width = self.wrap_width.unwrap_or(100.0);
            layout.undo(
                &selection,
                &font,
                size,
                Some(wrap_width),
                RtWrapMode::BreakWord,
            )
        };
        if let Some((new_cursor, new_selection)) = result {
            if let Some(layout) = self.rt_layout.as_ref() {
                self.text = layout.text().to_string();
            }
            let max = self.text.len();
            let anchor = new_selection.anchor().min(max);
            let active = new_selection.active().min(max);
            self.rt_selection = RtSelection::new(anchor, active);
            self.cursor_position = new_cursor.min(max);
            self.preferred_x = None;
            self.reset_cursor_blink();
            true
        } else {
            false
        }
    }

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
            let wrap_width = self.wrap_width.unwrap_or(100.0);
            layout.redo(
                &selection,
                &font,
                size,
                Some(wrap_width),
                RtWrapMode::BreakWord,
            )
        };
        if let Some((new_cursor, new_selection)) = result {
            if let Some(layout) = self.rt_layout.as_ref() {
                self.text = layout.text().to_string();
            }
            let max = self.text.len();
            let anchor = new_selection.anchor().min(max);
            let active = new_selection.active().min(max);
            self.rt_selection = RtSelection::new(anchor, active);
            self.cursor_position = new_cursor.min(max);
            self.preferred_x = None;
            self.reset_cursor_blink();
            true
        } else {
            false
        }
    }

    pub fn render(
        &mut self,
        canvas: &mut Canvas,
        z: i32,
        provider: &dyn engine_core::TextProvider,
    ) {
        let radius = 6.0;
        let rrect = RoundedRect {
            rect: self.rect,
            radii: RoundedRadii {
                tl: radius,
                tr: radius,
                br: radius,
                bl: radius,
            },
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

        // Update scroll before rendering
        self.update_scroll();

        // Calculate content area
        let content_x = self.rect.x + self.padding_x;
        let content_y = self.rect.y + self.padding_y;
        let content_width = self.rect.w - self.padding_x * 2.0;
        let content_height = self.rect.h - self.padding_y * 2.0;

        let content_rect = Rect {
            x: content_x,
            y: content_y,
            w: content_width,
            h: content_height,
        };
        canvas.push_clip_rect(content_rect);

        if !self.text.is_empty() {
            // Render selection using shared module
            if self.focused && !self.rt_selection.is_collapsed() {
                if let Some(layout) = self.rt_layout.as_ref() {
                    // Get baseline offset from first line for coordinate transformation
                    let baseline_offset = if let Some(line) = layout.lines().first() {
                        line.baseline_offset
                    } else {
                        self.text_size * 0.8
                    };

                    let selection_config = SelectionRenderConfig {
                        content_rect,
                        text_baseline_y: content_y + baseline_offset,
                        scroll_x: 0.0,
                        scroll_y: self.scroll_y,
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

            // Render text line by line
            if let Some(layout) = self.rt_layout.as_ref() {
                for line in layout.lines() {
                    let line_text = &self.text[line.text_range.clone()];
                    let text_x = content_x;
                    // Text baseline Y position: content top + line's y_offset (top of line box) + baseline_offset - scroll
                    let text_baseline_y =
                        content_y + line.y_offset + line.baseline_offset - self.scroll_y;

                    // Only render lines that are visible (check against line box bounds)
                    let line_top_y = content_y + line.y_offset - self.scroll_y;
                    let line_bottom_y = line_top_y + line.height;

                    if line_bottom_y >= content_y && line_top_y <= content_y + content_height {
                        canvas.draw_text_direct(
                            [text_x, text_baseline_y],
                            line_text,
                            self.text_size,
                            self.text_color,
                            provider,
                            z + 1, // Text z-index above box background
                        );
                    }
                }
            }

            // Render caret using shared module
            if self.focused {
                if let Some(layout) = self.rt_layout.as_ref() {
                    let caret_config = CaretRenderConfig {
                        content_rect,
                        scroll_x: 0.0,
                        scroll_y: self.scroll_y,
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
            // Placeholder text
            if let Some(ref placeholder) = self.placeholder {
                canvas.draw_text_direct(
                    [content_x, content_y + self.text_size],
                    placeholder,
                    self.text_size,
                    Color::rgba(120, 120, 130, 255),
                    provider,
                    z + 1, // Placeholder z-index above box background
                );
            }

            // Render caret at start if focused
            if self.focused && self.caret.visible {
                let cx = content_x;
                let cy0 = content_y;
                let cy1 = content_y + self.text_size * 1.2;

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
