pub mod types;

pub use types::RuneWindowEvent;

pub fn translate_window_event(event: &winit::event::WindowEvent) -> Option<RuneWindowEvent> {
    use winit::event::{ElementState, WindowEvent};
    match event {
        WindowEvent::Resized(sz) => Some(RuneWindowEvent::Resized(*sz)),
        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
            Some(RuneWindowEvent::ScaleFactorChanged(*scale_factor))
        }
        WindowEvent::CursorMoved { position, .. } => Some(RuneWindowEvent::CursorMoved {
            position: [position.x as f32, position.y as f32],
        }),
        WindowEvent::MouseInput { state, button, .. } => match state {
            ElementState::Pressed => Some(RuneWindowEvent::MousePressed(*button)),
            ElementState::Released => Some(RuneWindowEvent::MouseReleased(*button)),
        },
        WindowEvent::RedrawRequested => Some(RuneWindowEvent::RedrawRequested),
        WindowEvent::CloseRequested => Some(RuneWindowEvent::CloseRequested),
        _ => None,
    }
}
