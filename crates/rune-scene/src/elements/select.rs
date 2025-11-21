use crate::ir_adapter;
use engine_core::{Brush, Color, ColorLinPremul, Rect, RoundedRadii, RoundedRect, SvgStyle};
use rune_surface::Canvas;
use rune_surface::shapes::{self};

#[derive(Clone)]
pub struct Select {
    pub rect: Rect,
    pub label: String,
    pub label_size: f32,
    pub label_color: ColorLinPremul,
    pub open: bool,
    pub focused: bool,
    pub options: Vec<String>,
    pub selected_index: Option<usize>,
    pub padding_left: f32,
    pub padding_right: f32,
    pub padding_top: f32,
    pub padding_bottom: f32,
    pub bg_color: ColorLinPremul,
    pub border_color: ColorLinPremul,
    pub border_width: f32,
    pub radius: f32,
}

impl Select {
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

        // Background
        canvas.rounded_rect(rrect, Brush::Solid(self.bg_color), z);

        // Border
        let border_color = if self.focused {
            Color::rgba(63, 130, 246, 255)
        } else {
            self.border_color
        };
        let border_width = if self.focused {
            (self.border_width + 1.0).max(2.0)
        } else {
            self.border_width.max(1.0)
        };
        shapes::draw_rounded_rectangle(
            canvas,
            rrect,
            None,
            Some(border_width),
            Some(Brush::Solid(border_color)),
            z + 1,
        );

        // Label
        let content_w = (self.rect.w - self.padding_left - self.padding_right).max(0.0);
        let content_h = (self.rect.h - self.padding_top - self.padding_bottom).max(0.0);
        let tp = [
            self.rect.x + self.padding_left,
            self.rect.y + self.padding_top + content_h * 0.5 + self.label_size * 0.35,
        ];
        canvas.draw_text_run(
            tp,
            self.label.clone(),
            self.label_size,
            self.label_color,
            z + 2,
        );

        // Chevron icon (SVG)
        let icon_size = 20.0;
        let icon_x = (self.rect.x + self.rect.w - self.padding_right - icon_size)
            .max(self.rect.x + self.padding_left + content_w * 0.5);
        let icon_y = self.rect.y + (self.rect.h - icon_size) * 0.5;
        let chevron_path = if self.open {
            "images/chevron-up.svg"
        } else {
            "images/chevron-down.svg"
        };

        // Style the chevron icon with white stroke for maximum visibility
        let icon_style = SvgStyle::new()
            .with_stroke(Color::rgba(60, 70, 80, 255))
            .with_stroke_width(2.5);

        canvas.draw_svg_styled(
            chevron_path,
            [icon_x, icon_y],
            [icon_size, icon_size],
            icon_style,
            z + 3,
        );

        // Render dropdown overlay when open
        if self.open && !self.options.is_empty() {
            self.render_dropdown_overlay(canvas, z + 1000);
        }
    }

    fn render_dropdown_overlay(&self, canvas: &mut Canvas, z: i32) {
        let option_height = 36.0;
        let overlay_padding = 4.0;
        let overlay_height = (self.options.len() as f32 * option_height) + (overlay_padding * 2.0);

        // Position overlay below the select box
        let overlay_rect = Rect {
            x: self.rect.x,
            y: self.rect.y + self.rect.h + 4.0,
            w: self.rect.w,
            h: overlay_height,
        };

        let radius = 6.0;
        let overlay_rrect = RoundedRect {
            rect: overlay_rect,
            radii: RoundedRadii {
                tl: radius,
                tr: radius,
                br: radius,
                bl: radius,
            },
        };

        // Overlay background - solid, opaque background for better visibility
        let overlay_bg = Color::rgba(255, 255, 255, 255);
        canvas.rounded_rect(overlay_rrect, Brush::Solid(overlay_bg), z);

        // Overlay border - 1px
        let overlay_border = self.border_color;
        shapes::draw_rounded_rectangle(
            canvas,
            overlay_rrect,
            None,
            Some(1.0),
            Some(Brush::Solid(overlay_border)),
            z + 1,
        );

        // Render each option
        for (idx, option) in self.options.iter().enumerate() {
            let option_y = overlay_rect.y + overlay_padding + (idx as f32 * option_height);

            // Edge-to-edge highlight rect (no horizontal padding)
            let highlight_rect = Rect {
                x: overlay_rect.x,
                y: option_y,
                w: overlay_rect.w,
                h: option_height,
            };

            // Text rect with padding
            let text_rect = Rect {
                x: overlay_rect.x + overlay_padding + self.padding_left,
                y: option_y + self.padding_top * 0.25,
                w: overlay_rect.w
                    - (overlay_padding * 2.0)
                    - self.padding_left
                    - self.padding_right,
                h: option_height - self.padding_top * 0.5 - self.padding_bottom * 0.25,
            };

            // Highlight selected option - edge to edge
            let is_selected = self.selected_index == Some(idx);
            if is_selected {
                // No rounded corners for edge-to-edge highlight
                let highlight_bg = Color::rgba(220, 220, 224, 255);
                canvas.fill_rect(
                    highlight_rect.x,
                    highlight_rect.y,
                    highlight_rect.w,
                    highlight_rect.h,
                    Brush::Solid(highlight_bg),
                    z + 2,
                );
            }

            // Option text
            let text_x = text_rect.x;
            let text_y = text_rect.y + text_rect.h * 0.5 + self.label_size * 0.35;
            let text_color = if is_selected {
                Color::rgba(20, 24, 30, 255)
            } else {
                Color::rgba(34, 42, 52, 255)
            };

            canvas.draw_text_run(
                [text_x, text_y],
                option.clone(),
                self.label_size,
                text_color,
                z + 3,
            );
        }
    }

    // =========================================================================
    // EVENT HANDLING METHODS
    // =========================================================================

    /// Apply styling from a SurfaceStyle (background/border/padding).
    pub fn apply_surface_style(&mut self, style: &rune_ir::view::SurfaceStyle) {
        if let Some(bg) = &style.background {
            if let rune_ir::view::ViewBackground::Solid { color } = bg {
                if let Some(parsed) = ir_adapter::parse_color(color) {
                    self.bg_color = parsed;
                }
            }
        }

        if let Some(color) = style
            .border_color
            .as_ref()
            .and_then(|c| ir_adapter::parse_color(c))
        {
            self.border_color = color;
        }
        if let Some(width) = style.border_width {
            self.border_width = width as f32;
        }

        // Only override padding if explicitly set (non-zero), otherwise keep defaults
        if style.padding.left > 0.0 {
            self.padding_left = style.padding.left as f32;
        }
        if style.padding.right > 0.0 {
            self.padding_right = style.padding.right as f32;
        }
        if style.padding.top > 0.0 {
            self.padding_top = style.padding.top as f32;
        }
        if style.padding.bottom > 0.0 {
            self.padding_bottom = style.padding.bottom as f32;
        }

        if let Some(radius) = style.corner_radius {
            self.radius = radius as f32;
        }
    }

    /// Get the bounds of the dropdown overlay
    pub fn get_overlay_bounds(&self) -> Option<Rect> {
        if !self.open || self.options.is_empty() {
            return None;
        }

        let option_height = 36.0;
        let overlay_padding = 4.0;
        let overlay_height = (self.options.len() as f32 * option_height) + (overlay_padding * 2.0);

        Some(Rect {
            x: self.rect.x,
            y: self.rect.y + self.rect.h + 4.0,
            w: self.rect.w,
            h: overlay_height,
        })
    }

    /// Toggle the dropdown open/closed
    pub fn toggle_open(&mut self) {
        self.open = !self.open;
    }

    /// Close the dropdown
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Handle click on the select field (not the overlay)
    /// Returns true if the click was handled
    pub fn handle_field_click(&mut self, x: f32, y: f32) -> bool {
        // Check if click is on the field
        if x >= self.rect.x
            && x <= self.rect.x + self.rect.w
            && y >= self.rect.y
            && y <= self.rect.y + self.rect.h
        {
            self.toggle_open();
            true
        } else {
            false
        }
    }

    /// Handle click on the dropdown overlay
    /// Returns true if the click was handled (and an option was selected)
    pub fn handle_overlay_click(&mut self, x: f32, y: f32) -> bool {
        if !self.open || self.options.is_empty() {
            return false;
        }

        let overlay_bounds = match self.get_overlay_bounds() {
            Some(bounds) => bounds,
            None => return false,
        };

        // Check if click is inside overlay
        if x < overlay_bounds.x
            || x > overlay_bounds.x + overlay_bounds.w
            || y < overlay_bounds.y
            || y > overlay_bounds.y + overlay_bounds.h
        {
            return false;
        }

        let option_height = 36.0;
        let overlay_padding = 4.0;

        // Calculate which option was clicked
        let option_local_y = y - overlay_bounds.y - overlay_padding;

        if option_local_y >= 0.0 {
            let option_idx = (option_local_y / option_height) as usize;
            if option_idx < self.options.len() {
                // Update selected index
                self.selected_index = Some(option_idx);

                // Update label to show selected option
                self.label = self.options[option_idx].clone();

                // Close the dropdown
                self.open = false;

                return true;
            }
        }

        false
    }

    /// Hit test to find which option is at the given coordinates
    /// Returns Some(index) if an option is hit, None otherwise
    pub fn hit_test_option(&self, x: f32, y: f32) -> Option<usize> {
        if !self.open || self.options.is_empty() {
            return None;
        }

        let overlay_bounds = self.get_overlay_bounds()?;

        // Check if click is inside overlay
        if x < overlay_bounds.x
            || x > overlay_bounds.x + overlay_bounds.w
            || y < overlay_bounds.y
            || y > overlay_bounds.y + overlay_bounds.h
        {
            return None;
        }

        let option_height = 36.0;
        let overlay_padding = 4.0;

        let option_local_y = y - overlay_bounds.y - overlay_padding;

        if option_local_y >= 0.0 {
            let option_idx = (option_local_y / option_height) as usize;
            if option_idx < self.options.len() {
                return Some(option_idx);
            }
        }

        None
    }

    // ===== Keyboard Event Handling =====

    /// Handle keyboard input when select is focused or open
    ///
    /// Arrow Up/Down navigate options, Enter selects, Escape closes.
    pub fn handle_keyboard(&mut self, key: SelectKey) -> SelectKeyResult {
        match key {
            SelectKey::ArrowDown => {
                if !self.open {
                    // Open dropdown when closed
                    self.open = true;
                    SelectKeyResult::Opened
                } else if !self.options.is_empty() {
                    // Navigate to next option
                    let new_index = match self.selected_index {
                        Some(idx) => {
                            if idx + 1 < self.options.len() {
                                idx + 1
                            } else {
                                idx
                            }
                        }
                        None => 0,
                    };
                    self.selected_index = Some(new_index);
                    self.label = self.options[new_index].clone();
                    SelectKeyResult::Navigated
                } else {
                    SelectKeyResult::Ignored
                }
            }
            SelectKey::ArrowUp => {
                if self.open && !self.options.is_empty() {
                    // Navigate to previous option
                    let new_index = match self.selected_index {
                        Some(idx) => {
                            if idx > 0 {
                                idx - 1
                            } else {
                                idx
                            }
                        }
                        None => 0,
                    };
                    self.selected_index = Some(new_index);
                    self.label = self.options[new_index].clone();
                    SelectKeyResult::Navigated
                } else {
                    SelectKeyResult::Ignored
                }
            }
            SelectKey::Enter => {
                if self.open {
                    // Close dropdown and confirm selection
                    self.open = false;
                    SelectKeyResult::Selected
                } else {
                    // Open dropdown
                    self.open = true;
                    SelectKeyResult::Opened
                }
            }
            SelectKey::Escape => {
                if self.open {
                    // Close dropdown without changing selection
                    self.open = false;
                    SelectKeyResult::Closed
                } else {
                    SelectKeyResult::Ignored
                }
            }
            SelectKey::Space => {
                // Space toggles the dropdown
                self.toggle_open();
                if self.open {
                    SelectKeyResult::Opened
                } else {
                    SelectKeyResult::Closed
                }
            }
            _ => SelectKeyResult::Ignored,
        }
    }

    // ===== Utility Methods =====

    /// Check if this select is focused
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state for this select
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Check if a point is inside this select (field or dropdown)
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        // Check if click is on the field
        if x >= self.rect.x
            && x <= self.rect.x + self.rect.w
            && y >= self.rect.y
            && y <= self.rect.y + self.rect.h
        {
            return true;
        }

        // Check if click is on the dropdown overlay (if open)
        if let Some(overlay_bounds) = self.get_overlay_bounds() {
            if x >= overlay_bounds.x
                && x <= overlay_bounds.x + overlay_bounds.w
                && y >= overlay_bounds.y
                && y <= overlay_bounds.y + overlay_bounds.h
            {
                return true;
            }
        }

        false
    }

    /// Check if the dropdown is currently open
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Get the selected index
    pub fn get_selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Set the selected index and update the label
    pub fn set_selected_index(&mut self, index: Option<usize>) {
        self.selected_index = index;
        if let Some(idx) = index {
            if idx < self.options.len() {
                self.label = self.options[idx].clone();
            }
        }
    }

    /// Get the selected option text
    pub fn get_selected_option(&self) -> Option<&String> {
        self.selected_index.and_then(|idx| self.options.get(idx))
    }
}

// ===== Keyboard Event Handling Enums =====

/// Keys that the select dropdown responds to
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SelectKey {
    ArrowUp,
    ArrowDown,
    Enter,
    Escape,
    Space,
    Tab,
    Other,
}

/// Result of select keyboard handling
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SelectKeyResult {
    /// Dropdown was opened
    Opened,
    /// Dropdown was closed
    Closed,
    /// Navigation occurred (selected option changed)
    Navigated,
    /// Option was selected and dropdown closed
    Selected,
    /// Key was not handled
    Ignored,
}

// ===== EventHandler Trait Implementation =====

impl crate::event_handler::EventHandler for Select {
    /// Handle mouse click event
    ///
    /// Handles clicks on the field (to toggle dropdown) and overlay options.
    fn handle_mouse_click(
        &mut self,
        event: crate::event_handler::MouseClickEvent,
    ) -> crate::event_handler::EventResult {
        use winit::event::ElementState;

        // Only handle left mouse button press
        if event.button != winit::event::MouseButton::Left || event.state != ElementState::Pressed {
            return crate::event_handler::EventResult::Ignored;
        }

        // Try handling overlay click first (if open)
        if self.open && self.handle_overlay_click(event.x, event.y) {
            return crate::event_handler::EventResult::Handled;
        }

        // Try handling field click (to toggle dropdown)
        if self.handle_field_click(event.x, event.y) {
            return crate::event_handler::EventResult::Handled;
        }

        crate::event_handler::EventResult::Ignored
    }

    /// Handle keyboard input event
    ///
    /// Arrow keys navigate options, Enter selects, Escape closes.
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

        // Only handle keyboard events if focused or dropdown is open
        if !self.focused && !self.open {
            return crate::event_handler::EventResult::Ignored;
        }

        // Map KeyCode to SelectKey
        let select_key = match event.key {
            KeyCode::ArrowUp => SelectKey::ArrowUp,
            KeyCode::ArrowDown => SelectKey::ArrowDown,
            KeyCode::Enter => SelectKey::Enter,
            KeyCode::Escape => SelectKey::Escape,
            KeyCode::Space => SelectKey::Space,
            KeyCode::Tab => SelectKey::Tab,
            _ => SelectKey::Other,
        };

        // Handle the key
        match self.handle_keyboard(select_key) {
            SelectKeyResult::Opened => crate::event_handler::EventResult::Handled,
            SelectKeyResult::Closed => crate::event_handler::EventResult::Handled,
            SelectKeyResult::Navigated => crate::event_handler::EventResult::Handled,
            SelectKeyResult::Selected => crate::event_handler::EventResult::Handled,
            SelectKeyResult::Ignored => crate::event_handler::EventResult::Ignored,
        }
    }

    /// Check if this select is focused
    fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state for this select
    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Check if the point is inside this select
    fn contains_point(&self, x: f32, y: f32) -> bool {
        self.contains_point(x, y)
    }
}
