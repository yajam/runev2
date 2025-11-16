use engine_core::{ColorLinPremul, TextRun};

/// Wrapping strategy for text.
#[derive(Clone, Copy, Debug)]
pub enum Wrap {
    /// No wrapping; the entire text is a single line.
    NoWrap,
    /// Word wrap at the given maximum width (in pixels).
    Word(f32),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RunMetrics {
    pub width: f32,
    pub top: f32,
    pub bottom: f32,
    pub ascent: f32,
    pub descent: f32,
    pub height: f32,
}

/// Measure run metrics using rasterized glyph masks rather than provider baselines.
/// This avoids inconsistencies across shapers (e.g., cosmic-text) when using logical_px.
pub fn measure_run(provider: &dyn engine_core::TextProvider, text: &str, size_px: f32) -> Option<RunMetrics> {
    let run = TextRun { text: text.to_string(), pos: [0.0, 0.0], size: size_px.max(1.0), color: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]) };
    let glyphs = provider.rasterize_run(&run);
    if glyphs.is_empty() { return None; }
    let mut top = f32::INFINITY;
    let mut bottom = f32::NEG_INFINITY;
    let mut max_x = 0.0f32;
    for g in glyphs.iter() {
        top = top.min(g.offset[1]);
        bottom = bottom.max(g.offset[1] + g.mask.height as f32);
        max_x = max_x.max(g.offset[0] + g.mask.width as f32);
    }
    let ascent = (-top).max(0.0);
    let descent = bottom.max(0.0);
    let height = (bottom - top).max(size_px * 0.5);
    Some(RunMetrics { width: max_x, top, bottom, ascent, descent, height })
}

/// Simple word wrap: splits text into lines so that measured width <= max_width.
/// Fast approximation version that uses character count instead of expensive glyph rasterization.
pub fn wrap_text(_provider: &dyn engine_core::TextProvider, text: &str, size_px: f32, max_width: f32) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() { return vec![String::new()]; }
    
    // Use fast approximation: average character width is ~0.5-0.6 of font size for proportional fonts
    // This avoids expensive glyph rasterization during wrapping
    let avg_char_width = size_px * 0.55;
    let max_chars_per_line = (max_width / avg_char_width).floor() as usize;
    if max_chars_per_line == 0 { return vec![text.to_string()]; }
    
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    
    for w in words.iter() {
        let test_line = if cur.is_empty() { 
            (*w).to_string() 
        } else { 
            format!("{} {}", cur, w) 
        };
        
        // Simple character count check
        if test_line.len() <= max_chars_per_line {
            cur = test_line;
        } else {
            // Line would be too long
            if !cur.is_empty() { 
                lines.push(cur); 
            }
            
            // If single word is too long, break it
            if w.len() > max_chars_per_line {
                let mut remaining = *w;
                while remaining.len() > max_chars_per_line {
                    let (chunk, rest) = remaining.split_at(max_chars_per_line);
                    lines.push(chunk.to_string());
                    remaining = rest;
                }
                cur = remaining.to_string();
            } else {
                cur = (*w).to_string();
            }
        }
    }
    
    if !cur.is_empty() { lines.push(cur); }
    if lines.is_empty() { lines.push(String::new()); }
    lines
}

/// Word wrapping using rune-text's HarfBuzz-based layout engine.
///
/// This delegates wrapping to `rune-text`'s `TextLayout`, which handles
/// grapheme clusters and UAX-14 line breaking. It requires a font path
/// via the `RUNE_TEXT_FONT` environment variable; if unavailable or
/// loading fails, it falls back to the simple `wrap_text` heuristic.
pub fn wrap_text_rune(
    text: &str,
    size_px: f32,
    max_width: f32,
) -> Vec<String> {
    use rune_text::font::{FontCache, FontError};
    use rune_text::layout::{TextLayout, WrapMode};

    // Simple fallback using the same approximation as `wrap_text`,
    // but without needing a text provider.
    fn fallback_wrap(text: &str, size_px: f32, max_width: f32) -> Vec<String> {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.is_empty() {
            return vec![String::new()];
        }

        let avg_char_width = size_px * 0.55;
        let max_chars_per_line = (max_width / avg_char_width).floor() as usize;
        if max_chars_per_line == 0 {
            return vec![text.to_string()];
        }

        let mut lines: Vec<String> = Vec::new();
        let mut cur = String::new();

        for w in words.iter() {
            let test_line = if cur.is_empty() {
                (*w).to_string()
            } else {
                format!("{} {}", cur, w)
            };

            if test_line.len() <= max_chars_per_line {
                cur = test_line;
            } else {
                if !cur.is_empty() {
                    lines.push(cur);
                }

                if w.len() > max_chars_per_line {
                    let mut remaining = *w;
                    while remaining.len() > max_chars_per_line {
                        let (chunk, rest) = remaining.split_at(max_chars_per_line);
                        lines.push(chunk.to_string());
                        remaining = rest;
                    }
                    cur = remaining.to_string();
                } else {
                    cur = (*w).to_string();
                }
            }
        }

        if !cur.is_empty() {
            lines.push(cur);
        }
        if lines.is_empty() {
            lines.push(String::new());
        }
        lines
    }

    // Require a font path for rune-text; fall back if not provided.
    let font_path = match std::env::var("RUNE_TEXT_FONT") {
        Ok(p) if !p.is_empty() => p,
        _ => return fallback_wrap(text, size_px, max_width),
    };

    // Load font via rune-text. On failure, fall back to heuristic wrap.
    let mut cache = FontCache::new();
    let font = match cache.get_or_load(&font_path, 0) {
        Ok(f) => f,
        Err(FontError::Io(_)) | Err(FontError::InvalidFont) => {
            return fallback_wrap(text, size_px, max_width);
        }
    };

    let layout = TextLayout::with_wrap(
        text.to_string(),
        &font,
        size_px,
        Some(max_width),
        WrapMode::BreakWord,
    );

    let mut out = Vec::new();
    for line in layout.lines() {
        let range = line.text_range.clone();
        out.push(text[range].to_string());
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// Compute baseline Y positions for each line so that the visual spacing uses
/// descent of the previous line + ascent of the next line + pad.
/// Optional scale_factor snaps to device pixels (like PassManager).
pub fn baselines_for_lines(
    provider: &dyn engine_core::TextProvider,
    lines: &[String],
    size_px: f32,
    start_baseline_y: f32,
    pad: f32,
    scale_factor: Option<f32>,
) -> Vec<f32> {
    let snap = |v: f32| -> f32 {
        if let Some(sf) = scale_factor { if sf.is_finite() && sf > 0.0 { return (v * sf).round() / sf; } }
        v
    };
    let mut out = Vec::with_capacity(lines.len());
    if lines.is_empty() { return out; }
    // First line baseline
    out.push(snap(start_baseline_y));
    // Precompute metrics
    let metrics: Vec<RunMetrics> = lines
        .iter()
        .map(|s| measure_run(provider, s, size_px).unwrap_or(RunMetrics { width: 0.0, top: -size_px * 0.8, bottom: size_px * 0.2, ascent: size_px * 0.8, descent: size_px * 0.2, height: size_px }))
        .collect();
    for i in 1..lines.len() {
        let prev = metrics[i - 1];
        let next = metrics[i];
        let baseline_prev = out[i - 1];
        let baseline = baseline_prev + prev.descent + next.ascent + pad;
        out.push(snap(baseline));
    }
    out
}

/// High-level helper: one source of truth for layout based on provider rasterization.
pub struct LayoutOptions {
    pub size_px: f32,
    pub wrap: Wrap,
    pub start_baseline_y: f32,
    pub line_pad: f32,
    pub scale_factor: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct LayoutResult {
    pub lines: Vec<String>,
    pub baselines: Vec<f32>,
    pub line_height_est: f32,
    pub total_height: f32,
}

pub fn layout_text(
    provider: &dyn engine_core::TextProvider,
    text: &str,
    opts: &LayoutOptions,
) -> LayoutResult {
    // Disable automatic width-based wrapping for now; only respect
    // explicit newlines. This avoids heavy rune-text layout paths and
    // keeps rendering stable even when RUNE_TEXT_LAYOUT is set.
    let lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
    
    // Use simplified baseline calculation for performance
    // Instead of measuring every line, use a single sample measurement
    let sample_metrics = if !lines.is_empty() {
        // Measure first line as representative sample
        measure_run(provider, &lines[0], opts.size_px)
    } else {
        None
    };
    
    let (line_height, ascent, descent) = if let Some(m) = sample_metrics {
        (m.height, m.ascent, m.descent)
    } else {
        // Fallback estimates
        let h = opts.size_px * 1.2;
        (h, opts.size_px * 0.8, opts.size_px * 0.2)
    };
    
    // Compute baselines using uniform line height
    let snap = |v: f32| -> f32 {
        if let Some(sf) = opts.scale_factor { 
            if sf.is_finite() && sf > 0.0 { 
                return (v * sf).round() / sf; 
            } 
        }
        v
    };
    
    let mut baselines = Vec::with_capacity(lines.len());
    if !lines.is_empty() {
        baselines.push(snap(opts.start_baseline_y));
        for i in 1..lines.len() {
            let baseline = baselines[i - 1] + descent + ascent + opts.line_pad;
            baselines.push(snap(baseline));
        }
    }
    
    let total_h = if baselines.len() > 1 { 
        baselines.last().copied().unwrap_or(opts.start_baseline_y) - baselines[0] + line_height 
    } else { 
        line_height 
    };
    
    LayoutResult { lines, baselines, line_height_est: line_height, total_height: total_h }
}
