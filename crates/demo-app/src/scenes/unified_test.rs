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
        Self::new(1) // Default to solids + text
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
        // Simple dark background
        passes.paint_root_color(
            encoder,
            surface_view,
            ColorLinPremul::from_srgba_u8([30, 30, 40, 255]),
            queue,
        );
    }
}

fn build_test_dl(viewport: Viewport, test_level: u32) -> DisplayList {
    use engine_core::{Rect, TextRun};
    
    let mut painter = Painter::begin_frame(viewport);

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
    // (Would need actual image files - skip for now)

    if test_level < 3 {
        return painter.finish();
    }

    // Test Level 3: Add SVGs
    // (Would need actual SVG files - skip for now)

    painter.finish()
}
