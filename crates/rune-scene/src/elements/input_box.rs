use crate::text::{LayoutOptions, Wrap, measure_run};
use engine_core::{Brush, Color, ColorLinPremul, FillRule, Path, PathCmd, Rect, RoundedRadii, RoundedRect};
use rune_surface::Canvas;
use rune_surface::shapes::{self, BorderStyle, BorderWidths, RectStyle};

pub struct InputBox {
    pub rect: Rect,
    pub text: String,
    pub text_size: f32,
    pub text_color: ColorLinPremul,
    pub placeholder: Option<String>,
    pub focused: bool,
}

impl InputBox {
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

        // Text or placeholder
        let tp = [
            self.rect.x + 12.0,
            self.rect.y + self.rect.h * 0.5 + self.text_size * 0.35,
        ];
        
        if !self.text.is_empty() {
            canvas.draw_text_run(
                tp,
                self.text.clone(),
                self.text_size,
                self.text_color,
                z + 2,
            );
        } else if let Some(ref placeholder) = self.placeholder {
            canvas.draw_text_run(
                tp,
                placeholder.clone(),
                self.text_size,
                Color::rgba(120, 120, 130, 255),
                z + 2,
            );
        }

        // Caret if focused
        if self.focused {
            let cx = tp[0] + 2.0;
            let cy0 = self.rect.y + 8.0;
            let cy1 = self.rect.y + self.rect.h - 8.0;
            let mut caret = Path {
                cmds: Vec::new(),
                fill_rule: FillRule::NonZero,
            };
            caret.cmds.push(PathCmd::MoveTo([cx, cy0]));
            caret.cmds.push(PathCmd::LineTo([cx, cy1]));
            canvas.stroke_path(caret, 1.5, Color::rgba(63, 130, 246, 255), z + 3);
        }
    }

    /// Render using text layout helper (provider-agnostic, pixel-accurate).
    pub fn render_with_provider(
        &self,
        canvas: &mut Canvas,
        z: i32,
        provider: &dyn engine_core::TextProvider,
        scale_factor: Option<f32>,
    ) {
        // Background and border
        let bg = Color::rgba(255, 255, 255, 255);
        canvas.fill_rect(
            self.rect.x,
            self.rect.y,
            self.rect.w,
            self.rect.h,
            Brush::Solid(bg),
            z,
        );
        // Base border
        let base_border = Brush::Solid(Color::rgba(200, 200, 200, 255));
        let base_style = RectStyle {
            fill: None,
            border: Some(BorderStyle {
                widths: BorderWidths {
                    top: 1.0,
                    right: 1.0,
                    bottom: 1.0,
                    left: 1.0,
                },
                brush: base_border,
            }),
        };
        shapes::draw_rectangle(
            canvas,
            self.rect.x,
            self.rect.y,
            self.rect.w,
            self.rect.h,
            &base_style,
            z + 1,
        );
        // Focus outline
        if self.focused {
            let focus = Brush::Solid(Color::rgba(63, 130, 246, 255));
            let fo = RectStyle {
                fill: None,
                border: Some(BorderStyle {
                    widths: BorderWidths {
                        top: 2.0,
                        right: 2.0,
                        bottom: 2.0,
                        left: 2.0,
                    },
                    brush: focus,
                }),
            };
            shapes::draw_rectangle(
                canvas,
                self.rect.x,
                self.rect.y,
                self.rect.w,
                self.rect.h,
                &fo,
                z + 2,
            );
        }
        // Text layout: single line, centered vertically
        let pad_x = 8.0f32;
        let max_w = (self.rect.w - pad_x * 2.0).max(0.0);
        let opts = LayoutOptions {
            size_px: self.text_size,
            wrap: Wrap::NoWrap,
            start_baseline_y: 0.0,
            line_pad: 0.0,
            scale_factor,
        };
        // Measure to compute a centered baseline
        if let Some(m) = measure_run(provider, &self.text, self.text_size) {
            let baseline = self.rect.y + (self.rect.h - m.height).max(0.0) * 0.5 - m.top;
            let x = self.rect.x + pad_x;
            // Truncate visually if too wide; a full elide is out of scope here
            let mut text = self.text.clone();
            if m.width > max_w {
                // naive character fallback to fit
                let mut acc = String::new();
                for ch in text.chars() {
                    let cand = format!("{}{}", acc, ch);
                    if measure_run(provider, &cand, self.text_size)
                        .map(|mm| mm.width)
                        .unwrap_or(0.0)
                        <= max_w
                    {
                        acc = cand;
                    } else {
                        break;
                    }
                }
                text = acc;
            }
            canvas.draw_text_run(
                [x, baseline],
                text,
                self.text_size,
                Color::rgba(0, 0, 0, 255),
                z + 2,
            );
            // Caret
            if self.focused {
                let caret_x = x
                    + measure_run(provider, &self.text, self.text_size)
                        .map(|mm| mm.width.min(max_w))
                        .unwrap_or(0.0)
                    + 2.0;
                let cy0 = (baseline + m.top + 1.0).max(self.rect.y + 2.0);
                let cy1 = (baseline + m.bottom - 1.0).min(self.rect.y + self.rect.h - 2.0);
                let mut caret = Path {
                    cmds: Vec::new(),
                    fill_rule: FillRule::NonZero,
                };
                caret.cmds.push(PathCmd::MoveTo([caret_x, cy0]));
                caret.cmds.push(PathCmd::LineTo([caret_x, cy1]));
                canvas.stroke_path(caret, 1.0, Color::rgba(63, 130, 246, 255), z + 3);
            }
        }
        let _ = opts; // keep for future extension
    }
}
