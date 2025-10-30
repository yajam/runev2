use engine_core::{BoxShadowSpec, ColorLinPremul, Rect, RoundedRadii, RoundedRect};
use engine_core::{PassManager, Viewport};

use super::{Scene, SceneKind};

pub struct ShadowScene {
    viewport: Viewport,
}

impl Default for ShadowScene {
    fn default() -> Self {
        Self {
            viewport: Viewport {
                width: 1280,
                height: 720,
            },
        }
    }
}

impl Scene for ShadowScene {
    fn kind(&self) -> SceneKind {
        SceneKind::FullscreenBackground
    }

    fn init_display_list(&mut self, viewport: Viewport) -> Option<engine_core::DisplayList> {
        self.viewport = viewport;
        None
    }

    fn on_resize(&mut self, viewport: Viewport) -> Option<engine_core::DisplayList> {
        self.viewport = viewport;
        None
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
        // Light background via a full-screen rectangle (avoids any paint_root_color issues)
        let bg_rect = RoundedRect {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: width as f32,
                h: height as f32,
            },
            radii: RoundedRadii {
                tl: 0.0,
                tr: 0.0,
                br: 0.0,
                bl: 0.0,
            },
        };
        let bg_color = ColorLinPremul::from_srgba_u8([0xf4, 0xf6, 0xfa, 0xff]);
        passes.draw_filled_rounded_rect(
            encoder,
            surface_view,
            width,
            height,
            bg_rect,
            bg_color,
            queue,
        );

        // Centered rounded rect shadow. Use fixed logical pixels so it doesn't
        // scale with window size (matches CSS demo screenshots better).
        let rect_w = 480.0;
        let rect_h = 300.0;
        let x = (width as f32 - rect_w) * 0.5;
        let y = (height as f32 - rect_h) * 0.5;
        let rrect = RoundedRect {
            rect: Rect {
                x,
                y,
                w: rect_w,
                h: rect_h,
            },
            radii: RoundedRadii {
                tl: 22.0,
                tr: 22.0,
                br: 22.0,
                bl: 22.0,
            },
        };
        let spec = BoxShadowSpec {
            // Match CSS: box-shadow: 0px 18px 12px 8px rgba(0,0,0,0.3)
            // In y-down coords, positive Y moves the shadow downward.
            offset: [0.0, 18.0],
            spread: 8.0,
            blur_radius: 12.0,
            color: ColorLinPremul::from_srgba(0, 0, 0, 0.30),
        };
        passes.draw_box_shadow(encoder, surface_view, width, height, rrect, spec, queue);

        // Draw the semi-transparent white centered rectangle above the shadow
        // Slightly reduce alpha so the shadow visibility is clearer
        let fill = ColorLinPremul::from_srgba(0, 255, 255, 0.50);
        passes.draw_filled_rounded_rect(encoder, surface_view, width, height, rrect, fill, queue);
    }
}
