use engine_core::{DisplayList, HitResult, PassManager, Viewport};

pub enum SceneKind {
    Geometry,
    FullscreenBackground,
}

pub trait Scene {
    fn kind(&self) -> SceneKind;
    fn init_display_list(&mut self, viewport: Viewport) -> Option<DisplayList>;
    fn on_resize(&mut self, viewport: Viewport) -> Option<DisplayList> {
        let _ = viewport;
        None
    }
    // DPI scale factor (logical pixels). Default no-op for scenes that don't care.
    fn set_scale_factor(&mut self, _sf: f32) {}
    fn paint_root_background(
        &self,
        passes: &mut PassManager,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
    );

    // Pointer event hooks (Phase 6.5). Scenes can return an updated DisplayList
    // to reflect interactions (e.g., hover highlight), or None to keep current.
    fn on_pointer_move(&mut self, _pos: [f32; 2], _hit: Option<&HitResult>) -> Option<DisplayList> {
        None
    }
    fn on_pointer_down(&mut self, _pos: [f32; 2], _hit: Option<&HitResult>) -> Option<DisplayList> {
        None
    }
    fn on_pointer_up(&mut self, _pos: [f32; 2], _hit: Option<&HitResult>) -> Option<DisplayList> {
        None
    }
    fn on_click(&mut self, _pos: [f32; 2], _hit: Option<&HitResult>) -> Option<DisplayList> {
        None
    }
    fn on_drag(&mut self, _pos: [f32; 2], _hit: Option<&HitResult>) -> Option<DisplayList> {
        None
    }
}

pub mod centered_rect;
pub mod circle;
pub mod default;
pub mod hyperlinks;
pub mod images;
pub mod linear;
pub mod overlay;
pub mod path_demo;
pub mod radial;
pub mod shadow;
pub mod svg_geom;
pub mod ui;
pub mod unified_test;
pub mod zones;

#[cfg(any(feature = "cef", feature = "cdp"))]
pub mod cef;
