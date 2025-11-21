use engine_core::{Brush, Color, ColorLinPremul, Rect, RoundedRadii, RoundedRect, SvgStyle};
use rune_surface::Canvas;

pub struct Checkbox {
    pub rect: Rect,
    pub checked: bool,
    pub focused: bool,
    pub label: Option<String>,
    pub label_size: f32,
    pub color: ColorLinPremul,
}

impl Checkbox {
    // UI rendering for the checkbox
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        // Box with small rounded corners; draw fill then stroke so the border sits on top
        let base_fill = Brush::Solid(Color::rgba(240, 240, 240, 255));
        let base_rect = self.rect;
        canvas.fill_rect(
            base_rect.x,
            base_rect.y,
            base_rect.w,
            base_rect.h,
            base_fill,
            z,
        );
        // Top edge accent line, matching ui.rs
        let top_edge = Brush::Solid(Color::rgba(180, 180, 180, 255));
        canvas.fill_rect(base_rect.x, base_rect.y, base_rect.w, 1.0, top_edge, z + 1);
        // Focus outline (inside border)
        if self.focused {
            // Rounded focus outline to match demo-app ui.rs
            let focus_rr = RoundedRect {
                rect: base_rect,
                radii: RoundedRadii {
                    tl: 2.0,
                    tr: 2.0,
                    br: 2.0,
                    bl: 2.0,
                },
            };
            let focus = Brush::Solid(Color::rgba(63, 130, 246, 255));
            canvas.stroke_rounded_rect(focus_rr, 2.0, focus, z + 2);
        }
        // Checked state: fill inner square and draw a crisp white check mark
        if self.checked {
            let inset = 2.0f32;
            let inner = Rect {
                x: self.rect.x + inset,
                y: self.rect.y + inset,
                w: (self.rect.w - 2.0 * inset).max(0.0),
                h: (self.rect.h - 2.0 * inset).max(0.0),
            };
            // Snap inner rect to whole pixels to avoid subpixel blurring of the SVG
            let inner_snapped = Rect {
                x: inner.x.round(),
                y: inner.y.round(),
                w: inner.w.round(),
                h: inner.h.round(),
            };
            let inner_rr = RoundedRect {
                rect: inner_snapped,
                radii: RoundedRadii {
                    tl: 1.5,
                    tr: 1.5,
                    br: 1.5,
                    bl: 1.5,
                },
            };
            canvas.rounded_rect(
                inner_rr,
                Brush::Solid(Color::rgba(63, 130, 246, 255)),
                z + 2,
            );

            // Use the canonical SVG checkmark and center it in the inner rect.
            // Scale it relative to the inner width so it appears larger and
            // visually matches the design, instead of shrinking to min(w, h).
            let icon_size = inner_snapped.w * 0.9;
            let origin = [
                inner_snapped.x + (inner_snapped.w - icon_size) * 0.5,
                inner_snapped.y + (inner_snapped.h - icon_size) * 0.5,
            ];
            let style = SvgStyle::new()
                .with_stroke(Color::rgba(255, 255, 255, 255))
                .with_stroke_width(3.0);
            canvas.draw_svg_styled(
                "images/check.svg",
                origin,
                [icon_size, icon_size],
                style,
                z + 3,
            );
        }
        // Label
        if let Some(text) = &self.label {
            let tx = self.rect.x + self.rect.w + 8.0;
            let ty = self.rect.y + self.rect.h * 0.5 + self.label_size * 0.35;
            canvas.draw_text_run(
                [tx, ty],
                text.clone(),
                self.label_size,
                self.color, // Use the color passed in, which should be light colored
                z + 3,
            );
        }
    }

    // ===== Event Handling =====

    /// Hit test the checkbox box itself
    pub fn hit_test_box(&self, x: f32, y: f32) -> bool {
        x >= self.rect.x
            && x <= self.rect.x + self.rect.w
            && y >= self.rect.y
            && y <= self.rect.y + self.rect.h
    }

    /// Hit test the checkbox label (if it exists)
    pub fn hit_test_label(&self, x: f32, y: f32) -> bool {
        if let Some(label) = &self.label {
            let label_x = self.rect.x + self.rect.w + 8.0;
            let char_width = self.label_size * 0.5;
            let label_width = label.len() as f32 * char_width;
            let clickable_height = self.rect.h.max(self.label_size * 1.2);

            x >= label_x
                && x <= label_x + label_width
                && y >= self.rect.y
                && y <= self.rect.y + clickable_height
        } else {
            false
        }
    }

    /// Handle click event on the checkbox
    /// Returns CheckboxClickResult indicating what was clicked
    pub fn handle_click(&self, x: f32, y: f32) -> CheckboxClickResult {
        if self.hit_test_box(x, y) {
            CheckboxClickResult::CheckboxBox
        } else if self.hit_test_label(x, y) {
            CheckboxClickResult::Label
        } else {
            CheckboxClickResult::Ignored
        }
    }
}

/// Result of a checkbox click event
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CheckboxClickResult {
    /// The checkbox box itself was clicked (should toggle)
    CheckboxBox,
    /// The checkbox label was clicked (should toggle)
    Label,
    /// Click was outside the checkbox
    Ignored,
}

// ===== Additional Event Handling - To be ported from lib_old.rs =====

/// Keyboard keys that checkbox responds to
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CheckboxKey {
    /// Space key - toggles checkbox
    Space,
    /// Enter key - toggles checkbox
    Enter,
    /// Tab key - moves focus (handled by focus manager, not element)
    Tab,
    /// Other keys - ignored
    Other,
}

/// Result of checkbox keyboard event handling
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CheckboxKeyResult {
    /// Key was handled and checkbox state was toggled
    Toggled,
    /// Key was ignored
    Ignored,
}

impl Checkbox {
    // ===== Keyboard Event Handling (stub - to be implemented) =====

    /// Handle keyboard input when checkbox is focused
    ///
    /// Space and Enter keys toggle the checkbox.
    /// Returns CheckboxKeyResult::Toggled if the key toggled the checkbox,
    /// CheckboxKeyResult::Ignored otherwise.
    ///
    /// # Example
    /// ```ignore
    /// let mut checkbox = Checkbox { ... };
    /// let result = checkbox.handle_checkbox_key(CheckboxKey::Space);
    /// if result == CheckboxKeyResult::Toggled {
    ///     println!("Checkbox toggled to: {}", checkbox.checked);
    /// }
    /// ```
    pub fn handle_checkbox_key(&mut self, key: CheckboxKey) -> CheckboxKeyResult {
        match key {
            CheckboxKey::Space | CheckboxKey::Enter => {
                // Toggle checkbox when Space or Enter is pressed
                self.toggle();
                CheckboxKeyResult::Toggled
            }
            _ => CheckboxKeyResult::Ignored,
        }
    }

    // ===== Utility Methods (stubs - to be implemented) =====

    /// Toggle the checkbox checked state
    ///
    /// This is a convenience method for toggling the checked state.
    /// Typically called from event handlers.
    pub fn toggle(&mut self) {
        self.checked = !self.checked;
    }

    /// Check if this checkbox is focused
    ///
    /// Returns true if the checkbox currently has focus.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state for this checkbox
    ///
    /// Sets the focus state. Focus management is typically handled by
    /// the event router, which calls this method.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Check if a point is inside this checkbox (box or label)
    ///
    /// Returns true if the point is inside either the checkbox box or label.
    /// This is a convenience method that combines hit_test_box and hit_test_label.
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        self.hit_test_box(x, y) || self.hit_test_label(x, y)
    }
}

// ===== EventHandler Trait Implementation =====

impl crate::event_handler::EventHandler for Checkbox {
    /// Handle mouse click event
    ///
    /// Toggles the checkbox if the click is on the box or label.
    /// Sets focus on the checkbox when clicked.
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
            CheckboxClickResult::CheckboxBox | CheckboxClickResult::Label => {
                // Toggle the checkbox
                self.toggle();
                // Focus is managed by EventRouter, not here
                crate::event_handler::EventResult::Handled
            }
            CheckboxClickResult::Ignored => crate::event_handler::EventResult::Ignored,
        }
    }

    /// Handle keyboard input event
    ///
    /// Space and Enter keys toggle the checkbox when it's focused.
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

        // Map KeyCode to CheckboxKey
        let checkbox_key = match event.key {
            KeyCode::Space => CheckboxKey::Space,
            KeyCode::Enter => CheckboxKey::Enter,
            KeyCode::Tab => CheckboxKey::Tab,
            _ => CheckboxKey::Other,
        };

        // Handle the key
        match self.handle_checkbox_key(checkbox_key) {
            CheckboxKeyResult::Toggled => crate::event_handler::EventResult::Handled,
            CheckboxKeyResult::Ignored => crate::event_handler::EventResult::Ignored,
        }
    }

    /// Check if this checkbox is focused
    fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state for this checkbox
    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Check if the point is inside this checkbox
    fn contains_point(&self, x: f32, y: f32) -> bool {
        self.contains_point(x, y)
    }
}
