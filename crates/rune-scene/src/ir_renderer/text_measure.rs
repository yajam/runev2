use engine_core::wrap_text_fast;
use rune_ir::view::TextSpec;
use taffy::prelude::{AvailableSpace, Size};

/// Simple line layout estimation used for both measurement and rendering.
pub struct HeuristicLines {
    pub lines: Vec<String>,
    pub line_height: f32,
    pub ascent: f32,
    pub descent: f32,
    pub longest_chars: usize,
}

/// Compute wrapped lines and basic metrics based on text spec and wrap width.
pub fn compute_lines(spec: &TextSpec, text: &str, wrap_width: Option<f32>) -> HeuristicLines {
    let font_size = spec.style.font_size.unwrap_or(16.0) as f32;
    let line_height_factor = spec
        .style
        .line_height
        .map(|lh| (lh as f32) / font_size.max(1.0))
        .unwrap_or(1.2);
    let line_height = font_size * line_height_factor;
    let ascent = font_size * 0.8;
    let descent = font_size * 0.2;

    let mut longest_chars = 0usize;
    let mut out_lines: Vec<String> = Vec::new();

    for (idx, para) in text.split('\n').enumerate() {
        if idx > 0 {
            out_lines.push(String::new()); // explicit newline produces a blank line
        }

        let para = para.trim_end_matches('\r');
        if let Some(w) = wrap_width {
            let wrapped = wrap_text_fast(para, w.max(0.0), font_size, line_height_factor);
            if wrapped.lines.is_empty() {
                out_lines.push(String::new());
            } else {
                if let Some(max_len) = wrapped.lines.iter().map(|l| l.len()).max() {
                    longest_chars = longest_chars.max(max_len);
                }
                out_lines.extend(wrapped.lines);
            }
        } else {
            longest_chars = longest_chars.max(para.len());
            out_lines.push(para.to_string());
        }
    }

    if out_lines.is_empty() {
        out_lines.push(String::new());
    }

    HeuristicLines {
        lines: out_lines,
        line_height,
        ascent,
        descent,
        longest_chars,
    }
}

/// Estimate wrapped text size for IR text nodes (used by Taffy measurement).
pub fn measure_text_node(
    spec: &TextSpec,
    text: &str,
    known: Size<Option<f32>>,
    available: Size<AvailableSpace>,
) -> Size<f32> {
    let font_size = spec.style.font_size.unwrap_or(16.0) as f32;
    let pad_left = spec.style.padding.left as f32;
    let pad_right = spec.style.padding.right as f32;
    let pad_top = spec.style.padding.top as f32;
    let pad_bottom = spec.style.padding.bottom as f32;

    let available_width = known
        .width
        .or_else(|| match available.width {
            AvailableSpace::Definite(w) => Some(w),
            _ => None,
        })
        .map(|w| w.max(0.0));

    let wrap_width = available_width.map(|w| (w - pad_left - pad_right).max(0.0));

    let lines = compute_lines(spec, text, wrap_width);
    let line_count = lines.lines.len().max(1);

    let measured_height = pad_top
        + pad_bottom
        + lines.ascent
        + lines.descent
        + (line_count.saturating_sub(1) as f32) * lines.line_height;

    let measured_width = if let Some(w) = available_width {
        w
    } else {
        let avg_char = font_size * 0.55;
        pad_left + pad_right + (lines.longest_chars as f32) * avg_char
    };

    if std::env::var("RUNE_IR_DEBUG")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        let text_preview = if text.len() > 30 {
            format!("{}...", &text[..30])
        } else {
            text.to_string()
        };
        eprintln!(
            "  measure_text: '{}' -> {}x{} ({} lines, wrap_width={:?})",
            text_preview, measured_width, measured_height, line_count, wrap_width
        );
    }

    Size {
        width: measured_width.max(0.0),
        height: measured_height.max(0.0),
    }
}
