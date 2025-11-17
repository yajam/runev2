use engine_core::{Brush, ColorLinPremul, Rect};
use rune_surface::Canvas;
use rune_text::layout::{Selection, TextLayout};

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
