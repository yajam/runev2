///! Event handling traits and utilities for viewport elements
///!
///! This module provides a unified interface for interactive elements to handle
///! mouse and keyboard events, moving event logic from the centralized lib.rs
///! into the elements themselves.
use winit::event::{ElementState, MouseButton};
use winit::keyboard::{KeyCode, ModifiersState};

/// Result of an event handling operation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventResult {
    /// Event was handled and should not propagate
    Handled,
    /// Event was not handled, continue propagation
    Ignored,
}

impl EventResult {
    pub fn is_handled(&self) -> bool {
        matches!(self, EventResult::Handled)
    }
}

/// Mouse click event data
#[derive(Debug, Clone, Copy)]
pub struct MouseClickEvent {
    /// Mouse button that was clicked
    pub button: MouseButton,
    /// Button state (Pressed or Released)
    pub state: ElementState,
    /// X coordinate in viewport space
    pub x: f32,
    /// Y coordinate in viewport space
    pub y: f32,
    /// Number of consecutive clicks (1 = single, 2 = double, 3 = triple)
    pub click_count: u32,
}

/// Keyboard input event data
#[derive(Debug, Clone, Copy)]
pub struct KeyboardEvent {
    /// Key code
    pub key: KeyCode,
    /// Key state (Pressed or Released)
    pub state: ElementState,
    /// Keyboard modifiers (Ctrl, Shift, Alt, etc.)
    pub modifiers: ModifiersState,
}

/// Mouse move event data
#[derive(Debug, Clone, Copy)]
pub struct MouseMoveEvent {
    /// X coordinate in viewport space
    pub x: f32,
    /// Y coordinate in viewport space
    pub y: f32,
}

/// Unified event handler trait for interactive elements
///
/// Elements implement this trait to handle user input events.
/// The event loop in lib.rs dispatches events to elements, and elements
/// return EventResult to indicate whether the event was handled.
pub trait EventHandler {
    /// Handle mouse click event at viewport coordinates
    ///
    /// Returns Handled if the element processed the click, Ignored otherwise.
    fn handle_mouse_click(&mut self, event: MouseClickEvent) -> EventResult {
        let _ = event;
        EventResult::Ignored
    }

    /// Handle keyboard input event
    ///
    /// Returns Handled if the element processed the key, Ignored otherwise.
    fn handle_keyboard(&mut self, event: KeyboardEvent) -> EventResult {
        let _ = event;
        EventResult::Ignored
    }

    /// Handle mouse move event (for hover states, drag operations, etc.)
    ///
    /// Returns Handled if the element processed the move, Ignored otherwise.
    fn handle_mouse_move(&mut self, event: MouseMoveEvent) -> EventResult {
        let _ = event;
        EventResult::Ignored
    }

    /// Check if this element currently has focus
    fn is_focused(&self) -> bool {
        false
    }

    /// Set focus state for this element
    fn set_focused(&mut self, focused: bool) {
        let _ = focused;
    }

    /// Check if the point (x, y) in viewport coordinates is inside this element
    fn contains_point(&self, x: f32, y: f32) -> bool {
        let _ = (x, y);
        false
    }
}
