use engine_core::{ColorLinPremul, Hyperlink as EngineHyperlink};
use rune_surface::Canvas;

/// A clickable hyperlink element with optional underline decoration.
pub struct Link {
    pub text: String,
    pub pos: [f32; 2],
    pub size: f32,
    pub color: ColorLinPremul,
    pub url: String,
    pub underline: bool,
    pub underline_color: Option<ColorLinPremul>,
    pub focused: bool,
    pub hovered: bool,
}

impl Link {
    /// Create a new hyperlink with default styling (blue with underline).
    pub fn new(text: impl Into<String>, url: impl Into<String>, pos: [f32; 2], size: f32) -> Self {
        Self {
            text: text.into(),
            pos,
            size,
            color: ColorLinPremul::from_srgba_u8([0x00, 0x7a, 0xff, 0xff]), // Blue
            url: url.into(),
            underline: true,
            underline_color: None,
            focused: false,
            hovered: false,
        }
    }

    /// Set the text color.
    pub fn with_color(mut self, color: ColorLinPremul) -> Self {
        self.color = color;
        self
    }

    /// Set whether to show the underline.
    pub fn with_underline(mut self, underline: bool) -> Self {
        self.underline = underline;
        self
    }

    /// Set a custom underline color (different from text color).
    pub fn with_underline_color(mut self, color: ColorLinPremul) -> Self {
        self.underline_color = Some(color);
        self
    }

    /// Render the hyperlink to the canvas.
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        let hyperlink = EngineHyperlink {
            text: self.text.clone(),
            pos: self.pos,
            size: self.size,
            color: self.color,
            url: self.url.clone(),
            underline: self.underline,
            underline_color: self.underline_color,
        };
        canvas.draw_hyperlink(hyperlink, z);
    }
}

impl Default for Link {
    fn default() -> Self {
        Self::new("Link", "https://example.com", [0.0, 0.0], 16.0)
    }
}

// ===== Event Handling - Standard hyperlink interaction =====

/// Result of a link click event
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LinkClickResult {
    /// The link was clicked (should open URL)
    Clicked,
    /// Click was outside the link
    Ignored,
}

/// Keyboard keys that link responds to
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LinkKey {
    /// Enter key - activates link (opens URL)
    Enter,
    /// Space key - activates link (opens URL)
    Space,
    /// Tab key - moves focus (handled by focus manager)
    Tab,
    /// Other keys - ignored
    Other,
}

/// Result of link keyboard event handling
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LinkKeyResult {
    /// Key was handled and link was activated
    Activated,
    /// Key was ignored
    Ignored,
}

impl Link {
    // ===== Event Handling Methods =====

    /// Calculate the approximate bounding box for the link text
    ///
    /// This is a heuristic based on typical character widths.
    /// For more accurate hit testing, you'd need actual glyph metrics.
    fn get_bounds(&self) -> (f32, f32, f32, f32) {
        let char_width = self.size * 0.5;
        let text_width = self.text.len() as f32 * char_width;
        let text_height = self.size * 1.2;

        // x, y, width, height
        (
            self.pos[0],
            self.pos[1] - self.size * 0.8,
            text_width,
            text_height,
        )
    }

    /// Hit test the link text
    ///
    /// Returns true if the point is inside the link's clickable area.
    pub fn hit_test(&self, x: f32, y: f32) -> bool {
        let (bx, by, bw, bh) = self.get_bounds();
        x >= bx && x <= bx + bw && y >= by && y <= by + bh
    }

    /// Handle click event on the link
    ///
    /// Returns LinkClickResult indicating whether the link was clicked.
    pub fn handle_click(&self, x: f32, y: f32) -> LinkClickResult {
        if self.hit_test(x, y) {
            LinkClickResult::Clicked
        } else {
            LinkClickResult::Ignored
        }
    }

    /// Handle keyboard input when link is focused
    ///
    /// Enter and Space keys activate the link (open URL).
    /// Returns LinkKeyResult indicating how the key was handled.
    pub fn handle_link_key(&mut self, key: LinkKey) -> LinkKeyResult {
        match key {
            LinkKey::Enter | LinkKey::Space => {
                // Activate link (open URL)
                // Actual URL opening is handled by the event router/application
                LinkKeyResult::Activated
            }
            _ => LinkKeyResult::Ignored,
        }
    }

    // ===== Utility Methods =====

    /// Check if a point is inside this link's clickable area
    ///
    /// This is an alias for hit_test() to match the interface of other elements.
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        self.hit_test(x, y)
    }

    /// Get the URL of this link
    pub fn get_url(&self) -> &str {
        &self.url
    }

    /// Set the URL of this link
    pub fn set_url(&mut self, url: String) {
        self.url = url;
    }

    /// Check if this link is focused
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state for this link
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Check if this link is hovered
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Set hover state for this link
    pub fn set_hovered(&mut self, hovered: bool) {
        self.hovered = hovered;
    }
}

// ===== EventHandler Trait Implementation =====

impl crate::event_handler::EventHandler for Link {
    /// Handle mouse click event
    ///
    /// Opens the link URL if clicked.
    /// Note: Actual URL opening is handled by the application layer.
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
            LinkClickResult::Clicked => {
                // Link was clicked - application should open the URL
                // The URL can be retrieved via get_url()
                crate::event_handler::EventResult::Handled
            }
            LinkClickResult::Ignored => crate::event_handler::EventResult::Ignored,
        }
    }

    /// Handle keyboard input event
    ///
    /// Enter and Space keys activate the link when focused.
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

        // Map KeyCode to LinkKey
        let link_key = match event.key {
            KeyCode::Enter => LinkKey::Enter,
            KeyCode::Space => LinkKey::Space,
            KeyCode::Tab => LinkKey::Tab,
            _ => LinkKey::Other,
        };

        // Handle the key
        match self.handle_link_key(link_key) {
            LinkKeyResult::Activated => crate::event_handler::EventResult::Handled,
            LinkKeyResult::Ignored => crate::event_handler::EventResult::Ignored,
        }
    }

    /// Handle mouse move event (for hover states)
    fn handle_mouse_move(
        &mut self,
        event: crate::event_handler::MouseMoveEvent,
    ) -> crate::event_handler::EventResult {
        let was_hovered = self.hovered;
        self.hovered = self.contains_point(event.x, event.y);

        // Return Handled if hover state changed (to trigger redraw)
        if was_hovered != self.hovered {
            crate::event_handler::EventResult::Handled
        } else {
            crate::event_handler::EventResult::Ignored
        }
    }

    /// Check if this link is focused
    fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state for this link
    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Check if the point is inside this link
    fn contains_point(&self, x: f32, y: f32) -> bool {
        self.contains_point(x, y)
    }
}
