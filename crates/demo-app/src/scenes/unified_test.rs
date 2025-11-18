/// Minimal test scene for unified rendering
/// Tests each component (solids, text, images, SVGs) incrementally
use engine_core::{Brush, ColorLinPremul, DisplayList, Painter, PassManager, Viewport};

use super::{Scene, SceneKind};

pub struct UnifiedTestScene {
    /// Which test to run: 0=solids only, 1=+text, 2=+images, 3=+svgs
    test_level: u32,
}

impl UnifiedTestScene {
    pub fn new(test_level: u32) -> Self {
        Self { test_level }
    }
}

impl Default for UnifiedTestScene {
    fn default() -> Self {
        // Default to solids + text + images + svgs so unified rendering tests all key types.
        Self::new(3)
    }
}

impl Scene for UnifiedTestScene {
    fn kind(&self) -> SceneKind {
        SceneKind::Geometry
    }

    fn init_display_list(&mut self, viewport: Viewport) -> Option<DisplayList> {
        Some(build_test_dl(viewport, self.test_level))
    }

    fn on_resize(&mut self, viewport: Viewport) -> Option<DisplayList> {
        Some(build_test_dl(viewport, self.test_level))
    }

    fn paint_root_background(
        &self,
        passes: &mut PassManager,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        queue: &wgpu::Queue,
        _width: u32,
        _height: u32,
    ) {
        // Simple peach background
        passes.paint_root_color(
            encoder,
            surface_view,
            ColorLinPremul::from_srgba_u8([255, 229, 180, 255]),
            queue,
        );
    }
}

fn build_test_dl(viewport: Viewport, test_level: u32) -> DisplayList {
    use engine_core::{Rect, TextRun};
    
    let mut painter = Painter::begin_frame(viewport);

    // Global background so we don't see the clear color.
    // Use a peach tone to make layering easy to see.
    painter.rect(
        Rect {
            x: 0.0,
            y: 0.0,
            w: viewport.width as f32,
            h: viewport.height as f32,
        },
        Brush::Solid(ColorLinPremul::from_srgba_u8([255, 229, 200, 255])),
        -100,
    );

    // Test Level 0: Solids only
    // Draw bright colored rectangles to verify solid rendering and z-ordering

    // Red rectangle (z=10)
    painter.rect(
        Rect { x: 100.0, y: 100.0, w: 200.0, h: 150.0 },
        Brush::Solid(ColorLinPremul::from_srgba_u8([255, 0, 0, 255])),
        10,
    );

    // Green rectangle overlapping (z=20 - should be on top of red)
    painter.rect(
        Rect { x: 200.0, y: 150.0, w: 200.0, h: 150.0 },
        Brush::Solid(ColorLinPremul::from_srgba_u8([0, 255, 0, 255])),
        20,
    );

    // Blue rectangle (z=30 - should be on top of both)
    painter.rect(
        Rect { x: 300.0, y: 200.0, w: 200.0, h: 150.0 },
        Brush::Solid(ColorLinPremul::from_srgba_u8([0, 0, 255, 255])),
        30,
    );

    if test_level < 1 {
        return painter.finish();
    }

    // Test Level 1: Add text
    painter.text(
        TextRun {
            text: "Unified Rendering Test".to_string(),
            pos: [50.0, 50.0],
            size: 32.0,
            color: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
        },
        100,
    );

    painter.text(
        TextRun {
            text: "RED (z=10)".to_string(),
            pos: [110.0, 160.0],
            size: 20.0,
            color: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
        },
        15, // z=15 - should appear on top of red, below green
    );

    painter.text(
        TextRun {
            text: "GREEN (z=20)".to_string(),
            pos: [210.0, 210.0],
            size: 20.0,
            color: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]), // Changed to white for visibility
        },
        25, // z=25 - should appear on top of green, below blue
    );

    painter.text(
        TextRun {
            text: "BLUE (z=30)".to_string(),
            pos: [310.0, 260.0],
            size: 20.0,
            color: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
        },
        35, // z=35 - should appear on top of blue
    );

    if test_level < 2 {
        return painter.finish();
    }

    // Test Level 2: Add images
    // Draw a raster image overlapping the colored rectangles to verify z-index
    // For example, z=28 should appear above GREEN (z=20) but below BLUE (z=30).
    let img_origin = [220.0, 170.0];
    let img_size = [180.0, 180.0];
    painter.image("images/mountains.webp", img_origin, img_size, 28);

    // Label for the image at a higher z to confirm ordering.
    painter.text(
        TextRun {
            text: "IMAGE (z=28)".to_string(),
            pos: [img_origin[0] + 10.0, img_origin[1] + img_size[1] + 24.0],
            size: 18.0,
            color: ColorLinPremul::from_srgba_u8([255, 255, 0, 255]),
        },
        40,
    );

    if test_level < 3 {
        return painter.finish();
    }

    // Test Level 3: Add SVGs
    // Place the SVG clearly above the blue rect in Y so its z-order is easy to inspect.
    // Blue rect: x=300..500, y=200..350. Put SVG just above at roughly the same X range.
    let svg_origin = [320.0, 240.0];
    let svg_size = [120.0, 120.0];
    // z=34 — above BLUE (z=30) and IMAGE (z=28), but below the labels (z=40+).
    painter.svg("images/image.svg", svg_origin, svg_size, 34);

    // Label for the SVG at an even higher z to confirm ordering.
    painter.text(
        TextRun {
            text: "SVG (z=34)".to_string(),
            pos: [svg_origin[0] + 10.0, svg_origin[1] - 10.0],
            size: 18.0,
            color: ColorLinPremul::from_srgba_u8([180, 255, 180, 255]),
        },
        45,
    );

    painter.finish()
}
