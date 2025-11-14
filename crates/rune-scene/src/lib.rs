use anyhow::Result;
use winit::event::{Event, WindowEvent};
use winit::window::WindowBuilder;
use winit::event_loop::EventLoop;
use engine_core::{make_surface_config, SubpixelOrientation, Color, Rect, ColorLinPremul};

pub mod elements;
pub mod text;

// UI Element Data Structures
#[derive(Clone)]
struct CheckboxData {
    rect: Rect,
    checked: bool,
    focused: bool,
    label: Option<&'static str>,
    label_size: f32,
    color: ColorLinPremul,
}

#[derive(Clone)]
struct ButtonData {
    rect: Rect,
    radius: f32,
    bg: ColorLinPremul,
    fg: ColorLinPremul,
    label: &'static str,
    label_size: f32,
    focused: bool,
}

#[derive(Clone)]
struct TextData {
    pos: [f32; 2],
    text: &'static str,
    size: f32,
    color: ColorLinPremul,
}

#[derive(Clone)]
struct RadioData {
    center: [f32; 2],
    radius: f32,
    selected: bool,
    label: Option<&'static str>,
    label_size: f32,
    label_color: ColorLinPremul,
    focused: bool,
}

#[derive(Clone)]
struct InputBoxData {
    rect: Rect,
    text: &'static str,
    text_size: f32,
    text_color: ColorLinPremul,
    placeholder: Option<&'static str>,
    focused: bool,
}

#[derive(Clone)]
struct TextAreaData {
    rect: Rect,
    lines: Vec<&'static str>,
    text_size: f32,
    text_color: ColorLinPremul,
    focused: bool,
}

#[derive(Clone)]
struct SelectData {
    rect: Rect,
    label: &'static str,
    label_size: f32,
    label_color: ColorLinPremul,
    open: bool,
    focused: bool,
}

#[derive(Clone)]
struct LabelData {
    pos: [f32; 2],
    text: &'static str,
    size: f32,
    color: ColorLinPremul,
}

#[derive(Clone)]
struct ImageData {
    rect: Rect,
    path: Option<std::path::PathBuf>,
    tint: ColorLinPremul,
}

// Unified UI element enum for mixed rendering
#[allow(dead_code)]
enum UIElement {
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

// Direct port of working ui_canvas.rs from demo-app
// KNOWN ISSUE: Background scaling during resize may appear slightly less smooth compared to
// demo-app's geometry-based scenes. This is due to the full UI element rendering overhead.
// Current mitigation: continuous redraws for 100ms after resize events.
pub fn run() -> Result<()> {
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Rune Scene — UI Elements")
        .build(&event_loop)?;
    let window: &'static winit::window::Window = Box::leak(Box::new(window));

    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(window)?;
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }))
    .expect("No suitable GPU adapters found");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))?;

    let mut size = window.inner_size();
    let scale_factor = window.scale_factor() as f32;
    let mut config = make_surface_config(&adapter, &surface, size.width, size.height);
    surface.configure(&device, &config);

    // Canvas wrapper
    let mut surf = rune_surface::RuneSurface::new(
        std::sync::Arc::new(device),
        std::sync::Arc::new(queue),
        config.format,
    );
    surf.set_use_intermediate(true);
    surf.set_direct(true);
    surf.set_logical_pixels(true);
    surf.set_dpi_scale(scale_factor);

    // Provide a text provider (system fonts)
    let provider = engine_core::CosmicTextProvider::from_system_fonts(SubpixelOrientation::RGB);
    let provider = std::sync::Arc::new(provider);

    // Dirty flag: only redraw when something changes
    let mut needs_redraw = true;
    // Track last resize time to enable frequent redraws during resize
    let mut last_resize_time: Option<std::time::Instant> = None;

    // UI Layout constants
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

    // UI Element Data - defined once, reused across frames
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
            color: Color::rgba(0, 255, 255, 255), // Bright cyan
        },
    ];

    let checkboxes = vec![
        CheckboxData {
            rect: Rect { x: col1_x, y: checkbox_y, w: 18.0, h: 18.0 },
            checked: false,
            focused: false,
            label: Some("Checkbox"),
            label_size: 16.0,
            color: Color::rgba(240, 240, 240, 255),
        },
        CheckboxData {
            rect: Rect { x: col1_x + 160.0, y: checkbox_y, w: 18.0, h: 18.0 },
            checked: true,
            focused: true,
            label: Some("Checked + Focus"),
            label_size: 16.0,
            color: Color::rgba(240, 240, 240, 255),
        },
    ];

    let buttons = vec![
        ButtonData {
            rect: Rect { x: col1_x, y: button_y, w: 160.0, h: 36.0 },
            radius: 8.0,
            bg: Color::rgba(63, 130, 246, 255),
            fg: Color::rgba(255, 255, 255, 255),
            label: "Primary",
            label_size: 16.0,
            focused: false,
        },
        ButtonData {
            rect: Rect { x: col1_x + 176.0, y: button_y, w: 180.0, h: 36.0 },
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
            rect: Rect { x: col1_x, y: input_y, w: 200.0, h: 36.0 },
            text: "Hello World",
            text_size: 16.0,
            text_color: Color::rgba(240, 240, 240, 255),
            placeholder: None,
            focused: false,
        },
        InputBoxData {
            rect: Rect { x: col1_x + 220.0, y: input_y, w: 200.0, h: 36.0 },
            text: "",
            text_size: 16.0,
            text_color: Color::rgba(240, 240, 240, 255),
            placeholder: Some("Enter text..."),
            focused: true,
        },
    ];

    let text_areas = vec![
        TextAreaData {
            rect: Rect { x: col1_x, y: textarea_y, w: 420.0, h: 120.0 },
            lines: vec![
                "This is a multi-line text area.",
                "You can add multiple lines of text here.",
                "It supports scrolling and wrapping.",
            ],
            text_size: 14.0,
            text_color: Color::rgba(240, 240, 240, 255),
            focused: false,
        },
    ];

    let selects = vec![
        SelectData {
            rect: Rect { x: col1_x, y: select_y, w: 200.0, h: 36.0 },
            label: "Select an option",
            label_size: 16.0,
            label_color: Color::rgba(240, 240, 240, 255),
            open: false,
            focused: false,
        },
    ];

    let labels = vec![
        LabelData {
            pos: [col1_x, label_y],
            text: "This is a standalone label",
            size: 16.0,
            color: Color::rgba(180, 180, 200, 255),
        },
    ];

    let images = vec![
        ImageData {
            rect: Rect { x: col1_x, y: image_y, w: 120.0, h: 80.0 },
            path: Some("images/squirrel.jpg".into()),
            tint: Color::rgba(100, 150, 200, 255),
        },
        ImageData {
            rect: Rect { x: col1_x + 140.0, y: image_y, w: 120.0, h: 80.0 },
            path: Some("images/fire.jpg".into()),
            tint: Color::rgba(200, 100, 150, 255),
        },
    ];

    // Simple multiline text data
    let multiline_text_data = vec![
        (
            "This is a comprehensive multiline text element demonstration that showcases automatic word wrapping capabilities. The text wraps based on window width, maintaining optimal readability. This implementation uses simple character-based wrapping for fast performance.".to_string(),
            16.0f32,
            Color::rgba(220, 220, 220, 255),
            1.4f32,
        ),
        (
            "Here's a second paragraph with different styling to demonstrate versatility. This text has a slightly smaller font size and tighter line spacing, showing how the multiline text element can be customized for different use cases.".to_string(),
            14.0f32,
            Color::rgba(150, 200, 255, 255),
            1.2f32,
        ),
    ];
    
    // Track text width for resize detection
    let mut current_text_width: Option<f32> = None;

    // Track checkbox states for overlay rendering
    let _cb_size = 18.0f32;

    // Set overlay callback for crisp SVG tick rendering
    let checkboxes_for_overlay = checkboxes.clone();
    surf.set_overlay(Box::new(move |passes, encoder, view, queue, width, height| {
        // Draw checkbox ticks exactly like ui.rs does
        let inset = 2.0f32;
        
        for cb in checkboxes_for_overlay.iter() {
            if cb.checked {
                let inner_x = cb.rect.x + inset;
                let inner_y = cb.rect.y + inset;
                let inner_w = (cb.rect.w - 2.0 * inset).max(0.0);
                let inner_h = (cb.rect.h - 2.0 * inset).max(0.0);
                
                // Snap to whole pixels
                let inner_x = inner_x.round();
                let inner_y = inner_y.round();
                let inner_w = inner_w.round();
                let inner_h = inner_h.round();
                
                if let Some((tick_view, _sw, _sh)) = passes.rasterize_svg_to_view(
                    std::path::Path::new("images/check_white.svg"),
                    1.0,
                    queue,
                ) {
                    passes.draw_image_quad(
                        encoder,
                        view,
                        [inner_x, inner_y],
                        [inner_w, inner_h],
                        &tick_view,
                        queue,
                        width,
                        height,
                    );
                }
            }
        }
    }));

    Ok(event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { window_id, event } if window_id == window.id() => {
                match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::Resized(new_size) => {
                        size = new_size;
                        if size.width > 0 && size.height > 0 {
                            config.width = size.width;
                            config.height = size.height;
                            surface.configure(surf.device().as_ref(), &config);
                        }
                        last_resize_time = Some(std::time::Instant::now());
                        needs_redraw = true;
                        window.request_redraw();
                    }
                    WindowEvent::ScaleFactorChanged { scale_factor: sf, .. } => {
                        let new_scale = sf as f32;
                        surf.set_dpi_scale(new_scale);
                        needs_redraw = true;
                        window.request_redraw();
                    }
                    WindowEvent::RedrawRequested => {
                        if !needs_redraw || size.width == 0 || size.height == 0 { return; }
                        let frame = match surface.get_current_texture() {
                            Ok(f) => f,
                            Err(_) => { window.request_redraw(); return; }
                        };
                        let mut canvas = surf.begin_frame(size.width, size.height);
                        // Background - use clear() for efficient GPU clear operation
                        let bg = Color::rgba(26, 31, 51, 255);
                        canvas.clear(bg);
                        canvas.set_text_provider(provider.clone());

                        // Render all elements
                        // Render all text elements (z=10 for top-level text)
                        for text in texts.iter() {
                            canvas.draw_text_run(text.pos, text.text.to_string(), text.size, text.color, 10);
                        }

                        // Render all checkboxes (z=20, ticks drawn in overlay for crispness)
                        for cb_data in checkboxes.iter() {
                            let cb = elements::checkbox::Checkbox {
                                rect: cb_data.rect,
                                checked: cb_data.checked,
                                focused: cb_data.focused,
                                label: cb_data.label.map(|s| s.to_string()),
                                label_size: cb_data.label_size,
                                color: cb_data.color,
                            };
                            cb.render(&mut canvas, 20);
                        }

                        // Render all buttons (z=30)
                        for btn_data in buttons.iter() {
                            let btn = elements::button::Button {
                                rect: btn_data.rect,
                                radius: btn_data.radius,
                                bg: btn_data.bg,
                                fg: btn_data.fg,
                                label: btn_data.label.to_string(),
                                label_size: btn_data.label_size,
                                focused: btn_data.focused,
                            };
                            btn.render(&mut canvas, 30);
                        }

                        // Render all radio buttons (z=40)
                        for radio_data in radios.iter() {
                            let radio = elements::radio::Radio {
                                center: radio_data.center,
                                radius: radio_data.radius,
                                selected: radio_data.selected,
                                label: radio_data.label.map(|s| s.to_string()),
                                label_size: radio_data.label_size,
                                label_color: radio_data.label_color,
                                focused: radio_data.focused,
                            };
                            radio.render(&mut canvas, 40);
                        }

                        // Render all input boxes (z=50)
                        for input_data in input_boxes.iter() {
                            let input = elements::input_box::InputBox {
                                rect: input_data.rect,
                                text: input_data.text.to_string(),
                                text_size: input_data.text_size,
                                text_color: input_data.text_color,
                                placeholder: input_data.placeholder.map(|s| s.to_string()),
                                focused: input_data.focused,
                            };
                            input.render(&mut canvas, 50);
                        }

                        // Render all text areas (z=60)
                        for textarea_data in text_areas.iter() {
                            let textarea = elements::text_area::TextArea {
                                rect: textarea_data.rect,
                                lines: textarea_data.lines.iter().map(|s| s.to_string()).collect(),
                                text_size: textarea_data.text_size,
                                text_color: textarea_data.text_color,
                                focused: textarea_data.focused,
                                line_height_factor: Some(1.4),
                            };
                            textarea.render(&mut canvas, 60);
                        }

                        // Render all selects (z=70)
                        for select_data in selects.iter() {
                            let select = elements::select::Select {
                                rect: select_data.rect,
                                label: select_data.label.to_string(),
                                label_size: select_data.label_size,
                                label_color: select_data.label_color,
                                open: select_data.open,
                                focused: select_data.focused,
                            };
                            select.render(&mut canvas, 70);
                        }

                        // Render all labels (z=80)
                        for label_data in labels.iter() {
                            let label = elements::label::Label {
                                pos: label_data.pos,
                                text: label_data.text.to_string(),
                                size: label_data.size,
                                color: label_data.color,
                            };
                            label.render(&mut canvas, 80);
                        }

                        // Render all images (z=90)
                        for image_data in images.iter() {
                            let image = elements::image::ImageBox {
                                rect: image_data.rect,
                                path: image_data.path.clone(),
                                tint: image_data.tint,
                                fit: elements::image::ImageFit::Contain, // Default to contain
                            };
                            image.render(&mut canvas, 90);
                        }

                        // Render all multiline texts (z=100) using MultilineText element
                        // Calculate container width based on window size
                        let logical_width = size.width as f32 / scale_factor;
                        let right_margin = 40.0f32;
                        let container_width = (logical_width - col1_x - right_margin).max(200.0).min(1200.0);
                        
                        // Store current width for comparison
                        current_text_width = Some(container_width);
                        
                        let mut current_y = multiline_y;
                        
                        for (text_content, text_size, text_color, lh_factor) in multiline_text_data.iter() {
                            // Create MultilineText element with container width
                            let mtext = elements::multiline_text::MultilineText {
                                pos: [col1_x, current_y],
                                text: text_content.clone(),
                                size: *text_size,
                                color: *text_color,
                                max_width: Some(container_width),
                                line_height_factor: Some(*lh_factor),
                            };
                            
                            // Use fast rendering (character-count approximation)
                            mtext.render_fast(&mut canvas, 100);
                            
                            // Estimate height for next block (approximate)
                            let avg_char_width = text_size * 0.55;
                            let max_chars = (container_width / avg_char_width).floor() as usize;
                            let approx_lines = if max_chars > 0 {
                                (text_content.len() as f32 / max_chars as f32).ceil()
                            } else {
                                1.0
                            };
                            let line_height = text_size * lh_factor;
                            current_y += approx_lines * line_height + 30.0;
                        }
                        
                        surf.end_frame(frame, canvas).ok();
                        needs_redraw = false;
                    }
                    _ => {}
                }
            }
            Event::AboutToWait => {
                // During active resize (within 100ms of last resize event), request continuous redraws
                if let Some(last_time) = last_resize_time {
                    if last_time.elapsed() < std::time::Duration::from_millis(100) {
                        needs_redraw = true;
                        window.request_redraw();
                    } else {
                        // Resize ended - check if text width changed
                        last_resize_time = None;
                        
                        // Calculate new text width
                        let logical_width = size.width as f32 / scale_factor;
                        let right_margin = 40.0f32;
                        let new_text_width = (logical_width - col1_x - right_margin).max(200.0).min(1200.0);
                        
                        // Compare with current width (threshold of 10px to avoid minor changes)
                        let width_changed = current_text_width.map_or(true, |old_width| {
                            (new_text_width - old_width).abs() > 10.0
                        });
                        
                        if width_changed {
                            needs_redraw = true;
                            window.request_redraw();
                        }
                    }
                } else if needs_redraw {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    })?)
}
