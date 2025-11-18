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
