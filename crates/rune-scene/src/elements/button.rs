use engine_core::{Brush, ColorLinPremul, Rect, RoundedRadii, RoundedRect};
use rune_surface::Canvas;

pub struct Button {
    pub rect: Rect,
    pub radius: f32,
    pub bg: ColorLinPremul,
    pub fg: ColorLinPremul,
    pub label: String,
    pub label_size: f32,
    pub focused: bool,
    /// Intent to trigger when button is clicked (e.g., "show_modal:my_modal")
    pub on_click_intent: Option<String>,
}

impl Button {
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        let rrect = RoundedRect {
            rect: self.rect,
            radii: RoundedRadii {
                tl: self.radius,
                tr: self.radius,
                br: self.radius,
                bl: self.radius,
            },
        };
        // Draw rounded background
        canvas.rounded_rect(rrect, Brush::Solid(self.bg), z);
        // Label (centered)
        // Approximate text width for centering (rough heuristic: 0.5 * font_size per char)
        let approx_text_width = self.label.len() as f32 * self.label_size * 0.5;
        let text_x = self.rect.x + (self.rect.w - approx_text_width) * 0.5;
        let text_y = self.rect.y + self.rect.h * 0.5 + self.label_size * 0.35;
        canvas.draw_text_run(
            [text_x, text_y],
            self.label.clone(),
            self.label_size,
            self.fg,
            z + 2,
        );
        // No always-visible border; focus outline removed to avoid faint edges
    }

    // ===== Event Handling =====

    /// Hit test the button
    pub fn hit_test(&self, x: f32, y: f32) -> bool {
        x >= self.rect.x
            && x <= self.rect.x + self.rect.w
            && y >= self.rect.y
            && y <= self.rect.y + self.rect.h
    }

    /// Handle click event on the button
    /// Returns ButtonClickResult indicating whether the button was clicked
    pub fn handle_click(&self, x: f32, y: f32) -> ButtonClickResult {
        if self.hit_test(x, y) {
            ButtonClickResult::Clicked
        } else {
            ButtonClickResult::Ignored
        }
    }

    // ===== Focus Management =====

    /// Check if this button is focused
    ///
    /// Returns true if the button currently has focus.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state for this button
    ///
    /// Sets the focus state. Focus management is typically handled by
    /// the event router, which calls this method.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Check if the point is inside this button
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        self.hit_test(x, y)
    }
}

/// Result of a button click event
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonClickResult {
    /// The button was clicked
    Clicked,
    /// Click was outside the button
    Ignored,
}

// ===== EventHandler Trait Implementation =====

impl crate::event_handler::EventHandler for Button {
    /// Handle mouse click event
    ///
    /// Buttons respond to left mouse button clicks within their bounds.
    fn handle_mouse_click(
        &mut self,
        event: crate::event_handler::MouseClickEvent,
    ) -> crate::event_handler::EventResult {
        use winit::event::ElementState;

        // Only handle left mouse button
        if event.button != winit::event::MouseButton::Left {
            return crate::event_handler::EventResult::Ignored;
        }

        // Only handle press (not release)
        if event.state != ElementState::Pressed {
            return crate::event_handler::EventResult::Ignored;
        }

        // Check if click is inside the button
        if self.contains_point(event.x, event.y) {
            crate::event_handler::EventResult::Handled
        } else {
            crate::event_handler::EventResult::Ignored
        }
    }

    /// Handle keyboard input event
    ///
    /// Buttons can be activated with Space or Enter when focused.
    fn handle_keyboard(
        &mut self,
        event: crate::event_handler::KeyboardEvent,
    ) -> crate::event_handler::EventResult {
        use winit::event::ElementState;
        use winit::keyboard::KeyCode;

        // Only handle key press, not release
        if event.state != ElementState::Pressed {
            return crate::event_handler::EventResult::Ignored;
        }

        // Only handle keyboard events if focused
        if !self.focused {
            return crate::event_handler::EventResult::Ignored;
        }

        // Activate button on Space or Enter
        match event.key {
            KeyCode::Space | KeyCode::Enter => crate::event_handler::EventResult::Handled,
            _ => crate::event_handler::EventResult::Ignored,
        }
    }

    /// Handle mouse move event
    ///
    /// Buttons don't need mouse move handling.
    fn handle_mouse_move(
        &mut self,
        _event: crate::event_handler::MouseMoveEvent,
    ) -> crate::event_handler::EventResult {
        crate::event_handler::EventResult::Ignored
    }

    /// Check if this button is focused
    fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state for this button
    fn set_focused(&mut self, focused: bool) {
        Button::set_focused(self, focused);
    }

    /// Check if the point is inside this button
    fn contains_point(&self, x: f32, y: f32) -> bool {
        self.contains_point(x, y)
    }
}
