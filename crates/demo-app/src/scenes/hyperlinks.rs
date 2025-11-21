use super::{Scene, SceneKind};
use engine_core::{
    Brush, ColorLinPremul, DisplayList, HitKind, HitResult, HitShape, Hyperlink, Painter, Rect,
    Viewport,
};

pub struct HyperlinksScene {
    viewport: Viewport,
    last_clicked_url: Option<String>,
}

impl Default for HyperlinksScene {
    fn default() -> Self {
        Self {
            viewport: Viewport {
                width: 1280,
                height: 720,
            },
            last_clicked_url: None,
        }
    }
}

impl HyperlinksScene {
    fn build_display_list(&self) -> DisplayList {
        let mut p = Painter::begin_frame(self.viewport);

        // White background
        let white = ColorLinPremul::from_srgba_u8([0xff, 0xff, 0xff, 0xff]);
        p.rect(
            Rect {
                x: 0.0,
                y: 0.0,
                w: self.viewport.width as f32,
                h: self.viewport.height as f32,
            },
            Brush::Solid(white),
            0,
        );

        // Title area
        let dark_gray = ColorLinPremul::from_srgba_u8([0x2c, 0x2c, 0x2c, 0xff]);
        p.rect(
            Rect {
                x: 0.0,
                y: 0.0,
                w: self.viewport.width as f32,
                h: 80.0,
            },
            Brush::Solid(ColorLinPremul::from_srgba_u8([0xf5, 0xf6, 0xf7, 0xff])),
            1,
        );

        // Hyperlink examples
        let margin = 50.0;
        let mut y = 150.0;
        let link_spacing = 80.0;

        // Example 1: Blue underlined link
        let blue_link = Hyperlink {
            text: "Visit Claude AI".to_string(),
            pos: [margin, y],
            size: 32.0,
            color: ColorLinPremul::from_srgba_u8([0x00, 0x7a, 0xff, 0xff]),
            url: "https://claude.ai".to_string(),
            underline: true,
            underline_color: None,
        };
        p.hyperlink(blue_link, 10);
        y += link_spacing;

        // Example 2: Purple link with custom underline color
        let purple_link = Hyperlink {
            text: "Anthropic Homepage".to_string(),
            pos: [margin, y],
            size: 32.0,
            color: ColorLinPremul::from_srgba_u8([0x88, 0x00, 0xff, 0xff]),
            url: "https://anthropic.com".to_string(),
            underline: true,
            underline_color: Some(ColorLinPremul::from_srgba_u8([0xcc, 0x00, 0xff, 0xff])),
        };
        p.hyperlink(purple_link, 10);
        y += link_spacing;

        // Example 3: No underline link
        let no_underline_link = Hyperlink {
            text: "Rune Draw Repository (no underline)".to_string(),
            pos: [margin, y],
            size: 32.0,
            color: ColorLinPremul::from_srgba_u8([0x10, 0x90, 0x50, 0xff]),
            url: "https://github.com/rune-draw".to_string(),
            underline: false,
            underline_color: None,
        };
        p.hyperlink(no_underline_link, 10);
        y += link_spacing;

        // Example 4: Smaller link
        let small_link = Hyperlink {
            text: "Small link example".to_string(),
            pos: [margin, y],
            size: 20.0,
            color: ColorLinPremul::from_srgba_u8([0xff, 0x55, 0x00, 0xff]),
            url: "https://example.com/small".to_string(),
            underline: true,
            underline_color: None,
        };
        p.hyperlink(small_link, 10);
        y += link_spacing;

        // Display clicked URL feedback
        if let Some(ref url) = self.last_clicked_url {
            let feedback_y = self.viewport.height as f32 - 100.0;

            // Feedback box
            p.rect(
                Rect {
                    x: margin - 10.0,
                    y: feedback_y - 10.0,
                    w: self.viewport.width as f32 - 2.0 * margin + 20.0,
                    h: 60.0,
                },
                Brush::Solid(ColorLinPremul::from_srgba_u8([0xe8, 0xf4, 0xfd, 0xff])),
                5,
            );

            // Feedback text showing the clicked URL
            let feedback_text = Hyperlink {
                text: format!("Last clicked: {}", url),
                pos: [margin, feedback_y + 30.0],
                size: 24.0,
                color: dark_gray,
                url: url.clone(),
                underline: false,
                underline_color: None,
            };
            p.hyperlink(feedback_text, 15);
        }

        p.finish()
    }
}

impl Scene for HyperlinksScene {
    fn kind(&self) -> SceneKind {
        SceneKind::Geometry
    }

    fn init_display_list(&mut self, viewport: Viewport) -> Option<DisplayList> {
        self.viewport = viewport;
        Some(self.build_display_list())
    }

    fn on_resize(&mut self, viewport: Viewport) -> Option<DisplayList> {
        self.viewport = viewport;
        Some(self.build_display_list())
    }

    fn paint_root_background(
        &self,
        _passes: &mut engine_core::PassManager,
        _encoder: &mut wgpu::CommandEncoder,
        _surface_view: &wgpu::TextureView,
        _queue: &wgpu::Queue,
        _width: u32,
        _height: u32,
    ) {
    }

    fn on_click(&mut self, _pos: [f32; 2], hit: Option<&HitResult>) -> Option<DisplayList> {
        if let Some(h) = hit {
            // Check if we clicked on a hyperlink
            if h.kind == HitKind::Hyperlink {
                if let HitShape::Hyperlink { url, .. } = &h.shape {
                    println!("Hyperlink clicked: {}", url);
                    self.last_clicked_url = Some(url.clone());
                    return Some(self.build_display_list());
                }
            }
        }
        None
    }
}
