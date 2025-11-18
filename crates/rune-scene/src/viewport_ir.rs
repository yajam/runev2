/// Viewport IR - Incremental implementation starting from basics
/// Building layer by layer to ensure unified rendering works at each step
use engine_core::{Brush, ColorLinPremul};

/// Phase 0: Just a solid background to verify rendering pipeline works
pub struct ViewportContent {
    // Will add fields as we build up functionality
}

impl ViewportContent {
    pub fn new() -> Self {
        Self {}
    }

    /// Render viewport content to canvas
    /// Phase 1: Background + basic text from the legacy viewport_ir_old demo.
    pub fn render(
        &mut self,
        canvas: &mut rune_surface::Canvas,
        scale_factor: f32,
        viewport_width: u32,
        viewport_height: u32,
        _provider: &dyn engine_core::TextProvider,
        _text_cache: &engine_core::TextLayoutCache,
    ) -> f32 {
        // Background: simple solid fill so viewport content stands out from the zone background.
        // Drawn at z=-100 so all UI/text appears above it.
        canvas.fill_rect(
            0.0,
            0.0,
            viewport_width as f32,
            viewport_height as f32,
            Brush::Solid(ColorLinPremul::from_srgba_u8([40, 45, 65, 255])), // Slightly lighter than zone bg
            -100,
        );

        // Font sizes are in logical pixels - DPI scaling is handled by Canvas/Surface layers.
        // Do NOT divide by scale_factor here, as that causes double-scaling issues.
        let _sf = scale_factor; // Keep parameter for future use if needed
        let title_size = 22.0;
        let subtitle_size = 18.0;
        let test_line_size = 20.0;

        // Basic text content: reuse the header/subtitle/test lines from viewport_ir_old.
        // Coordinates are viewport-local and assume the viewport origin has already been
        // translated by the caller to the correct zone.
        let col1_x = 40.0f32;
        let title_y = 40.0f32;
        let subtitle_y = 80.0f32;

        // Title
        canvas.draw_text_run(
            [col1_x, title_y],
            "Rune Scene \u{2014} UI Elements".to_string(),
            title_size,
            ColorLinPremul::rgba(255, 255, 255, 255),
            10,
        );

        // Subtitle
        canvas.draw_text_run(
            [col1_x, subtitle_y],
            "Subtitle example text".to_string(),
            subtitle_size,
            ColorLinPremul::rgba(200, 200, 200, 255),
            10,
        );

        // Bright cyan test line
        canvas.draw_text_run(
            [col1_x, 350.0],
            "TEST: This should be BRIGHT CYAN".to_string(),
            test_line_size,
            ColorLinPremul::rgba(0, 255, 255, 255),
            10,
        );

        // Phase 1: Content height is at least the viewport height for now.
        viewport_height as f32
    }
}

impl Default for ViewportContent {
    fn default() -> Self {
        Self::new()
    }
}
