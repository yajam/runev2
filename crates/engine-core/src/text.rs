//! Text utilities: subpixel mask conversion and simple APIs.

/// LCD subpixel orientation along X axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubpixelOrientation {
    RGB,
    BGR,
}

/// Storage format for a subpixel coverage mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskFormat { Rgba8, Rgba16 }

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
    pub fn bytes_per_pixel(&self) -> usize { match self.format { MaskFormat::Rgba8 => 4, MaskFormat::Rgba16 => 8 } }
}

/// Convert an 8-bit grayscale coverage mask to an RGB subpixel coverage mask.
/// A very lightweight 3-tap kernel is used to distribute coverage among subpixels.
/// - For RGB orientation, left/center/right contributions map to R/G/B respectively.
/// - For BGR orientation, mapping is mirrored.
pub fn grayscale_to_subpixel_rgb(width: u32, height: u32, gray: &[u8], orientation: SubpixelOrientation) -> SubpixelMask {
    let w = width as usize;
    let h = height as usize;
    assert_eq!(gray.len(), w * h);
    let mut out = vec![0u8; w * h * 4];
    // Approximate sampling at x-1/3, x, x+1/3 via linear interpolation between neighbors.
    // This tends to show clearer differences between RGB and BGR orientations.
    for y in 0..h {
        for x in 0..w {
            let c0 = gray[y * w + x] as f32 / 255.0;
            let cl = if x > 0 { gray[y * w + (x - 1)] as f32 / 255.0 } else { c0 };
            let cr = if x + 1 < w { gray[y * w + (x + 1)] as f32 / 255.0 } else { c0 };
            let sample_left = (2.0/3.0) * c0 + (1.0/3.0) * cl;
            let sample_center = c0;
            let sample_right = (2.0/3.0) * c0 + (1.0/3.0) * cr;
            let (r_cov, g_cov, b_cov) = match orientation {
                SubpixelOrientation::RGB => (sample_left, sample_center, sample_right),
                SubpixelOrientation::BGR => (sample_right, sample_center, sample_left),
            };
            let (r, g, b) = match orientation {
                SubpixelOrientation::RGB => (r_cov, g_cov, b_cov),
                SubpixelOrientation::BGR => (b_cov, g_cov, r_cov),
            };
            let i = (y * w + x) * 4;
            out[i + 0] = (r * 255.0 + 0.5) as u8;
            out[i + 1] = (g * 255.0 + 0.5) as u8;
            out[i + 2] = (b * 255.0 + 0.5) as u8;
            out[i + 3] = 0u8; // alpha unused; output premul alpha computed in shader
        }
    }
    SubpixelMask { width, height, format: MaskFormat::Rgba8, data: out }
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
    SubpixelMask { width, height, format: MaskFormat::Rgba8, data: out }
}

/// 16-bit variants for higher precision masks. Channels are u16 in [0..65535],
/// packed little-endian into the data buffer. Alpha is unused.
pub fn grayscale_to_subpixel_rgb16(width: u32, height: u32, gray: &[u8], orientation: SubpixelOrientation) -> SubpixelMask {
    let w = width as usize;
    let h = height as usize;
    assert_eq!(gray.len(), w * h);
    let mut out = vec![0u8; w * h * 8];
    for y in 0..h {
        for x in 0..w {
            let c0 = gray[y * w + x] as f32 / 255.0;
            let cl = if x > 0 { gray[y * w + (x - 1)] as f32 / 255.0 } else { c0 };
            let cr = if x + 1 < w { gray[y * w + (x + 1)] as f32 / 255.0 } else { c0 };
            let sample_left = (2.0/3.0) * c0 + (1.0/3.0) * cl;
            let sample_center = c0;
            let sample_right = (2.0/3.0) * c0 + (1.0/3.0) * cr;
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
    SubpixelMask { width, height, format: MaskFormat::Rgba16, data: out }
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
            out[i + 0] = b[0]; out[i + 1] = b[1];
            out[i + 2] = b[0]; out[i + 3] = b[1];
            out[i + 4] = b[0]; out[i + 5] = b[1];
            out[i + 6] = 0;   out[i + 7] = 0;
        }
    }
    SubpixelMask { width, height, format: MaskFormat::Rgba16, data: out }
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
        layout.reset(&LayoutSettings { x: 0.0, y: 0.0, ..LayoutSettings::default() });
        layout.append(&[&self.font], &TextStyle::new(&run.text, run.size.max(1.0), 0));
        let mut out = Vec::new();
        for g in layout.glyphs() {
            // Patched fontdue returns RGB masks directly (u8 or u16). Prefer 16-bit when available.
            let mask = if let Some((w, h, data16)) = self.font.rasterize_rgb16_indexed(g.key.glyph_index, g.key.px) {
                SubpixelMask { width: w as u32, height: h as u32, format: MaskFormat::Rgba16, data: data16 }
            } else {
                let (w, h, data8) = self.font.rasterize_rgb8_indexed(g.key.glyph_index, g.key.px);
                SubpixelMask { width: w as u32, height: h as u32, format: MaskFormat::Rgba8, data: data8 }
            };
            out.push(RasterizedGlyph { offset: [g.x, g.y], mask });
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

/// Text provider interface. Implementations convert a `TextRun` into positioned glyph masks.
pub trait TextProvider: Send + Sync {
    fn rasterize_run(&self, run: &crate::scene::TextRun) -> Vec<RasterizedGlyph>;
    fn line_metrics(&self, px: f32) -> Option<LineMetrics> { let _ = px; None }
}

/// A simple fontdue-based provider. This is a naive ASCII-first layout using fontdue's layout.
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
            if metrics.width == 0 || metrics.height == 0 { continue; }
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
            out.push(RasterizedGlyph { offset: [ox, oy], mask });
        }
        out
    }
    fn line_metrics(&self, px: f32) -> Option<LineMetrics> {
        self.font.horizontal_line_metrics(px).map(|lm| LineMetrics { ascent: lm.ascent, descent: lm.descent, line_gap: lm.line_gap })
    }
}

/// Grayscale provider that replicates grayscale coverage to RGB channels equally.
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
        layout.reset(&LayoutSettings { x: 0.0, y: 0.0, ..LayoutSettings::default() });
        layout.append(&[&self.font], &TextStyle::new(&run.text, run.size.max(1.0), 0));
        let mut out = Vec::new();
        for g in layout.glyphs() {
            let (metrics, bitmap) = self.font.rasterize_indexed(g.key.glyph_index, g.key.px);
            if metrics.width == 0 || metrics.height == 0 { continue; }
            let mask = grayscale_to_rgb_equal(metrics.width as u32, metrics.height as u32, &bitmap);
            // See note above: use layout-provided top-left directly.
            let ox = g.x;
            let oy = g.y;
            out.push(RasterizedGlyph { offset: [ox, oy], mask });
        }
        out
    }
    fn line_metrics(&self, px: f32) -> Option<LineMetrics> {
        self.font.horizontal_line_metrics(px).map(|lm| LineMetrics { ascent: lm.ascent, descent: lm.descent, line_gap: lm.line_gap })
    }
}

/// Simplified line metrics
#[derive(Clone, Copy, Debug, Default)]
pub struct LineMetrics { pub ascent: f32, pub descent: f32, pub line_gap: f32 }
