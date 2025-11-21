//! Event Router - Paper-thin dispatcher to EventHandler implementations
//!
//! This module provides MINIMAL routing logic - finding which element to dispatch to
//! and calling its EventHandler methods. ALL business logic lives in the elements.
//!
//! Design principle: Find element → Dispatch event → Done. No heavy lifting.

use crate::elements;
use crate::event_handler::{EventHandler, KeyboardEvent, MouseClickEvent, MouseMoveEvent};
use crate::scene::{KeyEvent, MouseEvent, SceneResult};

/// Trait for collections of elements that can receive events
pub trait ElementCollection {
    /// Get mutable access to buttons
    fn buttons_mut(&mut self) -> &mut [elements::Button];

    /// Get mutable access to checkboxes
    fn checkboxes_mut(&mut self) -> &mut [elements::Checkbox];

    /// Get mutable access to radio buttons
    fn radios_mut(&mut self) -> &mut [elements::Radio];

    /// Get mutable access to input boxes
    fn inputs_mut(&mut self) -> &mut [elements::InputBox];

    /// Get mutable access to text areas
    fn textareas_mut(&mut self) -> &mut [elements::TextArea];

    /// Get mutable access to selects
    fn selects_mut(&mut self) -> &mut [elements::Select];

    /// Get mutable access to date pickers
    fn date_pickers_mut(&mut self) -> &mut [elements::DatePicker];

    /// Get mutable access to links
    fn links_mut(&mut self) -> &mut [elements::Link];

    /// Get mutable access to modals (optional - provide empty slice if not used)
    fn modals_mut(&mut self) -> &mut [elements::Modal] {
        &mut []
    }

    /// Clear focus on all elements except the specified type and index
    fn clear_focus_except(&mut self, element_type: ElementType, index: usize);
}

/// Element type identifier for focus management
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ElementType {
    Button,
    Checkbox,
    Radio,
    Input,
    TextArea,
    Select,
    DatePicker,
    Link,
    Modal,
}

/// Routes events to the appropriate elements - PAPER THIN!
pub struct EventRouter;

impl EventRouter {
    /// Route a mouse click event to elements
    ///
    /// Two-pass approach to avoid borrowing issues:
    /// 1. Find which element was clicked
    /// 2. Call its EventHandler and update focus
    pub fn route_mouse_click(
        collection: &mut impl ElementCollection,
        event: MouseEvent,
    ) -> SceneResult {
        let x = event.x;
        let y = event.y;

        // Convert to EventHandler event type
        let click_event = MouseClickEvent {
            button: match event.button {
                crate::scene::MouseButton::Left => winit::event::MouseButton::Left,
                crate::scene::MouseButton::Right => winit::event::MouseButton::Right,
                crate::scene::MouseButton::Middle => winit::event::MouseButton::Middle,
            },
            state: winit::event::ElementState::Pressed,
            x,
            y,
            click_count: event.click_count,
        };

        // First pass: Find which element (priority order: overlays first)
        enum ClickedElement {
            Modal(usize),
            DatePicker(usize),
            Select(usize),
            TextArea(usize),
            Input(usize),
            Checkbox(usize),
            Radio(usize),
            Link(usize),
            Button(usize),
            None,
        }

        let clicked = {
            // Modals (highest priority - overlays)
            if let Some(idx) = collection
                .modals_mut()
                .iter()
                .position(|m| m.contains_point(x, y))
            {
                ClickedElement::Modal(idx)
            }
            // Date pickers (popups)
            else if let Some(idx) = collection
                .date_pickers_mut()
                .iter()
                .position(|dp| dp.contains_point(x, y))
            {
                ClickedElement::DatePicker(idx)
            }
            // Selects (dropdowns)
            else if let Some(idx) = collection
                .selects_mut()
                .iter()
                .position(|s| s.contains_point(x, y))
            {
                ClickedElement::Select(idx)
            }
            // Text areas
            else if let Some(idx) = collection
                .textareas_mut()
                .iter()
                .position(|ta| ta.contains_point(x, y))
            {
                ClickedElement::TextArea(idx)
            }
            // Input boxes
            else if let Some(idx) = collection
                .inputs_mut()
                .iter()
                .position(|inp| inp.contains_point(x, y))
            {
                ClickedElement::Input(idx)
            }
            // Checkboxes
            else if let Some(idx) = collection
                .checkboxes_mut()
                .iter()
                .position(|cb| cb.contains_point(x, y))
            {
                ClickedElement::Checkbox(idx)
            }
            // Radios
            else if let Some(idx) = collection
                .radios_mut()
                .iter()
                .position(|r| r.contains_point(x, y))
            {
                ClickedElement::Radio(idx)
            }
            // Links
            else if let Some(idx) = collection
                .links_mut()
                .iter()
                .position(|l| l.contains_point(x, y))
            {
                ClickedElement::Link(idx)
            }
            // Buttons (lowest priority)
            else if let Some(idx) = collection
                .buttons_mut()
                .iter()
                .position(|b| b.hit_test(x, y))
            {
                ClickedElement::Button(idx)
            } else {
                ClickedElement::None
            }
        };

        // Second pass: Call EventHandler and manage focus
        match clicked {
            ClickedElement::Modal(idx) => {
                let result = collection.modals_mut()[idx].handle_mouse_click(click_event);
                if result.is_handled() {
                    collection.clear_focus_except(ElementType::Modal, idx);
                    collection.modals_mut()[idx].set_focused(true);
                    SceneResult::Handled
                } else {
                    SceneResult::Ignored
                }
            }
            ClickedElement::DatePicker(idx) => {
                let result = collection.date_pickers_mut()[idx].handle_mouse_click(click_event);
                if result.is_handled() {
                    collection.clear_focus_except(ElementType::DatePicker, idx);
                    collection.date_pickers_mut()[idx].set_focused(true);
                    SceneResult::Handled
                } else {
                    SceneResult::Ignored
                }
            }
            ClickedElement::Select(idx) => {
                let result = collection.selects_mut()[idx].handle_mouse_click(click_event);
                if result.is_handled() {
                    collection.clear_focus_except(ElementType::Select, idx);
                    collection.selects_mut()[idx].set_focused(true);
                    SceneResult::Handled
                } else {
                    SceneResult::Ignored
                }
            }
            ClickedElement::TextArea(idx) => {
                let result = collection.textareas_mut()[idx].handle_mouse_click(click_event);
                if result.is_handled() {
                    collection.clear_focus_except(ElementType::TextArea, idx);
                    collection.textareas_mut()[idx].set_focused(true);
                    SceneResult::Handled
                } else {
                    SceneResult::Ignored
                }
            }
            ClickedElement::Input(idx) => {
                let result = collection.inputs_mut()[idx].handle_mouse_click(click_event);
                if result.is_handled() {
                    collection.clear_focus_except(ElementType::Input, idx);
                    collection.inputs_mut()[idx].set_focused(true);
                    SceneResult::Handled
                } else {
                    SceneResult::Ignored
                }
            }
            ClickedElement::Checkbox(idx) => {
                let result = collection.checkboxes_mut()[idx].handle_mouse_click(click_event);
                if result.is_handled() {
                    collection.clear_focus_except(ElementType::Checkbox, idx);
                    collection.checkboxes_mut()[idx].set_focused(true);
                    SceneResult::Handled
                } else {
                    SceneResult::Ignored
                }
            }
            ClickedElement::Radio(idx) => {
                let result = collection.radios_mut()[idx].handle_mouse_click(click_event);
                if result.is_handled() {
                    // Radio group coordination - deselect all others
                    for (other_idx, radio) in collection.radios_mut().iter_mut().enumerate() {
                        if other_idx != idx {
                            radio.selected = false;
                        }
                    }
                    collection.clear_focus_except(ElementType::Radio, idx);
                    collection.radios_mut()[idx].set_focused(true);
                    SceneResult::Handled
                } else {
                    SceneResult::Ignored
                }
            }
            ClickedElement::Link(idx) => {
                let result = collection.links_mut()[idx].handle_mouse_click(click_event);
                if result.is_handled() {
                    collection.clear_focus_except(ElementType::Link, idx);
                    collection.links_mut()[idx].set_focused(true);
                    SceneResult::Handled
                } else {
                    SceneResult::Ignored
                }
            }
            ClickedElement::Button(idx) => {
                let result = collection.buttons_mut()[idx].handle_mouse_click(click_event);
                if result.is_handled() {
                    collection.clear_focus_except(ElementType::Button, idx);
                    collection.buttons_mut()[idx].set_focused(true);
                    SceneResult::Handled
                } else {
                    SceneResult::Ignored
                }
            }
            ClickedElement::None => SceneResult::Ignored,
        }
    }

    /// Route a mouse move event to elements (for drag operations)
    pub fn route_mouse_move(
        collection: &mut impl ElementCollection,
        x: f32,
        y: f32,
    ) -> SceneResult {
        let move_event = MouseMoveEvent { x, y };

        // Dispatch to focused text editing elements
        for textarea in collection.textareas_mut() {
            if textarea.is_focused() {
                if textarea.handle_mouse_move(move_event).is_handled() {
                    return SceneResult::Handled;
                }
            }
        }

        for input in collection.inputs_mut() {
            if input.is_focused() {
                if input.handle_mouse_move(move_event).is_handled() {
                    return SceneResult::Handled;
                }
            }
        }

        SceneResult::Ignored
    }

    /// Route a keyboard event to the focused element
    pub fn route_keyboard(collection: &mut impl ElementCollection, event: KeyEvent) -> SceneResult {
        // Convert to EventHandler event type
        let keyboard_event = KeyboardEvent {
            key: Self::convert_keycode(event.key),
            state: winit::event::ElementState::Pressed,
            modifiers: winit::keyboard::ModifiersState::from_bits_truncate(
                (if event.modifiers.shift { 1 } else { 0 })
                    | (if event.modifiers.ctrl { 1 << 1 } else { 0 })
                    | (if event.modifiers.alt { 1 << 2 } else { 0 })
                    | (if event.modifiers.cmd { 1 << 3 } else { 0 }),
            ),
        };

        // Dispatch to focused element
        for modal in collection.modals_mut() {
            if modal.is_focused() {
                if modal.handle_keyboard(keyboard_event).is_handled() {
                    return SceneResult::Handled;
                }
            }
        }

        // Date pickers - now using EventHandler::handle_keyboard!
        for picker in collection.date_pickers_mut() {
            if picker.is_focused() || picker.open {
                // Use EventHandler trait method explicitly
                if EventHandler::handle_keyboard(picker, keyboard_event).is_handled() {
                    return SceneResult::Handled;
                }
            }
        }

        // Selects - now using EventHandler::handle_keyboard!
        for select in collection.selects_mut() {
            if select.is_focused() || select.open {
                // Use EventHandler trait method explicitly
                if EventHandler::handle_keyboard(select, keyboard_event).is_handled() {
                    return SceneResult::Handled;
                }
            }
        }

        for textarea in collection.textareas_mut() {
            if textarea.is_focused() {
                if textarea.handle_keyboard(keyboard_event).is_handled() {
                    return SceneResult::Handled;
                }
            }
        }

        for input in collection.inputs_mut() {
            if input.is_focused() {
                if input.handle_keyboard(keyboard_event).is_handled() {
                    return SceneResult::Handled;
                }
            }
        }

        for checkbox in collection.checkboxes_mut() {
            if checkbox.is_focused() {
                if checkbox.handle_keyboard(keyboard_event).is_handled() {
                    return SceneResult::Handled;
                }
            }
        }

        for radio in collection.radios_mut() {
            if radio.is_focused() {
                if radio.handle_keyboard(keyboard_event).is_handled() {
                    return SceneResult::Handled;
                }
            }
        }

        for link in collection.links_mut() {
            if link.is_focused() {
                if link.handle_keyboard(keyboard_event).is_handled() {
                    return SceneResult::Handled;
                }
            }
        }

        // Buttons (Space/Enter to activate)
        for button in collection.buttons_mut() {
            if button.is_focused() {
                if button.handle_keyboard(keyboard_event).is_handled() {
                    return SceneResult::Handled;
                }
            }
        }

        SceneResult::Ignored
    }

    /// Convert scene::KeyCode to winit::keyboard::KeyCode
    fn convert_keycode(key: crate::scene::KeyCode) -> winit::keyboard::KeyCode {
        use winit::keyboard::KeyCode;
        match key {
            crate::scene::KeyCode::ArrowLeft => KeyCode::ArrowLeft,
            crate::scene::KeyCode::ArrowRight => KeyCode::ArrowRight,
            crate::scene::KeyCode::ArrowUp => KeyCode::ArrowUp,
            crate::scene::KeyCode::ArrowDown => KeyCode::ArrowDown,
            crate::scene::KeyCode::Enter => KeyCode::Enter,
            crate::scene::KeyCode::Escape => KeyCode::Escape,
            crate::scene::KeyCode::Backspace => KeyCode::Backspace,
            crate::scene::KeyCode::Delete => KeyCode::Delete,
            crate::scene::KeyCode::Tab => KeyCode::Tab,
            crate::scene::KeyCode::Char(c) => {
                // Map common characters to KeyCode
                match c {
                    'a' | 'A' => KeyCode::KeyA,
                    'c' | 'C' => KeyCode::KeyC,
                    'v' | 'V' => KeyCode::KeyV,
                    'x' | 'X' => KeyCode::KeyX,
                    'y' | 'Y' => KeyCode::KeyY,
                    'z' | 'Z' => KeyCode::KeyZ,
                    ' ' => KeyCode::Space,
                    _ => KeyCode::Space, // Fallback
                }
            }
            crate::scene::KeyCode::Other => KeyCode::Space, // Fallback
        }
    }
}
