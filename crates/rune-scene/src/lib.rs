//! Rune Scene - Clean architecture for UI rendering and interaction
//!
//! Architecture:
//! - lib.rs: Thin shim for window events
//! - scene: Trait for rendering approaches (manual/IR)
//! - event_router: Routes events to elements
//! - elements: Self-contained UI components (render + handlers)

use anyhow::Result;

pub mod elements;
pub mod event_handler;
pub mod event_router;
pub mod ir_adapter;
pub mod ir_renderer;
pub mod layout;
pub mod navigation;
mod persistence;
pub mod scene;
pub mod text;
pub mod zones;

/// Main entry point for IR rendering (sole supported path).
pub fn run() -> Result<()> {
    ir_renderer::run()
}
