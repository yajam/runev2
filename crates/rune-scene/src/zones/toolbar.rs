use super::common::ZoneStyle;
use crate::elements::InputBox;
use engine_core::{ColorLinPremul, Rect};

/// Toolbar configuration and state
pub struct Toolbar {
    pub style: ZoneStyle,
    pub address_bar: InputBox,
}

impl Toolbar {
    pub fn new() -> Self {
        // Create address bar input box (will be resized on first render)
        let address_bar = InputBox::new(
            Rect {
                x: 0.0,
                y: 0.0,
                w: 400.0,
                h: 32.0,
            },
            "https://example.com".to_string(),
            14.0,
            ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
            Some("Enter address...".to_string()),
            false,
        );

        Self {
            style: Self::default_style(),
            address_bar,
        }
    }

    pub fn default_style() -> ZoneStyle {
        ZoneStyle {
            bg_color: ColorLinPremul::from_srgba_u8([18, 23, 43, 255]),
            border_color: ColorLinPremul::from_srgba_u8([60, 65, 85, 255]),
            border_width: 1.0,
        }
    }

    /// Render toolbar content with navigation controls and address bar
    /// Render the toolbar.
    ///
    /// IMPORTANT: toolbar_rect is passed for dimensions and calculations,
    /// but we render in LOCAL coordinates (0,0 origin) because the caller applies a transform.
    /// Hit regions also use LOCAL coordinates since they ARE affected by transforms.
    pub fn render(
        &mut self,
        canvas: &mut rune_surface::Canvas,
        toolbar_rect: Rect,
        provider: &dyn engine_core::TextProvider,
    ) {
        // Layout constants
        const BUTTON_SIZE: f32 = 24.0;
        const BUTTON_GAP: f32 = 8.0;
        const SECTION_GAP: f32 = 16.0;
        const MARGIN: f32 = 12.0;
        const ADDRESS_HEIGHT: f32 = 32.0;

        // Calculate positions in LOCAL coordinates (both rendering and hit regions use local coords)
        let center_y = (toolbar_rect.h - BUTTON_SIZE) * 0.5;
        let address_y = (toolbar_rect.h - ADDRESS_HEIGHT) * 0.5;

        // Icon styling
        let white = engine_core::Color::rgba(255, 255, 255, 255);
        let icon_style = engine_core::SvgStyle::new()
            .with_stroke(white)
            .with_stroke_width(1.5);

        let mut x = MARGIN;

        // 1. Sidebar toggle button (left edge)
        let toggle_rect = Rect {
            x,
            y: center_y,
            w: BUTTON_SIZE,
            h: BUTTON_SIZE,
        };
        canvas.hit_region_rect(TOGGLE_BUTTON_REGION_ID, toggle_rect, 10150);
        canvas.draw_svg_styled(
            "images/panel-left.svg",
            [x, center_y],
            [BUTTON_SIZE, BUTTON_SIZE],
            icon_style.clone(),
            10200,
        );
        x += BUTTON_SIZE + SECTION_GAP;

        // 2. Back button
        let back_rect = Rect {
            x,
            y: center_y,
            w: BUTTON_SIZE,
            h: BUTTON_SIZE,
        };
        canvas.hit_region_rect(BACK_BUTTON_REGION_ID, back_rect, 10150);
        canvas.draw_svg_styled(
            "images/arrow-left.svg",
            [x, center_y],
            [BUTTON_SIZE, BUTTON_SIZE],
            icon_style.clone(),
            10200,
        );
        x += BUTTON_SIZE + BUTTON_GAP;

        // 3. Forward button
        let forward_rect = Rect {
            x,
            y: center_y,
            w: BUTTON_SIZE,
            h: BUTTON_SIZE,
        };
        canvas.hit_region_rect(FORWARD_BUTTON_REGION_ID, forward_rect, 10150);
        canvas.draw_svg_styled(
            "images/arrow-right.svg",
            [x, center_y],
            [BUTTON_SIZE, BUTTON_SIZE],
            icon_style.clone(),
            10200,
        );
        x += BUTTON_SIZE + SECTION_GAP;

        // 4. Address bar (expanding to fill remaining space)
        // Calculate width: total - used space - refresh button - devtools button - margins
        let refresh_and_devtools_width = BUTTON_SIZE + BUTTON_GAP + BUTTON_SIZE + MARGIN;
        let address_width =
            (toolbar_rect.w - x - refresh_and_devtools_width - SECTION_GAP).max(100.0);

        // Update address bar rect (LOCAL coordinates)
        self.address_bar.rect = Rect {
            x,
            y: address_y,
            w: address_width,
            h: ADDRESS_HEIGHT,
        };

        // Add hit region (LOCAL coordinates - transform will be applied)
        // Keep hit region above the rendered input visuals (which draw at z ≈ 10200)
        // so hit testing isn't overshadowed by the background/border shapes.
        canvas.hit_region_rect(ADDRESS_BAR_REGION_ID, self.address_bar.rect, 10250);

        // Render the address bar (editable input box)
        self.address_bar.render(canvas, 10200, provider);
        x += address_width + SECTION_GAP;

        // 5. Refresh button (after address bar)
        let refresh_rect = Rect {
            x,
            y: center_y,
            w: BUTTON_SIZE,
            h: BUTTON_SIZE,
        };
        canvas.hit_region_rect(REFRESH_BUTTON_REGION_ID, refresh_rect, 10150);
        canvas.draw_svg_styled(
            "images/refresh.svg",
            [x, center_y],
            [BUTTON_SIZE, BUTTON_SIZE],
            icon_style.clone(),
            10200,
        );

        // 6. DevTools button (right edge)
        let devtools_x = toolbar_rect.w - BUTTON_SIZE - MARGIN;
        let devtools_rect = Rect {
            x: devtools_x,
            y: center_y,
            w: BUTTON_SIZE,
            h: BUTTON_SIZE,
        };
        canvas.hit_region_rect(DEVTOOLS_BUTTON_REGION_ID, devtools_rect, 10150);
        canvas.draw_svg_styled(
            "images/inspection-panel.svg",
            [devtools_x, center_y],
            [BUTTON_SIZE, BUTTON_SIZE],
            icon_style,
            10200,
        );
    }
}

impl Default for Toolbar {
    fn default() -> Self {
        Self::new()
    }
}

/// Toggle button region ID for hit testing
pub const TOGGLE_BUTTON_REGION_ID: u32 = 1000;

/// DevTools button region ID for hit testing
pub const DEVTOOLS_BUTTON_REGION_ID: u32 = 1001;

/// Back button region ID for hit testing
pub const BACK_BUTTON_REGION_ID: u32 = 1002;

/// Forward button region ID for hit testing
pub const FORWARD_BUTTON_REGION_ID: u32 = 1003;

/// Refresh button region ID for hit testing
pub const REFRESH_BUTTON_REGION_ID: u32 = 1004;

/// Address bar region ID for hit testing
pub const ADDRESS_BAR_REGION_ID: u32 = 1005;
