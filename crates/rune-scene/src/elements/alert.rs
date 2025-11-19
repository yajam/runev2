use engine_core::{Brush, ColorLinPremul, Rect, RoundedRadii, RoundedRect};
use rune_surface::Canvas;
use rune_surface::shapes;

/// Positions for alert panels relative to the viewport.
#[derive(Clone, Copy)]
pub enum AlertPosition {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

/// Lightweight alert/toast panel similar to a modal, but without a
/// fullscreen background. Intended for transient status messages like
/// “Event has been created” with an optional action (e.g., Undo).
pub struct Alert {
    /// Viewport size used to position the alert.
    pub viewport_width: f32,
    pub viewport_height: f32,

    /// Panel size.
    pub width: f32,
    pub height: f32,

    /// Placement within the viewport.
    pub position: AlertPosition,

    /// Primary and secondary text.
    pub title: String,
    pub message: String,

    /// Optional action button label (e.g., "Undo").
    pub action_label: Option<String>,

    /// Visual styling.
    pub panel_bg: ColorLinPremul,
    pub panel_border_color: ColorLinPremul,
    pub title_color: ColorLinPremul,
    pub message_color: ColorLinPremul,
    pub action_bg: ColorLinPremul,
    pub action_fg: ColorLinPremul,

    /// Font sizes.
    pub title_size: f32,
    pub message_size: f32,

    /// Border radius for panel.
    pub panel_radius: f32,

    /// Base z-index (panel and contents render at and above this).
    pub base_z: i32,
}

impl Alert {
    /// Create a new alert with default sizing and styling.
    pub fn new(
        viewport_width: f32,
        viewport_height: f32,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            viewport_width,
            viewport_height,
            width: 420.0,
            height: 80.0,
            position: AlertPosition::TopCenter,
            title: title.into(),
            message: message.into(),
            action_label: None,
            panel_bg: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
            panel_border_color: ColorLinPremul::from_srgba_u8([220, 220, 220, 255]),
            title_color: ColorLinPremul::from_srgba_u8([20, 20, 20, 255]),
            message_color: ColorLinPremul::from_srgba_u8([120, 120, 120, 255]),
            action_bg: ColorLinPremul::from_srgba_u8([0, 0, 0, 255]),
            action_fg: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
            title_size: 16.0,
            message_size: 14.0,
            panel_radius: 12.0,
            // Slightly below the modal but above most UI chrome.
            base_z: 9200,
        }
    }

    /// Set the alert position (top/bottom and left/center/right).
    pub fn with_position(mut self, position: AlertPosition) -> Self {
        self.position = position;
        self
    }

    /// Enable an action button with the given label.
    pub fn with_action(mut self, label: impl Into<String>) -> Self {
        self.action_label = Some(label.into());
        self
    }

    /// Compute the panel rectangle for the chosen position.
    pub fn panel_rect(&self) -> Rect {
        let margin = 24.0;
        let max_width = (self.viewport_width - margin * 2.0).max(120.0);
        let w = self.width.min(max_width).max(120.0);
        let h = self.height.max(48.0);

        let x = match self.position {
            AlertPosition::TopLeft | AlertPosition::BottomLeft => margin,
            AlertPosition::TopCenter | AlertPosition::BottomCenter => {
                (self.viewport_width - w) * 0.5
            }
            AlertPosition::TopRight | AlertPosition::BottomRight => {
                self.viewport_width - w - margin
            }
        };

        let y = match self.position {
            AlertPosition::TopLeft | AlertPosition::TopCenter | AlertPosition::TopRight => margin,
            AlertPosition::BottomLeft
            | AlertPosition::BottomCenter
            | AlertPosition::BottomRight => self.viewport_height - h - margin,
        };

        Rect { x, y, w, h }
    }

    /// Compute the action button rectangle for a given panel rect, if an action is present.
    fn action_rect_for_panel(&self, panel: Rect) -> Option<Rect> {
        if self.action_label.is_none() {
            return None;
        }

        let button_height = 32.0;
        let horizontal_padding = 16.0;
        let label_len = self.action_label.as_ref().map(|s| s.len()).unwrap_or(0) as f32;
        let approx_text_width = label_len * self.message_size * 0.55;
        let button_width = (approx_text_width + horizontal_padding * 2.0).max(80.0);

        let button_x = panel.x + panel.w - 20.0 - button_width;
        let button_y = panel.y + (panel.h - button_height) * 0.5;

        Some(Rect {
            x: button_x,
            y: button_y,
            w: button_width,
            h: button_height,
        })
    }

    /// Public helper for hit-testing: returns the action button rect in
    /// viewport coordinates, if an action label is set.
    pub fn action_rect(&self) -> Option<Rect> {
        let panel = self.panel_rect();
        self.action_rect_for_panel(panel)
    }

    /// Render the alert panel and its contents.
    pub fn render(&self, canvas: &mut Canvas) {
        let z = self.base_z;
        let panel = self.panel_rect();

        // Subtle drop shadow below the panel.
        let shadow_offset_y = 6.0;
        canvas.fill_rect(
            panel.x,
            panel.y + shadow_offset_y,
            panel.w,
            panel.h,
            Brush::Solid(ColorLinPremul::from_srgba_u8([0, 0, 0, 30])),
            z - 10,
        );

        let rrect = RoundedRect {
            rect: panel,
            radii: RoundedRadii {
                tl: self.panel_radius,
                tr: self.panel_radius,
                br: self.panel_radius,
                bl: self.panel_radius,
            },
        };

        // Panel background.
        canvas.rounded_rect(rrect, Brush::Solid(self.panel_bg), z);

        // Thin border to crispen edges on light backgrounds.
        shapes::draw_rounded_rectangle(
            canvas,
            rrect,
            None,
            Some(1.0),
            Some(Brush::Solid(self.panel_border_color)),
            z + 1,
        );

        // Text layout: title + secondary line, vertically centered as a block.
        let text_left = panel.x + 20.0;
        let text_block_height = self.title_size + self.message_size * 1.6;
        let block_top = panel.y + (panel.h - text_block_height) * 0.5;
        // Baselines: first line at bottom of its box, second line offset below it.
        let title_y = block_top + self.title_size;
        canvas.draw_text_run(
            [text_left, title_y],
            self.title.clone(),
            self.title_size,
            self.title_color,
            z + 2,
        );

        let message_y = block_top + self.title_size + self.message_size * 1.6;
        canvas.draw_text_run(
            [text_left, message_y],
            self.message.clone(),
            self.message_size,
            self.message_color,
            z + 2,
        );

        // Optional action pill aligned to the right.
        if let (Some(label), Some(rect)) = (&self.action_label, self.action_rect_for_panel(panel)) {
            let rrect = RoundedRect {
                rect,
                radii: RoundedRadii {
                    tl: rect.h * 0.5,
                    tr: rect.h * 0.5,
                    br: rect.h * 0.5,
                    bl: rect.h * 0.5,
                },
            };

            canvas.rounded_rect(rrect, Brush::Solid(self.action_bg), z + 3);

            let label_len = label.len() as f32;
            let approx_text_width = label_len * self.message_size * 0.55;
            let text_x = rect.x + (rect.w - approx_text_width) * 0.5;
            let text_y = rect.y + rect.h * 0.5 + self.message_size * 0.35;

            canvas.draw_text_run(
                [text_x, text_y],
                label.clone(),
                self.message_size,
                self.action_fg,
                z + 4,
            );
        }
    }
}
