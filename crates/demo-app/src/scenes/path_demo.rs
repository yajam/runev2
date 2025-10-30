use super::{Scene, SceneKind};
use engine_core::{DisplayList, Painter, Viewport, ColorLinPremul, Path, PathCmd, FillRule};

pub struct PathDemoScene;

impl Default for PathDemoScene { fn default() -> Self { Self } }

impl Scene for PathDemoScene {
    fn kind(&self) -> SceneKind { SceneKind::Geometry }

    fn init_display_list(&mut self, viewport: Viewport) -> Option<DisplayList> {
        // Build a simple star path as a proof of geometry path support via lyon
        let mut p = Painter::begin_frame(viewport);
        let cx = viewport.width as f32 * 0.5;
        let cy = viewport.height as f32 * 0.5;
        let r0 = (viewport.height as f32).min(viewport.width as f32) * 0.35;
        let r1 = r0 * 0.45;
        let mut cmds = Vec::new();
        let n = 5;
        for i in 0..(n * 2) {
            let ang = (i as f32) * std::f32::consts::PI / (n as f32);
            let r = if i % 2 == 0 { r0 } else { r1 };
            let x = cx + r * ang.sin();
            let y = cy - r * ang.cos();
            if i == 0 { cmds.push(PathCmd::MoveTo([x, y])); }
            else { cmds.push(PathCmd::LineTo([x, y])); }
        }
        cmds.push(PathCmd::Close);
        let path = Path { cmds, fill_rule: FillRule::NonZero };
        let yellow = ColorLinPremul::from_srgba(255, 220, 30, 1.0);
        p.fill_path(path, yellow, 0);
        Some(p.finish())
    }

    fn paint_root_background(
        &self,
        _passes: &mut engine_core::PassManager,
        _encoder: &mut wgpu::CommandEncoder,
        _surface_view: &wgpu::TextureView,
        _queue: &wgpu::Queue,
        _width: u32,
        _height: u32,
    ) { }
}

