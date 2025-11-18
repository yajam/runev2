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
    let adjusted_month = if month < 3 {
        month + 12
    } else {
        month
    };
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
    buttons: Vec<elements::Button>,
    pub(crate) checkboxes: Vec<CheckboxData>,
    pub(crate) radios: Vec<RadioData>,
    pub(crate) input_boxes: Vec<elements::InputBox>,
    pub(crate) text_areas: Vec<elements::TextArea>,
    pub(crate) selects: Vec<SelectData>,
    pub(crate) date_pickers: Vec<DatePickerData>,
    images: Vec<ImageData>,
    links: Vec<elements::Link>,
    wrapped_paragraphs: Vec<WrappedParagraph>,
    col1_x: f32,
    multiline_y: f32,
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
        let image_y = 520.0f32;  // Moved images up to be visible initially
        let select_y = 640.0f32;  // Moved select down below images
        let datepicker_y = 700.0f32;  // Date picker after select
        let multiline_y = 780.0f32;

        let checkboxes = vec![
            CheckboxData {
                rect: Rect {
                    x: col1_x,
                    y: checkbox_y,
                    w: 18.0,
                    h: 18.0,
                },
                checked: false,
                focused: false,
                label: Some("Checkbox"),
                label_size: 16.0,
                color: ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
            },
            CheckboxData {
                rect: Rect {
                    x: col1_x + 160.0,
                    y: checkbox_y,
                    w: 18.0,
                    h: 18.0,
                },
                checked: true,
                focused: true,
                label: Some("Checked + Focus"),
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
            },
        ];

        let radios = vec![
            RadioData {
                center: [col1_x + 9.0, radio_y + 9.0],
                radius: 9.0,
                selected: true,
                label: Some("Option 1"),
                label_size: 16.0,
                label_color: ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
                focused: false,
            },
            RadioData {
                center: [col1_x + 140.0, radio_y + 9.0],
                radius: 9.0,
                selected: false,
                label: Some("Option 2"),
                label_size: 16.0,
                label_color: ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
                focused: false,
            },
            RadioData {
                center: [col1_x + 280.0, radio_y + 9.0],
                radius: 9.0,
                selected: false,
                label: Some("Option 3"),
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

        let selects = vec![SelectData {
            rect: Rect {
                x: col1_x,
                y: select_y,
                w: 200.0,
                h: 36.0,
            },
            label: "Option 1".to_string(), // Initialize with first selected option
            label_size: 16.0,
            label_color: ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
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
        }];

        let date_pickers = vec![
            DatePickerData {
                rect: Rect {
                    x: col1_x,
                    y: datepicker_y,
                    w: 200.0,
                    h: 36.0,
                },
                label_size: 16.0,
                label_color: ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
                open: false,
                focused: false,
                selected_date: Some((2025, 3, 15)), // Example: March 15, 2025
                current_view_month: 3,
                current_view_year: 2025,
                picker_mode: elements::date_picker::PickerMode::Days,
            },
            DatePickerData {
                rect: Rect {
                    x: col1_x + 220.0,
                    y: datepicker_y,
                    w: 200.0,
                    h: 36.0,
                },
                label_size: 16.0,
                label_color: ColorLinPremul::from_srgba_u8([240, 240, 240, 255]),
                open: false,
                focused: false,
                selected_date: None, // No date selected
                current_view_month: 11, // Default to current month (November 2025 demo date)
                current_view_year: 2025,
                picker_mode: elements::date_picker::PickerMode::Days,
            },
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
        let subtitle_size = 16.0;
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
        for cb_data in &self.checkboxes {
            let cb = elements::Checkbox {
                rect: cb_data.rect,
                checked: cb_data.checked,
                focused: cb_data.focused,
                label: cb_data.label.map(|s| s.to_string()),
                label_size: cb_data.label_size,
                color: cb_data.color,
            };
            cb.render(canvas, 20);
        }

        // Render buttons with z-index 30
        for button in &self.buttons {
            button.render(canvas, 30);
        }

        // Render radio buttons with z-index 40
        for radio_data in &self.radios {
            let radio = elements::Radio {
                center: radio_data.center,
                radius: radio_data.radius,
                selected: radio_data.selected,
                label: radio_data.label.map(|s| s.to_string()),
                label_size: radio_data.label_size,
                label_color: radio_data.label_color,
                focused: radio_data.focused,
            };
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
            let select = elements::Select {
                rect: select_data.rect,
                label: select_data.label.to_string(),
                label_size: select_data.label_size,
                label_color: select_data.label_color,
                open: false, // Render closed state first
                focused: select_data.focused,
                options: select_data.options.clone(),
                selected_index: select_data.selected_index,
            };
            select.render(canvas, 70);
        }

        // Render date pickers with z-index 80 (closed state only)
        for date_picker_data in &self.date_pickers {
            let date_picker = elements::DatePicker {
                rect: date_picker_data.rect,
                label_size: date_picker_data.label_size,
                label_color: date_picker_data.label_color,
                open: false, // Render closed state first
                focused: date_picker_data.focused,
                selected_date: date_picker_data.selected_date,
                current_view_month: date_picker_data.current_view_month,
                current_view_year: date_picker_data.current_view_year,
                picker_mode: date_picker_data.picker_mode,
            };
            date_picker.render(canvas, 80);
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
                let select = elements::Select {
                    rect: select_data.rect,
                    label: select_data.label.to_string(),
                    label_size: select_data.label_size,
                    label_color: select_data.label_color,
                    open: select_data.open,
                    focused: select_data.focused,
                    options: select_data.options.clone(),
                    selected_index: select_data.selected_index,
                };
                select.render(canvas, 8000);
            }
        }

        // Render date picker calendar popups when open (same high z-index as selects)
        for date_picker_data in &self.date_pickers {
            if date_picker_data.open {
                let date_picker = elements::DatePicker {
                    rect: date_picker_data.rect,
                    label_size: date_picker_data.label_size,
                    label_color: date_picker_data.label_color,
                    open: date_picker_data.open,
                    focused: date_picker_data.focused,
                    selected_date: date_picker_data.selected_date,
                    current_view_month: date_picker_data.current_view_month,
                    current_view_year: date_picker_data.current_view_year,
                    picker_mode: date_picker_data.picker_mode,
                };
                date_picker.render(canvas, 8000);
            }
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
