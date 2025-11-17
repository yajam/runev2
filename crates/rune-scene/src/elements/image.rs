use engine_core::{Brush, ColorLinPremul, Rect};
use rune_surface::{Canvas, ImageFitMode};
use std::path::PathBuf;

/// How the image should fit within the rect bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFit {
    /// Stretch to fill the rect (may distort aspect ratio)
    Fill,
    /// Fit inside the rect maintaining aspect ratio (letterbox/pillarbox)
    Contain,
    /// Fill the rect maintaining aspect ratio (may crop edges)
    Cover,
}

impl Default for ImageFit {
    fn default() -> Self {
        Self::Contain
    }
}

impl ImageFit {
    /// Convert to the rune_surface ImageFitMode
    fn to_fit_mode(self) -> ImageFitMode {
        match self {
            ImageFit::Fill => ImageFitMode::Fill,
            ImageFit::Contain => ImageFitMode::Contain,
            ImageFit::Cover => ImageFitMode::Cover,
        }
    }
}

/// Image box that renders actual images from disk.
pub struct ImageBox {
    pub rect: Rect,
    pub path: Option<PathBuf>,
    pub tint: ColorLinPremul, // Fallback color if image fails to load
    pub fit: ImageFit,
}

impl ImageBox {
    /// Create a new ImageBox with an image path
    pub fn new(rect: Rect, path: impl Into<PathBuf>) -> Self {
        Self {
            rect,
            path: Some(path.into()),
            tint: ColorLinPremul::from_srgba_u8([200, 200, 200, 255]), // Default gray fallback
            fit: ImageFit::default(),
        }
    }

    /// Create a new ImageBox with an image path and custom fit mode
    pub fn new_with_fit(rect: Rect, path: impl Into<PathBuf>, fit: ImageFit) -> Self {
        Self {
            rect,
            path: Some(path.into()),
            tint: ColorLinPremul::from_srgba_u8([200, 200, 200, 255]),
            fit,
        }
    }

    /// Create a new ImageBox with just a colored rect (no image)
    pub fn new_colored(rect: Rect, tint: ColorLinPremul) -> Self {
        Self {
            rect,
            path: None,
            tint,
            fit: ImageFit::default(),
        }
    }

    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        if let Some(ref path) = self.path {
            // Draw the actual image at the rect position and size with fit mode
            canvas.draw_image(
                path.clone(),
                [self.rect.x, self.rect.y],
                [self.rect.w, self.rect.h],
                self.fit.to_fit_mode(),
                z,
            );
        } else {
            // Fallback to colored rect if no path is provided
            canvas.fill_rect(
                self.rect.x,
                self.rect.y,
                self.rect.w,
                self.rect.h,
                Brush::Solid(self.tint),
                z,
            );
        }
    }
}
