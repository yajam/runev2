use crate::elements;
/// Viewport IR module - contains the extended UI/IR elements for the viewport zone
/// (formerly `sample_ui`, now evolving into the extended IR implementation)
use engine_core::{Color, ColorLinPremul, Rect};

// Hit region IDs for checkboxes
pub const CHECKBOX1_REGION_ID: u32 = 2001;
pub const CHECKBOX2_REGION_ID: u32 = 2002;

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
        elements::input_box::InputBox::new(
            Rect {
                x: col1_x,
                y: input_y,
                w: 200.0,
                h: 36.0,
            },
            "Hello World".to_string(),
            16.0,
            Color::rgba(240, 240, 240, 255),
            None,
            false,
        ),
        elements::input_box::InputBox::new(
            Rect {
                x: col1_x + 220.0,
                y: input_y,
                w: 200.0,
                h: 36.0,
            },
            "".to_string(),
            16.0,
            Color::rgba(240, 240, 240, 255),
            Some("Enter text...".to_string()),
            true,
        ),
    ];

    let text_areas = vec![
        elements::text_area::TextArea::new(
            Rect {
                x: col1_x,
                y: textarea_y,
                w: 420.0,
                h: 120.0,
            },
            "This is a multi-line text area.\nYou can add multiple lines of text here.\nIt supports scrolling and wrapping.".to_string(),
            16.0,
            Color::rgba(240, 240, 240, 255),
            Some("Enter multi-line text...".to_string()),
            false,
        ),
    ];

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
        (
            "Paragraph 6: Testing DPI scaling with this additional paragraph. The text should render 
             at the correct size for your display, whether it's a standard monitor or a high-DPI 
             Retina display. Font metrics should remain consistent across different scales.",
            14.5f32,
            Color::rgba(255, 200, 150, 255),
            1.3f32,
        ),
        (
            "Paragraph 7: Zone-relative positioning is critical for proper layout. Each zone (viewport, 
             toolbar, sidebar) has its own coordinate system, and text must respect these boundaries. 
             This paragraph tests that the transform stack is working correctly.",
            15.5f32,
            Color::rgba(255, 180, 200, 255),
            1.28f32,
        ),
        (
            "Paragraph 8: Performance testing with batched rendering. All glyphs should be drawn in a 
             single GPU call rather than individual calls per glyph. This prevents the UI from hanging 
             when rendering large amounts of text like these paragraphs.",
            14.0f32,
            Color::rgba(180, 220, 255, 255),
            1.32f32,
        ),
        (
            "Paragraph 9: HarfBuzz shaping via harfrust provides proper text layout with correct glyph 
             positioning, kerning, and ligatures. The rune-text provider uses swash for rasterization, 
             producing high-quality subpixel-rendered glyphs.",
            15.0f32,
            Color::rgba(220, 180, 255, 255),
            1.35f32,
        ),
        (
            "Paragraph 10: Direct rasterization bypasses the complex display list path. Text is 
             rasterized immediately when draw_text_run is called, making the code simpler and more 
             reliable. This is the same approach used by the working harfrust_text scene.",
            14.5f32,
            Color::rgba(255, 220, 180, 255),
            1.3f32,
        ),
        (
            "Paragraph 11: Color support is per-vertex, allowing each glyph to have its own color. 
             This paragraph uses a warm orange tone to demonstrate the color variety. The shader 
             applies the color to the rasterized mask for proper blending.",
            15.0f32,
            Color::rgba(255, 165, 80, 255),
            1.33f32,
        ),
        (
            "Paragraph 12: Baseline alignment ensures text sits at the correct vertical position. 
             The baseline is calculated from font metrics and adjusted for the current DPI scale. 
             All text in a line should align properly regardless of font size.",
            14.0f32,
            Color::rgba(180, 255, 220, 255),
            1.27f32,
        ),
        (
            "Paragraph 13: Subpixel rendering improves text clarity on LCD displays by using RGB 
             subpixel coverage. The grayscale_to_subpixel_rgb function converts grayscale masks to 
             RGB masks with proper orientation (RGB or BGR) for your display.",
            15.5f32,
            Color::rgba(220, 255, 180, 255),
            1.34f32,
        ),
        (
            "Paragraph 14: Transform stack support means text respects push_transform and pop_transform 
             calls. When rendering in different zones, the canvas automatically applies the correct 
             translation to position text relative to the zone origin.",
            14.5f32,
            Color::rgba(255, 200, 220, 255),
            1.31f32,
        ),
        (
            "Paragraph 15: This final paragraph completes the test suite. If you can read all 15 
             paragraphs with proper sizing, positioning, and colors, then the simplified text 
             rendering system is working correctly! The text should be crisp and performant.",
            16.0f32,
            Color::rgba(200, 220, 255, 255),
            1.4f32,
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
    pub input_boxes: Vec<elements::input_box::InputBox>,
    pub text_areas: Vec<elements::text_area::TextArea>,
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
    /// Returns the total content height.
    pub fn render(
        &mut self,
        canvas: &mut rune_surface::Canvas,
        scale_factor: f32,
        window_width: u32,
        provider: &dyn engine_core::TextProvider,
        text_cache: &engine_core::TextLayoutCache,
    ) -> f32 {
        // Render all text elements using direct rasterization (simpler, more reliable)
        for text in self.texts.iter() {
            canvas.draw_text_direct(text.pos, text.text, text.size, text.color, provider);
        }

        // Render all checkboxes (z=20, ticks drawn in overlay for crispness)
        for (idx, cb_data) in self.checkboxes.iter().enumerate() {
            let cb = elements::checkbox::Checkbox {
                rect: cb_data.rect,
                checked: cb_data.checked,
                focused: cb_data.focused,
                label: cb_data.label.map(|s| s.to_string()),
                label_size: cb_data.label_size,
                color: cb_data.color,
            };
            cb.render(canvas, 20);
            
            // Register hit region for checkbox
            let region_id = match idx {
                0 => CHECKBOX1_REGION_ID,
                1 => CHECKBOX2_REGION_ID,
                _ => 2000 + idx as u32, // fallback for additional checkboxes
            };
            canvas.hit_region_rect(region_id, cb_data.rect, 20);
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
        for input in self.input_boxes.iter_mut() {
            input.render(canvas, 50, provider);
        }

        // Render all text areas (z=60)
        for textarea in self.text_areas.iter_mut() {
            textarea.render(canvas, 60, provider);
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

        // Calculate max width for text wrapping based on viewport width
        let right_margin = 40.0f32;
        let text_max_width = (window_width as f32 - self.col1_x - right_margin)
            .max(200.0)
            .min(1200.0);

        let multiline = elements::multiline_text::MultilineText {
            pos: [self.col1_x, self.multiline_y],
            text: multi_paragraph_text,
            size: 16.0,
            color: Color::rgba(230, 230, 240, 255),
            max_width: Some(text_max_width),
            line_height_factor: Some(1.4),
        };
        // Use cached rendering for efficient resize performance
        let _ = (scale_factor, provider); // keep signature for now
        let multiline_height = multiline.render_cached(canvas, 100, text_cache);

        // Calculate total content height
        // Find the bottom-most element by checking all fixed elements
        let mut max_y = 0.0f32;

        // Check all text elements
        for text in self.texts.iter() {
            max_y = max_y.max(text.pos[1] + text.size);
        }

        // Check checkboxes
        for cb in self.checkboxes.iter() {
            max_y = max_y.max(cb.rect.y + cb.rect.h + 20.0); // +20 for label
        }

        // Check buttons
        for btn in self.buttons.iter() {
            max_y = max_y.max(btn.rect.y + btn.rect.h);
        }

        // Check radios
        for radio in self.radios.iter() {
            max_y = max_y.max(radio.center[1] + radio.radius + 20.0); // +20 for label
        }

        // Check input boxes
        for input in self.input_boxes.iter() {
            max_y = max_y.max(input.rect.y + input.rect.h);
        }

        // Check text areas
        for textarea in self.text_areas.iter() {
            max_y = max_y.max(textarea.rect.y + textarea.rect.h);
        }

        // Check selects
        for select in self.selects.iter() {
            max_y = max_y.max(select.rect.y + select.rect.h);
        }

        // Check labels
        for label in self.labels.iter() {
            max_y = max_y.max(label.pos[1] + label.size);
        }

        // Check images
        for image in self.images.iter() {
            max_y = max_y.max(image.rect.y + image.rect.h);
        }

        // Account for multiline text height based on actual layout result
        let multiline_bottom = self.multiline_y + multiline_height;
        max_y = max_y.max(multiline_bottom);

        max_y
    }
}
