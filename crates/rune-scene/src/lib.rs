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
pub mod manual_render;
mod persistence;
pub mod scene;
pub mod text;
pub mod viewport_ir;
pub mod zones;

// Legacy monolithic implementation - being refactored
mod lib_old;

/// Main entry point - routes to either legacy or IR implementation
/// based on configuration (rune.toml) or USE_IR environment variable
///
/// Accepts: USE_IR=1, USE_IR=true, USE_IR=yes (any case)
pub fn run() -> Result<()> {
    // Load config from rune.toml with environment variable overrides
    let config = rune_config::RuneConfig::load();
    let use_ir = config.ir.use_ir;

    if use_ir {
        ir_renderer::run()
    } else {
        lib_old::run()
    }
}
