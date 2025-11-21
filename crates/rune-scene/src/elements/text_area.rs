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
    pub bg_color: ColorLinPremul,
    pub border_color: ColorLinPremul,
    pub border_width: f32,
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
            bg_color: ColorLinPremul::from_srgba_u8([248, 250, 252, 255]),
            border_color: ColorLinPremul::from_srgba_u8([148, 163, 184, 255]),
            border_width: 1.0,
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

    /// Apply styling from a SurfaceStyle (background/border/padding).
    pub fn apply_surface_style(&mut self, style: &rune_ir::view::SurfaceStyle) {
        if let Some(bg) = &style.background {
            if let rune_ir::view::ViewBackground::Solid { color } = bg {
                if let Some(parsed) = crate::ir_adapter::parse_color(color) {
                    self.bg_color = parsed;
                }
            }
        }

        if let Some(color) = style
            .border_color
            .as_ref()
            .and_then(|c| crate::ir_adapter::parse_color(c))
        {
            self.border_color = color;
        }

        if let Some(width) = style.border_width {
            self.border_width = width as f32;
        }

        self.padding_x = style.padding.left as f32;
        self.padding_y = style.padding.top as f32;

        // Recompute wrap width based on new padding.
        self.set_rect(self.rect);
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

    fn clamp_selection_to_len(selection: &RtSelection, max: usize) -> RtSelection {
        let anchor = selection.anchor().min(max);
        let active = selection.active().min(max);
        RtSelection::new(anchor, active)
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
        let normalized_selection = {
            let a = selection_before.anchor();
            let b = selection_before.active();
            RtSelection::new(a.min(b), a.max(b))
        };
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
            let (new_cursor, new_selection) = f(layout, &font, &normalized_selection, size);
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
        // If there's a selection, delete it regardless of cursor position
        if !self.rt_selection.is_collapsed() {
            self.with_layout_edit(|layout, font, selection, size| {
                let wrap_width = layout.max_line_width().max(100.0);
                let new_cursor = layout.delete_selection(
                    selection,
                    font,
                    size,
                    Some(wrap_width),
                    RtWrapMode::BreakWord,
                );
                (new_cursor, RtSelection::collapsed(new_cursor))
            });
            self.preferred_x = None;
            return;
        }

        // Otherwise, check if we can delete backward
        if self.text.is_empty() || self.cursor_position == 0 {
            return;
        }
        self.with_layout_edit(|layout, font, selection, size| {
            let wrap_width = layout.max_line_width().max(100.0);
            let new_cursor = layout.delete_backward(
                selection.active(),
                font,
                size,
                Some(wrap_width),
                RtWrapMode::BreakWord,
            );
            (new_cursor, RtSelection::collapsed(new_cursor))
        });
        // Horizontal position may have changed after delete; recompute on next up/down.
        self.preferred_x = None;
    }

    pub fn delete_after_cursor(&mut self) {
        // If there's a selection, delete it regardless of cursor position
        if !self.rt_selection.is_collapsed() {
            self.with_layout_edit(|layout, font, selection, size| {
                let wrap_width = layout.max_line_width().max(100.0);
                let new_cursor = layout.delete_selection(
                    selection,
                    font,
                    size,
                    Some(wrap_width),
                    RtWrapMode::BreakWord,
                );
                (new_cursor, RtSelection::collapsed(new_cursor))
            });
            self.preferred_x = None;
            return;
        }

        // Otherwise, check if we can delete forward
        if self.text.is_empty() || self.cursor_position >= self.text.len() {
            return;
        }
        self.with_layout_edit(|layout, font, selection, size| {
            let wrap_width = layout.max_line_width().max(100.0);
            let new_cursor = layout.delete_forward(
                selection.active(),
                font,
                size,
                Some(wrap_width),
                RtWrapMode::BreakWord,
            );
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
            let clamped = Self::clamp_selection_to_len(&new_selection, layout.text().len());
            self.rt_selection = clamped;
            self.cursor_position = clamped.active();
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
            let clamped = Self::clamp_selection_to_len(&new_selection, layout.text().len());
            self.rt_selection = clamped;
            self.cursor_position = clamped.active();
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
                // Preserve the anchor/active relationship from the layout
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
            // Preserve the anchor/active relationship from the layout
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
                // Preserve the anchor/active relationship from the layout
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
            // Preserve the anchor/active relationship from the layout
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
        canvas.rounded_rect(rrect, Brush::Solid(self.bg_color), z);

        // Border
        let base_border_color = self.border_color;
        let base_border_width = self.border_width;
        let border_color = if self.focused {
            Color::rgba(63, 130, 246, 255)
        } else {
            base_border_color
        };
        let border_width = if self.focused {
            base_border_width.max(2.0)
        } else {
            base_border_width
        };
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
                        z: z + 2, // Selection behind text
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
                            z + 3, // Text z-index above selection
                        );
                    }
                }
            }

            // Render caret using shared module (only when no selection)
            if self.focused && self.rt_selection.is_collapsed() {
                if let Some(layout) = self.rt_layout.as_ref() {
                    let baseline_offset = layout
                        .lines()
                        .first()
                        .map(|l| l.baseline_offset)
                        .unwrap_or(self.text_size * 0.8);
                    // Baseline position for the first line; cursor_rect carries per-line offsets.
                    let text_baseline_y = content_y + baseline_offset;
                    let caret_config = CaretRenderConfig {
                        content_rect,
                        text_baseline_y,
                        baseline_offset,
                        scroll_x: 0.0,
                        scroll_y: self.scroll_y,
                        color: Color::rgba(63, 130, 246, 255),
                        width: 1.5,
                        z: z + 4, // Caret on top of text
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
                    z + 3, // Placeholder z-index matches text
                );
            }

            // Render caret at start if focused (only when no selection)
            if self.focused && self.rt_selection.is_collapsed() && self.caret.visible {
                let cx = content_x;
                let cy0 = content_y;
                let cy1 = content_y + self.text_size * 1.2;

                let mut caret = Path {
                    cmds: Vec::new(),
                    fill_rule: FillRule::NonZero,
                };
                caret.cmds.push(PathCmd::MoveTo([cx, cy0]));
                caret.cmds.push(PathCmd::LineTo([cx, cy1]));
                canvas.stroke_path(caret, 1.5, Color::rgba(63, 130, 246, 255), z + 4);
            }
        }

        canvas.pop_clip();
    }

    // ===== Focus Management =====

    /// Check if this text area is focused
    ///
    /// Returns true if the text area currently has focus.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state for this text area
    ///
    /// Sets the focus state. Focus management is typically handled by
    /// the event router, which calls this method.
    /// When focused, the caret becomes visible and keyboard input is accepted.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if focused {
            self.reset_cursor_blink();
        }
    }
}

// ===== EventHandler Trait Implementation =====

impl crate::event_handler::EventHandler for TextArea {
    /// Handle mouse click event
    ///
    /// Single-click: Position cursor at click location
    /// Double-click: Select word at click location
    /// Triple-click: Select entire line at click location
    fn handle_mouse_click(
        &mut self,
        event: crate::event_handler::MouseClickEvent,
    ) -> crate::event_handler::EventResult {
        use winit::event::ElementState;

        // Only handle left mouse button
        if event.button != winit::event::MouseButton::Left {
            return crate::event_handler::EventResult::Ignored;
        }

        match event.state {
            ElementState::Pressed => {
                // Only start interaction on presses within bounds.
                if !self.contains_point(event.x, event.y) {
                    return crate::event_handler::EventResult::Ignored;
                }

                // Handle different click counts
                match event.click_count {
                    1 => {
                        // Single-click: position cursor
                        self.start_mouse_selection(event.x, event.y);
                    }
                    2 => {
                        // Double-click: select word
                        self.start_word_selection(event.x, event.y);
                    }
                    3 => {
                        // Triple-click: select line
                        self.start_line_selection(event.x, event.y);
                    }
                    _ => {
                        // More than triple-click: treat as single-click
                        self.start_mouse_selection(event.x, event.y);
                    }
                }
                crate::event_handler::EventResult::Handled
            }
            ElementState::Released => {
                // End mouse selection on release even if the cursor drifted outside.
                if self.mouse_selecting || self.contains_point(event.x, event.y) {
                    self.end_mouse_selection();
                    crate::event_handler::EventResult::Handled
                } else {
                    crate::event_handler::EventResult::Ignored
                }
            }
        }
    }

    /// Handle keyboard input event
    ///
    /// Supports full multi-line text editing keyboard shortcuts:
    /// - Character input: Insert characters (handled via separate character events)
    /// - Backspace/Delete: Delete characters
    /// - Arrow keys: Move cursor (with Shift for selection)
    /// - Arrow Up/Down: Move between lines
    /// - Cmd/Ctrl+A: Select all
    /// - Cmd/Ctrl+C: Copy
    /// - Cmd/Ctrl+X: Cut
    /// - Cmd/Ctrl+V: Paste
    /// - Cmd/Ctrl+Z: Undo
    /// - Cmd/Ctrl+Shift+Z or Cmd/Ctrl+Y: Redo
    /// - Home/End: Move to start/end of line
    /// - Cmd/Ctrl+Home/End: Move to start/end of document
    fn handle_keyboard(
        &mut self,
        event: crate::event_handler::KeyboardEvent,
    ) -> crate::event_handler::EventResult {
        use winit::event::ElementState;
        use winit::keyboard::{KeyCode, ModifiersState};

        // Only handle key press, not release
        if event.state != ElementState::Pressed {
            return crate::event_handler::EventResult::Ignored;
        }

        // Only handle keyboard events if focused
        if !self.focused {
            return crate::event_handler::EventResult::Ignored;
        }

        let shift = event.modifiers.contains(ModifiersState::SHIFT);
        let ctrl = event.modifiers.contains(ModifiersState::CONTROL);
        let alt = event.modifiers.contains(ModifiersState::ALT);
        let cmd = event.modifiers.contains(ModifiersState::SUPER);

        // Accept either Cmd or Ctrl for primary shortcuts (matches legacy behavior).
        let modifier = cmd || ctrl;

        // Word movement: Alt (mac) or Ctrl (win/linux); accept either to be resilient.
        let word_modifier = alt || ctrl;

        match event.key {
            // Text input is handled separately via ReceivedCharacter event
            // These are navigation and editing keys only
            KeyCode::Backspace => {
                self.delete_before_cursor();
                self.update_scroll();
                crate::event_handler::EventResult::Handled
            }

            KeyCode::Delete => {
                self.delete_after_cursor();
                self.update_scroll();
                crate::event_handler::EventResult::Handled
            }

            KeyCode::ArrowLeft => {
                if cmd && shift {
                    // Cmd+Shift+Left: extend selection to start of line
                    self.extend_selection_to_line_start();
                } else if cmd {
                    // Cmd+Left: move to start of line
                    self.move_cursor_line_start();
                } else if shift && word_modifier {
                    // Shift+Alt+Left (macOS) or Shift+Ctrl+Left (Windows/Linux): Extend selection left by word
                    self.extend_selection_left_word();
                } else if word_modifier {
                    // Alt+Left (macOS) or Ctrl+Left (Windows/Linux): Move cursor left by word
                    self.move_cursor_left_word();
                } else if shift {
                    // Shift+Left: Extend selection left
                    self.extend_selection_left();
                } else {
                    // Left: Move cursor left
                    self.move_cursor_left();
                }
                self.update_scroll();
                crate::event_handler::EventResult::Handled
            }

            KeyCode::ArrowRight => {
                if cmd && shift {
                    // Cmd+Shift+Right: extend selection to end of line
                    self.extend_selection_to_line_end();
                } else if cmd {
                    // Cmd+Right: move to end of line
                    self.move_cursor_line_end();
                } else if shift && word_modifier {
                    // Shift+Alt+Right (macOS) or Shift+Ctrl+Right (Windows/Linux): Extend selection right by word
                    self.extend_selection_right_word();
                } else if word_modifier {
                    // Alt+Right (macOS) or Ctrl+Right (Windows/Linux): Move cursor right by word
                    self.move_cursor_right_word();
                } else if shift {
                    // Shift+Right: Extend selection right
                    self.extend_selection_right();
                } else {
                    // Right: Move cursor right
                    self.move_cursor_right();
                }
                self.update_scroll();
                crate::event_handler::EventResult::Handled
            }

            KeyCode::ArrowUp => {
                if shift {
                    // Shift+Up: Extend selection up
                    self.extend_selection_up();
                } else {
                    // Up: Move cursor up
                    self.move_cursor_up();
                }
                self.update_scroll();
                crate::event_handler::EventResult::Handled
            }

            KeyCode::ArrowDown => {
                if shift {
                    // Shift+Down: Extend selection down
                    self.extend_selection_down();
                } else {
                    // Down: Move cursor down
                    self.move_cursor_down();
                }
                self.update_scroll();
                crate::event_handler::EventResult::Handled
            }

            KeyCode::Home => {
                if modifier && shift {
                    // Cmd+Shift+Home (macOS) or Ctrl+Shift+Home (Windows/Linux): Extend selection to document start
                    self.extend_selection_to_document_start();
                } else if modifier {
                    // Cmd+Home (macOS) or Ctrl+Home (Windows/Linux): Move to document start
                    self.move_cursor_to_document_start();
                } else if shift {
                    // Shift+Home: Extend selection to line start
                    self.extend_selection_to_line_start();
                } else {
                    // Home: Move to line start
                    self.move_cursor_line_start();
                }
                self.update_scroll();
                crate::event_handler::EventResult::Handled
            }

            KeyCode::End => {
                if modifier && shift {
                    // Cmd+Shift+End (macOS) or Ctrl+Shift+End (Windows/Linux): Extend selection to document end
                    self.extend_selection_to_document_end();
                } else if modifier {
                    // Cmd+End (macOS) or Ctrl+End (Windows/Linux): Move to document end
                    self.move_cursor_to_document_end();
                } else if shift {
                    // Shift+End: Extend selection to line end
                    self.extend_selection_to_line_end();
                } else {
                    // End: Move to line end
                    self.move_cursor_line_end();
                }
                self.update_scroll();
                crate::event_handler::EventResult::Handled
            }

            // Clipboard and editing shortcuts
            KeyCode::KeyA if modifier => {
                // Cmd+A (macOS) or Ctrl+A (Windows/Linux): Select all
                self.select_all();
                self.update_scroll();
                crate::event_handler::EventResult::Handled
            }

            KeyCode::KeyC if modifier => {
                // Cmd+C (macOS) or Ctrl+C (Windows/Linux): Copy
                let _ = self.copy_to_clipboard();
                crate::event_handler::EventResult::Handled
            }

            KeyCode::KeyX if modifier => {
                // Cmd+X (macOS) or Ctrl+X (Windows/Linux): Cut
                let _ = self.cut_to_clipboard();
                self.update_scroll();
                crate::event_handler::EventResult::Handled
            }

            KeyCode::KeyV if modifier => {
                // Cmd+V (macOS) or Ctrl+V (Windows/Linux): Paste
                let _ = self.paste_from_clipboard();
                self.update_scroll();
                crate::event_handler::EventResult::Handled
            }

            KeyCode::KeyZ if modifier && shift => {
                // Cmd+Shift+Z (macOS) or Ctrl+Shift+Z (Windows/Linux): Redo
                self.redo();
                self.update_scroll();
                crate::event_handler::EventResult::Handled
            }

            KeyCode::KeyZ if modifier => {
                // Cmd+Z (macOS) or Ctrl+Z (Windows/Linux): Undo
                self.undo();
                self.update_scroll();
                crate::event_handler::EventResult::Handled
            }

            KeyCode::KeyY if modifier => {
                // Cmd+Y (macOS) or Ctrl+Y (Windows/Linux): Redo (alternative)
                self.redo();
                self.update_scroll();
                crate::event_handler::EventResult::Handled
            }

            _ => crate::event_handler::EventResult::Ignored,
        }
    }

    /// Handle mouse move event (for text selection)
    ///
    /// Extends the current selection when the mouse is dragged with the button held.
    fn handle_mouse_move(
        &mut self,
        event: crate::event_handler::MouseMoveEvent,
    ) -> crate::event_handler::EventResult {
        if self.mouse_selecting {
            self.extend_mouse_selection(event.x, event.y);
            self.update_scroll();
            crate::event_handler::EventResult::Handled
        } else {
            crate::event_handler::EventResult::Ignored
        }
    }

    /// Check if this text area is focused
    fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state for this text area
    fn set_focused(&mut self, focused: bool) {
        // Call the TextArea's own set_focused method (not recursive)
        TextArea::set_focused(self, focused);
    }

    /// Check if the point is inside this text area
    fn contains_point(&self, x: f32, y: f32) -> bool {
        self.contains_point(x, y)
    }
}
