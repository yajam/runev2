use engine_core::{DisplayList, PassManager, Viewport};

use super::{Scene, SceneKind};

pub struct LinearBgScene;

impl Default for LinearBgScene { fn default() -> Self { Self } }

impl Scene for LinearBgScene {
    fn kind(&self) -> SceneKind { SceneKind::FullscreenBackground }

    fn init_display_list(&mut self, _viewport: Viewport) -> Option<DisplayList> { None }

    fn paint_root_background(
        &self,
        passes: &mut PassManager,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        queue: &wgpu::Queue,
        _width: u32,
        _height: u32,
    ) {
        // Multi-stop angled linear gradient (45 degrees, top-left to bottom-right)
        let stops_linear = vec![
            (0.0, engine_core::ColorLinPremul::from_srgba_u8([0xff, 0x6b, 0x6b, 0xff])),
            (0.25, engine_core::ColorLinPremul::from_srgba_u8([0xff, 0xd9, 0x3d, 0xff])),
            (0.5, engine_core::ColorLinPremul::from_srgba_u8([0x6b, 0xcf, 0x63, 0xff])),
            (0.75, engine_core::ColorLinPremul::from_srgba_u8([0x4d, 0x96, 0xff, 0xff])),
            (1.0, engine_core::ColorLinPremul::from_srgba_u8([0xc7, 0x7d, 0xff, 0xff])),
        ];
        passes.paint_root_linear_gradient_multi(
            encoder,
            surface_view,
            [0.0, 0.0],
            [1.0, 1.0],
            &stops_linear,
            queue,
        );
    }
}
