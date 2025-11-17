# TextArea Implementation Plan

## Overview

Plan for implementing a multi-line `TextArea` widget that reuses the proven `TextLayout` + `Selection` + `CaretBlink` pattern from `InputBox`, while avoiding code duplication.

## Key Differences from InputBox

| Feature | InputBox | TextArea |
|---------|----------|----------|
| **Lines** | Single-line (NoWrap) | Multi-line (word wrap) |
| **Horizontal Scroll** | Yes, with scroll_x | No scrolling |
| **Vertical Scroll** | No | Yes, with scroll_y |
| **Size Behavior** | Fixed height | Expanding height (optional: min/max) |
| **Wrap Mode** | `WrapMode::NoWrap` | `WrapMode::WordWrap(width)` |
| **Navigation** | Left/Right, Home/End | Left/Right/Up/Down, Home/End, PageUp/PageDown |

## Architecture: Shared Components

### 1. Selection Rendering Module

**Location**: `crates/rune-scene/src/elements/selection_renderer.rs`

Extract the selection clipping and rendering logic into a reusable module:

```rust
/// Configuration for rendering text selections with proper clipping.
pub struct SelectionRenderConfig {
    /// Content area bounds (for clipping)
    pub content_rect: Rect,
    /// Text baseline Y position (for alignment)
    pub text_baseline_y: f32,
    /// Horizontal scroll offset (0.0 for TextArea)
    pub scroll_x: f32,
    /// Vertical scroll offset (0.0 for InputBox)
    pub scroll_y: f32,
    /// Selection color
    pub color: ColorLinPremul,
    /// Z-index for drawing
    pub z: i32,
}

/// Render selection rectangles with manual clipping.
///
/// # Why Manual Clipping?
/// Generic rect clipping is not yet implemented in the GPU pipeline,
/// so we manually clip selection rects against the content rect to keep
/// them aligned with text clipping.
///
/// # Process
/// 1. Get selection_rects from TextLayout
/// 2. Get baseline_offset from first line
/// 3. For each rect:
///    - Transform to screen coordinates
///    - Clip horizontally against content bounds
///    - Clip vertically against content bounds
///    - Skip if empty after clipping
///    - Draw the clipped rectangle
pub fn render_selection(
    canvas: &mut Canvas,
    layout: &TextLayout,
    selection: &Selection,
    config: &SelectionRenderConfig,
) {
    if selection.is_collapsed() {
        return;
    }

    let selection_rects = layout.selection_rects(selection);
    
    // Get baseline offset from layout for proper alignment
    let baseline_offset = if let Some(line) = layout.lines().first() {
        line.baseline_offset
    } else {
        return; // No lines, nothing to render
    };
    
    // Precompute clipping bounds
    let clip_left = config.content_rect.x;
    let clip_right = config.content_rect.x + config.content_rect.w;
    let clip_top = config.content_rect.y;
    let clip_bottom = config.content_rect.y + config.content_rect.h;

    for sel_rect in selection_rects {
        // Transform layout coordinates to screen coordinates
        // text_baseline_y is where the text baseline is drawn
        // We need to offset by -baseline_offset to get to the top of the line
        let mut highlight_x = config.content_rect.x - config.scroll_x + sel_rect.x;
        let mut highlight_y = config.text_baseline_y - baseline_offset + sel_rect.y - config.scroll_y;
        let mut highlight_w = sel_rect.width;
        let mut highlight_h = sel_rect.height;

        // Horizontal clip against content rect
        let rect_right = highlight_x + highlight_w;
        let clipped_left = highlight_x.max(clip_left);
        let clipped_right = rect_right.min(clip_right);

        if clipped_right <= clipped_left {
            continue; // Fully clipped horizontally
        }

        highlight_x = clipped_left;
        highlight_w = clipped_right - clipped_left;

        // Vertical clip against content rect
        let rect_bottom = highlight_y + highlight_h;
        let clipped_top = highlight_y.max(clip_top);
        let clipped_bottom = rect_bottom.min(clip_bottom);

        if clipped_bottom <= clipped_top {
            continue; // Fully clipped vertically
        }

        highlight_y = clipped_top;
        highlight_h = clipped_bottom - clipped_top;

        // Draw the clipped selection rectangle
        canvas.fill_rect(
            highlight_x,
            highlight_y,
            highlight_w,
            highlight_h,
            Brush::Solid(config.color),
            config.z,
        );
    }
}
```

### 2. Caret Rendering Module

**Location**: `crates/rune-scene/src/elements/caret_renderer.rs`

Extract caret rendering logic:

```rust
/// Configuration for rendering the text caret.
pub struct CaretRenderConfig {
    /// Content area bounds (for positioning)
    pub content_rect: Rect,
    /// Horizontal scroll offset (0.0 for TextArea)
    pub scroll_x: f32,
    /// Vertical scroll offset (0.0 for InputBox)
    pub scroll_y: f32,
    /// Caret color
    pub color: ColorLinPremul,
    /// Caret width in pixels
    pub width: f32,
    /// Z-index for drawing
    pub z: i32,
}

/// Render the text caret at the cursor position.
pub fn render_caret(
    canvas: &mut Canvas,
    layout: &TextLayout,
    cursor_position: usize,
    caret_blink: &CaretBlink,
    config: &CaretRenderConfig,
) {
    if !caret_blink.visible {
        return;
    }

    let cursor_pos = CursorPosition::new(cursor_position.min(layout.text().len()));
    let cursor_rect = match layout.cursor_rect_at_position(cursor_pos) {
        Some(rect) => rect,
        None => return,
    };

    // Transform to screen coordinates
    let cx = config.content_rect.x - config.scroll_x + cursor_rect.x;
    let cy0 = config.content_rect.y - config.scroll_y + cursor_rect.y;
    let cy1 = cy0 + cursor_rect.height;

    let mut caret = Path {
        cmds: Vec::new(),
        fill_rule: FillRule::NonZero,
    };
    caret.cmds.push(PathCmd::MoveTo([cx, cy0]));
    caret.cmds.push(PathCmd::LineTo([cx, cy1]));
    canvas.stroke_path(caret, config.width, config.color, config.z);
}
```

### 3. Shared Editing Trait

**Location**: `crates/rune-scene/src/elements/text_editor.rs`

Define a trait for common text editing operations:

```rust
/// Common interface for text editing widgets (InputBox, TextArea).
pub trait TextEditor {
    // Text access
    fn text(&self) -> &str;
    fn layout(&self) -> Option<&TextLayout>;
    fn layout_mut(&mut self) -> Option<&mut TextLayout>;
    
    // Selection and cursor
    fn selection(&self) -> &Selection;
    fn selection_mut(&mut self) -> &mut Selection;
    fn cursor_position(&self) -> usize;
    fn set_cursor_position(&mut self, pos: usize);
    
    // Caret blink
    fn caret_blink(&self) -> &CaretBlink;
    fn caret_blink_mut(&mut self) -> &mut CaretBlink;
    fn reset_cursor_blink(&mut self);
    
    // Focus
    fn is_focused(&self) -> bool;
    fn set_focused(&mut self, focused: bool);
    
    // Font settings
    fn text_size(&self) -> f32;
    
    // Sync after edit
    fn sync_from_layout(&mut self);
}

// Then implement default methods for common operations:
impl<T: TextEditor> T {
    fn insert_char_impl(&mut self, ch: char) {
        // Common implementation using the trait methods
    }
    
    fn delete_before_cursor_impl(&mut self) {
        // Common implementation
    }
    
    // ... etc for all editing operations
}
```

## TextArea Struct Design

**Location**: `crates/rune-scene/src/elements/text_area.rs`

```rust
pub struct TextArea {
    // Visual properties
    pub rect: Rect,
    pub text: String,
    pub text_size: f32,
    pub text_color: ColorLinPremul,
    pub placeholder: Option<String>,
    pub focused: bool,
    
    // Padding
    padding_x: f32,
    padding_y: f32,
    
    // Size constraints
    min_height: Option<f32>,
    max_height: Option<f32>,
    
    // Vertical scrolling (no horizontal scroll)
    scroll_y: f32,
    
    // TextLayout backend (with word wrap)
    rt_layout: Option<TextLayout>,
    
    // Selection and cursor
    rt_selection: Selection,
    pub cursor_position: usize,
    
    // Caret blink
    caret: CaretBlink,
    
    // Mouse selection state
    mouse_selecting: bool,
    last_mouse_pos: Option<(f32, f32)>,
    
    // Wrap width (content_width - padding)
    wrap_width: Option<f32>,
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
        
        // Calculate wrap width from rect
        let padding_x = 8.0;
        let wrap_width = (rect.w - padding_x * 2.0).max(10.0);
        
        // Build TextLayout with word wrap
        let rt_layout = load_system_default_font()
            .ok()
            .map(|font| TextLayout::with_wrap(
                text.clone(),
                &font,
                text_size,
                Some(wrap_width),
                WrapMode::WordWrap(wrap_width),
            ));
        
        Self {
            rect,
            text,
            text_size,
            text_color,
            placeholder,
            focused,
            padding_x,
            padding_y: 8.0,
            min_height: Some(60.0),
            max_height: None,
            scroll_y: 0.0,
            rt_layout,
            rt_selection: Selection::collapsed(initial_cursor),
            cursor_position: initial_cursor,
            caret: CaretBlink::new(focused),
            mouse_selecting: false,
            last_mouse_pos: None,
            wrap_width: Some(wrap_width),
        }
    }
    
    /// Update the wrap width when rect changes.
    pub fn set_rect(&mut self, rect: Rect) {
        let old_wrap_width = self.wrap_width;
        let new_wrap_width = (rect.w - self.padding_x * 2.0).max(10.0);
        
        self.rect = rect;
        self.wrap_width = Some(new_wrap_width);
        
        // Rewrap if width changed
        if old_wrap_width != Some(new_wrap_width) {
            self.rewrap_layout();
        }
    }
    
    /// Rewrap the layout with the current wrap width.
    fn rewrap_layout(&mut self) {
        if let (Some(layout), Some(wrap_width)) = (self.rt_layout.as_mut(), self.wrap_width) {
            if let Ok(font) = load_system_default_font() {
                *layout = TextLayout::with_wrap(
                    self.text.clone(),
                    &font,
                    self.text_size,
                    Some(wrap_width),
                    WrapMode::WordWrap(wrap_width),
                );
            }
        }
    }
    
    /// Calculate the desired height based on content.
    pub fn calculate_content_height(&self) -> f32 {
        let content_height = if let Some(layout) = self.rt_layout.as_ref() {
            layout.total_height()
        } else {
            self.text_size * 1.2 // Fallback
        };
        
        let total_height = content_height + self.padding_y * 2.0;
        
        // Apply min/max constraints
        let mut height = total_height;
        if let Some(min) = self.min_height {
            height = height.max(min);
        }
        if let Some(max) = self.max_height {
            height = height.min(max);
        }
        
        height
    }
    
    /// Update scroll to keep cursor visible.
    pub fn update_scroll(&mut self) {
        let layout = match self.rt_layout.as_ref() {
            Some(l) => l,
            None => return,
        };
        
        let cursor_pos = CursorPosition::new(self.cursor_position.min(layout.text().len()));
        let cursor_rect = match layout.cursor_rect_at_position(cursor_pos) {
            Some(r) => r,
            None => return,
        };
        
        let content_height = self.rect.h - self.padding_y * 2.0;
        let margin = 10.0;
        
        // Vertical scroll to keep cursor visible
        let cursor_top = cursor_rect.y;
        let cursor_bottom = cursor_rect.y + cursor_rect.height;
        
        // Minimum scroll to keep cursor visible at bottom
        let min_scroll = (cursor_bottom - content_height + margin).max(0.0);
        
        // Maximum scroll to keep cursor visible at top
        let max_scroll = (cursor_top - margin).max(0.0);
        
        // Clamp current scroll
        self.scroll_y = self.scroll_y.clamp(min_scroll, max_scroll);
        
        // Ensure we don't scroll past the content
        let max_content_scroll = (layout.total_height() - content_height).max(0.0);
        self.scroll_y = self.scroll_y.min(max_content_scroll);
    }
}
```

## TextArea-Specific Navigation

### Vertical Movement

```rust
impl TextArea {
    /// Move cursor up by one line.
    pub fn move_cursor_up(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let pos = self.cursor_position.min(layout.text().len());
            let new_pos = layout.move_cursor_up(pos);
            self.cursor_position = new_pos;
            self.rt_selection = Selection::collapsed(self.cursor_position);
            self.reset_cursor_blink();
        }
    }
    
    /// Move cursor down by one line.
    pub fn move_cursor_down(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let pos = self.cursor_position.min(layout.text().len());
            let new_pos = layout.move_cursor_down(pos);
            self.cursor_position = new_pos;
            self.rt_selection = Selection::collapsed(self.cursor_position);
            self.reset_cursor_blink();
        }
    }
    
    /// Extend selection up by one line (Shift+Up).
    pub fn extend_selection_up(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let new_selection = layout.extend_selection(&self.rt_selection, |offset| {
                layout.move_cursor_up(offset)
            });
            self.rt_selection = new_selection;
            self.cursor_position = new_selection.active();
            self.reset_cursor_blink();
        }
    }
    
    /// Extend selection down by one line (Shift+Down).
    pub fn extend_selection_down(&mut self) {
        if let Some(layout) = self.rt_layout.as_ref() {
            let new_selection = layout.extend_selection(&self.rt_selection, |offset| {
                layout.move_cursor_down(offset)
            });
            self.rt_selection = new_selection;
            self.cursor_position = new_selection.active();
            self.reset_cursor_blink();
        }
    }
    
    /// Move cursor to start of document (Cmd+Home).
    pub fn move_cursor_to_document_start(&mut self) {
        self.cursor_position = 0;
        self.scroll_y = 0.0;
        self.rt_selection = Selection::collapsed(0);
        self.reset_cursor_blink();
    }
    
    /// Move cursor to end of document (Cmd+End).
    pub fn move_cursor_to_document_end(&mut self) {
        self.cursor_position = self.text.len();
        self.rt_selection = Selection::collapsed(self.cursor_position);
        self.reset_cursor_blink();
        self.update_scroll();
    }
}
```

## TextArea Rendering

```rust
impl TextArea {
    pub fn render(&mut self, canvas: &mut Canvas, z: i32, provider: &dyn TextProvider) {
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
        
        // Update scroll before rendering
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
        
        if !self.text.is_empty() {
            // Render selection using shared module
            if self.focused && !self.rt_selection.is_collapsed() {
                if let Some(layout) = self.rt_layout.as_ref() {
                    let selection_config = SelectionRenderConfig {
                        content_rect,
                        text_baseline_y: content_y - self.scroll_y,
                        scroll_x: 0.0, // No horizontal scroll for TextArea
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
            
            // Render text (multi-line, with vertical scroll)
            // Note: This needs a multi-line text rendering approach
            // For now, render line by line from the layout
            if let Some(layout) = self.rt_layout.as_ref() {
                for line in layout.lines() {
                    let line_text = &self.text[line.text_range.clone()];
                    let text_x = content_x;
                    let text_y = content_y + line.y_offset + line.baseline_offset - self.scroll_y;
                    
                    // Only render lines that are visible
                    if text_y + line.height >= content_y && text_y <= content_y + content_height {
                        canvas.draw_text_direct(
                            [text_x, text_y],
                            line_text,
                            self.text_size,
                            self.text_color,
                            provider,
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
```

## Code Reuse Strategy

### Phase 1: Extract Shared Modules ✅
1. [x] Create `selection_renderer.rs` with `render_selection()` function
2. [x] Create `caret_renderer.rs` with `render_caret()` function
3. [x] Update `InputBox` to use these shared modules

### Phase 2: Refactor InputBox ✅
1. [x] Replace inline selection rendering with `selection_renderer::render_selection()`
2. [x] Replace inline caret rendering with `caret_renderer::render_caret()`
3. [x] Verify InputBox still works correctly

### Phase 3: Implement TextArea ✅
1. [x] Create `text_area.rs` with basic structure
2. [x] Reuse all editing methods from InputBox (copy initially, then extract to trait)
3. [x] Add vertical navigation methods (up/down)
4. [x] Use shared selection and caret renderers
5. [x] Implement vertical scrolling

### Phase 4: Extract Common Trait
1. [ ] Define `TextEditor` trait with common interface
2. [ ] Implement trait for both `InputBox` and `TextArea`
3. [ ] Move common editing logic to trait default implementations

## Keyboard Shortcuts for TextArea

Additional shortcuts beyond InputBox:

| Shortcut | Action |
|----------|--------|
| **Up/Down** | Move cursor up/down by line |
| **Shift+Up/Down** | Extend selection up/down by line |
| **Cmd+Up/Down** (macOS) | Move to document start/end |
| **Cmd+Shift+Up/Down** (macOS) | Select to document start/end |
| **PageUp/PageDown** | Scroll by viewport height |
| **Shift+PageUp/PageDown** | Extend selection by viewport |
| **Enter** | Insert newline |
| **Cmd+Enter** (macOS) | Submit (optional, for forms) |

## Testing Checklist

- [ ] Word wrap works correctly with various widths
- [ ] Vertical scrolling keeps cursor visible
- [ ] Selection rendering clips correctly (horizontal and vertical)
- [ ] Up/Down navigation maintains horizontal position preference
- [ ] Multi-line selection works across wrapped lines
- [ ] Copy/paste preserves newlines
- [ ] Undo/redo works with multi-line edits
- [ ] Mouse selection works across multiple lines
- [ ] Double-click selects word across line breaks
- [ ] Triple-click selects entire line/paragraph
- [ ] Expanding height updates correctly as content grows
- [ ] Min/max height constraints work properly

## Future Enhancements

1. **Line numbers** (optional gutter)
2. **Syntax highlighting** (via TextLayout styling)
3. **Find/replace** (reuse TextLayout search APIs)
4. **Auto-indent** (preserve indentation on Enter)
5. **Tab handling** (insert spaces or tab character)
6. **Soft wrap indicators** (visual markers for wrapped lines)
7. **Scrollbar rendering** (visual indicator of scroll position)

## Notes

- The manual selection clipping approach is critical for both InputBox and TextArea
- TextArea uses `WrapMode::WordWrap(width)` instead of `NoWrap`
- TextArea has vertical scroll (`scroll_y`) but no horizontal scroll (`scroll_x = 0.0`)
- Both widgets share the same selection color, caret style, and interaction patterns
- The expanding height feature makes TextArea suitable for forms and comment boxes
- For fixed-height TextArea (like code editors), set `min_height = max_height`
