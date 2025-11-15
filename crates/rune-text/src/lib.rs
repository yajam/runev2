//! rune-text: Custom text layout and shaping engine.
//!
//! Phase 1: core foundation pieces for rune-text.
//! - 1.2: font management layer (font loading, metrics, glyph outlines/bitmaps)
//! - 1.3: basic text shaping using rustybuzz
//! - 1.4: Unicode grapheme handling (clusters, combining marks, emoji/ZWJ)

pub mod font;
pub mod shaping;
pub mod unicode;

pub use font::{
    face::FontFace,
    loader::{FontCache, FontKey},
    metrics::{FontMetrics, ScaledFontMetrics},
    FontError,
};

/// Simple helper to allow smoke tests to link against this crate.
pub fn is_available() -> bool {
    true
}
