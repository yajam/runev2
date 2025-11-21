use engine_core::{Brush, Color, ColorLinPremul};
use rune_surface::Canvas;
use rune_surface::shapes;

pub struct Radio {
    pub center: [f32; 2],
    pub radius: f32,
    pub selected: bool,
    pub label: Option<String>,
    pub label_size: f32,
    pub label_color: ColorLinPremul,
    pub focused: bool,
}

impl Radio {
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        // Base circle background
        let bg = Color::rgba(45, 52, 71, 255);
        canvas.ellipse(self.center, [self.radius, self.radius], Brush::Solid(bg), z);

        // Border circle
        let border_color = Color::rgba(80, 90, 110, 255);
        shapes::draw_ellipse(
            canvas,
            self.center,
            [self.radius, self.radius],
            None,
            Some(1.0),
            Some(Brush::Solid(border_color)),
            z + 1,
        );

        // Selected inner dot
        if self.selected {
            let inner = self.radius * 0.6;
            let col = Color::rgba(63, 130, 246, 255);
            canvas.ellipse(self.center, [inner, inner], Brush::Solid(col), z + 2);
        }

        // Focus ring
        if self.focused {
            let focus_radius = self.radius + 2.0;
            shapes::draw_ellipse(
                canvas,
                self.center,
                [focus_radius, focus_radius],
                None,
                Some(2.0),
                Some(Brush::Solid(Color::rgba(63, 130, 246, 255))),
                z + 3,
            );
        }

        // Label
        if let Some(text) = &self.label {
            let pos = [
                self.center[0] + self.radius + 8.0,
                self.center[1] + self.label_size * 0.35,
            ];
            canvas.draw_text_run(pos, text.clone(), self.label_size, self.label_color, z + 3);
        }
    }

    // ===== Event Handling =====

    /// Hit test the radio button circle itself
    pub fn hit_test_circle(&self, x: f32, y: f32) -> bool {
        let dx = x - self.center[0];
        let dy = y - self.center[1];
        let dist_squared = dx * dx + dy * dy;
        dist_squared <= self.radius * self.radius
    }

    /// Hit test the radio button label (if it exists)
    pub fn hit_test_label(&self, x: f32, y: f32) -> bool {
        if let Some(label) = &self.label {
            let label_x = self.center[0] + self.radius + 8.0;
            let label_y_center = self.center[1];
            let char_width = self.label_size * 0.5;
            let label_width = label.len() as f32 * char_width;
            let clickable_height = (self.radius * 2.0).max(self.label_size * 1.2);

            x >= label_x
                && x <= label_x + label_width
                && y >= label_y_center - clickable_height / 2.0
                && y <= label_y_center + clickable_height / 2.0
        } else {
            false
        }
    }

    /// Handle click event on the radio button
    /// Returns RadioClickResult indicating what was clicked
    pub fn handle_click(&self, x: f32, y: f32) -> RadioClickResult {
        if self.hit_test_circle(x, y) {
            RadioClickResult::RadioCircle
        } else if self.hit_test_label(x, y) {
            RadioClickResult::Label
        } else {
            RadioClickResult::Ignored
        }
    }
}

/// Result of a radio button click event
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RadioClickResult {
    /// The radio circle itself was clicked (should select this radio)
    RadioCircle,
    /// The radio label was clicked (should select this radio)
    Label,
    /// Click was outside the radio button
    Ignored,
}

// ===== Additional Event Handling - To be ported from lib_old.rs =====

/// Keyboard keys that radio button responds to
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RadioKey {
    /// Space key - selects radio
    Space,
    /// Enter key - selects radio
    Enter,
    /// Arrow keys for navigation between radio buttons in a group
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    /// Tab key - moves focus (handled by focus manager, not element)
    Tab,
    /// Other keys - ignored
    Other,
}

/// Result of radio button keyboard event handling
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RadioKeyResult {
    /// Key was handled and radio was selected
    Selected,
    /// Key was handled for navigation (arrow keys)
    Navigate,
    /// Key was ignored
    Ignored,
}

impl Radio {
    // ===== Keyboard Event Handling (stub - to be implemented) =====

    /// Handle keyboard input when radio button is focused
    ///
    /// Space and Enter keys select the radio button.
    /// Arrow keys can navigate between radio buttons in a group.
    /// Returns RadioKeyResult indicating how the key was handled.
    ///
    /// # Example
    /// ```ignore
    /// let mut radio = Radio { ... };
    /// let result = radio.handle_radio_key(RadioKey::Space);
    /// if result == RadioKeyResult::Selected {
    ///     println!("Radio button selected");
    /// }
    /// ```
    pub fn handle_radio_key(&mut self, key: RadioKey) -> RadioKeyResult {
        match key {
            RadioKey::Space | RadioKey::Enter => {
                // Select this radio button
                self.select();
                RadioKeyResult::Selected
            }
            RadioKey::ArrowUp
            | RadioKey::ArrowDown
            | RadioKey::ArrowLeft
            | RadioKey::ArrowRight => {
                // Navigate to next/previous radio in group
                // Navigation is handled by the event router, not the element itself
                RadioKeyResult::Navigate
            }
            _ => RadioKeyResult::Ignored,
        }
    }

    // ===== Utility Methods (stubs - to be implemented) =====

    /// Select this radio button
    ///
    /// This sets the selected state to true.
    /// Note: In a radio group, you should also deselect all other radios.
    /// This is typically handled by the event router.
    pub fn select(&mut self) {
        self.selected = true;
    }

    /// Deselect this radio button
    ///
    /// This sets the selected state to false.
    pub fn deselect(&mut self) {
        self.selected = false;
    }

    /// Check if this radio button is selected
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    /// Check if this radio button is focused
    ///
    /// Returns true if the radio button currently has focus.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state for this radio button
    ///
    /// Sets the focus state. Focus management is typically handled by
    /// the event router, which calls this method.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Check if a point is inside this radio button (circle or label)
    ///
    /// Returns true if the point is inside either the radio circle or label.
    /// This is a convenience method that combines hit_test_circle and hit_test_label.
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        self.hit_test_circle(x, y) || self.hit_test_label(x, y)
    }
}

// ===== EventHandler Trait Implementation =====

impl crate::event_handler::EventHandler for Radio {
    /// Handle mouse click event
    ///
    /// Selects the radio button if the click is on the circle or label.
    /// Note: Deselecting other radios in the group is handled by EventRouter.
    fn handle_mouse_click(
        &mut self,
        event: crate::event_handler::MouseClickEvent,
    ) -> crate::event_handler::EventResult {
        use winit::event::ElementState;

        // Only handle left mouse button press
        if event.button != winit::event::MouseButton::Left || event.state != ElementState::Pressed {
            return crate::event_handler::EventResult::Ignored;
        }

        let click_result = self.handle_click(event.x, event.y);
        match click_result {
            RadioClickResult::RadioCircle | RadioClickResult::Label => {
                // Select this radio button
                // Note: EventRouter will deselect other radios in the group
                self.select();
                crate::event_handler::EventResult::Handled
            }
            RadioClickResult::Ignored => crate::event_handler::EventResult::Ignored,
        }
    }

    /// Handle keyboard input event
    ///
    /// Space and Enter keys select the radio button when it's focused.
    /// Arrow keys navigate between radio buttons in a group.
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

        // Map KeyCode to RadioKey
        let radio_key = match event.key {
            KeyCode::Space => RadioKey::Space,
            KeyCode::Enter => RadioKey::Enter,
            KeyCode::ArrowUp => RadioKey::ArrowUp,
            KeyCode::ArrowDown => RadioKey::ArrowDown,
            KeyCode::ArrowLeft => RadioKey::ArrowLeft,
            KeyCode::ArrowRight => RadioKey::ArrowRight,
            KeyCode::Tab => RadioKey::Tab,
            _ => RadioKey::Other,
        };

        // Handle the key
        match self.handle_radio_key(radio_key) {
            RadioKeyResult::Selected => crate::event_handler::EventResult::Handled,
            RadioKeyResult::Navigate => {
                // Navigation is handled by EventRouter
                crate::event_handler::EventResult::Handled
            }
            RadioKeyResult::Ignored => crate::event_handler::EventResult::Ignored,
        }
    }

    /// Check if this radio button is focused
    fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state for this radio button
    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Check if the point is inside this radio button
    fn contains_point(&self, x: f32, y: f32) -> bool {
        self.contains_point(x, y)
    }
}
