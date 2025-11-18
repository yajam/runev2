/// Viewport IR - Incremental implementation starting from basics
/// Building layer by layer to ensure unified rendering works at each step
use engine_core::{Brush, Color, ColorLinPremul, Rect, SvgStyle};

/// Phase 0: Just a solid background to verify rendering pipeline works
pub struct ViewportContent {
    // Will add fields as we build up functionality
}

impl ViewportContent {
    pub fn new() -> Self {
        Self {}
    }

    /// Render viewport content to canvas
    /// Phase 1: Background + test box element
    pub fn render(
        &mut self,
        canvas: &mut rune_surface::Canvas,
        _scale_factor: f32,
        viewport_width: u32,
        viewport_height: u32,
        _provider: &dyn engine_core::TextProvider,
        _text_cache: &engine_core::TextLayoutCache,
    ) -> f32 {
        // Phase 0: Render a simple colored background to verify unified rendering works
        // This mimics what unified_test does - paint a fullscreen background

        // Fullscreen background at z=-100 (behind everything)
        // Use a distinct color so we can see it's rendering
        canvas.fill_rect(
            0.0,
            0.0,
            viewport_width as f32,
            viewport_height as f32,
            Brush::Solid(ColorLinPremul::from_srgba_u8([40, 45, 65, 255])), // Slightly lighter than zone bg
            -100,
        );

        // Phase 1: Add a test box element to verify shape rendering with z-ordering
        // Red box at z=10
        canvas.fill_rect(
            100.0,
            100.0,
            200.0,
            150.0,
            Brush::Solid(ColorLinPremul::from_srgba_u8([255, 0, 0, 255])), // Bright red
            10,
        );

        // Green box overlapping red at z=20 (should be on top)
        canvas.fill_rect(
            200.0,
            150.0,
            200.0,
            150.0,
            Brush::Solid(ColorLinPremul::from_srgba_u8([0, 255, 0, 255])), // Bright green
            20,
        );

        // Blue box overlapping both at z=30 (should be on top of both)
        canvas.fill_rect(
            300.0,
            200.0,
            200.0,
            150.0,
            Brush::Solid(ColorLinPremul::from_srgba_u8([0, 0, 255, 255])), // Bright blue
            30,
        );

        // Phase 1: Return minimal content height
        viewport_height as f32
    }
}

impl Default for ViewportContent {
    fn default() -> Self {
        Self::new()
    }
}
