use engine_core::{ColorLinPremul, SubpixelOrientation, TextRun};
use engine_core::{DisplayList, PassManager, Viewport};
use engine_core::{LineMetrics, TextProvider};

use super::{Scene, SceneKind};

/// Demo scene that renders many lines of text using rune-text + harfrust
/// via `RuneTextProvider`, bypassing PassManager's text layout path.
pub struct HarfrustTextScene {
    provider: engine_core::RuneTextProvider,
    scale_factor: f32,
    base_size: f32,
    glyphs: engine_core::GlyphBatch,
}

impl HarfrustTextScene {
    fn build_provider() -> engine_core::RuneTextProvider {
        if let Ok(path) = std::env::var("DEMO_FONT") {
            if let Ok(bytes) = std::fs::read(path) {
                if let Ok(p) =
                    engine_core::RuneTextProvider::from_bytes(&bytes, SubpixelOrientation::RGB)
                {
                    return p;
                }
            }
        }
        engine_core::RuneTextProvider::from_system_fonts(SubpixelOrientation::RGB)
            .expect("rune-text: failed to load a system font via fontdb")
    }
}

impl Default for HarfrustTextScene {
    fn default() -> Self {
        let size_env = std::env::var("DEMO_TEXT_SIZE")
            .ok()
            .and_then(|s| s.parse::<f32>().ok());
        let provider = Self::build_provider();
        let mut s = Self {
            provider,
            scale_factor: 1.0,
            base_size: size_env.unwrap_or(18.0),
            glyphs: engine_core::GlyphBatch::new(),
        };
        s.rebuild_glyphs();
        s
    }
}

impl HarfrustTextScene {
    fn rebuild_glyphs(&mut self) {
        let scale = if self.scale_factor.is_finite() && self.scale_factor > 0.0 {
            self.scale_factor
        } else {
            1.0
        };
        let font_px = (self.base_size * scale).max(1.0);
        let margin_x = 40.0 * scale;
        let top = 80.0 * scale;

        let color = ColorLinPremul::from_srgba_u8([255, 255, 255, 255]);
        let lm: LineMetrics = self.provider.line_metrics(font_px).unwrap_or(LineMetrics {
            ascent: font_px * 0.8,
            descent: font_px * 0.2,
            line_gap: font_px * 0.2,
        });
        let line_height = lm.ascent + lm.descent + lm.line_gap;
        let baseline0 = top + lm.ascent;

        let mut glyph_batch = engine_core::GlyphBatch::new();

        // Header line to label the demo.
        let header = "Harfrust text demo (rune-text + harfrust)";
        let header_run = TextRun {
            text: header.to_string(),
            pos: [0.0, 0.0],
            size: font_px * 1.1,
            color,
        };
        for g in self.provider.rasterize_run(&header_run) {
            let origin_x = margin_x + g.offset[0];
            let origin_y = baseline0 + g.offset[1];
            glyph_batch
                .glyphs
                .push((g.mask, [origin_x, origin_y], color));
        }

        // 200 short lines to stress shaping + baseline math without overflowing buffers.
        let mut y = baseline0 + line_height * 1.5;
        for i in 0..200 {
            let text = format!("Harfrust {:03}: quick brown fox", i);
            let run = TextRun {
                text,
                pos: [0.0, 0.0],
                size: font_px,
                color,
            };
            for g in self.provider.rasterize_run(&run) {
                let origin_x = margin_x + g.offset[0];
                let origin_y = y + g.offset[1];
                glyph_batch
                    .glyphs
                    .push((g.mask, [origin_x, origin_y], color));
            }
            y += line_height;
        }

        self.glyphs = glyph_batch;
    }
}

impl Scene for HarfrustTextScene {
    fn kind(&self) -> SceneKind {
        SceneKind::FullscreenBackground
    }

    fn init_display_list(&mut self, _viewport: Viewport) -> Option<DisplayList> {
        None
    }

    fn set_scale_factor(&mut self, sf: f32) {
        let sf = if sf.is_finite() && sf > 0.0 { sf } else { 1.0 };
        if (sf - self.scale_factor).abs() > f32::EPSILON {
            self.scale_factor = sf;
            self.rebuild_glyphs();
        }
    }

    fn paint_root_background(
        &self,
        passes: &mut PassManager,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
    ) {
        // Soft gradient background.
        passes.paint_root_linear_gradient_multi(
            encoder,
            surface_view,
            [0.0, 0.0],
            [1.0, 1.0],
            &[
                (0.0, ColorLinPremul::from_srgba_u8([30, 34, 44, 255])),
                (1.0, ColorLinPremul::from_srgba_u8([10, 12, 18, 255])),
            ],
            queue,
        );

        if !self.glyphs.is_empty() {
            passes.draw_text_mask(
                encoder,
                surface_view,
                width,
                height,
                &self.glyphs.glyphs,
                queue,
                0.0,
            );
        }
    }
}
