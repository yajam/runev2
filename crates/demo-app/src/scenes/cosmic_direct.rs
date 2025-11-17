use engine_core::TextProvider;
use engine_core::{ColorLinPremul, SubpixelOrientation, TextRun};
use engine_core::{DisplayList, PassManager, Viewport};

use super::{Scene, SceneKind};

/// Demo scene that renders long text directly from cosmic-text glyph masks,
/// bypassing PassManager's text layout/baseline heuristics.
pub struct CosmicDirectScene {
    provider: engine_core::CosmicTextProvider,
    scale_factor: f32,
    base_size: f32,
    glyphs: engine_core::GlyphBatch,
}

impl CosmicDirectScene {
    fn build_provider() -> engine_core::CosmicTextProvider {
        if let Ok(path) = std::env::var("DEMO_FONT") {
            if let Ok(bytes) = std::fs::read(path) {
                if let Ok(p) =
                    engine_core::CosmicTextProvider::from_bytes(&bytes, SubpixelOrientation::RGB)
                {
                    return p;
                }
            }
        }
        engine_core::CosmicTextProvider::from_system_fonts(SubpixelOrientation::RGB)
    }
}

impl Default for CosmicDirectScene {
    fn default() -> Self {
        let size_env = std::env::var("DEMO_TEXT_SIZE")
            .ok()
            .and_then(|s| s.parse::<f32>().ok());
        let provider = Self::build_provider();
        let mut s = Self {
            provider,
            scale_factor: 1.0,
            base_size: size_env.unwrap_or(20.0),
            glyphs: engine_core::GlyphBatch::new(),
        };
        s.rebuild_glyphs();
        s
    }
}

impl CosmicDirectScene {
    fn rebuild_glyphs(&mut self) {
        let scale = if self.scale_factor.is_finite() && self.scale_factor > 0.0 {
            self.scale_factor
        } else {
            1.0
        };
        let font_px = (self.base_size * scale).max(1.0);
        let margin_x = 40.0 * scale;
        let top = 120.0 * scale;

        let color = ColorLinPremul::from_srgba_u8([255, 255, 255, 255]);
        let lm = self
            .provider
            .line_metrics(font_px)
            .unwrap_or(engine_core::LineMetrics {
                ascent: font_px * 0.8,
                descent: font_px * 0.2,
                line_gap: font_px * 0.2,
            });
        let line_height = lm.ascent + lm.descent + lm.line_gap;
        let baseline0 = top + lm.ascent;

        // Long-ish sample text to stress shaping, spacing, and baseline math.
        const LINES: &[&str] = &[
            "Cosmic-text direct rendering (no PassManager text layout).",
            "This scene draws glyph masks exactly where cosmic-text positions them,",
            "so you can compare against the standard text demo when debugging",
            "baseline snapping, DPI scaling, or long-text artifacts.",
        ];

        let mut glyph_batch = engine_core::GlyphBatch::new();
        // Header lines
        for (i, line) in LINES.iter().enumerate() {
            let baseline_y = baseline0 + (i as f32) * line_height;
            let run = TextRun {
                text: line.to_string(),
                pos: [0.0, 0.0],
                size: font_px,
                color,
            };
            for g in self.provider.rasterize_run(&run) {
                let origin_x = margin_x + g.offset[0];
                let origin_y = baseline_y + g.offset[1];
                glyph_batch
                    .glyphs
                    .push((g.mask, [origin_x, origin_y], color));
            }
        }

        // Stress-test: 200 additional long lines using the same cosmic-text provider.
        let mut y = baseline0 + (LINES.len() as f32) * line_height;
        for i in 0..200 {
            let text = format!("Cosmic {:03}: {}", i, LINES[0]);
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

impl Scene for CosmicDirectScene {
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
        // Soft gradient background so text contrast is easy to see.
        passes.paint_root_linear_gradient_multi(
            encoder,
            surface_view,
            [0.0, 0.0],
            [1.0, 1.0],
            &[
                (0.0, ColorLinPremul::from_srgba_u8([24, 28, 36, 255])),
                (1.0, ColorLinPremul::from_srgba_u8([6, 8, 12, 255])),
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
