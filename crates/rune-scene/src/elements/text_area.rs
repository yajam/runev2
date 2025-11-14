use engine_core::{Brush, Rect, Color, ColorLinPremul, RoundedRadii, RoundedRect};
use rune_surface::shapes::{self, BorderStyle, BorderWidths, RectStyle};
use rune_surface::Canvas;
use crate::text::{layout_text, LayoutOptions, Wrap};

pub struct TextArea {
    pub rect: Rect,
    pub lines: Vec<String>,
    pub text_size: f32,
    pub text_color: ColorLinPremul,
    pub focused: bool,
    /// Optional line height multiplier. When None, defaults to 1.2x.
    pub line_height_factor: Option<f32>,
}

impl TextArea {
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        let radius = 6.0;
        let rrect = RoundedRect {
            rect: self.rect,
            radii: RoundedRadii { tl: radius, tr: radius, br: radius, bl: radius },
        };
        
        // Background
        let bg = Color::rgba(45, 52, 71, 255);
        canvas.rounded_rect(rrect, Brush::Solid(bg), z);
        
        // Border
        let border_color = if self.focused {
            Color::rgba(63, 130, 246, 255)
        } else {
            Color::rgba(80, 90, 110, 255)
        };
        let border_width = if self.focused { 2.0 } else { 1.0 };
        shapes::draw_rounded_rectangle(
            canvas,
            rrect,
            None,
            Some(border_width),
            Some(Brush::Solid(border_color)),
            z + 1,
        );

        // Lines (simple layout with dynamic line height factor; default 1.2)
        let lh_factor = self.line_height_factor.unwrap_or(1.2);
        let lh = self.text_size * lh_factor.max(0.5).min(3.0);
        let mut y = self.rect.y + 12.0 + self.text_size;
        for line in &self.lines {
            if y > self.rect.y + self.rect.h - 12.0 { break; }
            canvas.draw_text_run([self.rect.x + 12.0, y], line.clone(), self.text_size, self.text_color, z + 2);
            y += lh;
        }
    }

    /// Render using layout_text helper (wrapping + baselines derived from raster metrics).
    pub fn render_with_provider(
        &self,
        canvas: &mut Canvas,
        z: i32,
        provider: &dyn engine_core::TextProvider,
        scale_factor: Option<f32>,
    ) {
        // Background + borders as in render()
        let bg = Color::rgba(255, 255, 255, 255);
        canvas.fill_rect(self.rect.x, self.rect.y, self.rect.w, self.rect.h, Brush::Solid(bg), z);
        let base_border = Brush::Solid(Color::rgba(200, 200, 200, 255));
        let base_style = RectStyle { fill: None, border: Some(BorderStyle { widths: BorderWidths { top: 1.0, right: 1.0, bottom: 1.0, left: 1.0 }, brush: base_border }) };
        shapes::draw_rectangle(canvas, self.rect.x, self.rect.y, self.rect.w, self.rect.h, &base_style, z + 1);
        if self.focused {
            let focus = Brush::Solid(Color::rgba(63, 130, 246, 255));
            let fo = RectStyle { fill: None, border: Some(BorderStyle { widths: BorderWidths { top: 2.0, right: 2.0, bottom: 2.0, left: 2.0 }, brush: focus }) };
            shapes::draw_rectangle(canvas, self.rect.x, self.rect.y, self.rect.w, self.rect.h, &fo, z + 2);
        }

        // Compute layout using helper
        let pad_x = 8.0f32;
        let pad_y = 8.0f32;
        let max_w = (self.rect.w - 2.0 * pad_x).max(0.0);
        let lh_factor = self.line_height_factor.unwrap_or(1.2);
        let line_pad = (lh_factor - 1.0).max(0.0) * self.text_size;
        let opts = LayoutOptions {
            size_px: self.text_size,
            wrap: Wrap::Word(max_w),
            start_baseline_y: self.rect.y + pad_y,
            line_pad,
            scale_factor,
        };
        let content = if self.lines.is_empty() { String::new() } else { self.lines.join(" ") };
        let res = layout_text(provider, &content, &opts);
        let x = self.rect.x + pad_x;
        let bottom_limit = self.rect.y + self.rect.h - pad_y;
        for (i, line) in res.lines.iter().enumerate() {
            if i >= res.baselines.len() { break; }
            let baseline = res.baselines[i];
            if baseline + res.line_height_est * 0.8 > bottom_limit { break; }
            canvas.draw_text_run([x, baseline], line.clone(), self.text_size, Color::rgba(16, 16, 16, 255), z + 2);
        }
    }
}
