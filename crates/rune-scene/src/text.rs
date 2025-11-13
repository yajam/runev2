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
pub fn wrap_text(provider: &dyn engine_core::TextProvider, text: &str, size_px: f32, max_width: f32) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() { return vec![String::new()]; }
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for (i, w) in words.iter().enumerate() {
        let cand = if cur.is_empty() { (*w).to_string() } else { format!("{} {}", cur, w) };
        let fits = match measure_run(provider, &cand, size_px) { Some(m) => m.width <= max_width, None => true };
        if fits { cur = cand; } else {
            if !cur.is_empty() { lines.push(cur); }
            // If a single word exceeds width, force-break it
            let mw = measure_run(provider, w, size_px).map(|m| m.width).unwrap_or(0.0);
            if mw > max_width && w.len() > 1 {
                // naive character fallback
                let mut chunk = String::new();
                for ch in w.chars() {
                    let cand2 = format!("{}{}", chunk, ch);
                    if measure_run(provider, &cand2, size_px).map(|m| m.width).unwrap_or(0.0) <= max_width {
                        chunk = cand2;
                    } else {
                        if !chunk.is_empty() { lines.push(chunk); }
                        chunk = ch.to_string();
                    }
                }
                cur = chunk;
            } else {
                cur = (*w).to_string();
            }
        }
        if i == words.len() - 1 { lines.push(cur.clone()); }
    }
    if lines.is_empty() { lines.push(cur); }
    lines
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
    // Wrap
    let lines: Vec<String> = match opts.wrap {
        Wrap::NoWrap => vec![text.to_string()],
        Wrap::Word(max_w) => wrap_text(provider, text, opts.size_px, max_w),
    };
    // Baselines using raster metrics
    let baselines = baselines_for_lines(provider, &lines, opts.size_px, opts.start_baseline_y, opts.line_pad, opts.scale_factor);
    // Estimate line height as average of (ascent+descent)
    let mut sum_h = 0.0;
    let mut count = 0;
    for line in &lines {
        if let Some(m) = measure_run(provider, line, opts.size_px) { sum_h += m.height; count += 1; }
    }
    let est = if count > 0 { sum_h / (count as f32) } else { opts.size_px * 1.2 };
    let total_h = if baselines.len() > 1 { baselines.last().copied().unwrap_or(opts.start_baseline_y) - baselines[0] + est } else { est };
    LayoutResult { lines, baselines, line_height_est: est, total_height: total_h }
}
