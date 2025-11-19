//! Text rendering providers for rune-draw.
//!
//! The primary provider is [`RuneTextProvider`] which uses:
//! - `harfrust` for text shaping (HarfBuzz implementation)
//! - `swash` for glyph rasterization
//! - `fontdb` for font discovery and fallback
//!
//! This provides high-quality text rendering with:
//! - Proper kerning and ligatures
//! - Subpixel RGB rendering
//! - BiDi support
//! - Complex script support
//!
//! # Example
//! ```no_run
//! use engine_core::{RuneTextProvider, SubpixelOrientation, TextRun, ColorLinPremul};
//!
//! let provider = RuneTextProvider::from_system_fonts(SubpixelOrientation::RGB)
//!     .expect("Failed to load fonts");
//!
//! let run = TextRun {
//!     text: "Hello, world!".to_string(),
//!     pos: [0.0, 0.0],
//!     size: 16.0,
//!     color: ColorLinPremul::rgba(255, 255, 255, 255),
//! };
//!
//! let glyphs = provider.rasterize_run(&run);
//! ```

use std::hash::Hash;

/// LCD subpixel orientation along X axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubpixelOrientation {
    RGB,
    BGR,
}

/// Storage format for a subpixel coverage mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskFormat {
    Rgba8,
    Rgba16,
}

/// Subpixel mask in RGB coverage format stored in RGBA (A is unused).
/// Supports 8-bit or 16-bit per-channel storage.
#[derive(Clone, Debug)]
pub struct SubpixelMask {
    pub width: u32,
    pub height: u32,
    pub format: MaskFormat,
    /// Pixel data, row-major. For Rgba8, 4 bytes/pixel. For Rgba16, 8 bytes/pixel (little-endian u16s).
    pub data: Vec<u8>,
}

impl SubpixelMask {
    pub fn bytes_per_pixel(&self) -> usize {
        match self.format {
            MaskFormat::Rgba8 => 4,
            MaskFormat::Rgba16 => 8,
        }
    }
}

/// GPU-ready batch of glyph masks with positions and color.
/// This is the canonical representation used when sending text to the GPU.
#[derive(Clone, Debug)]
pub struct GlyphBatch {
    pub glyphs: Vec<(SubpixelMask, [f32; 2], crate::scene::ColorLinPremul)>,
}

impl GlyphBatch {
    pub fn new() -> Self {
        Self { glyphs: Vec::new() }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            glyphs: Vec::with_capacity(cap),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.glyphs.len()
    }
}

/// Simple global cache for glyph runs keyed by (text, size, provider pointer).
/// Used by direct text rendering paths (e.g., rune-surface Canvas) to avoid
/// re-shaping and re-rasterizing identical text on every frame.
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct GlyphRunKey {
    text_hash: u64,
    size_bits: u32,
    provider_id: usize,
}

struct GlyphRunCache {
    map: std::sync::Mutex<
        std::collections::HashMap<GlyphRunKey, std::sync::Arc<Vec<RasterizedGlyph>>>,
    >,
    max_entries: usize,
}

impl GlyphRunCache {
    fn new(max_entries: usize) -> Self {
        Self {
            map: std::sync::Mutex::new(std::collections::HashMap::new()),
            max_entries: max_entries.max(1),
        }
    }

    fn get(&self, key: &GlyphRunKey) -> Option<std::sync::Arc<Vec<RasterizedGlyph>>> {
        let map = self.map.lock().unwrap();
        map.get(key).cloned()
    }

    fn insert(
        &self,
        key: GlyphRunKey,
        glyphs: Vec<RasterizedGlyph>,
    ) -> std::sync::Arc<Vec<RasterizedGlyph>> {
        let mut map = self.map.lock().unwrap();

        // Simple eviction strategy to keep memory bounded:
        // when we grow past 2x capacity with new keys, clear everything.
        if map.len() >= self.max_entries * 2 && !map.contains_key(&key) {
            map.clear();
        }

        if let Some(existing) = map.get(&key) {
            return existing.clone();
        }

        let arc = std::sync::Arc::new(glyphs);
        map.insert(key, arc.clone());
        arc
    }
}

static GLYPH_RUN_CACHE: std::sync::OnceLock<GlyphRunCache> = std::sync::OnceLock::new();

fn global_glyph_run_cache() -> &'static GlyphRunCache {
    GLYPH_RUN_CACHE.get_or_init(|| GlyphRunCache::new(2048))
}

/// Convert an 8-bit grayscale coverage mask to an RGB subpixel mask.
/// Uses a gentle subpixel shift for improved clarity on small text.
pub fn grayscale_to_subpixel_rgb(
    width: u32,
    height: u32,
    gray: &[u8],
    orientation: SubpixelOrientation,
) -> SubpixelMask {
    let w = width as usize;
    let h = height as usize;
    assert_eq!(gray.len(), w * h);
    let mut out = vec![0u8; w * h * 4];

    // Gentle subpixel rendering: slight horizontal shift per channel
    // Much lighter than the original 3-tap kernel to avoid blurring
    for y in 0..h {
        for x in 0..w {
            let c0 = gray[y * w + x] as f32 / 255.0;
            let cl = if x > 0 {
                gray[y * w + (x - 1)] as f32 / 255.0
            } else {
                c0
            };
            let cr = if x + 1 < w {
                gray[y * w + (x + 1)] as f32 / 255.0
            } else {
                c0
            };

            // Very light blending (10% neighbor influence instead of 33%)
            let sample_left = 0.9 * c0 + 0.1 * cl;
            let sample_center = c0;
            let sample_right = 0.9 * c0 + 0.1 * cr;

            let (r_cov, g_cov, b_cov) = match orientation {
                SubpixelOrientation::RGB => (sample_left, sample_center, sample_right),
                SubpixelOrientation::BGR => (sample_right, sample_center, sample_left),
            };

            let i = (y * w + x) * 4;
            out[i + 0] = (r_cov * 255.0 + 0.5) as u8;
            out[i + 1] = (g_cov * 255.0 + 0.5) as u8;
            out[i + 2] = (b_cov * 255.0 + 0.5) as u8;
            out[i + 3] = 0u8; // alpha unused; output premul alpha computed in shader
        }
    }
    SubpixelMask {
        width,
        height,
        format: MaskFormat::Rgba8,
        data: out,
    }
}

/// Convert an 8-bit grayscale coverage mask to an RGB mask with equal channels (grayscale AA).
pub fn grayscale_to_rgb_equal(width: u32, height: u32, gray: &[u8]) -> SubpixelMask {
    let w = width as usize;
    let h = height as usize;
    assert_eq!(gray.len(), w * h);
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let g = gray[y * w + x];
            let i = (y * w + x) * 4;
            out[i + 0] = g;
            out[i + 1] = g;
            out[i + 2] = g;
            out[i + 3] = 0u8;
        }
    }
    SubpixelMask {
        width,
        height,
        format: MaskFormat::Rgba8,
        data: out,
    }
}

/// 16-bit variants for higher precision masks. Channels are u16 in [0..65535],
/// packed little-endian into the data buffer. Alpha is unused.
pub fn grayscale_to_subpixel_rgb16(
    width: u32,
    height: u32,
    gray: &[u8],
    orientation: SubpixelOrientation,
) -> SubpixelMask {
    let w = width as usize;
    let h = height as usize;
    assert_eq!(gray.len(), w * h);
    let mut out = vec![0u8; w * h * 8];
    for y in 0..h {
        for x in 0..w {
            let c0 = gray[y * w + x] as f32 / 255.0;
            let cl = if x > 0 {
                gray[y * w + (x - 1)] as f32 / 255.0
            } else {
                c0
            };
            let cr = if x + 1 < w {
                gray[y * w + (x + 1)] as f32 / 255.0
            } else {
                c0
            };
            let sample_left = (2.0 / 3.0) * c0 + (1.0 / 3.0) * cl;
            let sample_center = c0;
            let sample_right = (2.0 / 3.0) * c0 + (1.0 / 3.0) * cr;
            let (r_cov, g_cov, b_cov) = match orientation {
                SubpixelOrientation::RGB => (sample_left, sample_center, sample_right),
                SubpixelOrientation::BGR => (sample_right, sample_center, sample_left),
            };
            let (r, g, b) = match orientation {
                SubpixelOrientation::RGB => (r_cov, g_cov, b_cov),
                SubpixelOrientation::BGR => (b_cov, g_cov, r_cov),
            };
            let i = (y * w + x) * 8;
            let write_u16 = |buf: &mut [u8], idx: usize, v: u16| {
                let b = v.to_le_bytes();
                buf[idx] = b[0];
                buf[idx + 1] = b[1];
            };
            write_u16(&mut out, i + 0, (r * 65535.0 + 0.5) as u16);
            write_u16(&mut out, i + 2, (g * 65535.0 + 0.5) as u16);
            write_u16(&mut out, i + 4, (b * 65535.0 + 0.5) as u16);
            write_u16(&mut out, i + 6, 0u16);
        }
    }
    SubpixelMask {
        width,
        height,
        format: MaskFormat::Rgba16,
        data: out,
    }
}

pub fn grayscale_to_rgb_equal16(width: u32, height: u32, gray: &[u8]) -> SubpixelMask {
    let w = width as usize;
    let h = height as usize;
    assert_eq!(gray.len(), w * h);
    let mut out = vec![0u8; w * h * 8];
    for y in 0..h {
        for x in 0..w {
            let g = (gray[y * w + x] as u16) * 257; // 255->65535 scale
            let i = (y * w + x) * 8;
            let b = g.to_le_bytes();
            out[i + 0] = b[0];
            out[i + 1] = b[1];
            out[i + 2] = b[0];
            out[i + 3] = b[1];
            out[i + 4] = b[0];
            out[i + 5] = b[1];
            out[i + 6] = 0;
            out[i + 7] = 0;
        }
    }
    SubpixelMask {
        width,
        height,
        format: MaskFormat::Rgba16,
        data: out,
    }
}

// Optional provider that consumes a patched fontdue fork emitting RGB masks directly.
// Behind a feature flag so it doesn't affect default builds.
#[cfg(feature = "fontdue-rgb-patch")]
pub struct PatchedFontdueProvider {
    font: fontdue_rgb::Font,
}

#[cfg(feature = "fontdue-rgb-patch")]
impl PatchedFontdueProvider {
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let font = fontdue_rgb::Font::from_bytes(bytes, fontdue_rgb::FontSettings::default())?;
        Ok(Self { font })
    }
}

#[cfg(feature = "fontdue-rgb-patch")]
impl TextProvider for PatchedFontdueProvider {
    fn rasterize_run(&self, run: &crate::scene::TextRun) -> Vec<RasterizedGlyph> {
        use fontdue_rgb::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: 0.0,
            y: 0.0,
            ..LayoutSettings::default()
        });
        layout.append(
            &[&self.font],
            &TextStyle::new(&run.text, run.size.max(1.0), 0),
        );
        let mut out = Vec::new();
        for g in layout.glyphs() {
            // Patched fontdue returns RGB masks directly (u8 or u16). Prefer 16-bit when available.
            let mask = if let Some((w, h, data16)) = self
                .font
                .rasterize_rgb16_indexed(g.key.glyph_index, g.key.px)
            {
                SubpixelMask {
                    width: w as u32,
                    height: h as u32,
                    format: MaskFormat::Rgba16,
                    data: data16,
                }
            } else {
                let (w, h, data8) = self
                    .font
                    .rasterize_rgb8_indexed(g.key.glyph_index, g.key.px);
                SubpixelMask {
                    width: w as u32,
                    height: h as u32,
                    format: MaskFormat::Rgba8,
                    data: data8,
                }
            };
            out.push(RasterizedGlyph {
                offset: [g.x, g.y],
                mask,
            });
        }
        out
    }
}

/// A glyph with its top-left offset relative to the run origin and an RGB subpixel mask.
#[derive(Clone, Debug)]
pub struct RasterizedGlyph {
    pub offset: [f32; 2],
    pub mask: SubpixelMask,
}

/// Minimal shaped glyph information for paragraph-level wrapping.
#[derive(Clone, Debug)]
pub struct ShapedGlyph {
    /// Glyph's starting UTF-8 byte index in the source text (Harfbuzz cluster).
    pub cluster: u32,
    /// Advance width in pixels.
    pub x_advance: f32,
}

/// Shaped paragraph representation for efficient wrapping.
#[derive(Clone, Debug)]
pub struct ShapedParagraph {
    pub glyphs: Vec<ShapedGlyph>,
}

/// Text provider interface. Implementations convert a `TextRun` into positioned glyph masks.
pub trait TextProvider: Send + Sync {
    fn rasterize_run(&self, run: &crate::scene::TextRun) -> Vec<RasterizedGlyph>;

    /// Optional paragraph shaping hook for advanced wrappers.
    ///
    /// Implementors that can expose Harfbuzz/cosmic-text shaping results should
    /// return glyphs with cluster indices and advances. The default implementation
    /// returns `None`, in which case callers must fall back to approximate methods.
    fn shape_paragraph(&self, _text: &str, _px: f32) -> Option<ShapedParagraph> {
        None
    }

    /// Optional cache tag to distinguish providers in text caches.
    /// The default implementation returns 0, which is sufficient when
    /// a single provider is used with a given PassManager.
    fn cache_tag(&self) -> u64 {
        0
    }

    fn line_metrics(&self, px: f32) -> Option<LineMetrics> {
        let _ = px;
        None
    }
}

/// Rasterize a text run using a global glyph-run cache.
///
/// This is intended for direct text rendering paths that repeatedly render the
/// same text (e.g., during scrolling) and want to avoid re-shaping and
/// re-rasterizing glyphs every frame. The cache key is based on:
/// - text contents
/// - run size in pixels
/// - the concrete text provider instance
pub fn rasterize_run_cached(
    provider: &dyn TextProvider,
    run: &crate::scene::TextRun,
) -> std::sync::Arc<Vec<RasterizedGlyph>> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;

    let mut hasher = DefaultHasher::new();
    run.text.hash(&mut hasher);
    let text_hash = hasher.finish();
    let size_bits = run.size.to_bits();
    // Use the concrete provider data pointer as a stable identifier for this run.
    let provider_id = (provider as *const dyn TextProvider as *const ()) as usize;
    let key = GlyphRunKey {
        text_hash,
        size_bits,
        provider_id,
    };

    let cache = global_glyph_run_cache();
    if let Some(hit) = cache.get(&key) {
        return hit;
    }

    let glyphs = provider.rasterize_run(run);
    cache.insert(key, glyphs)
}

/// LEGACY: Simple fontdue-based provider.
///
/// **NOT RECOMMENDED**: Use [`RuneTextProvider`] (harfrust + swash) instead.
/// This provider is kept for compatibility and testing purposes only.
///
/// Limitations:
/// - Basic ASCII-first layout
/// - No advanced shaping features
/// - Lower quality than swash rasterization
pub struct SimpleFontdueProvider {
    font: fontdue::Font,
    orientation: SubpixelOrientation,
}

impl SimpleFontdueProvider {
    pub fn from_bytes(bytes: &[u8], orientation: SubpixelOrientation) -> anyhow::Result<Self> {
        let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(Self { font, orientation })
    }
}

impl TextProvider for SimpleFontdueProvider {
    fn rasterize_run(&self, run: &crate::scene::TextRun) -> Vec<RasterizedGlyph> {
        use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: 0.0,
            y: 0.0,
            ..LayoutSettings::default()
        });
        layout.append(
            &[&self.font],
            &TextStyle::new(&run.text, run.size.max(1.0), 0),
        );

        let mut out = Vec::new();
        for g in layout.glyphs() {
            // Rasterize individual glyph to grayscale
            let (metrics, bitmap) = self.font.rasterize_indexed(g.key.glyph_index, g.key.px);
            if metrics.width == 0 || metrics.height == 0 {
                continue;
            }
            // Convert to subpixel mask
            let mask = grayscale_to_subpixel_rgb(
                metrics.width as u32,
                metrics.height as u32,
                &bitmap,
                self.orientation,
            );
            // Layout already provides the glyph's top-left (x, y) in pixel space for the
            // chosen CoordinateSystem. Using those directly avoids double-applying the
            // font bearing which would incorrectly shift glyphs vertically (clipping
            // descenders). We keep offsets relative to the run's origin; PassManager
            // snaps the run once using line metrics.
            let ox = g.x;
            let oy = g.y;
            out.push(RasterizedGlyph {
                offset: [ox, oy],
                mask,
            });
        }
        out
    }
    fn line_metrics(&self, px: f32) -> Option<LineMetrics> {
        self.font.horizontal_line_metrics(px).map(|lm| {
            let ascent = lm.ascent;
            // Fontdue typically reports descent as a negative number; normalize to positive magnitude.
            let descent = lm.descent.abs();
            let line_gap = lm.line_gap.max(0.0);
            LineMetrics {
                ascent,
                descent,
                line_gap,
            }
        })
    }
}

/// LEGACY: Grayscale fontdue provider.
///
/// **NOT RECOMMENDED**: Use [`RuneTextProvider`] (harfrust + swash) instead.
/// This provider is kept for compatibility and testing purposes only.
///
/// Replicates grayscale coverage to RGB channels equally (no subpixel rendering).
pub struct GrayscaleFontdueProvider {
    font: fontdue::Font,
}

impl GrayscaleFontdueProvider {
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(Self { font })
    }
}

impl TextProvider for GrayscaleFontdueProvider {
    fn rasterize_run(&self, run: &crate::scene::TextRun) -> Vec<RasterizedGlyph> {
        use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: 0.0,
            y: 0.0,
            ..LayoutSettings::default()
        });
        layout.append(
            &[&self.font],
            &TextStyle::new(&run.text, run.size.max(1.0), 0),
        );
        let mut out = Vec::new();
        for g in layout.glyphs() {
            let (metrics, bitmap) = self.font.rasterize_indexed(g.key.glyph_index, g.key.px);
            if metrics.width == 0 || metrics.height == 0 {
                continue;
            }
            let mask = grayscale_to_rgb_equal(metrics.width as u32, metrics.height as u32, &bitmap);
            // See note above: use layout-provided top-left directly.
            let ox = g.x;
            let oy = g.y;
            out.push(RasterizedGlyph {
                offset: [ox, oy],
                mask,
            });
        }
        out
    }
    fn line_metrics(&self, px: f32) -> Option<LineMetrics> {
        self.font.horizontal_line_metrics(px).map(|lm| {
            let ascent = lm.ascent;
            let descent = lm.descent.abs();
            let line_gap = lm.line_gap.max(0.0);
            LineMetrics {
                ascent,
                descent,
                line_gap,
            }
        })
    }
}

/// Simplified line metrics
#[derive(Clone, Copy, Debug, Default)]
pub struct LineMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub line_gap: f32,
}

/// Text provider backed by rune-text (HarfBuzz) for shaping and swash for rasterization.
///
/// This uses a single `rune-text` `FontFace` and delegates shaping to
/// `TextShaper::shape_ltr`, then rasterizes glyphs via swash bitmap images.
pub struct RuneTextProvider {
    font: rune_text::FontFace,
    orientation: SubpixelOrientation,
}

impl RuneTextProvider {
    pub fn from_bytes(bytes: &[u8], orientation: SubpixelOrientation) -> anyhow::Result<Self> {
        let font = rune_text::FontFace::from_vec(bytes.to_vec(), 0)?;
        Ok(Self { font, orientation })
    }

    /// Construct from a reasonable system sans-serif font using `fontdb`.
    pub fn from_system_fonts(orientation: SubpixelOrientation) -> anyhow::Result<Self> {
        use fontdb::{Database, Family, Query, Source, Stretch, Style, Weight};

        let mut db = Database::new();
        db.load_system_fonts();

        let id = db
            .query(&Query {
                families: &[
                    Family::SansSerif,
                    Family::Name("Segoe UI".into()),
                    Family::Name("SF Pro Text".into()),
                    Family::Name("Arial".into()),
                ],
                weight: Weight::NORMAL,
                stretch: Stretch::Normal,
                style: Style::Normal,
                ..Query::default()
            })
            .ok_or_else(|| anyhow::anyhow!("no suitable system font found for rune-text"))?;

        let face = db
            .face(id)
            .ok_or_else(|| anyhow::anyhow!("fontdb face missing for system font id"))?;

        let bytes: Vec<u8> = match &face.source {
            Source::File(path) => std::fs::read(path)?,
            Source::Binary(data) => data.as_ref().as_ref().to_vec(),
            Source::SharedFile(_, data) => data.as_ref().as_ref().to_vec(),
        };

        let font = rune_text::FontFace::from_vec(bytes, face.index as usize)?;
        Ok(Self { font, orientation })
    }

    /// Layout a paragraph using rune-text's `TextLayout` with optional width-based wrapping.
    ///
    /// This exposes rune-text's multi-line layout (including per-line baselines) so that
    /// callers can build GPU-ready glyph batches without relying on `PassManager`
    /// baseline heuristics.
    pub fn layout_paragraph(
        &self,
        text: &str,
        size_px: f32,
        max_width: Option<f32>,
    ) -> rune_text::layout::TextLayout {
        use rune_text::layout::{TextLayout, WrapMode};

        let wrap = if max_width.is_some() {
            WrapMode::BreakWord
        } else {
            WrapMode::NoWrap
        };

        TextLayout::with_wrap(
            text.to_string(),
            &self.font,
            size_px.max(1.0),
            max_width,
            wrap,
        )
    }
}

impl TextProvider for RuneTextProvider {
    fn rasterize_run(&self, run: &crate::scene::TextRun) -> Vec<RasterizedGlyph> {
        use rune_text::shaping::TextShaper;
        use swash::scale::image::Content;
        use swash::scale::{Render, ScaleContext, Source, StrikeWith};
        use swash::{FontRef, GlyphId};

        let size = run.size.max(1.0);
        let shaped = TextShaper::shape_ltr(&run.text, 0..run.text.len(), &self.font, 0, size);

        // Build a swash scaler + renderer for this font/size that can rasterize
        // outlines into coverage masks. This mirrors the docs/rune-text
        // `GlyphRasterizer` pipeline but uses the older `Render` API from
        // swash 0.1.x.
        let font_bytes = self.font.as_bytes();
        let font_ref = FontRef::from_index(&font_bytes, 0)
            .expect("rune-text FontFace bytes should be a valid swash FontRef");
        let mut ctx = ScaleContext::new();
        let mut scaler = ctx.builder(font_ref).size(size).hint(true).build();
        let renderer = Render::new(&[
            // Prefer scalable outlines; fall back to bitmaps when available.
            Source::Outline,
            Source::Bitmap(StrikeWith::BestFit),
            Source::ColorBitmap(StrikeWith::BestFit),
        ]);

        let mut out = Vec::new();
        for (gid, pos) in shaped.glyphs.iter().zip(shaped.positions.iter()) {
            // Rasterize via swash scaler. This uses outlines for typical
            // fonts and falls back to bitmaps when present, avoiding the
            // "embedded bitmap only" issue from `glyph_bitmap`.
            let glyph_id: GlyphId = *gid;
            if let Some(img) = renderer.render(&mut scaler, glyph_id) {
                let w = img.placement.width as u32;
                let h = img.placement.height as u32;
                if w == 0 || h == 0 {
                    continue;
                }

                let mask = match img.content {
                    Content::Mask => grayscale_to_subpixel_rgb(w, h, &img.data, self.orientation),
                    Content::SubpixelMask => SubpixelMask {
                        width: w,
                        height: h,
                        format: MaskFormat::Rgba8,
                        data: img.data.clone(),
                    },
                    Content::Color => {
                        // Derive coverage from alpha channel.
                        let mut gray = Vec::with_capacity((w * h) as usize);
                        let mut i = 0usize;
                        while i + 3 < img.data.len() {
                            gray.push(img.data[i + 3]);
                            i += 4;
                        }
                        grayscale_to_subpixel_rgb(w, h, &gray, self.orientation)
                    }
                };

                let ox = pos.x_offset + img.placement.left as f32;
                let oy = pos.y_offset - img.placement.top as f32;
                out.push(RasterizedGlyph {
                    offset: [ox, oy],
                    mask,
                });
            }
        }

        out
    }

    fn line_metrics(&self, px: f32) -> Option<LineMetrics> {
        let m = self.font.scaled_metrics(px.max(1.0));
        Some(LineMetrics {
            ascent: m.ascent,
            descent: m.descent,
            line_gap: m.line_gap,
        })
    }
}

// Advanced shaper: integrate cosmic-text for shaping + swash rasterization (optional feature)
#[cfg(feature = "cosmic_text_shaper")]
mod cosmic_provider {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache};

    /// Legacy cosmic-text provider for compatibility.
    ///
    /// **NOT RECOMMENDED**: Use [`RuneTextProvider`] (harfrust + swash) instead.
    /// Only kept for testing/comparison purposes.
    ///
    /// A text provider backed by cosmic-text for shaping and swash for rasterization.
    /// Produces RGB subpixel masks from swash grayscale coverage.
    pub struct CosmicTextProvider {
        font_system: Mutex<FontSystem>,
        swash_cache: Mutex<SwashCache>,
        orientation: SubpixelOrientation,
        // Cache approximate line metrics per pixel size to aid baseline snapping
        metrics_cache: Mutex<HashMap<u32, LineMetrics>>, // key: px rounded
    }

    impl CosmicTextProvider {
        /// Construct with a custom font (preferred for demo parity with SimpleFontdueProvider)
        pub fn from_bytes(bytes: &[u8], orientation: SubpixelOrientation) -> anyhow::Result<Self> {
            use std::sync::Arc;
            let src = cosmic_text::fontdb::Source::Binary(Arc::new(bytes.to_vec()));
            let fs = FontSystem::new_with_fonts([src]);
            Ok(Self {
                font_system: Mutex::new(fs),
                swash_cache: Mutex::new(SwashCache::new()),
                orientation,
                metrics_cache: Mutex::new(HashMap::new()),
            })
        }

        /// Construct using system fonts (fallbacks handled by cosmic-text)
        #[allow(dead_code)]
        pub fn from_system_fonts(orientation: SubpixelOrientation) -> Self {
            Self {
                font_system: Mutex::new(FontSystem::new()),
                swash_cache: Mutex::new(SwashCache::new()),
                orientation,
                metrics_cache: Mutex::new(HashMap::new()),
            }
        }

        fn shape_once(fs: &mut FontSystem, buffer: &mut Buffer, text: &str, px: f32) {
            let mut b = buffer.borrow_with(fs);
            b.set_metrics_and_size(Metrics::new(px, (px * 1.2).max(px + 2.0)), None, None);
            b.set_text(text, &Attrs::new(), Shaping::Advanced, None);
            b.shape_until_scroll(true);
        }
    }

    impl TextProvider for CosmicTextProvider {
        fn rasterize_run(&self, run: &crate::scene::TextRun) -> Vec<RasterizedGlyph> {
            let mut out = Vec::new();
            let mut fs = self.font_system.lock().unwrap();
            // Shape into a temporary buffer first; drop borrow before rasterization
            let mut buffer = Buffer::new(
                &mut fs,
                Metrics::new(run.size.max(1.0), (run.size * 1.2).max(run.size + 2.0)),
            );
            Self::shape_once(&mut fs, &mut buffer, &run.text, run.size.max(1.0));
            drop(fs);

            // Iterate runs and rasterize glyphs
            let runs = buffer.layout_runs().collect::<Vec<_>>();
            let mut fs = self.font_system.lock().unwrap();
            let mut cache = self.swash_cache.lock().unwrap();
            for lr in runs.iter() {
                for g in lr.glyphs.iter() {
                    // Compute glyph position relative to the run baseline (not absolute).
                    // cosmic-text's own draw path uses: final_y = run.line_y + physical.y + image_y.
                    // Here we want offsets relative to baseline, so omit run.line_y.
                    let pg = g.physical((0.0, 0.0), 1.0);
                    if let Some(img) = cache.get_image(&mut fs, pg.cache_key) {
                        let w = img.placement.width as u32;
                        let h = img.placement.height as u32;
                        if w == 0 || h == 0 {
                            continue;
                        }
                        match img.content {
                            cosmic_text::SwashContent::Mask => {
                                let mask =
                                    grayscale_to_subpixel_rgb(w, h, &img.data, self.orientation);
                                // Placement to top-left relative to baseline-origin
                                let ox = pg.x as f32 + img.placement.left as f32;
                                let oy = pg.y as f32 - img.placement.top as f32;
                                out.push(RasterizedGlyph {
                                    offset: [ox, oy],
                                    mask,
                                });
                            }
                            cosmic_text::SwashContent::Color => {
                                // Derive coverage from alpha channel
                                let mut gray = Vec::with_capacity((w * h) as usize);
                                let mut i = 0usize;
                                while i + 3 < img.data.len() {
                                    gray.push(img.data[i + 3]); // A
                                    i += 4;
                                }
                                let mask = grayscale_to_subpixel_rgb(w, h, &gray, self.orientation);
                                let ox = pg.x as f32 + img.placement.left as f32;
                                let oy = pg.y as f32 - img.placement.top as f32;
                                out.push(RasterizedGlyph {
                                    offset: [ox, oy],
                                    mask,
                                });
                            }
                            cosmic_text::SwashContent::SubpixelMask => {
                                // Fallback: treat as grayscale for now (rare)
                                let mask =
                                    grayscale_to_subpixel_rgb(w, h, &img.data, self.orientation);
                                let ox = pg.x as f32 + img.placement.left as f32;
                                let oy = pg.y as f32 - img.placement.top as f32;
                                out.push(RasterizedGlyph {
                                    offset: [ox, oy],
                                    mask,
                                });
                            }
                        }
                    }
                }
            }
            out
        }

        fn line_metrics(&self, px: f32) -> Option<LineMetrics> {
            // Cache by integer pixel size to avoid repeated shaping
            let key = px.max(1.0).round() as u32;
            if let Some(m) = self.metrics_cache.lock().unwrap().get(&key).copied() {
                return Some(m);
            }
            let mut fs = self.font_system.lock().unwrap();
            // Shape a representative string and read layout line ascent/descent
            let mut buffer =
                Buffer::new(&mut fs, Metrics::new(px.max(1.0), (px * 1.2).max(px + 2.0)));
            // Borrow with fs to access line_layout API
            {
                let mut b = buffer.borrow_with(&mut fs);
                b.set_metrics_and_size(
                    Metrics::new(px.max(1.0), (px * 1.2).max(px + 2.0)),
                    None,
                    None,
                );
                b.set_text("Ag", &Attrs::new(), Shaping::Advanced, None);
                b.shape_until_scroll(true);
                if let Some(lines) = b.line_layout(0) {
                    if let Some(ll) = lines.get(0) {
                        let ascent = ll.max_ascent;
                        let descent = ll.max_descent;
                        let line_gap = (px * 1.2 - (ascent + descent)).max(0.0);
                        let lm = LineMetrics {
                            ascent,
                            descent,
                            line_gap,
                        };
                        self.metrics_cache.lock().unwrap().insert(key, lm);
                        return Some(lm);
                    }
                }
            }
            // Fallback heuristic
            let ascent = px * 0.8;
            let descent = px * 0.2;
            let line_gap = (px * 1.2 - (ascent + descent)).max(0.0);
            let lm = LineMetrics {
                ascent,
                descent,
                line_gap,
            };
            self.metrics_cache.lock().unwrap().insert(key, lm);
            Some(lm)
        }
    }

    pub use CosmicTextProvider as Provider;
}

#[cfg(feature = "cosmic_text_shaper")]
pub use cosmic_provider::Provider as CosmicTextProvider;

// High-quality rasterizer: shape via cosmic-text, rasterize via FreeType LCD + hinting (optional)
#[cfg(feature = "freetype_ffi")]
mod freetype_provider {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping};
    use freetype;

    /// Text provider that uses cosmic-text for shaping and FreeType for hinted LCD rasterization.
    pub struct FreeTypeProvider {
        font_system: Mutex<FontSystem>,
        orientation: SubpixelOrientation,
        // Keep font bytes; create FT library/face on demand to avoid Send/Sync issues
        ft_bytes: Vec<u8>,
        // Cache simple line metrics per integer pixel size
        metrics_cache: Mutex<HashMap<u32, LineMetrics>>, // key: px rounded
    }

    impl FreeTypeProvider {
        pub fn from_bytes(bytes: &[u8], orientation: SubpixelOrientation) -> anyhow::Result<Self> {
            use std::sync::Arc;
            let src = cosmic_text::fontdb::Source::Binary(Arc::new(bytes.to_vec()));
            let fs = FontSystem::new_with_fonts([src]);
            // Initialize FreeType and construct a memory face
            let data = bytes.to_vec();
            Ok(Self {
                font_system: Mutex::new(fs),
                orientation,
                ft_bytes: data,
                metrics_cache: Mutex::new(HashMap::new()),
            })
        }

        fn shape_once(fs: &mut FontSystem, buffer: &mut Buffer, text: &str, px: f32) {
            buffer.set_metrics_and_size(fs, Metrics::new(px, (px * 1.2).max(px + 2.0)), None, None);
            buffer.set_text(fs, text, &Attrs::new(), Shaping::Advanced, None);
            buffer.shape_until_scroll(fs, true);
        }
    }

    impl TextProvider for FreeTypeProvider {
        fn rasterize_run(&self, run: &crate::scene::TextRun) -> Vec<RasterizedGlyph> {
            let mut out = Vec::new();
            // Shape with cosmic-text
            let mut fs = self.font_system.lock().unwrap();
            let mut buffer = Buffer::new(
                &mut fs,
                Metrics::new(run.size.max(1.0), (run.size * 1.2).max(run.size + 2.0)),
            );
            Self::shape_once(&mut fs, &mut buffer, &run.text, run.size.max(1.0));
            drop(fs);

            // Iterate runs and rasterize glyphs using FreeType
            let runs = buffer.layout_runs().collect::<Vec<_>>();
            for lr in runs.iter() {
                for g in lr.glyphs.iter() {
                    // Map to physical glyph to access cache_key (contains glyph_id)
                    let pg = g.physical((0.0, 0.0), 1.0);
                    let glyph_index = pg.cache_key.glyph_id as u32;
                    let (w, h, ox, oy, data) = {
                        // Create FT library/face on demand to keep provider Send+Sync-compatible
                        if let Ok(lib) = freetype::Library::init() {
                            let _ = lib.set_lcd_filter(freetype::LcdFilter::LcdFilterDefault);
                            if let Ok(face) = lib.new_memory_face(self.ft_bytes.clone(), 0) {
                                // Use char size with 26.6 precision for better spacing parity
                                let target_ppem = (run.size.max(1.0) * 64.0) as isize; // 26.6 fixed
                                let _ = face.set_char_size(0, target_ppem, 72, 72);
                                let _ = face.set_pixel_sizes(0, run.size.max(1.0).ceil() as u32);
                                // Load & render glyph in LCD mode with hinting
                                use freetype::face::LoadFlag;
                                use freetype::render_mode::RenderMode;
                                let _ = face.load_glyph(
                                    glyph_index as u32,
                                    LoadFlag::DEFAULT | LoadFlag::TARGET_LCD | LoadFlag::COLOR,
                                );
                                let _ = face.glyph().render_glyph(RenderMode::Lcd);
                                let slot = face.glyph();
                                let bmp = slot.bitmap();
                                let width = (bmp.width() as u32).saturating_div(3); // LCD has 3 bytes per pixel horizontally
                                let height = bmp.rows() as u32;
                                if width == 0 || height == 0 {
                                    (0, 0, 0.0f32, 0.0f32, Vec::new())
                                } else {
                                    let left = slot.bitmap_left();
                                    let top = slot.bitmap_top();
                                    let ox = pg.x as f32 + left as f32;
                                    let oy = pg.y as f32 - top as f32;
                                    // Convert FT's LCD bitmap (packed RGBRGB...) into our RGBA mask rows
                                    let pitch = bmp.pitch().abs() as usize;
                                    let src = bmp.buffer();
                                    let mut rgba = vec![0u8; (width * height * 4) as usize];
                                    for row in 0..height as usize {
                                        let row_start = row * pitch;
                                        let row_end = row_start + (width as usize * 3);
                                        let src_row = &src[row_start..row_end];
                                        for x in 0..width as usize {
                                            let r = src_row[3 * x + 0];
                                            let g = src_row[3 * x + 1];
                                            let b = src_row[3 * x + 2];
                                            let i = (row * (width as usize) + x) * 4;
                                            match self.orientation {
                                                SubpixelOrientation::RGB => {
                                                    rgba[i + 0] = r;
                                                    rgba[i + 1] = g;
                                                    rgba[i + 2] = b;
                                                }
                                                SubpixelOrientation::BGR => {
                                                    rgba[i + 0] = b;
                                                    rgba[i + 1] = g;
                                                    rgba[i + 2] = r;
                                                }
                                            }
                                            rgba[i + 3] = 0;
                                        }
                                    }
                                    (width, height, ox, oy, rgba)
                                }
                            } else {
                                (0, 0, 0.0, 0.0, Vec::new())
                            }
                        } else {
                            (0, 0, 0.0, 0.0, Vec::new())
                        }
                    };
                    if w > 0 && h > 0 {
                        out.push(RasterizedGlyph {
                            offset: [ox, oy],
                            mask: SubpixelMask {
                                width: w,
                                height: h,
                                format: MaskFormat::Rgba8,
                                data,
                            },
                        });
                    }
                }
            }
            out
        }

        fn line_metrics(&self, px: f32) -> Option<LineMetrics> {
            let key = px.max(1.0).round() as u32;
            if let Some(m) = self.metrics_cache.lock().unwrap().get(&key).copied() {
                return Some(m);
            }
            // Use FreeType size metrics if available
            if let Ok(lib) = freetype::Library::init() {
                if let Ok(face) = lib.new_memory_face(self.ft_bytes.clone(), 0) {
                    let target_ppem = (px.max(1.0) * 64.0) as isize; // 26.6 fixed
                    let _ = face.set_char_size(0, target_ppem, 72, 72);
                    if let Some(sm) = face.size_metrics() {
                        // Values are in 26.6ths of a point; convert to pixels
                        let asc = (sm.ascender >> 6) as f32;
                        let desc = ((-sm.descender) >> 6) as f32;
                        let height = (sm.height >> 6) as f32;
                        let line_gap = (height - (asc + desc)).max(0.0);
                        let lm = LineMetrics {
                            ascent: asc,
                            descent: desc,
                            line_gap,
                        };
                        self.metrics_cache.lock().unwrap().insert(key, lm);
                        return Some(lm);
                    }
                }
            }
            // Fallback heuristic
            let ascent = px * 0.8;
            let descent = px * 0.2;
            let line_gap = (px * 1.2 - (ascent + descent)).max(0.0);
            let lm = LineMetrics {
                ascent,
                descent,
                line_gap,
            };
            self.metrics_cache.lock().unwrap().insert(key, lm);
            Some(lm)
        }
    }

    pub use FreeTypeProvider as Provider;
}

#[cfg(feature = "freetype_ffi")]
pub use freetype_provider::Provider as FreeTypeProvider;
