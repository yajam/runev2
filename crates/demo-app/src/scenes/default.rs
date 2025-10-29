use engine_core::{DisplayList, Painter, PassManager, Viewport};
use engine_core::{Brush, ColorLinPremul, Rect};

use super::{Scene, SceneKind};

pub struct DefaultScene;

impl Default for DefaultScene { fn default() -> Self { Self } }

impl Scene for DefaultScene {
    fn kind(&self) -> SceneKind { SceneKind::Geometry }

    fn init_display_list(&mut self, viewport: Viewport) -> Option<DisplayList> {
        let mut painter = Painter::begin_frame(viewport);
        // Solid rect
        painter.rect(
            Rect { x: 60.0, y: 60.0, w: 300.0, h: 200.0 },
            Brush::Solid(ColorLinPremul::from_srgba_u8([82, 167, 232, 255])),
            0,
        );
        // Linear gradient rect (left→right), positioned lower to avoid overlaps
        painter.rect(
            Rect { x: 380.0, y: 600.0, w: 380.0, h: 180.0 },
            Brush::LinearGradient {
                start: [0.0, 0.0],
                end: [1.0, 0.0],
                stops: vec![
                    (0.0, ColorLinPremul::from_srgba_u8([255, 140, 140, 255])),
                    (0.5, ColorLinPremul::from_srgba_u8([140, 180, 255, 255])),
                    (1.0, ColorLinPremul::from_srgba_u8([140, 255, 180, 255])),
                ],
            },
            0,
        );
        // Rounded rect
        painter.rounded_rect(
            engine_core::RoundedRect {
                rect: Rect { x: 420.0, y: 80.0, w: 300.0, h: 180.0 },
                radii: engine_core::RoundedRadii { tl: 24.0, tr: 32.0, br: 24.0, bl: 24.0 },
            },
            Brush::Solid(ColorLinPremul::from_srgba_u8([238, 154, 106, 255])),
            0,
        );
        // Circle
        painter.circle(
            [210.0, 360.0],
            100.0,
            Brush::Solid(ColorLinPremul::from_srgba_u8([133, 235, 190, 255])),
            0,
        );
        // Ellipse with radial gradient
        painter.ellipse(
            [600.0, 420.0],
            [180.0, 110.0],
            Brush::RadialGradient {
                center: [0.5, 0.5],
                radius: 1.0,
                stops: vec![
                    (0.0, ColorLinPremul::from_srgba_u8([255, 255, 190, 255])),
                    (1.0, ColorLinPremul::from_srgba_u8([220, 220, 80, 255])),
                ],
            },
            0,
        );

        Some(painter.finish())
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
        // Layered background: a soft diagonal linear gradient, then a subtle radial vignette
        passes.paint_root_linear_gradient_multi(
            encoder,
            surface_view,
            [0.0, 0.0],
            [1.0, 1.0],
            &[
                (0.0, ColorLinPremul::from_srgba_u8([72, 76, 88, 255])),
                (0.5, ColorLinPremul::from_srgba_u8([80, 85, 97, 255])),
                (1.0, ColorLinPremul::from_srgba_u8([60, 64, 74, 255])),
            ],
            queue,
        );
        passes.paint_root_radial_gradient_multi(
            encoder,
            surface_view,
            [0.75, 0.3],
            0.5,
            &[
                (0.0, ColorLinPremul::from_srgba_u8([40, 40, 40, 0])),
                (1.0, ColorLinPremul::from_srgba_u8([40, 40, 40, 64])),
            ],
            queue,
            width,
            height,
        );
    }
}
