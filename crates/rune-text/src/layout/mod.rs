pub mod line_breaker;
pub mod line_box;
pub mod text_layout;
pub mod prefix_sums;

pub use line_breaker::{LineBreak, LineBreakKind, WordBoundary, WordBoundaryKind};
pub use line_box::LineBox;
pub use text_layout::TextLayout;
pub use prefix_sums::PrefixSums;

/// Line wrapping strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapMode {
    /// Do not perform automatic wrapping (only explicit newlines).
    NoWrap,
    /// Wrap at word boundaries where possible, falling back to
    /// grapheme boundaries for long words.
    BreakWord,
    /// Allow breaking between all grapheme clusters (aggressive).
    BreakAll,
}
