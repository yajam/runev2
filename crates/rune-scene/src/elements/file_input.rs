use engine_core::{Brush, Color, ColorLinPremul, Rect, RoundedRadii, RoundedRect};
use rune_surface::Canvas;
use std::path::PathBuf;

/// FileInput element for single or multi-file selection
///
/// Displays a label and a button that opens a native file picker dialog.
/// Shows selected file names when files are chosen.
pub struct FileInput {
    /// Position and size of the entire file input area
    pub rect: Rect,
    /// Label displayed above the file input (e.g., "Picture", "Documents")
    pub label: Option<String>,
    /// Label text size
    pub label_size: f32,
    /// Label text color
    pub label_color: ColorLinPremul,
    /// Selected file paths
    pub selected_files: Vec<PathBuf>,
    /// Allow multiple file selection
    pub multi: bool,
    /// Placeholder text when no files selected
    pub placeholder: String,
    /// File type filter (e.g., Some(vec!["png", "jpg"]))
    pub accept: Option<Vec<String>>,
    /// Focus state
    pub focused: bool,
    /// Background color
    pub bg_color: ColorLinPremul,
    /// Button background color
    pub button_bg_color: ColorLinPremul,
    /// Button text color
    pub button_text_color: ColorLinPremul,
    /// File name text color
    pub file_text_color: ColorLinPremul,
    /// Corner radius for rounded corners
    pub radius: f32,
}

impl FileInput {
    /// Create a new FileInput with default styling
    pub fn new(rect: Rect, multi: bool) -> Self {
        Self {
            rect,
            label: None,
            label_size: 14.0,
            // Default label + text color: black
            label_color: ColorLinPremul::from_srgba_u8([0, 0, 0, 255]),
            selected_files: Vec::new(),
            multi,
            placeholder: if multi {
                "Choose files".to_string()
            } else {
                "Choose File".to_string()
            },
            accept: None,
            focused: false,
            // Default background: transparent so parent/background style shows through
            bg_color: ColorLinPremul::from_srgba_u8([0, 0, 0, 0]),
            button_bg_color: ColorLinPremul::from_srgba_u8([230, 230, 230, 255]),
            // Default placeholder + file name text color: black
            button_text_color: ColorLinPremul::from_srgba_u8([0, 0, 0, 255]),
            file_text_color: ColorLinPremul::from_srgba_u8([0, 0, 0, 255]),
            radius: 4.0,
        }
    }

    /// Apply styling from a SurfaceStyle (background, radius, etc.).
    pub fn apply_surface_style(&mut self, style: &rune_ir::view::SurfaceStyle) {
        if let Some(bg) = &style.background {
            if let rune_ir::view::ViewBackground::Solid { color } = bg {
                if let Some(parsed) = crate::ir_adapter::parse_color(color) {
                    self.bg_color = parsed;
                }
            }
        }

        if let Some(radius) = style.corner_radius {
            self.radius = radius as f32;
        }
    }

    /// Set the label text
    pub fn with_label(mut self, label: String) -> Self {
        self.label = Some(label);
        self
    }

    /// Set the placeholder text
    pub fn with_placeholder(mut self, placeholder: String) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// Set the file type filter
    pub fn with_accept(mut self, accept: Vec<String>) -> Self {
        self.accept = Some(accept);
        self
    }

    /// Get the button rect (the "Choose File" button area)
    fn button_rect(&self) -> Rect {
        let label_height = if self.label.is_some() {
            self.label_size * 1.5
        } else {
            0.0
        };

        let button_width = 120.0;
        let button_height = 40.0;

        Rect {
            x: self.rect.x,
            y: self.rect.y + label_height,
            w: button_width,
            h: button_height,
        }
    }

    /// Get the full container rect (button + file display area)
    fn container_rect(&self) -> Rect {
        let button_rect = self.button_rect();
        Rect {
            x: button_rect.x,
            y: button_rect.y,
            w: self.rect.w,
            h: button_rect.h,
        }
    }

    /// Render the file input element
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        // Render label if present
        if let Some(label_text) = &self.label {
            let label_y = self.rect.y + self.label_size;
            canvas.draw_text_run(
                [self.rect.x, label_y],
                label_text.clone(),
                self.label_size,
                self.label_color,
                z,
            );
        }

        let button_rect = self.button_rect();

        // Calculate the main input container rect (button + file icon + filename)
        let container_rect = Rect {
            x: button_rect.x,
            y: button_rect.y,
            w: self.rect.w,
            h: button_rect.h,
        };

        // Render container background with full rounded corners
        let container_rrect = RoundedRect {
            rect: container_rect,
            radii: RoundedRadii {
                tl: self.radius,
                tr: self.radius,
                br: self.radius,
                bl: self.radius,
            },
        };

        canvas.rounded_rect(container_rrect, Brush::Solid(self.bg_color), z + 1);

        // Container border
        rune_surface::shapes::draw_rounded_rectangle(
            canvas,
            container_rrect,
            None,
            Some(1.0),
            Some(Brush::Solid(Color::rgba(200, 200, 200, 255))),
            z + 2,
        );

        // Button text (no background, just text)
        let button_text = self.placeholder.clone();
        let text_x = button_rect.x + 12.0;
        let text_y = button_rect.y + button_rect.h * 0.5 + 5.0;
        canvas.draw_text_run(
            [text_x, text_y],
            button_text.clone(),
            13.0,
            self.button_text_color,
            z + 3,
        );

        // Calculate position after button text
        let text_width = button_text.len() as f32 * 7.5;
        let after_text_x = text_x + text_width + 12.0;

        // File icon and filename only shown when files are selected
        if !self.selected_files.is_empty() {
            // Draw file icon using SVG
            let icon_size = 16.0;
            let icon_x = after_text_x;
            let icon_y = button_rect.y + (button_rect.h - icon_size) * 0.5;

            canvas.draw_svg(
                "images/file.svg",
                [icon_x, icon_y],
                [icon_size, icon_size],
                z + 3,
            );

            // Display selected file name inline (next to icon)
            let file_name_x = icon_x + icon_size + 8.0;
            let file_text_y = button_rect.y + button_rect.h * 0.5 + 5.0;
            let file_name = if self.multi {
                self.selected_files
                    .iter()
                    .filter_map(|p| p.file_name().and_then(|n| n.to_str()))
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                self.selected_files
                    .first()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string()
            };

            // Truncate if too long. Compute an approximate character budget based on
            // available width, then trim on Unicode scalar boundaries to avoid
            // slicing in the middle of a UTF-8 codepoint (which would panic).
            let available_width = container_rect.x + container_rect.w - file_name_x - 12.0;
            let approx_max = if available_width > 0.0 {
                (available_width / 7.0) as usize
            } else {
                0
            };

            let display_text = if approx_max > 3 && file_name.chars().count() > approx_max {
                let keep = approx_max.saturating_sub(3);
                let mut truncated = String::new();
                for (i, ch) in file_name.chars().enumerate() {
                    if i >= keep {
                        break;
                    }
                    truncated.push(ch);
                }
                truncated.push_str("...");
                truncated
            } else {
                file_name
            };

            canvas.draw_text_run(
                [file_name_x, file_text_y],
                display_text,
                14.0,
                self.file_text_color,
                z + 5,
            );
        }

        // Focus outline
        if self.focused {
            let focus_rect = Rect {
                x: container_rect.x - 2.0,
                y: container_rect.y - 2.0,
                w: container_rect.w + 4.0,
                h: container_rect.h + 4.0,
            };

            let focus_rrect = RoundedRect {
                rect: focus_rect,
                radii: RoundedRadii {
                    tl: self.radius + 2.0,
                    tr: self.radius + 2.0,
                    br: self.radius + 2.0,
                    bl: self.radius + 2.0,
                },
            };

            rune_surface::shapes::draw_rounded_rectangle(
                canvas,
                focus_rrect,
                None,
                Some(2.0),
                Some(Brush::Solid(Color::rgba(63, 130, 246, 255))),
                z + 4,
            );
        }
    }

    // ===== Event Handling =====

    /// Hit test the file input (container area)
    pub fn hit_test(&self, x: f32, y: f32) -> bool {
        let container = self.container_rect();

        // Check if point is in container rect
        x >= container.x
            && x <= container.x + container.w
            && y >= container.y
            && y <= container.y + container.h
    }

    /// Open the file picker dialog
    pub fn open_file_picker(&mut self) {
        // NOTE: On macOS when embedded via CEF (`webview-cef` feature), spawning a native
        // file dialog directly from the IR renderer thread crashes the host app
        // (AppKit dialogs must be driven from the primary Cocoa runloop / CEF host).
        //
        // To avoid crashing the macOS app, we currently no-op in that configuration.
        // The host is expected to provide its own file dialog integration for IR
        // FileInput widgets in CEF builds.
        #[cfg(all(target_os = "macos", feature = "webview-cef"))]
        {
            log::warn!(
                "FileInput::open_file_picker is disabled on macOS CEF builds; \
                 host integration must provide a file picker."
            );
            return;
        }

        // For non-CEF / winit-based builds, use rfd directly.
        #[cfg(not(all(target_os = "macos", feature = "webview-cef")))]
        {
            use rfd::FileDialog;

            let mut dialog = FileDialog::new();

            // Set file filters if provided
            if let Some(ref accept) = self.accept {
                // Group extensions by common types
                let extensions: Vec<&str> = accept.iter().map(|s| s.as_str()).collect();
                if !extensions.is_empty() {
                    dialog = dialog.add_filter("Accepted files", &extensions);
                }
            }

            // Open dialog based on single/multi mode
            if self.multi {
                if let Some(files) = dialog.pick_files() {
                    self.selected_files = files;
                }
            } else if let Some(file) = dialog.pick_file() {
                self.selected_files = vec![file];
            }
        }
    }

    /// Get the selected file paths
    pub fn selected_files(&self) -> &[PathBuf] {
        &self.selected_files
    }

    /// Clear the selected files
    pub fn clear_selection(&mut self) {
        self.selected_files.clear();
    }

    /// Check if this file input is focused
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state for this file input
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Check if the point is inside this file input
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        self.hit_test(x, y)
    }
}

/// Result of a file input click event
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileInputClickResult {
    /// The file input was clicked (should open file picker)
    Clicked,
    /// Click was outside the file input
    Ignored,
}

impl FileInput {
    /// Handle click event on the file input
    pub fn handle_click(&mut self, x: f32, y: f32) -> FileInputClickResult {
        if self.hit_test(x, y) {
            // Open the file picker when clicked
            self.open_file_picker();
            FileInputClickResult::Clicked
        } else {
            FileInputClickResult::Ignored
        }
    }
}

// ===== EventHandler Trait Implementation =====

impl crate::event_handler::EventHandler for FileInput {
    /// Handle mouse click event
    ///
    /// Opens the file picker dialog when the file input is clicked.
    fn handle_mouse_click(
        &mut self,
        event: crate::event_handler::MouseClickEvent,
    ) -> crate::event_handler::EventResult {
        use winit::event::ElementState;

        // Only handle left mouse button press
        if event.button != winit::event::MouseButton::Left || event.state != ElementState::Pressed {
            return crate::event_handler::EventResult::Ignored;
        }

        match self.handle_click(event.x, event.y) {
            FileInputClickResult::Clicked => crate::event_handler::EventResult::Handled,
            FileInputClickResult::Ignored => crate::event_handler::EventResult::Ignored,
        }
    }

    /// Handle keyboard input event
    ///
    /// Opens the file picker when Space or Enter is pressed while focused.
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

        // Open file picker on Space or Enter
        match event.key {
            KeyCode::Space | KeyCode::Enter => {
                self.open_file_picker();
                crate::event_handler::EventResult::Handled
            }
            _ => crate::event_handler::EventResult::Ignored,
        }
    }

    /// Handle mouse move event
    fn handle_mouse_move(
        &mut self,
        _event: crate::event_handler::MouseMoveEvent,
    ) -> crate::event_handler::EventResult {
        crate::event_handler::EventResult::Ignored
    }

    /// Check if this file input is focused
    fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state for this file input
    fn set_focused(&mut self, focused: bool) {
        FileInput::set_focused(self, focused);
    }

    /// Check if the point is inside this file input
    fn contains_point(&self, x: f32, y: f32) -> bool {
        self.contains_point(x, y)
    }
}
