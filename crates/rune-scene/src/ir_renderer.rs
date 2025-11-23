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

use rune_ir::{data::document::DataDocument, view::ViewDocument};

/// Render IR documents to a canvas (convenience function for FFI).
///
/// This creates a temporary IrRenderer and renders the documents.
/// For repeated rendering, prefer using `IrRenderer` directly.
pub fn render_ir_document(
    canvas: &mut rune_surface::Canvas,
    data_doc: &DataDocument,
    view_doc: &ViewDocument,
    width: f32,
    height: f32,
    provider: &dyn engine_core::TextProvider,
) {
    let mut renderer = IrRenderer::new();
    let _ = renderer.render_canvas(canvas, data_doc, view_doc, width, height, provider);
}

#[cfg(test)]
mod tests;
