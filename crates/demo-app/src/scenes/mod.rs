use engine_core::{DisplayList, PassManager, Viewport};

pub enum SceneKind {
    Geometry,
    FullscreenBackground,
}

pub trait Scene {
    fn kind(&self) -> SceneKind;
    fn init_display_list(&mut self, viewport: Viewport) -> Option<DisplayList>;
    fn on_resize(&mut self, viewport: Viewport) -> Option<DisplayList> { let _ = viewport; None }
    fn paint_root_background(
        &self,
        passes: &mut PassManager,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
    );
}

pub mod default;
pub mod circle;
pub mod radial;
pub mod linear;
pub mod centered_rect;
