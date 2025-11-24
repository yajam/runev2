use super::common::ZoneStyle;
use crate::elements::InputBox;
use engine_core::{ColorLinPremul, Rect};
use std::time::Instant;

/// Toolbar configuration and state
pub struct Toolbar {
    pub style: ZoneStyle,
    pub address_bar: InputBox,
    /// Whether the page is currently loading
    pub is_loading: bool,
    /// Loading indicator blink state
    loading_icon_visible: bool,
    /// Blink timer accumulator
    blink_time: f32,
    /// Last click time for double-click detection on Home button
    last_home_click: Option<Instant>,
}

impl Toolbar {
    pub fn new() -> Self {
        // Create address bar input box (will be resized on first render)
        // Start with an empty URL - will be updated when CEF or navigation loads the first page
        let address_bar = InputBox::new(
            Rect {
                x: 0.0,
                y: 0.0,
                w: 400.0,
                h: 32.0,
            },
            String::new(),
            14.0,
            ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
            Some("Enter address...".to_string()),
            false,
        );

        Self {
            style: Self::default_style(),
            address_bar,
            // Start with is_loading=false - the navigation state will update this
            // based on whether we're loading CEF content or showing IR content
            is_loading: false,
            loading_icon_visible: true,
            blink_time: 0.0,
            last_home_click: None,
        }
    }

    /// Handle Home button click. Returns true if this was a double-click (should open Dock).
    pub fn handle_home_click(&mut self) -> bool {
        const DOUBLE_CLICK_THRESHOLD_MS: u128 = 400;

        let now = Instant::now();
        let is_double_click = if let Some(last) = self.last_home_click {
            now.duration_since(last).as_millis() < DOUBLE_CLICK_THRESHOLD_MS
        } else {
            false
        };

        if is_double_click {
            // Reset to prevent triple-click counting as another double
            self.last_home_click = None;
        } else {
            self.last_home_click = Some(now);
        }

        is_double_click
    }

    /// Set loading state
    pub fn set_loading(&mut self, loading: bool) {
        if loading && !self.is_loading {
            // Starting to load - reset blink state
            self.loading_icon_visible = true;
            self.blink_time = 0.0;
        }
        self.is_loading = loading;
    }

    /// Update loading indicator blink (call each frame while loading).
    /// Uses ~16ms frame time assumption for 60fps.
    pub fn update_loading_blink(&mut self) {
        if self.is_loading {
            const BLINK_INTERVAL: f32 = 0.3; // 300ms on/off cycle
            const FRAME_TIME: f32 = 1.0 / 60.0; // ~16ms

            self.blink_time += FRAME_TIME;
            if self.blink_time >= BLINK_INTERVAL {
                self.blink_time -= BLINK_INTERVAL;
                self.loading_icon_visible = !self.loading_icon_visible;
            }
        }
    }

    /// Check if loading icon should be visible this frame
    pub fn is_loading_icon_visible(&self) -> bool {
        self.is_loading && self.loading_icon_visible
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
    ///
    /// `can_go_back` and `can_go_forward` control the visual state of nav buttons.
    pub fn render(
        &mut self,
        canvas: &mut rune_surface::Canvas,
        toolbar_rect: Rect,
        provider: &dyn engine_core::TextProvider,
        can_go_back: bool,
        can_go_forward: bool,
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

        // Icon styling - active (white) vs muted (gray)
        let white = engine_core::Color::rgba(255, 255, 255, 255);
        let muted = engine_core::Color::rgba(100, 100, 110, 255);
        let icon_style = engine_core::SvgStyle::new()
            .with_stroke(white)
            .with_stroke_width(1.5);
        let muted_style = engine_core::SvgStyle::new()
            .with_stroke(muted)
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
        x += BUTTON_SIZE + BUTTON_GAP;

        // 2. Home button (navigates to Home tab, double-click opens Dock)
        let home_rect = Rect {
            x,
            y: center_y,
            w: BUTTON_SIZE,
            h: BUTTON_SIZE,
        };
        canvas.hit_region_rect(HOME_BUTTON_REGION_ID, home_rect, 10150);
        canvas.draw_svg_styled(
            "images/layout-grid.svg",
            [x, center_y],
            [BUTTON_SIZE, BUTTON_SIZE],
            icon_style.clone(),
            10200,
        );
        x += BUTTON_SIZE + SECTION_GAP;

        // 3. Back button (muted when no history)
        let back_rect = Rect {
            x,
            y: center_y,
            w: BUTTON_SIZE,
            h: BUTTON_SIZE,
        };
        if can_go_back {
            canvas.hit_region_rect(BACK_BUTTON_REGION_ID, back_rect, 10150);
        }
        canvas.draw_svg_styled(
            "images/arrow-left.svg",
            [x, center_y],
            [BUTTON_SIZE, BUTTON_SIZE],
            if can_go_back { icon_style.clone() } else { muted_style.clone() },
            10200,
        );
        x += BUTTON_SIZE + BUTTON_GAP;

        // 4. Forward button (muted when no forward history)
        let forward_rect = Rect {
            x,
            y: center_y,
            w: BUTTON_SIZE,
            h: BUTTON_SIZE,
        };
        if can_go_forward {
            canvas.hit_region_rect(FORWARD_BUTTON_REGION_ID, forward_rect, 10150);
        }
        canvas.draw_svg_styled(
            "images/arrow-right.svg",
            [x, center_y],
            [BUTTON_SIZE, BUTTON_SIZE],
            if can_go_forward { icon_style.clone() } else { muted_style.clone() },
            10200,
        );
        x += BUTTON_SIZE + SECTION_GAP;

        // 5. Address bar (expanding to fill remaining space)
        // Calculate width: total - used space - refresh + chat + devtools buttons - margins
        let right_buttons_width =
            BUTTON_SIZE * 3.0 + BUTTON_GAP * 2.0 + MARGIN;
        let address_width =
            (toolbar_rect.w - x - right_buttons_width - SECTION_GAP).max(100.0);

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

        // 6. Refresh/Spinner button (after address bar, aligned with right cluster)
        let devtools_x = toolbar_rect.w - BUTTON_SIZE - MARGIN;
        let chat_x = devtools_x - BUTTON_SIZE - BUTTON_GAP;
        let refresh_x = chat_x - BUTTON_SIZE - BUTTON_GAP;

        let refresh_rect = Rect {
            x: refresh_x,
            y: center_y,
            w: BUTTON_SIZE,
            h: BUTTON_SIZE,
        };
        canvas.hit_region_rect(REFRESH_BUTTON_REGION_ID, refresh_rect, 10150);

        if self.is_loading {
            // Draw blinking loader icon while loading (clicking stops the load)
            // Only draw when visible (blink on phase)
            if self.loading_icon_visible {
                canvas.draw_svg_styled(
                    "images/loader.svg",
                    [refresh_x, center_y],
                    [BUTTON_SIZE, BUTTON_SIZE],
                    icon_style.clone(),
                    10200,
                );
            }
        } else {
            // Draw static refresh icon
            canvas.draw_svg_styled(
                "images/refresh.svg",
                [refresh_x, center_y],
                [BUTTON_SIZE, BUTTON_SIZE],
                icon_style.clone(),
                10200,
            );
        }

        // 7. Chat button (opens Peco chat panel)
        let chat_rect = Rect {
            x: chat_x,
            y: center_y,
            w: BUTTON_SIZE,
            h: BUTTON_SIZE,
        };
        canvas.hit_region_rect(CHAT_BUTTON_REGION_ID, chat_rect, 10150);
        canvas.draw_svg_styled(
            "images/message-circle.svg",
            [chat_x, center_y],
            [BUTTON_SIZE, BUTTON_SIZE],
            icon_style.clone(),
            10200,
        );

        // 8. DevTools button (right edge)
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

/// Home button region ID for hit testing
pub const HOME_BUTTON_REGION_ID: u32 = 1006;

/// Chat button region ID for hit testing
pub const CHAT_BUTTON_REGION_ID: u32 = 1007;
