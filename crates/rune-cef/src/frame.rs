//! Frame buffer for captured browser content.

use bytemuck::{Pod, Zeroable};

/// A dirty rectangle region that needs updating.
#[derive(Debug, Clone, Copy, Default)]
pub struct DirtyRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl DirtyRect {
    /// Check if this rect is valid (non-zero size).
    pub fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0
    }

    /// Calculate the byte offset for this rect in a buffer with given stride.
    pub fn byte_offset(&self, stride: u32) -> usize {
        (self.y * stride + self.x * 4) as usize
    }

    /// Calculate the byte size for this rect.
    pub fn byte_size(&self) -> usize {
        (self.width * self.height * 4) as usize
    }
}

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
    /// Dirty rectangles that were updated (empty = full frame).
    pub dirty_rects: Vec<DirtyRect>,
    /// Whether this is a full frame update or partial.
    pub is_full_frame: bool,
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
            dirty_rects: Vec::new(),
            is_full_frame: true,
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
            dirty_rects: Vec::new(),
            is_full_frame: true,
        }
    }

    /// Create a frame buffer with dirty rects for partial updates.
    pub fn from_raw_with_dirty_rects(
        data: Vec<u8>,
        width: u32,
        height: u32,
        stride: u32,
        format: PixelFormat,
        dirty_rects: Vec<DirtyRect>,
    ) -> Self {
        let is_full_frame = dirty_rects.is_empty()
            || (dirty_rects.len() == 1
                && dirty_rects[0].x == 0
                && dirty_rects[0].y == 0
                && dirty_rects[0].width == width
                && dirty_rects[0].height == height);
        Self {
            data,
            width,
            height,
            stride,
            format,
            dirty_rects,
            is_full_frame,
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

    /// Get BGRA data (converts if necessary, returns a copy).
    pub fn as_bgra(&self) -> Vec<u8> {
        match self.format {
            PixelFormat::Bgra8 => self.data.clone(),
            PixelFormat::Rgba8 => {
                let mut bgra = self.data.clone();
                for row in 0..self.height {
                    let row_start = (row * self.stride) as usize;
                    for col in 0..self.width {
                        let offset = row_start + (col * 4) as usize;
                        bgra.swap(offset, offset + 2);
                    }
                }
                bgra
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

    /// Check if this frame has dirty rects for partial update.
    pub fn has_dirty_rects(&self) -> bool {
        !self.dirty_rects.is_empty() && !self.is_full_frame
    }

    /// Get the dirty rects, or a single full-frame rect if none specified.
    pub fn get_dirty_rects(&self) -> Vec<DirtyRect> {
        if self.dirty_rects.is_empty() || self.is_full_frame {
            vec![DirtyRect {
                x: 0,
                y: 0,
                width: self.width,
                height: self.height,
            }]
        } else {
            self.dirty_rects.clone()
        }
    }

    /// Extract pixel data for a specific dirty rect.
    /// Returns the pixel data for just that region (row by row).
    pub fn extract_rect_data(&self, rect: &DirtyRect) -> Vec<u8> {
        if !rect.is_valid() || rect.x + rect.width > self.width || rect.y + rect.height > self.height
        {
            return Vec::new();
        }

        let mut data = Vec::with_capacity((rect.width * rect.height * 4) as usize);
        for row in 0..rect.height {
            let src_offset = ((rect.y + row) * self.stride + rect.x * 4) as usize;
            let row_len = (rect.width * 4) as usize;
            if src_offset + row_len <= self.data.len() {
                data.extend_from_slice(&self.data[src_offset..src_offset + row_len]);
            }
        }
        data
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
