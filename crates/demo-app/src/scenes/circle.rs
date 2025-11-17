use engine_core::{Brush, ColorLinPremul};
use engine_core::{DisplayList, Painter, PassManager, Viewport};

use super::{Scene, SceneKind};

pub struct CircleScene;

impl Default for CircleScene {
    fn default() -> Self {
        Self
    }
}

impl Scene for CircleScene {
    fn kind(&self) -> SceneKind {
        SceneKind::Geometry
    }

    fn init_display_list(&mut self, viewport: Viewport) -> Option<DisplayList> {
        Some(build_circle_dl(viewport))
    }

    fn on_resize(&mut self, viewport: Viewport) -> Option<DisplayList> {
        Some(build_circle_dl(viewport))
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
        // Same background as default scene
        passes.paint_root_linear_gradient_multi(
            encoder,
            surface_view,
            [0.0, 0.0],
            [1.0, 1.0],
            &[
                (
                    0.0,
                    engine_core::ColorLinPremul::from_srgba_u8([72, 76, 88, 255]),
                ),
                (
                    0.5,
                    engine_core::ColorLinPremul::from_srgba_u8([80, 85, 97, 255]),
                ),
                (
                    1.0,
                    engine_core::ColorLinPremul::from_srgba_u8([60, 64, 74, 255]),
                ),
            ],
            queue,
        );
        passes.paint_root_radial_gradient_multi(
            encoder,
            surface_view,
            [0.75, 0.3],
            0.5,
            &[
                (
                    0.0,
                    engine_core::ColorLinPremul::from_srgba_u8([40, 40, 40, 0]),
                ),
                (
                    1.0,
                    engine_core::ColorLinPremul::from_srgba_u8([40, 40, 40, 64]),
                ),
            ],
            queue,
            width,
            height,
        );
    }
}

fn build_circle_dl(viewport: Viewport) -> DisplayList {
    let mut painter = Painter::begin_frame(viewport);
    let center = [viewport.width as f32 * 0.5, viewport.height as f32 * 0.5];
    let radius = (viewport.width.min(viewport.height) as f32) * 0.30;
    painter.circle(
        center,
        radius,
        Brush::RadialGradient {
            center: [0.5, 0.5],
            radius: 1.0,
            stops: vec![
                (0.0, ColorLinPremul::from_srgba_u8([0x3b, 0x82, 0xf6, 0xff])),
                (0.5, ColorLinPremul::from_srgba_u8([0x1e, 0x3a, 0x8a, 0xff])),
                (
                    0.75,
                    ColorLinPremul::from_srgba_u8([0x10, 0xb9, 0x81, 0xff]),
                ),
                (1.0, ColorLinPremul::from_srgba_u8([0x10, 0xb9, 0x81, 0xff])),
            ],
        },
        0,
    );
    painter.finish()
}
