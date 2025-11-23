//! Frame buffer for captured browser content.

use bytemuck::{Pod, Zeroable};

/// A captured frame from the headless browser.
#[derive(Debug, Clone)]
pub struct FrameBuffer {
    /// Raw pixel data in BGRA format (CEF native format).
    pub data: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Bytes per row (may include padding).
    pub stride: u32,
    /// Pixel format.
    pub format: PixelFormat,
}

/// Pixel format of the frame buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PixelFormat {
    /// BGRA 8-bit per channel (CEF native, little-endian: B, G, R, A).
    #[default]
    Bgra8,
    /// RGBA 8-bit per channel.
    Rgba8,
}

impl FrameBuffer {
    /// Create a new empty frame buffer.
    pub fn new(width: u32, height: u32) -> Self {
        let stride = width * 4;
        let size = (stride * height) as usize;
        Self {
            data: vec![0u8; size],
            width,
            height,
            stride,
            format: PixelFormat::Bgra8,
        }
    }

    /// Create a frame buffer with existing data.
    pub fn from_raw(data: Vec<u8>, width: u32, height: u32, stride: u32, format: PixelFormat) -> Self {
        Self {
            data,
            width,
            height,
            stride,
            format,
        }
    }

    /// Get the size in bytes.
    pub fn size_bytes(&self) -> usize {
        self.data.len()
    }

    /// Convert BGRA to RGBA in place.
    pub fn convert_bgra_to_rgba(&mut self) {
        if self.format != PixelFormat::Bgra8 {
            return;
        }

        for row in 0..self.height {
            let row_start = (row * self.stride) as usize;
            for col in 0..self.width {
                let offset = row_start + (col * 4) as usize;
                // Swap B and R
                self.data.swap(offset, offset + 2);
            }
        }
        self.format = PixelFormat::Rgba8;
    }

    /// Get RGBA data (converts if necessary, returns a copy).
    pub fn as_rgba(&self) -> Vec<u8> {
        match self.format {
            PixelFormat::Rgba8 => self.data.clone(),
            PixelFormat::Bgra8 => {
                let mut rgba = self.data.clone();
                for row in 0..self.height {
                    let row_start = (row * self.stride) as usize;
                    for col in 0..self.width {
                        let offset = row_start + (col * 4) as usize;
                        rgba.swap(offset, offset + 2);
                    }
                }
                rgba
            }
        }
    }

    /// Get a reference to the raw pixel data.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Check if the frame is empty (all zeros or no data).
    pub fn is_empty(&self) -> bool {
        self.data.is_empty() || self.width == 0 || self.height == 0
    }
}

/// BGRA pixel for direct memory mapping.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct BgraPixel {
    pub b: u8,
    pub g: u8,
    pub r: u8,
    pub a: u8,
}

/// RGBA pixel for wgpu textures.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct RgbaPixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl From<BgraPixel> for RgbaPixel {
    fn from(bgra: BgraPixel) -> Self {
        Self {
            r: bgra.r,
            g: bgra.g,
            b: bgra.b,
            a: bgra.a,
        }
    }
}
