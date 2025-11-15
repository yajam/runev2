use engine_core::{Brush, Color, ColorLinPremul, Rect, RoundedRadii, RoundedRect, SvgStyle};
use rune_surface::Canvas;

pub struct Checkbox {
    pub rect: Rect,
    pub checked: bool,
    pub focused: bool,
    pub label: Option<String>,
    pub label_size: f32,
    pub color: ColorLinPremul,
}

impl Checkbox {
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        // Box with small rounded corners; draw fill then stroke so the border sits on top
        let base_fill = Brush::Solid(Color::rgba(240, 240, 240, 255));
        let base_rect = self.rect;
        canvas.fill_rect(
            base_rect.x,
            base_rect.y,
            base_rect.w,
            base_rect.h,
            base_fill,
            z,
        );
        // Top edge accent line, matching ui.rs
        let top_edge = Brush::Solid(Color::rgba(180, 180, 180, 255));
        canvas.fill_rect(base_rect.x, base_rect.y, base_rect.w, 1.0, top_edge, z + 1);
        // Focus outline (inside border)
        if self.focused {
            // Rounded focus outline to match demo-app ui.rs
            let focus_rr = RoundedRect {
                rect: base_rect,
                radii: RoundedRadii {
                    tl: 2.0,
                    tr: 2.0,
                    br: 2.0,
                    bl: 2.0,
                },
            };
            let focus = Brush::Solid(Color::rgba(63, 130, 246, 255));
            canvas.stroke_rounded_rect(focus_rr, 2.0, focus, z + 2);
        }
        // Checked state: fill inner square and draw a crisp white check mark
        if self.checked {
            let inset = 2.0f32;
            let inner = Rect {
                x: self.rect.x + inset,
                y: self.rect.y + inset,
                w: (self.rect.w - 2.0 * inset).max(0.0),
                h: (self.rect.h - 2.0 * inset).max(0.0),
            };
            // Snap inner rect to whole pixels to avoid subpixel blurring of the SVG
            let inner_snapped = Rect {
                x: inner.x.round(),
                y: inner.y.round(),
                w: inner.w.round(),
                h: inner.h.round(),
            };
            let inner_rr = RoundedRect {
                rect: inner_snapped,
                radii: RoundedRadii {
                    tl: 1.5,
                    tr: 1.5,
                    br: 1.5,
                    bl: 1.5,
                },
            };
            canvas.rounded_rect(
                inner_rr,
                Brush::Solid(Color::rgba(63, 130, 246, 255)),
                z + 2,
            );

            // Use the canonical SVG checkmark and center it in the inner rect.
            // Scale it relative to the inner width so it appears larger and
            // visually matches the design, instead of shrinking to min(w, h).
            let icon_size = inner_snapped.w * 0.9;
            let origin = [
                inner_snapped.x + (inner_snapped.w - icon_size) * 0.5,
                inner_snapped.y + (inner_snapped.h - icon_size) * 0.5,
            ];
            let style = SvgStyle::new()
                .with_stroke(Color::rgba(255, 255, 255, 255))
                .with_stroke_width(3.0);
            canvas.draw_svg_styled(
                "images/check.svg",
                origin,
                [icon_size, icon_size],
                style,
                z + 3,
            );
        }
        // Label
        if let Some(text) = &self.label {
            let tx = self.rect.x + self.rect.w + 8.0;
            let ty = self.rect.y + self.rect.h * 0.5 + self.label_size * 0.35;
            canvas.draw_text_run(
                [tx, ty],
                text.clone(),
                self.label_size,
                self.color, // Use the color passed in, which should be light colored
                z + 3,
            );
        }
    }
}
