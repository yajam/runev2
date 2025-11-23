use crate::elements::caret::CaretBlink;
use engine_core::{ColorLinPremul, FillRule, Path, PathCmd, Rect};
use rune_surface::Canvas;
use rune_text::layout::{CursorPosition, TextLayout};

/// Configuration for rendering the text caret.
pub struct CaretRenderConfig {
    /// Content area bounds (for positioning)
    pub content_rect: Rect,
    /// Baseline Y used when text was drawn (e.g. content_y + baseline_offset)
    pub text_baseline_y: f32,
    /// Baseline offset from the layout (line.baseline_offset)
    pub baseline_offset: f32,
    /// Horizontal alignment offset applied to the text content
    pub align_x: f32,
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
    let cx = config.content_rect.x + config.align_x - config.scroll_x + cursor_rect.x;
    let cy0 = config.text_baseline_y - config.baseline_offset + cursor_rect.y - config.scroll_y;
    let cy1 = cy0 + cursor_rect.height;

    let mut caret = Path {
        cmds: Vec::new(),
        fill_rule: FillRule::NonZero,
    };
    caret.cmds.push(PathCmd::MoveTo([cx, cy0]));
    caret.cmds.push(PathCmd::LineTo([cx, cy1]));
    canvas.stroke_path(caret, config.width, config.color, config.z);
}
