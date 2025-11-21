//! IR renderer entry module.
//!
//! This module is intentionally kept small and delegates to
//! submodules so the IR renderer does not become a monolith.
//! The implementation is split into:
//! - `runner`: window / event-loop / zone orchestration
//! - `core`: `IrRenderer` + Taffy integration
//! - `elements`: element-level Canvas rendering helpers
//! - `style`: shared style / color helpers

mod core;
mod elements;
mod hit_region;
mod painter_backend;
mod runner;
mod state;
mod style;
mod text_measure;

pub use core::IrRenderer;
pub use hit_region::HitRegionRegistry;
pub use runner::run;
pub use state::{IrElementState, IrElementType};

#[cfg(test)]
mod tests;
