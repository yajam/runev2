use crate::viewport_ir;
use engine_core::TextLayoutCache;
use rune_surface::Canvas;

/// Update blink state for manual viewport input widgets (input boxes and
/// text areas). Returns true if any editable is focused, indicating a
/// redraw is recommended.
pub fn update_editable_blinks(
    viewport_ir: &std::sync::Arc<std::sync::Mutex<viewport_ir::ViewportContent>>,
    delta_time: f32,
    toolbar_address_focused: bool,
) -> bool {
    let mut lock = viewport_ir.lock().unwrap();
    for input_box in lock.input_boxes.iter_mut() {
        input_box.update_blink(delta_time);
    }
    for textarea in lock.text_areas.iter_mut() {
        textarea.update_blink(delta_time);
    }

    toolbar_address_focused
        || lock.input_boxes.iter().any(|ib| ib.focused)
        || lock.text_areas.iter().any(|ta| ta.focused)
}

/// Handle mouse-drag selection updates for viewport input boxes and text areas.
/// Returns true if any selection changed and a redraw is needed.
pub fn update_selection_drag(
    viewport_ir: &std::sync::Arc<std::sync::Mutex<viewport_ir::ViewportContent>>,
    viewport_local_x: f32,
    viewport_local_y: f32,
    click_count: u32,
) -> bool {
    let mut lock = viewport_ir.lock().unwrap();
    let mut changed = false;

    // Check if any input box is in mouse selection mode
    for input_box in lock.input_boxes.iter_mut() {
        if input_box.focused {
            // Extend selection based on click count
            if click_count == 3 {
                input_box.extend_line_selection(viewport_local_x, viewport_local_y);
            } else if click_count == 2 {
                input_box.extend_word_selection(viewport_local_x, viewport_local_y);
            } else {
                input_box.extend_mouse_selection(viewport_local_x, viewport_local_y);
            }
            input_box.update_scroll();
            changed = true;
            break;
        }
    }

    // Check if any text area is in mouse selection mode
    if !changed {
        for textarea in lock.text_areas.iter_mut() {
            if textarea.focused {
                if click_count == 3 {
                    textarea.extend_line_selection(viewport_local_x, viewport_local_y);
                } else if click_count == 2 {
                    textarea.extend_word_selection(viewport_local_x, viewport_local_y);
                } else {
                    textarea.extend_mouse_selection(viewport_local_x, viewport_local_y);
                }
                textarea.update_scroll();
                changed = true;
                break;
            }
        }
    }

    changed
}

/// Render the manual viewport content when IR rendering is disabled.
pub fn render_viewport(
    viewport_ir: &std::sync::Arc<std::sync::Mutex<viewport_ir::ViewportContent>>,
    canvas: &mut Canvas,
    scale_factor: f32,
    viewport_width: u32,
    viewport_height: u32,
    provider: &dyn engine_core::TextProvider,
    text_cache: &TextLayoutCache,
) -> f32 {
    let mut lock = viewport_ir.lock().unwrap();
    lock.render(
        canvas,
        scale_factor,
        viewport_width,
        viewport_height,
        provider,
        text_cache,
    )
}
