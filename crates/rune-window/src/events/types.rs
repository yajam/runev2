use winit::dpi::PhysicalSize;
use winit::event::MouseButton;

#[derive(Debug, Clone)]
pub enum RuneWindowEvent {
    Resized(PhysicalSize<u32>),
    ScaleFactorChanged(f64),
    CursorMoved { position: [f32; 2] },
    MousePressed(MouseButton),
    MouseReleased(MouseButton),
    RedrawRequested,
    CloseRequested,
}
