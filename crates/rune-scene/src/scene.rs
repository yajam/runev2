//! Scene trait - Abstraction for different rendering approaches
//!
//! Both manual and IR rendering implement this trait, allowing clean separation
//! and reuse of event handling logic.

use rune_surface::Canvas;

/// Result of an event handling operation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SceneResult {
    /// Event was handled, redraw needed
    Handled,
    /// Event was not handled
    Ignored,
}

/// Mouse button event
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    pub x: f32,
    pub y: f32,
    pub button: MouseButton,
    pub click_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Keyboard event
#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub key: KeyCode,
    pub modifiers: Modifiers,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyCode {
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Enter,
    Escape,
    Backspace,
    Delete,
    Tab,
    Char(char),
    Other,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub cmd: bool,
}

/// Trait for scene implementations (manual or IR-based)
pub trait Scene {
    /// Handle mouse click event
    fn handle_mouse_click(&mut self, event: MouseEvent) -> SceneResult;

    /// Handle keyboard event
    fn handle_keyboard(&mut self, event: KeyEvent) -> SceneResult;

    /// Render the scene to canvas
    fn render(&self, canvas: &mut Canvas);

    /// Check if scene needs redraw
    fn needs_redraw(&self) -> bool {
        false
    }
}
