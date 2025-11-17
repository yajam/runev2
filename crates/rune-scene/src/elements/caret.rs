/// Shared caret blink state for editable controls (input boxes, text areas).
///
/// Controls should:
/// - call `update` each frame with `delta_time` and current focus state
/// - call `reset_manual` after edits (typing, cursor moves)
#[derive(Clone, Copy, Debug)]
pub struct CaretBlink {
    pub visible: bool,
    blink_time: f32,
    blink_interval: f32,
    focused: bool,
    was_focused: bool,
}

impl CaretBlink {
    /// Create a new caret blink state.
    pub fn new(initial_focused: bool) -> Self {
        Self {
            visible: true,
            blink_time: 0.0,
            blink_interval: 0.5,
            focused: initial_focused,
            was_focused: initial_focused,
        }
    }

    /// Update blink timer and handle focus transitions.
    pub fn update(&mut self, delta_time: f32, focused: bool) {
        self.focused = focused;

        if self.focused && !self.was_focused {
            // Gained focus: ensure caret is visible and timer reset.
            self.visible = true;
            self.blink_time = 0.0;
        } else if !self.focused && self.was_focused {
            // Lost focus: hide caret and reset timer.
            self.visible = false;
            self.blink_time = 0.0;
        }
        self.was_focused = self.focused;

        if self.focused {
            self.blink_time += delta_time;
            if self.blink_time >= self.blink_interval {
                self.blink_time -= self.blink_interval;
                self.visible = !self.visible;
            }
        }
    }

    /// Make caret visible and reset blink phase (call after edits).
    pub fn reset_manual(&mut self) {
        self.visible = true;
        self.blink_time = 0.0;
    }
}
