//! Bidirectional (BiDi) text support built on `unicode-bidi`.
//!
//! Phase 3.1-3.2:
//! - Paragraph-level direction detection
//! - Embedding levels and visual reordering
//! - Mixed-direction runs and bracket mirroring helpers

pub mod levels;
pub mod reorder;
pub mod mirror;

pub use levels::{BaseDirection, ParagraphBidi, ParagraphDirection};
pub use reorder::{BidiRun, reorder_line, visual_runs};
pub use mirror::mirrored_bracket;

