use crate::elements;
/// Sample UI module - contains example UI elements for demonstration
/// This will be replaced with IR-based rendering into the viewport zone
use engine_core::{Color, ColorLinPremul, Rect};

// UI Element Data Structures
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
pub struct ButtonData {
    pub rect: Rect,
    pub radius: f32,
    pub bg: ColorLinPremul,
    pub fg: ColorLinPremul,
    pub label: &'static str,
    pub label_size: f32,
    pub focused: bool,
}

#[derive(Clone)]
pub struct TextData {
    pub pos: [f32; 2],
    pub text: &'static str,
    pub size: f32,
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

#[derive(Clone)]
pub struct InputBoxData {
    pub rect: Rect,
    pub text: &'static str,
    pub text_size: f32,
    pub text_color: ColorLinPremul,
    pub placeholder: Option<&'static str>,
    pub focused: bool,
}

#[derive(Clone)]
pub struct TextAreaData {
    pub rect: Rect,
    pub lines: Vec<&'static str>,
    pub text_size: f32,
    pub text_color: ColorLinPremul,
    pub focused: bool,
}

#[derive(Clone)]
pub struct SelectData {
    pub rect: Rect,
    pub label: &'static str,
    pub label_size: f32,
    pub label_color: ColorLinPremul,
    pub open: bool,
    pub focused: bool,
}

#[derive(Clone)]
pub struct LabelData {
    pub pos: [f32; 2],
    pub text: &'static str,
    pub size: f32,
    pub color: ColorLinPremul,
}

#[derive(Clone)]
pub struct ImageData {
    pub rect: Rect,
    pub path: Option<std::path::PathBuf>,
    pub tint: ColorLinPremul,
}

// Unified UI element enum for mixed rendering
#[allow(dead_code)]
pub enum UIElement {
    Text(TextData),
    Checkbox(CheckboxData),
    Button(ButtonData),
    Radio(RadioData),
    InputBox(InputBoxData),
    TextArea(TextAreaData),
    Select(SelectData),
    Label(LabelData),
    Image(ImageData),
}

/// Create sample UI elements for demonstration
pub fn create_sample_elements() -> SampleUIElements {
    let col1_x = 40.0f32;
    let title_y = 40.0f32;
    let subtitle_y = 80.0f32;
    let checkbox_y = 130.0f32;
    let button_y = 180.0f32;
    let radio_y = 240.0f32;
    let input_y = 290.0f32;
    let textarea_y = 380.0f32;
    let select_y = 520.0f32;
    let label_y = 580.0f32;
    let image_y = 620.0f32;
    let multiline_y = 730.0f32;

    let texts = vec![
        TextData {
            pos: [col1_x, title_y],
            text: "Rune Scene \u{2014} UI Elements",
            size: 22.0,
            color: Color::rgba(255, 255, 255, 255),
        },
        TextData {
            pos: [col1_x, subtitle_y],
            text: "Subtitle example text",
            size: 18.0,
            color: Color::rgba(200, 200, 200, 255),
        },
        TextData {
            pos: [col1_x, 350.0],
            text: "TEST: This should be BRIGHT CYAN",
            size: 20.0,
            color: Color::rgba(0, 255, 255, 255),
        },
    ];

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
            color: Color::rgba(240, 240, 240, 255),
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
            color: Color::rgba(240, 240, 240, 255),
        },
    ];

    let buttons = vec![
        ButtonData {
            rect: Rect {
                x: col1_x,
                y: button_y,
                w: 160.0,
                h: 36.0,
            },
            radius: 8.0,
            bg: Color::rgba(63, 130, 246, 255),
            fg: Color::rgba(255, 255, 255, 255),
            label: "Primary",
            label_size: 16.0,
            focused: false,
        },
        ButtonData {
            rect: Rect {
                x: col1_x + 176.0,
                y: button_y,
                w: 180.0,
                h: 36.0,
            },
            radius: 8.0,
            bg: Color::rgba(99, 104, 118, 255),
            fg: Color::rgba(255, 255, 255, 255),
            label: "Secondary",
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
            label_color: Color::rgba(240, 240, 240, 255),
            focused: false,
        },
        RadioData {
            center: [col1_x + 140.0, radio_y + 9.0],
            radius: 9.0,
            selected: false,
            label: Some("Option 2"),
            label_size: 16.0,
            label_color: Color::rgba(240, 240, 240, 255),
            focused: false,
        },
        RadioData {
            center: [col1_x + 280.0, radio_y + 9.0],
            radius: 9.0,
            selected: false,
            label: Some("Option 3"),
            label_size: 16.0,
            label_color: Color::rgba(240, 240, 240, 255),
            focused: true,
        },
    ];

    let input_boxes = vec![
        InputBoxData {
            rect: Rect {
                x: col1_x,
                y: input_y,
                w: 200.0,
                h: 36.0,
            },
            text: "Hello World",
            text_size: 16.0,
            text_color: Color::rgba(240, 240, 240, 255),
            placeholder: None,
            focused: false,
        },
        InputBoxData {
            rect: Rect {
                x: col1_x + 220.0,
                y: input_y,
                w: 200.0,
                h: 36.0,
            },
            text: "",
            text_size: 16.0,
            text_color: Color::rgba(240, 240, 240, 255),
            placeholder: Some("Enter text..."),
            focused: true,
        },
    ];

    let text_areas = vec![TextAreaData {
        rect: Rect {
            x: col1_x,
            y: textarea_y,
            w: 420.0,
            h: 120.0,
        },
        lines: vec![
            "This is a multi-line text area.",
            "You can add multiple lines of text here.",
            "It supports scrolling and wrapping.",
        ],
        text_size: 14.0,
        text_color: Color::rgba(240, 240, 240, 255),
        focused: false,
    }];

    let selects = vec![SelectData {
        rect: Rect {
            x: col1_x,
            y: select_y,
            w: 200.0,
            h: 36.0,
        },
        label: "Select an option",
        label_size: 16.0,
        label_color: Color::rgba(240, 240, 240, 255),
        open: false,
        focused: false,
    }];

    let labels = vec![LabelData {
        pos: [col1_x, label_y],
        text: "This is a standalone label",
        size: 16.0,
        color: Color::rgba(180, 180, 200, 255),
    }];

    let images = vec![
        ImageData {
            rect: Rect {
                x: col1_x,
                y: image_y,
                w: 120.0,
                h: 80.0,
            },
            path: Some("images/squirrel.jpg".into()),
            tint: Color::rgba(100, 150, 200, 255),
        },
        ImageData {
            rect: Rect {
                x: col1_x + 140.0,
                y: image_y,
                w: 120.0,
                h: 80.0,
            },
            path: Some("images/fire.jpg".into()),
            tint: Color::rgba(200, 100, 150, 255),
        },
    ];

    // Simple paragraphs without wrapping - just single lines
    let paragraph_data = vec![
        (
            "Paragraph 1: This demonstrates rune-text multi-line wrapping inside the viewport. 
             Resize the window to watch this paragraph reflow across several lines while keeping 
             the overall block shape consistent.",
            16.0f32,
            Color::rgba(220, 220, 220, 255),
            1.4f32,
        ),
        (
            "Paragraph 2: Each paragraph is long enough to wrap into multiple visual lines. 
             The layout engine uses Unicode line breaking rules, so spaces, punctuation, and 
             explicit newlines all behave as expected.",
            14.0f32,
            Color::rgba(150, 200, 255, 255),
            1.2f32,
        ),
        (
            "Paragraph 3: This block mixes shorter and longer sentences to exercise different 
             wrapping positions. It helps verify that baseline spacing stays stable even when 
             lines expand or contract on resize.",
            15.0f32,
            Color::rgba(200, 180, 255, 255),
            1.3f32,
        ),
        (
            "Paragraph 4: This one includes numbers (1234567890) and mixed-case text to ensure 
             glyph metrics and kerning behave correctly across a variety of glyph shapes.",
            14.0f32,
            Color::rgba(200, 230, 180, 255),
            1.25f32,
        ),
        (
            "Paragraph 5: Multiple wrapped paragraphs are rendered one after another with extra 
             spacing in between. This makes it easy to visually confirm that paragraph breaks 
             are preserved while line wrapping happens only within each block.",
            15.0f32,
            Color::rgba(150, 255, 200, 255),
            1.35f32,
        ),
    ];

    // Create simple single-line paragraphs (no wrapping)
    let mut wrapped_paragraphs = Vec::new();

    for (text, size, color, lh_factor) in paragraph_data {
        wrapped_paragraphs.push(WrappedParagraph {
            lines: vec![text.to_string()],
            size,
            color,
            line_height: size * lh_factor,
        });
    }

    SampleUIElements {
        texts,
        checkboxes,
        buttons,
        radios,
        input_boxes,
        text_areas,
        selects,
        labels,
        images,
        wrapped_paragraphs,
        col1_x,
        multiline_y,
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

pub struct SampleUIElements {
    pub texts: Vec<TextData>,
    pub checkboxes: Vec<CheckboxData>,
    pub buttons: Vec<ButtonData>,
    pub radios: Vec<RadioData>,
    pub input_boxes: Vec<InputBoxData>,
    pub text_areas: Vec<TextAreaData>,
    pub selects: Vec<SelectData>,
    pub labels: Vec<LabelData>,
    pub images: Vec<ImageData>,
    pub wrapped_paragraphs: Vec<WrappedParagraph>,
    pub col1_x: f32,
    pub multiline_y: f32,
}

impl SampleUIElements {
    /// Render all sample UI elements to a canvas in local coordinates.
    /// Caller should set up transform for zone positioning.
    pub fn render(
        &self,
        canvas: &mut rune_surface::Canvas,
        scale_factor: f32,
        window_width: u32,
        provider: &dyn engine_core::TextProvider,
    ) {
        // Render all text elements using direct rasterization (simpler, more reliable)
        for text in self.texts.iter() {
            canvas.draw_text_direct(text.pos, text.text, text.size, text.color, provider);
        }

        // Render all checkboxes (z=20, ticks drawn in overlay for crispness)
        for cb_data in self.checkboxes.iter() {
            let cb = elements::checkbox::Checkbox {
                rect: cb_data.rect,
                checked: cb_data.checked,
                focused: cb_data.focused,
                label: cb_data.label.map(|s| s.to_string()),
                label_size: cb_data.label_size,
                color: cb_data.color,
            };
            cb.render(canvas, 20);
        }

        // Render all buttons (z=30)
        for btn_data in self.buttons.iter() {
            let btn = elements::button::Button {
                rect: btn_data.rect,
                radius: btn_data.radius,
                bg: btn_data.bg,
                fg: btn_data.fg,
                label: btn_data.label.to_string(),
                label_size: btn_data.label_size,
                focused: btn_data.focused,
            };
            btn.render(canvas, 30);
        }

        // Render all radio buttons (z=40)
        for radio_data in self.radios.iter() {
            let radio = elements::radio::Radio {
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

        // Render all input boxes (z=50)
        for input_data in self.input_boxes.iter() {
            let input = elements::input_box::InputBox {
                rect: input_data.rect,
                text: input_data.text.to_string(),
                text_size: input_data.text_size,
                text_color: input_data.text_color,
                placeholder: input_data.placeholder.map(|s| s.to_string()),
                focused: input_data.focused,
            };
            input.render(canvas, 50);
        }

        // Render all text areas (z=60)
        for textarea_data in self.text_areas.iter() {
            let textarea = elements::text_area::TextArea {
                rect: textarea_data.rect,
                lines: textarea_data.lines.iter().map(|s| s.to_string()).collect(),
                text_size: textarea_data.text_size,
                text_color: textarea_data.text_color,
                focused: textarea_data.focused,
                line_height_factor: Some(1.4),
            };
            textarea.render(canvas, 60);
        }

        // Render all selects (z=70)
        for select_data in self.selects.iter() {
            let select = elements::select::Select {
                rect: select_data.rect,
                label: select_data.label.to_string(),
                label_size: select_data.label_size,
                label_color: select_data.label_color,
                open: select_data.open,
                focused: select_data.focused,
            };
            select.render(canvas, 70);
        }

        // Render all labels (z=80)
        for label_data in self.labels.iter() {
            let label = elements::label::Label {
                pos: label_data.pos,
                text: label_data.text.to_string(),
                size: label_data.size,
                color: label_data.color,
            };
            label.render(canvas, 80);
        }

        // Render all images (z=90)
        for image_data in self.images.iter() {
            let image = elements::image::ImageBox {
                rect: image_data.rect,
                path: image_data.path.clone(),
                tint: image_data.tint,
                fit: elements::image::ImageFit::Contain,
            };
            image.render(canvas, 90);
        }

        // Rune-text multi-paragraph wrapping demo (z=100)
        // Build a single multi-paragraph string from the sample paragraphs.
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

        let multiline = elements::multiline_text::MultilineText {
            pos: [self.col1_x, self.multiline_y],
            text: multi_paragraph_text,
            size: 16.0,
            color: Color::rgba(230, 230, 240, 255),
            max_width: None,
            line_height_factor: Some(1.4),
        };
        // Render without any automatic width-based wrapping; only explicit
        // newlines in the text produce line breaks.
        let _ = (scale_factor, provider); // keep signature for now
        multiline.render_simple(canvas, 100);
    }
}
