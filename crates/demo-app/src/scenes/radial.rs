use engine_core::{DisplayList, PassManager, Viewport};

use super::{Scene, SceneKind};

pub struct RadialBgScene;

impl Default for RadialBgScene { fn default() -> Self { Self } }

impl Scene for RadialBgScene {
    fn kind(&self) -> SceneKind { SceneKind::FullscreenBackground }

    fn init_display_list(&mut self, _viewport: Viewport) -> Option<DisplayList> { None }

    fn paint_root_background(
        &self,
        passes: &mut PassManager,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
    ) {
        let stops_circle = vec![
            (0.0, engine_core::ColorLinPremul::from_srgba_u8([0x3b, 0x82, 0xf6, 0xff])),
            (0.5, engine_core::ColorLinPremul::from_srgba_u8([0x1e, 0x3a, 0x8a, 0xff])),
            (0.75, engine_core::ColorLinPremul::from_srgba_u8([0x10, 0xb9, 0x81, 0xff])),
            (1.0, engine_core::ColorLinPremul::from_srgba_u8([0x10, 0xb9, 0x81, 0xff])),
        ];
        let center_uv = [0.5f32, 0.5f32];
        let radius = 0.5f32 / std::f32::consts::SQRT_2;
        passes.paint_root_radial_gradient_multi(
            encoder,
            surface_view,
            center_uv,
            radius,
            &stops_circle,
            queue,
            width,
            height,
        );
    }
}
