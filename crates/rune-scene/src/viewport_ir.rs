/// Viewport IR - Incremental implementation starting from basics
/// Building layer by layer to ensure unified rendering works at each step
use crate::elements;
use engine_core::{Brush, ColorLinPremul, Rect};

/// Get the number of days in a given month/year
pub fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            // Leap year calculation
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30, // Fallback
    }
}

/// Get the first day of the month (0 = Sunday, 1 = Monday, etc.)
/// This is a simplified version - in production you'd use a proper date library
pub fn first_day_of_month(year: u32, month: u32) -> u32 {
    // Simplified Zeller's congruence algorithm
    let adjusted_month = if month < 3 { month + 12 } else { month };
    let adjusted_year = if month < 3 { year - 1 } else { year };

    let q = 1; // First day of month
    let m = adjusted_month;
    let k = adjusted_year % 100;
    let j = adjusted_year / 100;

    let h = (q + ((13 * (m + 1)) / 5) + k + (k / 4) + (j / 4) - (2 * j)) % 7;

    // Convert from Zeller (0=Saturday) to standard (0=Sunday)
    ((h + 6) % 7) as u32
}

/// Phase 0: Just a solid background to verify rendering pipeline works
pub struct ViewportContent {
    pub(crate) buttons: Vec<elements::Button>,
    pub(crate) checkboxes: Vec<elements::Checkbox>,
    pub(crate) radios: Vec<elements::Radio>,
    pub(crate) input_boxes: Vec<elements::InputBox>,
    pub(crate) text_areas: Vec<elements::TextArea>,
    pub(crate) selects: Vec<elements::Select>,
    pub(crate) date_pickers: Vec<elements::DatePicker>,
    images: Vec<ImageData>,
    links: Vec<elements::Link>,
    wrapped_paragraphs: Vec<WrappedParagraph>,
    col1_x: f32,
    multiline_y: f32,
    pub(crate) alert_visible: bool,
    pub(crate) alert_position: elements::AlertPosition,
    pub(crate) modal_open: bool,
    pub(crate) modal_close_on_background_click: bool,
    pub(crate) modal_title: String,
    pub(crate) modal_content_lines: Vec<String>,
    pub(crate) modal_buttons: Vec<elements::ModalButton>,
    pub(crate) confirm_open: bool,
    pub(crate) confirm_title: String,
    pub(crate) confirm_message: String,
    pub(crate) confirm_close_on_background_click: bool,
}

#[derive(Clone)]
pub struct CheckboxData {
    pub rect: Rect,
    pub checked: bool,
    pub focused: bool,
    pub label: Option<&'static str>,
    pub label_size: f32,
    pub color: ColorLinPremul,
}

impl CheckboxData {
    /// Convert to Checkbox element
    pub fn to_element(&self) -> elements::Checkbox {
        elements::Checkbox {
            rect: self.rect,
            checked: self.checked,
            focused: self.focused,
            label: self.label.map(|s| s.to_string()),
            label_size: self.label_size,
            color: self.color,
        }
    }

    /// Update from Checkbox element
    pub fn from_element(&mut self, element: &elements::Checkbox) {
        self.checked = element.checked;
        self.focused = element.focused;
    }
}

#[derive(Clone)]
pub struct RadioData {
    pub center: [f32; 2],
    pub radius: f32,
    pub selected: bool,
    pub label: Option<&'static str>,
    pub label_size: f32,
    pub label_color: ColorLinPremul,
    pub focused: bool,
}

impl RadioData {
    /// Convert to Radio element
    pub fn to_element(&self) -> elements::Radio {
        elements::Radio {
            center: self.center,
            radius: self.radius,
            selected: self.selected,
            label: self.label.map(|s| s.to_string()),
            label_size: self.label_size,
            label_color: self.label_color,
            focused: self.focused,
        }
    }

    /// Update from Radio element
    pub fn from_element(&mut self, element: &elements::Radio) {
        self.selected = element.selected;
        self.focused = element.focused;
    }
}

/// Pre-wrapped paragraph data for efficient rendering
#[derive(Clone)]
pub struct WrappedParagraph {
    pub lines: Vec<String>,
    pub size: f32,
    pub color: ColorLinPremul,
    pub line_height: f32,
}

#[derive(Clone)]
pub struct SelectData {
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

impl SelectData {
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

    /// Convert to Select element
    pub fn to_element(&self) -> elements::Select {
        elements::Select {
            rect: self.rect,
            label: self.label.clone(),
            label_size: self.label_size,
            label_color: self.label_color,
            open: self.open,
            focused: self.focused,
            options: self.options.clone(),
            selected_index: self.selected_index,
            padding_left: self.padding_left,
            padding_right: self.padding_right,
            padding_top: self.padding_top,
            padding_bottom: self.padding_bottom,
            bg_color: self.bg_color,
            border_color: self.border_color,
            border_width: self.border_width,
            radius: self.radius,
        }
    }

    /// Update from Select element
    pub fn from_element(&mut self, element: &elements::Select) {
        self.label = element.label.clone();
        self.open = element.open;
        self.focused = element.focused;
        self.selected_index = element.selected_index;
        self.padding_left = element.padding_left;
        self.padding_right = element.padding_right;
        self.padding_top = element.padding_top;
        self.padding_bottom = element.padding_bottom;
        self.bg_color = element.bg_color;
        self.border_color = element.border_color;
        self.border_width = element.border_width;
        self.radius = element.radius;
    }

    /// Handle click on the dropdown overlay
    pub fn handle_overlay_click(&mut self, x: f32, y: f32) -> bool {
        let mut select = self.to_element();
        let handled = select.handle_overlay_click(x, y);
        if handled {
            self.from_element(&select);
        }
        handled
    }

    /// Handle click on the select field
    pub fn handle_field_click(&mut self, x: f32, y: f32) -> bool {
        let mut select = self.to_element();
        let handled = select.handle_field_click(x, y);
        if handled {
            self.from_element(&select);
        }
        handled
    }

    /// Toggle the dropdown open/closed
    pub fn toggle_open(&mut self) {
        self.open = !self.open;
    }

    /// Close the dropdown
    pub fn close(&mut self) {
        self.open = false;
    }
}

#[derive(Clone)]
pub struct ImageData {
    pub rect: Rect,
    pub path: Option<std::path::PathBuf>,
    pub tint: ColorLinPremul,
}

#[derive(Clone)]
pub struct DatePickerData {
    pub rect: Rect,
    pub label_size: f32,
    pub label_color: ColorLinPremul,
    pub open: bool,
    pub focused: bool,
    pub selected_date: Option<(u32, u32, u32)>,
    pub current_view_month: u32,
    pub current_view_year: u32,
    pub picker_mode: elements::date_picker::PickerMode,
    // Styling fields
    pub padding_left: f32,
    pub padding_right: f32,
    pub padding_top: f32,
    pub padding_bottom: f32,
    pub bg_color: ColorLinPremul,
    pub border_color: ColorLinPremul,
    pub border_width: f32,
    pub radius: f32,
    pub viewport_height: f32,
}

impl DatePickerData {
    /// Convert to DatePicker element
    pub fn to_element(&self) -> elements::DatePicker {
        elements::DatePicker {
            rect: self.rect,
            label_size: self.label_size,
            label_color: self.label_color,
            open: self.open,
            focused: self.focused,
            selected_date: self.selected_date,
            current_view_month: self.current_view_month,
            current_view_year: self.current_view_year,
            picker_mode: self.picker_mode,
            padding_left: self.padding_left,
            padding_right: self.padding_right,
            padding_top: self.padding_top,
            padding_bottom: self.padding_bottom,
            bg_color: self.bg_color,
            border_color: self.border_color,
            border_width: self.border_width,
            radius: self.radius,
            viewport_height: self.viewport_height,
        }
    }

    /// Update from DatePicker element
    pub fn from_element(&mut self, element: &elements::DatePicker) {
        self.open = element.open;
        self.focused = element.focused;
        self.selected_date = element.selected_date;
        self.current_view_month = element.current_view_month;
        self.current_view_year = element.current_view_year;
        self.picker_mode = element.picker_mode;
    }

    /// Get the bounds of the popup calendar
    pub fn get_popup_bounds(&self) -> Option<Rect> {
        if !self.open {
            return None;
        }

        let popup_width = 280.0;
        let popup_height = match self.picker_mode {
            elements::date_picker::PickerMode::Days => 334.0,
            elements::date_picker::PickerMode::Months => 280.0,
            elements::date_picker::PickerMode::Years => 240.0,
        };

        Some(Rect {
            x: self.rect.x,
            y: self.rect.y - popup_height - 4.0,
            w: popup_width,
            h: popup_height,
        })
    }

    /// Convert to DatePicker for event handling
    fn to_date_picker(&self) -> elements::DatePicker {
        elements::DatePicker {
            rect: self.rect,
            label_size: self.label_size,
            label_color: self.label_color,
            open: self.open,
            focused: self.focused,
            selected_date: self.selected_date,
            current_view_month: self.current_view_month,
            current_view_year: self.current_view_year,
            picker_mode: self.picker_mode,
            padding_left: self.padding_left,
            padding_right: self.padding_right,
            padding_top: self.padding_top,
            padding_bottom: self.padding_bottom,
            bg_color: self.bg_color,
            border_color: self.border_color,
            border_width: self.border_width,
            radius: self.radius,
            viewport_height: self.viewport_height,
        }
    }

    /// Update from DatePicker after event handling
    fn update_from_date_picker(&mut self, picker: &elements::DatePicker) {
        self.open = picker.open;
        self.focused = picker.focused;
        self.selected_date = picker.selected_date;
        self.current_view_month = picker.current_view_month;
        self.current_view_year = picker.current_view_year;
        self.picker_mode = picker.picker_mode;
    }

    /// Handle click on the popup calendar
    pub fn handle_popup_click(&mut self, x: f32, y: f32) -> bool {
        let mut picker = self.to_date_picker();
        let handled = picker.handle_popup_click(x, y);
        if handled {
            self.update_from_date_picker(&picker);
        }
        handled
    }

    /// Handle click on the date picker field
    pub fn handle_field_click(&mut self, x: f32, y: f32) -> bool {
        let mut picker = self.to_date_picker();
        let handled = picker.handle_field_click(x, y);
        if handled {
            self.update_from_date_picker(&picker);
        }
        handled
    }

    /// Navigate to previous month/year/decade
    pub fn navigate_previous(&mut self) {
        use elements::date_picker::PickerMode;
        match self.picker_mode {
            PickerMode::Days => {
                if self.current_view_month == 1 {
                    self.current_view_month = 12;
                    self.current_view_year -= 1;
                } else {
                    self.current_view_month -= 1;
                }
            }
            PickerMode::Months => {
                self.current_view_year -= 1;
            }
            PickerMode::Years => {
                self.current_view_year -= 9;
            }
        }
    }

    /// Navigate to next month/year/decade
    pub fn navigate_next(&mut self) {
        use elements::date_picker::PickerMode;
        match self.picker_mode {
            PickerMode::Days => {
                if self.current_view_month == 12 {
                    self.current_view_month = 1;
                    self.current_view_year += 1;
                } else {
                    self.current_view_month += 1;
                }
            }
            PickerMode::Months => {
                self.current_view_year += 1;
            }
            PickerMode::Years => {
                self.current_view_year += 9;
            }
        }
    }

    /// Close the popup
    pub fn close_popup(&mut self) {
        self.open = false;
        self.picker_mode = elements::date_picker::PickerMode::Days;
    }
}

impl ViewportContent {
    pub fn new() -> Self {
        let col1_x = 40.0f32;
        let checkbox_y = 130.0f32;
        let button_y = 180.0f32;
        let radio_y = 240.0f32;
        let input_y = 290.0f32;
        // Place hyperlinks just below the subtitle (subtitle_y = 80.0)
        let link_y = 110.0f32;
        let textarea_y = 380.0f32;
        let image_y = 520.0f32; // Moved images up to be visible initially
        let select_y = 640.0f32; // Moved select down below images
        let datepicker_y = 700.0f32; // Date picker after select
        let multiline_y = 780.0f32;

        let checkboxes = vec![
            elements::Checkbox {
                rect: Rect {
                    x: col1_x,
                    y: checkbox_y,
                    w: 18.0,
                    h: 18.0,
                },
                checked: false,
                focused: false,
                label: Some("Checkbox".to_string()),
                label_size: 16.0,
                color: ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
            },
            elements::Checkbox {
                rect: Rect {
                    x: col1_x + 160.0,
                    y: checkbox_y,
                    w: 18.0,
                    h: 18.0,
                },
                checked: true,
                focused: true,
                label: Some("Checked + Focus".to_string()),
                label_size: 16.0,
                color: ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
            },
        ];

        let buttons = vec![
            elements::Button {
                rect: Rect {
                    x: col1_x,
                    y: button_y,
                    w: 160.0,
                    h: 36.0,
                },
                radius: 8.0,
                bg: ColorLinPremul::from_srgba_u8([63, 130, 246, 255]),
                fg: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
                label: "Primary".to_string(),
                label_size: 16.0,
                focused: false,
                on_click_intent: None,
            },
            elements::Button {
                rect: Rect {
                    x: col1_x + 176.0,
                    y: button_y,
                    w: 180.0,
                    h: 36.0,
                },
                radius: 8.0,
                bg: ColorLinPremul::from_srgba_u8([99, 104, 118, 255]),
                fg: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
                label: "Secondary".to_string(),
                label_size: 16.0,
                focused: true,
                on_click_intent: None,
            },
            elements::Button {
                rect: Rect {
                    x: col1_x + 372.0,
                    y: button_y,
                    w: 160.0,
                    h: 36.0,
                },
                radius: 8.0,
                bg: ColorLinPremul::from_srgba_u8([147, 51, 234, 255]), // Purple
                fg: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
                label: "Open Modal".to_string(),
                label_size: 16.0,
                focused: false,
                on_click_intent: None,
            },
            elements::Button {
                rect: Rect {
                    x: col1_x + 372.0,
                    y: button_y + 48.0,
                    w: 160.0,
                    h: 36.0,
                },
                radius: 8.0,
                bg: ColorLinPremul::from_srgba_u8([220, 38, 38, 255]), // Red
                fg: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
                label: "Show Confirm".to_string(),
                label_size: 16.0,
                focused: false,
                on_click_intent: None,
            },
            elements::Button {
                rect: Rect {
                    x: col1_x + 552.0,
                    y: button_y,
                    w: 180.0,
                    h: 36.0,
                },
                radius: 8.0,
                bg: ColorLinPremul::from_srgba_u8([31, 41, 55, 255]), // Dark gray
                fg: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
                label: "Show Alert".to_string(),
                label_size: 16.0,
                focused: false,
                on_click_intent: None,
            },
        ];

        let radios = vec![
            elements::Radio {
                center: [col1_x + 9.0, radio_y + 9.0],
                radius: 9.0,
                selected: true,
                label: Some("Option 1".to_string()),
                label_size: 16.0,
                label_color: ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
                focused: false,
            },
            elements::Radio {
                center: [col1_x + 140.0, radio_y + 9.0],
                radius: 9.0,
                selected: false,
                label: Some("Option 2".to_string()),
                label_size: 16.0,
                label_color: ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
                focused: false,
            },
            elements::Radio {
                center: [col1_x + 280.0, radio_y + 9.0],
                radius: 9.0,
                selected: false,
                label: Some("Option 3".to_string()),
                label_size: 16.0,
                label_color: ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
                focused: true,
            },
        ];

        let input_boxes = vec![
            elements::InputBox::new(
                Rect {
                    x: col1_x,
                    y: input_y,
                    w: 200.0,
                    h: 36.0,
                },
                "Hello World".to_string(),
                16.0,
                ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
                None,
                false,
            ),
            elements::InputBox::new(
                Rect {
                    x: col1_x + 220.0,
                    y: input_y,
                    w: 200.0,
                    h: 36.0,
                },
                "".to_string(),
                16.0,
                ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
                Some("Enter text...".to_string()),
                true,
            ),
        ];

        let links = vec![
            elements::Link::new(
                "Visit Rust Homepage",
                "https://www.rust-lang.org",
                [col1_x, link_y],
                16.0,
            ),
            elements::Link::new(
                "Learn More",
                "https://doc.rust-lang.org",
                [col1_x + 200.0, link_y],
                16.0,
            )
            .with_color(ColorLinPremul::from_srgba_u8([150, 200, 255, 255])),
            elements::Link::new(
                "GitHub",
                "https://github.com",
                [col1_x + 350.0, link_y],
                16.0,
            )
            .with_underline(false),
        ];

        let text_areas = vec![elements::TextArea::new(
            Rect {
                x: col1_x,
                y: textarea_y,
                w: 420.0,
                h: 120.0,
            },
            "This is a multi-line text area.\nYou can add multiple lines of text here.\nIt supports scrolling and wrapping.".to_string(),
            16.0,
            ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
            Some("Enter multi-line text...".to_string()),
            false,
        )];

        let selects = vec![elements::Select {
            rect: Rect {
                x: col1_x,
                y: select_y,
                w: 200.0,
                h: 36.0,
            },
            label: "Option 1".to_string(), // Initialize with first selected option
            label_size: 16.0,
            label_color: ColorLinPremul::from_srgba_u8([20, 24, 30, 255]),
            open: false, // Set to true for testing
            focused: false,
            options: vec![
                "Option 1".to_string(),
                "Option 2".to_string(),
                "Option 3".to_string(),
                "Option 4".to_string(),
                "Option 5".to_string(),
            ],
            selected_index: Some(0),
            padding_left: 14.0,
            padding_right: 14.0,
            padding_top: 10.0,
            padding_bottom: 10.0,
            bg_color: ColorLinPremul::from_srgba_u8([245, 247, 250, 255]),
            border_color: ColorLinPremul::from_srgba_u8([200, 208, 216, 255]),
            border_width: 1.0,
            radius: 8.0,
        }];

        let date_pickers = vec![
            elements::DatePicker::new(
                Rect {
                    x: col1_x,
                    y: datepicker_y,
                    w: 200.0,
                    h: 36.0,
                },
                16.0,
                ColorLinPremul::from_srgba_u8([30, 35, 45, 255]),
                Some((2025, 3, 15)), // Example: March 15, 2025
            ),
            elements::DatePicker::new(
                Rect {
                    x: col1_x + 220.0,
                    y: datepicker_y,
                    w: 200.0,
                    h: 36.0,
                },
                16.0,
                ColorLinPremul::from_srgba_u8([30, 35, 45, 255]),
                None, // No date selected
            ),
        ];

        let images = vec![
            ImageData {
                rect: Rect {
                    x: col1_x,
                    y: image_y,
                    w: 120.0,
                    h: 80.0,
                },
                path: Some("images/squirrel.jpg".into()),
                tint: ColorLinPremul::from_srgba_u8([100, 150, 200, 255]),
            },
            ImageData {
                rect: Rect {
                    x: col1_x + 140.0,
                    y: image_y,
                    w: 120.0,
                    h: 80.0,
                },
                path: Some("images/fire.jpg".into()),
                tint: ColorLinPremul::from_srgba_u8([200, 100, 150, 255]),
            },
        ];

        // Simple paragraphs for multi-line text wrapping demonstration
        let paragraph_data = vec![
            (
                "Paragraph 1: This demonstrates rune-text multi-line wrapping inside the viewport. \
                 Resize the window to watch this paragraph reflow across several lines while keeping \
                 the overall block shape consistent.",
                16.0f32,
                ColorLinPremul::from_srgba_u8([220, 220, 220, 255]),
                1.4f32,
            ),
            (
                "Paragraph 2: Each paragraph is long enough to wrap into multiple visual lines. \
                 The layout engine uses Unicode line breaking rules, so spaces, punctuation, and \
                 explicit newlines all behave as expected.",
                14.0f32,
                ColorLinPremul::from_srgba_u8([150, 200, 255, 255]),
                1.2f32,
            ),
            (
                "Paragraph 3: This block mixes shorter and longer sentences to exercise different \
                 wrapping positions. It helps verify that baseline spacing stays stable even when \
                 lines expand or contract on resize.",
                15.0f32,
                ColorLinPremul::from_srgba_u8([200, 180, 255, 255]),
                1.3f32,
            ),
            (
                "Paragraph 4: This one includes numbers (1234567890) and mixed-case text to ensure \
                 glyph metrics and kerning behave correctly across a variety of glyph shapes.",
                14.0f32,
                ColorLinPremul::from_srgba_u8([200, 230, 180, 255]),
                1.25f32,
            ),
            (
                "Paragraph 5: Multiple wrapped paragraphs are rendered one after another with extra \
                 spacing in between. This makes it easy to visually confirm that paragraph breaks \
                 are preserved while line wrapping happens only within each block.",
                15.0f32,
                ColorLinPremul::from_srgba_u8([150, 255, 200, 255]),
                1.35f32,
            ),
        ];

        // Create simple single-line paragraphs (wrapping will be handled by MultilineText)
        let mut wrapped_paragraphs = Vec::new();
        for (text, size, color, lh_factor) in paragraph_data {
            wrapped_paragraphs.push(WrappedParagraph {
                lines: vec![text.to_string()],
                size,
                color,
                line_height: size * lh_factor,
            });
        }

        // Default modal configuration (title, content lines, buttons)
        let modal_title = "Confirm Action".to_string();
        let modal_content_lines = vec![
            "Are you sure you want to continue?".to_string(),
            "This action cannot be undone.".to_string(),
        ];
        let modal_buttons = vec![
            elements::ModalButton::new("Cancel"),
            elements::ModalButton::primary("Continue"),
        ];

        // Default confirm dialog content
        let confirm_title = "Delete Item?".to_string();
        let confirm_message =
            "This action cannot be undone. Are you sure you want to continue?".to_string();

        Self {
            buttons,
            checkboxes,
            radios,
            input_boxes,
            text_areas,
            selects,
            date_pickers,
            images,
            links,
            wrapped_paragraphs,
            col1_x,
            multiline_y,
            alert_visible: false,
            alert_position: elements::AlertPosition::TopCenter,
            modal_open: false,
            modal_close_on_background_click: true,
            modal_title,
            modal_content_lines,
            modal_buttons,
            confirm_open: false,
            confirm_title,
            confirm_message,
            confirm_close_on_background_click: false,
        }
    }

    /// Render viewport content to canvas
    pub fn render(
        &mut self,
        canvas: &mut rune_surface::Canvas,
        scale_factor: f32,
        viewport_width: u32,
        viewport_height: u32,
        provider: &dyn engine_core::TextProvider,
        text_cache: &engine_core::TextLayoutCache,
    ) -> f32 {
        // Background: simple solid fill so viewport content stands out from the zone background.
        // Drawn at z=-100 so all UI/text appears above it.
        canvas.fill_rect(
            0.0,
            0.0,
            viewport_width as f32,
            viewport_height as f32,
            Brush::Solid(ColorLinPremul::from_srgba_u8([40, 45, 65, 255])), // Slightly lighter than zone bg
            -100,
        );

        // Font sizes are in logical pixels - DPI scaling is handled by Canvas/Surface layers.
        // Do NOT divide by scale_factor here, as that causes double-scaling issues.
        let _sf = scale_factor; // Keep parameter for future use if needed
        let title_size = 22.0;
        let subtitle_size = 12.0;
        let test_line_size = 20.0;

        // Basic text content with headers, subtitles, and test lines.
        // Coordinates are viewport-local and assume the viewport origin has already been
        // translated by the caller to the correct zone.
        let col1_x = 40.0f32;
        let title_y = 40.0f32;
        let subtitle_y = 80.0f32;

        // Title
        canvas.draw_text_run(
            [col1_x, title_y],
            "Rune Scene \u{2014} UI Elements".to_string(),
            title_size,
            ColorLinPremul::rgba(255, 255, 255, 255),
            10,
        );

        // Subtitle
        canvas.draw_text_run(
            [col1_x, subtitle_y],
            "Subtitle example text".to_string(),
            subtitle_size,
            ColorLinPremul::rgba(200, 200, 200, 255),
            10,
        );

        // Bright cyan test line
        canvas.draw_text_run(
            [col1_x, 360.0],
            "TEST: This should be BRIGHT CYAN".to_string(),
            test_line_size,
            ColorLinPremul::rgba(0, 255, 255, 255),
            10,
        );

        // Render checkboxes with z-index 20
        for checkbox in &self.checkboxes {
            checkbox.render(canvas, 20);
        }

        // Render buttons with z-index 30
        for button in &self.buttons {
            button.render(canvas, 30);
        }

        // Render radio buttons with z-index 40
        for radio in &self.radios {
            radio.render(canvas, 40);
        }

        // Render input boxes with z-index 50
        for input in self.input_boxes.iter_mut() {
            input.render(canvas, 50, provider);
        }

        // Render links with z-index 55
        for link in &self.links {
            link.render(canvas, 55);
        }

        // Render text areas with z-index 60
        for textarea in self.text_areas.iter_mut() {
            textarea.render(canvas, 60, provider);
        }

        // Render selects with z-index 70 (closed state only)
        for select_data in &self.selects {
            let mut select = select_data.clone();
            select.open = false; // Render closed state first
            select.render(canvas, 70);
        }

        // Render date pickers with z-index 80 (closed state only)
        for date_picker in &self.date_pickers {
            let mut picker = date_picker.clone();
            picker.open = false; // Render closed state first
            picker.render(canvas, 80);
        }

        // Render images with z-index 90
        for image_data in &self.images {
            let image = elements::ImageBox {
                rect: image_data.rect,
                path: image_data.path.clone(),
                tint: image_data.tint,
                fit: elements::ImageFit::Contain,
            };
            image.render(canvas, 90);
        }

        // Rune-text multi-paragraph wrapping demo (z=100)
        // Build a single multi-paragraph string from the sample paragraphs.
        let multiline_height = if !self.wrapped_paragraphs.is_empty() {
            let multi_paragraph_text = if self.wrapped_paragraphs.is_empty() {
                "Rune-text wrapping demo.\n\n\
                 Paragraph 1: This demonstrates rune-text-driven multi-line wrapping \
                 inside the viewport zone.\n\n\
                 Paragraph 2: Resize the window horizontally to see lines reflow \
                 while preserving explicit paragraph breaks."
                    .to_string()
            } else {
                let paras: Vec<String> = self
                    .wrapped_paragraphs
                    .iter()
                    .map(|p| p.lines.join(" "))
                    .collect();
                paras.join("\n\n")
            };

            // Calculate max width for text wrapping based on viewport width
            let right_margin = 40.0f32;
            let text_max_width = (viewport_width as f32 - self.col1_x - right_margin)
                .max(200.0)
                .min(1200.0);

            let multiline = elements::MultilineText {
                pos: [self.col1_x, self.multiline_y],
                text: multi_paragraph_text,
                size: 16.0,
                color: ColorLinPremul::from_srgba_u8([230, 230, 240, 255]),
                max_width: Some(text_max_width),
                line_height_factor: Some(1.4),
            };
            // Use cached rendering for efficient resize performance
            let _ = scale_factor; // keep parameter for now
            multiline.render_cached(canvas, 100, text_cache)
        } else {
            0.0
        };

        // Render select dropdown overlays after all primary content with a very high z-index.
        // This guarantees the dropdown and its options appear above images and wrapped text,
        // even if a non-unified rendering path is used where draw order still matters.
        // Note: z-index valid range is [-10000, 10000], so we use 8000 here;
        // the dropdown overlay itself will be at z + 1000 = 9000.
        for select_data in &self.selects {
            if select_data.open {
                let select = select_data.clone();
                select.render(canvas, 8000);
            }
        }

        // Render date picker calendar popups when open (same high z-index as selects)
        for date_picker in &self.date_pickers {
            if date_picker.open {
                date_picker.render(canvas, 8000);
            }
        }

        // Render primary modal if open (highest z-index to appear above everything)
        if self.modal_open {
            // Build modal from current configuration
            let modal = elements::Modal::new(
                viewport_width as f32,
                viewport_height as f32,
                self.modal_title.clone(),
                // Keep content string for default rendering paths, but primary
                // modal body is driven by `modal_content_lines` below.
                self.modal_content_lines.join("\n"),
                self.modal_buttons.clone(),
            )
            .with_close_on_background_click(self.modal_close_on_background_click);

            let z = modal.base_z;

            // Render chrome (shadow, panel, border, close icon)
            modal.render_chrome(canvas, z);

            // Use layout helpers so content is easy to customize
            let layout = modal.layout();

            // Render title text
            canvas.draw_text_run(
                layout.title_pos,
                self.modal_title.clone(),
                modal.title_size,
                modal.title_color,
                z + 4,
            );

            // Render body lines into the content area
            for (i, line) in self.modal_content_lines.iter().enumerate() {
                let pos = [
                    layout.content_origin[0],
                    layout.content_origin[1] + i as f32 * layout.content_line_height,
                ];
                canvas.draw_text_run(
                    pos,
                    line.clone(),
                    modal.content_size,
                    modal.content_color,
                    z + 4,
                );
            }

            // Render buttons using the precomputed button rects
            for (i, (button, rect)) in self
                .modal_buttons
                .iter()
                .zip(layout.button_rects.iter())
                .enumerate()
            {
                modal.render_button(canvas, button, *rect, z + 5 + i as i32 * 5);
            }
        }

        // Render confirm dialog when open (shares the same viewport, slightly lower z than modal).
        if self.confirm_open {
            let dialog = elements::ConfirmDialog::new(
                viewport_width as f32,
                viewport_height as f32,
                self.confirm_title.clone(),
                self.confirm_message.clone(),
            );
            dialog.render(canvas);
        }

        // Render alert panel when enabled. This showcases the Alert element
        // without a fullscreen background; it is positioned relative to the
        // viewport using the configured alert_position.
        if self.alert_visible {
            let alert = elements::Alert::new(
                viewport_width as f32,
                viewport_height as f32,
                "Event has been created",
                "Sunday, December 03, 2023 at 9:00 AM",
            )
            .with_action("Ok")
            .with_position(self.alert_position);
            alert.render(canvas);
        }

        // Calculate total content height
        let multiline_bottom = self.multiline_y + multiline_height;
        let content_height = multiline_bottom.max(viewport_height as f32);

        content_height
    }
}

impl Default for ViewportContent {
    fn default() -> Self {
        Self::new()
    }
}

/// Example: Render IR-based content using rune_ir and the ir_adapter.
///
/// This function demonstrates a minimal example of using IrAdapter to convert
/// rune-ir ViewNodes to rendering elements.
///
/// # Usage
///
/// Set `USE_IR=1` environment variable to enable IR rendering mode.
/// Then use `IrRenderer` (in `ir_renderer.rs`) for full layout integration,
/// or use `IrAdapter` directly for manual layout scenarios.
///
/// ```ignore
/// use rune_ir::view::{ViewDocument, ViewNodeKind};
/// use crate::ir_adapter::IrAdapter;
///
/// // Example: Convert a ButtonSpec from IR to a Button element
/// for node in &view_doc.nodes {
///     if let ViewNodeKind::Button(spec) = &node.kind {
///         let rect = Rect { x: 40.0, y: 100.0, w: 160.0, h: 36.0 };
///         let button = IrAdapter::button_from_spec(spec, rect, Some("Click Me".to_string()));
///         button.render(&mut canvas, 30);
///     }
/// }
/// ```
#[allow(dead_code)]
pub fn render_from_ir_example() {
    // See ir_renderer.rs for the full implementation using Taffy layout
    // See ir_adapter.rs for conversion helpers
}

// ===== ElementCollection Implementation =====

impl crate::event_router::ElementCollection for ViewportContent {
    fn buttons_mut(&mut self) -> &mut [elements::Button] {
        &mut self.buttons
    }

    fn checkboxes_mut(&mut self) -> &mut [elements::Checkbox] {
        &mut self.checkboxes
    }

    fn radios_mut(&mut self) -> &mut [elements::Radio] {
        &mut self.radios
    }

    fn inputs_mut(&mut self) -> &mut [elements::InputBox] {
        &mut self.input_boxes
    }

    fn textareas_mut(&mut self) -> &mut [elements::TextArea] {
        &mut self.text_areas
    }

    fn selects_mut(&mut self) -> &mut [elements::Select] {
        &mut self.selects
    }

    fn date_pickers_mut(&mut self) -> &mut [elements::DatePicker] {
        &mut self.date_pickers
    }

    fn links_mut(&mut self) -> &mut [elements::Link] {
        &mut self.links
    }

    fn clear_focus_except(&mut self, element_type: crate::event_router::ElementType, index: usize) {
        // Clear focus from all elements
        for b in &mut self.buttons {
            b.focused = false;
        }
        for cb in &mut self.checkboxes {
            cb.focused = false;
        }
        for r in &mut self.radios {
            r.focused = false;
        }
        for inp in &mut self.input_boxes {
            inp.focused = false;
        }
        for ta in &mut self.text_areas {
            ta.focused = false;
        }
        for s in &mut self.selects {
            s.focused = false;
        }
        for dp in &mut self.date_pickers {
            dp.focused = false;
        }
        for link in &mut self.links {
            link.focused = false;
        }

        // Set focus on the specified element
        match element_type {
            crate::event_router::ElementType::Button => {
                if let Some(btn) = self.buttons.get_mut(index) {
                    btn.focused = true;
                }
            }
            crate::event_router::ElementType::Checkbox => {
                if let Some(cb) = self.checkboxes.get_mut(index) {
                    cb.focused = true;
                }
            }
            crate::event_router::ElementType::Radio => {
                if let Some(r) = self.radios.get_mut(index) {
                    r.focused = true;
                }
            }
            crate::event_router::ElementType::Input => {
                if let Some(inp) = self.input_boxes.get_mut(index) {
                    inp.focused = true;
                }
            }
            crate::event_router::ElementType::TextArea => {
                if let Some(ta) = self.text_areas.get_mut(index) {
                    ta.focused = true;
                }
            }
            crate::event_router::ElementType::Select => {
                if let Some(s) = self.selects.get_mut(index) {
                    s.focused = true;
                }
            }
            crate::event_router::ElementType::DatePicker => {
                if let Some(dp) = self.date_pickers.get_mut(index) {
                    dp.focused = true;
                }
            }
            crate::event_router::ElementType::Link => {
                if let Some(link) = self.links.get_mut(index) {
                    link.focused = true;
                }
            }
            crate::event_router::ElementType::Modal => {
                // Modals not used in ViewportContent
            }
        }
    }
}
