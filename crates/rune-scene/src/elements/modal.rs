use crate::ir_renderer::HitRegionRegistry;
use engine_core::{Brush, ColorLinPremul, Rect, RoundedRadii, RoundedRect};
use rune_ir::view::ViewNodeId;
use rune_surface::Canvas;
use rune_surface::shapes;

/// Result of a modal click event
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModalClickResult {
    /// Close button was clicked
    CloseButton,
    /// A modal button was clicked (with button index)
    Button(usize),
    /// Background/scrim was clicked (outside panel)
    Background,
    /// Panel was clicked but not a specific interactive element
    Panel,
    /// Click was not handled by modal
    Ignored,
}

/// Button configuration for modal buttons
#[derive(Clone)]
pub struct ModalButton {
    pub label: String,
    pub primary: bool, // Primary button gets different styling
}

impl ModalButton {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            primary: false,
        }
    }

    pub fn primary(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            primary: true,
        }
    }
}

/// Modal window with translucent overlay, centered panel, close button, and customizable content
pub struct ModalLayout {
    pub panel: Rect,
    pub title_pos: [f32; 2],
    pub content_origin: [f32; 2],
    pub content_line_height: f32,
    pub button_rects: Vec<Rect>,
    pub close_button_rect: Rect,
}

/// Modal window with translucent overlay, centered panel, close button, and customizable content
pub struct Modal {
    /// Screen dimensions for centering and overlay
    pub screen_width: f32,
    pub screen_height: f32,

    /// Modal panel dimensions
    pub panel_width: f32,
    pub panel_height: f32,

    /// Title text
    pub title: String,

    /// Content text (can be multi-line)
    pub content: String,

    /// Buttons to render at the bottom
    pub buttons: Vec<ModalButton>,

    /// Visual styling
    pub overlay_color: ColorLinPremul,
    pub panel_bg: ColorLinPremul,
    pub panel_border_color: ColorLinPremul,
    pub title_color: ColorLinPremul,
    pub content_color: ColorLinPremul,
    pub close_icon_color: ColorLinPremul,

    /// Font sizes
    pub title_size: f32,
    pub content_size: f32,
    pub button_label_size: f32,

    /// Border radius for panel
    pub panel_radius: f32,

    /// Base z-index (overlay and panel will use z and z+increments)
    pub base_z: i32,

/// Whether to draw the panel shadow
pub show_shadow: bool,

/// Whether clicking on the background scrim (outside the panel)
/// should close the modal. This is only used by higher-level
/// hit-testing code; the renderer itself is unaware of this flag.
pub close_on_background_click: bool,
}

impl Modal {
    /// Color used for the modal scrim overlay.
    pub fn scrim_color(&self) -> ColorLinPremul {
        ColorLinPremul::from_srgba_u8([0, 0, 0, 140])
    }

    /// Create a new modal with default styling
    pub fn new(
        screen_width: f32,
        screen_height: f32,
        title: impl Into<String>,
        content: impl Into<String>,
        buttons: Vec<ModalButton>,
    ) -> Self {
        Self {
            screen_width,
            screen_height,
            panel_width: 480.0,
            panel_height: 300.0,
            title: title.into(),
            content: content.into(),
            buttons,
            // Semi-transparent dark overlay to dim background
            overlay_color: ColorLinPremul::from_srgba_u8([0, 0, 0, 190]),
            // Light panel background
            panel_bg: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
            panel_border_color: ColorLinPremul::from_srgba_u8([200, 200, 200, 255]),
            title_color: ColorLinPremul::from_srgba_u8([20, 20, 20, 255]),
            content_color: ColorLinPremul::from_srgba_u8([60, 60, 60, 255]),
            close_icon_color: ColorLinPremul::from_srgba_u8([100, 100, 100, 255]),
            title_size: 20.0,
            content_size: 14.0,
            button_label_size: 14.0,
            panel_radius: 8.0,
            base_z: 9500, // Above everything including dropdowns (8000)
            show_shadow: true,
            close_on_background_click: true,
        }
    }

    /// Configure whether clicking the background/scrim (outside the panel)
    /// should close the modal. This is an optional behavior flag used by
    /// hit-testing logic in the host app.
    pub fn with_close_on_background_click(mut self, close: bool) -> Self {
        self.close_on_background_click = close;
        self
    }

    /// Get the panel rectangle (centered on screen)
    pub fn get_panel_rect(&self) -> Rect {
        Rect {
            x: (self.screen_width - self.panel_width) * 0.5,
            y: (self.screen_height - self.panel_height) * 0.5,
            w: self.panel_width,
            h: self.panel_height,
        }
    }

    /// Compute layout information (panel, title/content positions, buttons, close button).
    /// This allows callers to render custom content inside the modal while reusing the
    /// standard panel geometry and spacing.
    pub fn layout(&self) -> ModalLayout {
        let panel = self.get_panel_rect();
        let close_button_rect = self.get_close_button_rect();
        let button_rects = self.get_button_rects();

        let title_pos = [panel.x + 20.0, panel.y + 30.0];
        let content_origin = [panel.x + 20.0, title_pos[1] + 40.0];
        let content_line_height = self.content_size * 1.4;

        ModalLayout {
            panel,
            title_pos,
            content_origin,
            content_line_height,
            button_rects,
            close_button_rect,
        }
    }

    /// Get the close button rectangle (top right of panel)
    pub fn get_close_button_rect(&self) -> Rect {
        let panel = self.get_panel_rect();
        let close_size = 32.0;
        Rect {
            x: panel.x + panel.w - close_size - 8.0,
            y: panel.y + 8.0,
            w: close_size,
            h: close_size,
        }
    }

    /// Get rectangles for all buttons
    pub fn get_button_rects(&self) -> Vec<Rect> {
        let panel = self.get_panel_rect();
        let button_height = 36.0;
        let button_spacing = 12.0;
        let button_margin_bottom = 20.0;
        let button_width = 100.0;

        let num_buttons = self.buttons.len();
        if num_buttons == 0 {
            return vec![];
        }

        // Calculate total width needed for all buttons
        let total_width =
            (button_width * num_buttons as f32) + (button_spacing * (num_buttons - 1) as f32);

        // Start x position for first button (centered)
        let start_x = panel.x + (panel.w - total_width) * 0.5;
        let y = panel.y + panel.h - button_margin_bottom - button_height;

        (0..num_buttons)
            .map(|i| Rect {
                x: start_x + (button_width + button_spacing) * i as f32,
                y,
                w: button_width,
                h: button_height,
            })
            .collect()
    }

    /// Compute scrim bands (top, bottom, left, right) that darken everything
    /// except the modal panel. Useful for overlay renderers that want to
    /// reproduce the legacy four-band approach without duplicating geometry
    /// logic. Bands with zero size are omitted.
    ///
    /// Coordinates are in viewport-local space (0,0 is top-left of viewport).
    /// The canvas transform will position them at the correct screen location.
    pub fn scrim_bands(&self, viewport_width: f32, viewport_height: f32) -> Vec<Rect> {
        let panel = self.get_panel_rect();

        let mut bands = Vec::with_capacity(16);
        let _r = self.panel_radius;
        let _slices = 8;

        // Main bands with exact rectangular hole (no overlap with panel)
        // Top band
        if panel.y > 0.0 {
            bands.push(Rect {
                x: 0.0,
                y: 0.0,
                w: viewport_width,
                h: panel.y,
            });
        }

        // Bottom band
        let panel_bottom = panel.y + panel.h;
        if panel_bottom < viewport_height {
            bands.push(Rect {
                x: 0.0,
                y: panel_bottom,
                w: viewport_width,
                h: viewport_height - panel_bottom,
            });
        }

        // Left band
        if panel.x > 0.0 {
            bands.push(Rect {
                x: 0.0,
                y: panel.y,
                w: panel.x,
                h: panel.h,
            });
        }

        // Right band
        let panel_right = panel.x + panel.w;
        if panel_right < viewport_width {
            bands.push(Rect {
                x: panel_right,
                y: panel.y,
                w: viewport_width - panel_right,
                h: panel.h,
            });
        }

        bands
    }

    /// Render the modal using its built-in title, content, and buttons.
    pub fn render(&self, canvas: &mut Canvas) {
        let z = self.base_z;

        self.render_chrome(canvas, z);
        self.render_default_content(canvas, z);
    }

    /// Render the modal chrome (shadow, panel, border, close icon) without any content.
    /// This is useful when callers want to supply their own content inside the panel.
    pub fn render_chrome(&self, canvas: &mut Canvas, z: i32) {
        let layout = self.layout();
        let panel = layout.panel;

        // Shadow disabled for now - investigating corner artifacts
        let _ = self.show_shadow;

        // 2. Render centered panel (background + border in one call)
        let panel_rrect = RoundedRect {
            rect: panel,
            radii: RoundedRadii {
                tl: self.panel_radius,
                tr: self.panel_radius,
                br: self.panel_radius,
                bl: self.panel_radius,
            },
        };

        // Panel background and border (at base z, text renders above)
        shapes::draw_rounded_rectangle(
            canvas,
            panel_rrect,
            Some(Brush::Solid(self.panel_bg)),
            Some(1.0),
            Some(Brush::Solid(self.panel_border_color)),
            z,
        );

        // 3. Render close button (X icon) in top right
        let close_btn = layout.close_button_rect;
        let close_center_x = close_btn.x + close_btn.w * 0.5;
        let close_center_y = close_btn.y + close_btn.h * 0.5;

        // Draw X using two diagonal lines
        let cross_size = 10.0;
        let line_thickness = 2.0;

        // Line 1: top-left to bottom-right
        self.draw_line(
            canvas,
            close_center_x - cross_size * 0.5,
            close_center_y - cross_size * 0.5,
            close_center_x + cross_size * 0.5,
            close_center_y + cross_size * 0.5,
            line_thickness,
            self.close_icon_color,
            z + 3,
        );

        // Line 2: top-right to bottom-left
        self.draw_line(
            canvas,
            close_center_x + cross_size * 0.5,
            close_center_y - cross_size * 0.5,
            close_center_x - cross_size * 0.5,
            close_center_y + cross_size * 0.5,
            line_thickness,
            self.close_icon_color,
            z + 3,
        );
    }

    /// Render the default title, content text, and buttons inside the modal panel.
    /// Callers that want fully custom content can skip this and draw into the
    /// layout returned by `layout()`.
    pub fn render_default_content(&self, canvas: &mut Canvas, z: i32) {
        let layout = self.layout();

        // Title (render above panel background)
        canvas.draw_text_run(
            layout.title_pos,
            self.title.clone(),
            self.title_size,
            self.title_color,
            z + 10,
        );

        // Content text (multi-line, split by '\n')
        let lines: Vec<&str> = self.content.split('\n').collect();
        for (i, line) in lines.iter().enumerate() {
            let pos = [
                layout.content_origin[0],
                layout.content_origin[1] + i as f32 * layout.content_line_height,
            ];
            canvas.draw_text_run(
                pos,
                line.to_string(),
                self.content_size,
                self.content_color,
                z + 10,
            );
        }

        // Buttons using precomputed button rects
        for (i, (button, rect)) in self
            .buttons
            .iter()
            .zip(layout.button_rects.iter())
            .enumerate()
        {
            self.render_button(canvas, button, *rect, z + 20 + i as i32 * 5);
        }
    }

    /// Render the complete modal overlay with scrim bands and hit regions.
    ///
    /// This is the primary method for rendering modals in the IR renderer.
    /// It handles:
    /// - Scrim bands (darkened areas around the modal) with optional hit regions
    /// - Panel hit region
    /// - Modal chrome (shadow, panel, border, close button)
    /// - Default content (title, content text, buttons)
    /// - Hit regions for buttons and close button
    pub fn render_overlay(
        &self,
        canvas: &mut Canvas,
        hit_registry: &mut HitRegionRegistry,
        overlay_id: &ViewNodeId,
        dismissible: bool,
        show_close: bool,
        viewport_width: f32,
        viewport_height: f32,
    ) {
        let panel_rect = self.get_panel_rect();
        let overlay_z = self.base_z - 10; // Scrim below modal

        // 1. Render scrim bands using the depth-bypass scrim pipeline so background
        // content stays visible and the panel remains undimmed.
        let scrim_color = self.scrim_color();
        let panel_rrect = RoundedRect {
            rect: panel_rect,
            radii: RoundedRadii {
                tl: self.panel_radius,
                tr: self.panel_radius,
                br: self.panel_radius,
                bl: self.panel_radius,
            },
        };
        canvas.fill_scrim_with_cutout(panel_rrect, scrim_color);

        // Register scrim hit region for dismissal (outside panel area)
        // Use bands for hit testing to exclude the panel area
        if dismissible {
            let scrim_region_id = hit_registry.register(&format!("__scrim__{}", overlay_id));
            let full = Rect {
                x: 0.0,
                y: 0.0,
                w: viewport_width,
                h: viewport_height,
            };
            canvas.hit_region_rect(scrim_region_id, full, overlay_z + 1);
        }

        // 2. Register panel hit region
        let panel_region_id = hit_registry.register(overlay_id);
        canvas.hit_region_rect(panel_region_id, panel_rect, self.base_z + 30);

        // 3. Render modal chrome (shadow, panel, border, close icon)
        self.render_chrome(canvas, self.base_z);

        // 4. Render default content (title, content text, buttons)
        self.render_default_content(canvas, self.base_z);

        // 5. Register hit regions for buttons
        let layout = self.layout();
        for (i, rect) in layout.button_rects.iter().enumerate() {
            let region_id = hit_registry.register(&format!("__modalbtn{}__{}", i, overlay_id));
            canvas.hit_region_rect(region_id, *rect, self.base_z + 60 + i as i32);
        }

        // 6. Register hit region for close button
        if show_close {
            let close_rect = layout.close_button_rect;
            let close_region_id = hit_registry.register(&format!("__close__{}", overlay_id));
            canvas.hit_region_rect(close_region_id, close_rect, self.base_z + 70);
        }
    }

    /// Render a single button
    pub(crate) fn render_button(
        &self,
        canvas: &mut Canvas,
        button: &ModalButton,
        rect: Rect,
        z: i32,
    ) {
        let radius = 6.0;
        let rrect = RoundedRect {
            rect,
            radii: RoundedRadii {
                tl: radius,
                tr: radius,
                br: radius,
                bl: radius,
            },
        };

        // Button colors based on primary/secondary
        let (bg_color, fg_color, border_color) = if button.primary {
            (
                ColorLinPremul::from_srgba_u8([59, 130, 246, 255]), // Blue
                ColorLinPremul::from_srgba_u8([255, 255, 255, 255]), // White text
                ColorLinPremul::from_srgba_u8([37, 99, 235, 255]),  // Darker blue border
            )
        } else {
            (
                ColorLinPremul::from_srgba_u8([240, 240, 240, 255]), // Light gray
                ColorLinPremul::from_srgba_u8([60, 60, 60, 255]),    // Dark text
                ColorLinPremul::from_srgba_u8([200, 200, 200, 255]), // Gray border
            )
        };

        // Draw rounded background
        canvas.rounded_rect(rrect, Brush::Solid(bg_color), z);

        // Draw border
        shapes::draw_rounded_rectangle(
            canvas,
            rrect,
            None,
            Some(1.0),
            Some(Brush::Solid(border_color)),
            z + 1,
        );

        // Draw label (centered)
        let approx_text_width = button.label.len() as f32 * self.button_label_size * 0.5;
        let text_x = rect.x + (rect.w - approx_text_width) * 0.5;
        let text_y = rect.y + rect.h * 0.5 + self.button_label_size * 0.35;

        canvas.draw_text_run(
            [text_x, text_y],
            button.label.clone(),
            self.button_label_size,
            fg_color,
            z + 2,
        );
    }

    /// Helper to draw a line (using a rotated rectangle)
    fn draw_line(
        &self,
        canvas: &mut Canvas,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        thickness: f32,
        color: ColorLinPremul,
        z: i32,
    ) {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let length = (dx * dx + dy * dy).sqrt();

        // Use a simple rectangle for the line
        // This is a simplified approach - for production you might want proper rotation

        // Draw as a series of small rectangles to approximate the line
        let steps = (length / 1.0).ceil() as i32;
        for i in 0..steps {
            let t = i as f32 / steps as f32;
            let x = x1 + dx * t;
            let y = y1 + dy * t;

            canvas.fill_rect(
                x - thickness * 0.5,
                y - thickness * 0.5,
                thickness,
                thickness,
                Brush::Solid(color),
                z,
            );
        }
    }

    // =========================================================================
    // EVENT HANDLING METHODS
    // =========================================================================

    /// Hit test the close button
    pub fn hit_test_close_button(&self, x: f32, y: f32) -> bool {
        let close_btn = self.get_close_button_rect();
        x >= close_btn.x
            && x <= close_btn.x + close_btn.w
            && y >= close_btn.y
            && y <= close_btn.y + close_btn.h
    }

    /// Hit test modal buttons, returning the button index if hit
    pub fn hit_test_buttons(&self, x: f32, y: f32) -> Option<usize> {
        let button_rects = self.get_button_rects();
        for (idx, btn_rect) in button_rects.iter().enumerate() {
            if x >= btn_rect.x
                && x <= btn_rect.x + btn_rect.w
                && y >= btn_rect.y
                && y <= btn_rect.y + btn_rect.h
            {
                return Some(idx);
            }
        }
        None
    }

    /// Hit test the modal panel
    pub fn hit_test_panel(&self, x: f32, y: f32) -> bool {
        let panel_rect = self.get_panel_rect();
        x >= panel_rect.x
            && x <= panel_rect.x + panel_rect.w
            && y >= panel_rect.y
            && y <= panel_rect.y + panel_rect.h
    }

    /// Handle click event on the modal
    /// Returns ModalClickResult indicating what was clicked
    pub fn handle_click(&self, x: f32, y: f32) -> ModalClickResult {
        // Check if click is on panel first
        if self.hit_test_panel(x, y) {
            // Check close button
            if self.hit_test_close_button(x, y) {
                return ModalClickResult::CloseButton;
            }

            // Check modal buttons
            if let Some(button_idx) = self.hit_test_buttons(x, y) {
                return ModalClickResult::Button(button_idx);
            }

            // Click was on panel but not on any interactive element
            return ModalClickResult::Panel;
        } else {
            // Click was outside panel (background/scrim)
            return ModalClickResult::Background;
        }
    }

    /// Check if a point is inside the modal (panel or background)
    /// This always returns true since modals capture all input when visible
    pub fn contains_point(&self, _x: f32, _y: f32) -> bool {
        // Modal captures all input when visible (fullscreen overlay)
        true
    }
}

// ===== Keyboard Event Handling =====

/// Keyboard keys that modal responds to
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModalKey {
    /// Escape key - closes modal
    Escape,
    /// Enter key - activates primary button
    Enter,
    /// Tab key - cycles focus between buttons
    Tab,
    /// Other keys - ignored
    Other,
}

/// Result of modal keyboard event handling
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModalKeyResult {
    /// Modal should be closed
    Close,
    /// Primary button should be activated
    ActivatePrimary,
    /// Navigate to next button
    Navigate,
    /// Key was ignored
    Ignored,
}

impl Modal {
    /// Handle keyboard input when modal is open
    ///
    /// Escape key closes the modal.
    /// Enter key activates the primary button.
    /// Returns ModalKeyResult indicating how the key was handled.
    pub fn handle_modal_key(&self, key: ModalKey) -> ModalKeyResult {
        match key {
            ModalKey::Escape => ModalKeyResult::Close,
            ModalKey::Enter => {
                // Activate primary button if exists
                if self.buttons.iter().any(|b| b.primary) {
                    ModalKeyResult::ActivatePrimary
                } else {
                    ModalKeyResult::Ignored
                }
            }
            ModalKey::Tab => ModalKeyResult::Navigate,
            _ => ModalKeyResult::Ignored,
        }
    }

    /// Get the index of the primary button, if any
    pub fn get_primary_button_index(&self) -> Option<usize> {
        self.buttons.iter().position(|b| b.primary)
    }
}

// ===== EventHandler Trait Implementation =====

impl crate::event_handler::EventHandler for Modal {
    /// Handle mouse click event
    ///
    /// Returns Handled for any click (modals capture all input).
    /// The ModalClickResult can be retrieved via handle_click().
    fn handle_mouse_click(
        &mut self,
        event: crate::event_handler::MouseClickEvent,
    ) -> crate::event_handler::EventResult {
        use winit::event::ElementState;

        // Only handle left mouse button press
        if event.button != winit::event::MouseButton::Left || event.state != ElementState::Pressed {
            return crate::event_handler::EventResult::Ignored;
        }

        // Modal always captures clicks (it's a fullscreen overlay)
        // The specific action is determined by handle_click()
        let _result = self.handle_click(event.x, event.y);
        crate::event_handler::EventResult::Handled
    }

    /// Handle keyboard input event
    ///
    /// Escape closes the modal, Enter activates primary button.
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

        // Map KeyCode to ModalKey
        let modal_key = match event.key {
            KeyCode::Escape => ModalKey::Escape,
            KeyCode::Enter => ModalKey::Enter,
            KeyCode::Tab => ModalKey::Tab,
            _ => ModalKey::Other,
        };

        // Handle the key
        match self.handle_modal_key(modal_key) {
            ModalKeyResult::Close => crate::event_handler::EventResult::Handled,
            ModalKeyResult::ActivatePrimary => crate::event_handler::EventResult::Handled,
            ModalKeyResult::Navigate => crate::event_handler::EventResult::Handled,
            ModalKeyResult::Ignored => crate::event_handler::EventResult::Ignored,
        }
    }

    /// Handle mouse move event
    ///
    /// Modal captures mouse move to prevent hover on underlying elements.
    fn handle_mouse_move(
        &mut self,
        _event: crate::event_handler::MouseMoveEvent,
    ) -> crate::event_handler::EventResult {
        // Modal captures all mouse movement
        crate::event_handler::EventResult::Handled
    }

    /// Check if the point is inside this modal
    ///
    /// Always returns true since modals are fullscreen overlays.
    fn contains_point(&self, x: f32, y: f32) -> bool {
        self.contains_point(x, y)
    }
}
