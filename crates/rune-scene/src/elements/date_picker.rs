use crate::ir_adapter;
use engine_core::{Brush, Color, ColorLinPremul, Rect, RoundedRadii, RoundedRect, SvgStyle};
use rune_surface::Canvas;
use rune_surface::shapes;

/// Picker mode for the calendar popup
#[derive(Clone, Copy, PartialEq)]
pub enum PickerMode {
    Days,
    Months,
    Years,
}

/// Date-picker widget with input field and calendar popup.
///
/// Features:
/// - Input field displaying the selected date
/// - Calendar icon button to open/close the popup
/// - Calendar popup with month/year navigation
/// - Clickable day grid for date selection
#[derive(Clone)]
pub struct DatePicker {
    pub rect: Rect,
    pub label_size: f32,
    pub label_color: ColorLinPremul,
    pub open: bool,
    pub focused: bool,
    pub selected_date: Option<(u32, u32, u32)>, // (year, month, day)
    pub current_view_month: u32,                // Month being viewed (1-12)
    pub current_view_year: u32,                 // Year being viewed
    pub picker_mode: PickerMode,                // Days, Months, or Years view
    // Styling fields
    pub padding_left: f32,
    pub padding_right: f32,
    pub padding_top: f32,
    pub padding_bottom: f32,
    pub bg_color: ColorLinPremul,
    pub border_color: ColorLinPremul,
    pub border_width: f32,
    pub radius: f32,
    /// Viewport/content height for smart popup positioning
    pub viewport_height: f32,
}

impl DatePicker {
    /// Create a new DatePicker with optional initial date
    pub fn new(
        rect: Rect,
        label_size: f32,
        label_color: ColorLinPremul,
        initial_date: Option<(u32, u32, u32)>,
    ) -> Self {
        // Default to current month/year if no date selected
        let (view_year, view_month) = if let Some((y, m, _)) = initial_date {
            (y, m)
        } else {
            // Default to January 2025 for demo purposes
            (2025, 1)
        };

        Self {
            rect,
            label_size,
            label_color,
            open: false,
            focused: false,
            selected_date: initial_date,
            current_view_month: view_month,
            current_view_year: view_year,
            picker_mode: PickerMode::Days,
            // Default styling - neutral colors like select
            padding_left: 14.0,
            padding_right: 14.0,
            padding_top: 10.0,
            padding_bottom: 10.0,
            bg_color: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
            border_color: ColorLinPremul::from_srgba_u8([200, 208, 216, 255]),
            border_width: 1.0,
            radius: 8.0,
            viewport_height: 800.0, // Default, should be set by caller
        }
    }

    /// Set the viewport height for smart popup positioning
    pub fn set_viewport_height(&mut self, height: f32) {
        self.viewport_height = height;
    }

    /// Calculate popup Y position - opens down by default, up if not enough space below
    fn calculate_popup_y(&self, popup_height: f32) -> f32 {
        let gap = 4.0;
        let space_below = self.viewport_height - (self.rect.y + self.rect.h + gap);
        let space_above = self.rect.y - gap;

        // Default: open downward if enough space
        if space_below >= popup_height {
            self.rect.y + self.rect.h + gap // Below the input
        } else if space_above >= popup_height {
            self.rect.y - popup_height - gap // Above the input
        } else {
            // Not enough space either way, prefer down but clamp
            self.rect.y + self.rect.h + gap
        }
    }

    /// Apply styling from a SurfaceStyle (background/border/padding).
    pub fn apply_surface_style(&mut self, style: &rune_ir::view::SurfaceStyle) {
        if let Some(bg) = &style.background {
            if let rune_ir::view::ViewBackground::Solid { color } = bg {
                if let Some(parsed) = ir_adapter::parse_color(color) {
                    self.bg_color = parsed;
                }
            }
        }

        if let Some(color) = style
            .border_color
            .as_ref()
            .and_then(|c| ir_adapter::parse_color(c))
        {
            self.border_color = color;
        }
        if let Some(width) = style.border_width {
            self.border_width = width as f32;
        }
        if let Some(radius) = style.corner_radius {
            self.radius = radius as f32;
        }

        // Only override padding if explicitly set (non-zero), otherwise keep defaults
        if style.padding.left > 0.0 {
            self.padding_left = style.padding.left as f32;
        }
        if style.padding.right > 0.0 {
            self.padding_right = style.padding.right as f32;
        }
        if style.padding.top > 0.0 {
            self.padding_top = style.padding.top as f32;
        }
        if style.padding.bottom > 0.0 {
            self.padding_bottom = style.padding.bottom as f32;
        }
    }

    /// Format the selected date as a string for display
    fn format_date(&self) -> String {
        if let Some((year, month, day)) = self.selected_date {
            format!("{:02}/{:02}/{:04}", month, day, year)
        } else {
            "Select a date...".to_string()
        }
    }

    /// Get the number of days in a given month/year
    fn days_in_month(year: u32, month: u32) -> u32 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                // Leap year calculation
                if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                    29
                } else {
                    28
                }
            }
            _ => 30, // Fallback
        }
    }

    /// Get the month name
    fn month_name(month: u32) -> &'static str {
        match month {
            1 => "January",
            2 => "February",
            3 => "March",
            4 => "April",
            5 => "May",
            6 => "June",
            7 => "July",
            8 => "August",
            9 => "September",
            10 => "October",
            11 => "November",
            12 => "December",
            _ => "Unknown",
        }
    }

    /// Get the first day of the month (0 = Sunday, 1 = Monday, etc.)
    /// This is a simplified version - in production you'd use a proper date library
    fn first_day_of_month(year: u32, month: u32) -> u32 {
        // Simplified Zeller's congruence algorithm
        let adjusted_month = if month < 3 { month + 12 } else { month };
        let adjusted_year = if month < 3 { year - 1 } else { year };

        let q = 1; // First day of month
        let m = adjusted_month;
        let k = adjusted_year % 100;
        let j = adjusted_year / 100;

        let h = (q + ((13 * (m + 1)) / 5) + k + (k / 4) + (j / 4) - (2 * j)) % 7;

        // Convert from Zeller (0=Saturday) to standard (0=Sunday)
        ((h + 6) % 7) as u32
    }

    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        let rrect = RoundedRect {
            rect: self.rect,
            radii: RoundedRadii {
                tl: self.radius,
                tr: self.radius,
                br: self.radius,
                bl: self.radius,
            },
        };

        // Background from styling
        canvas.rounded_rect(rrect, Brush::Solid(self.bg_color), z);

        // Border - use styling, blue when focused
        let border_color = if self.focused {
            Color::rgba(63, 130, 246, 255)
        } else {
            self.border_color
        };
        let border_width = if self.focused {
            (self.border_width + 1.0).max(2.0)
        } else {
            self.border_width.max(1.0)
        };
        shapes::draw_rounded_rectangle(
            canvas,
            rrect,
            None,
            Some(border_width),
            Some(Brush::Solid(border_color)),
            z + 1,
        );

        // Date text - dark for light background
        let date_text = self.format_date();
        let text_color = if self.selected_date.is_some() {
            Color::rgba(30, 35, 45, 255) // Dark text
        } else {
            Color::rgba(160, 170, 180, 255) // Placeholder color
        };

        let tp = [
            self.rect.x + self.padding_left,
            self.rect.y + self.rect.h * 0.5 + self.label_size * 0.35,
        ];
        canvas.draw_text_run(tp, date_text, self.label_size, text_color, z + 2);

        // Calendar icon (SVG) - dark gray for visibility on light background
        let icon_size = 18.0;
        let icon_x = self.rect.x + self.rect.w - icon_size - self.padding_right;
        let icon_y = self.rect.y + (self.rect.h - icon_size) * 0.5;

        let icon_style = SvgStyle::new()
            .with_stroke(Color::rgba(100, 110, 120, 255))
            .with_stroke_width(2.0);

        canvas.draw_svg_styled(
            "images/calendar.svg",
            [icon_x, icon_y],
            [icon_size, icon_size],
            icon_style,
            z + 3,
        );

        // Render calendar popup when open
        if self.open {
            self.render_calendar_popup(canvas, z + 1000);
        }
    }

    fn render_calendar_popup(&self, canvas: &mut Canvas, z: i32) {
        // Dispatch to appropriate rendering based on picker mode
        match self.picker_mode {
            PickerMode::Days => self.render_days_grid(canvas, z),
            PickerMode::Months => self.render_months_grid(canvas, z),
            PickerMode::Years => self.render_years_grid(canvas, z),
        }
    }

    fn render_days_grid(&self, canvas: &mut Canvas, z: i32) {
        let popup_width = 280.0;
        let popup_height = 334.0; // Reduced: only need 5 rows max
        let header_height = 40.0;
        let day_cell_size = 36.0;
        let button_height = 36.0;
        let button_margin = 8.0;

        // Smart positioning: default down, up if not enough space below
        let popup_rect = Rect {
            x: self.rect.x,
            y: self.calculate_popup_y(popup_height),
            w: popup_width,
            h: popup_height,
        };

        let radius = 8.0;
        let popup_rrect = RoundedRect {
            rect: popup_rect,
            radii: RoundedRadii {
                tl: radius,
                tr: radius,
                br: radius,
                bl: radius,
            },
        };

        // Popup background - white for light theme
        let popup_bg = Color::rgba(255, 255, 255, 255);
        canvas.rounded_rect(popup_rrect, Brush::Solid(popup_bg), z);

        // Border - 1px gray
        let popup_border = Color::rgba(200, 208, 216, 255);
        shapes::draw_rounded_rectangle(
            canvas,
            popup_rrect,
            None,
            Some(1.0),
            Some(Brush::Solid(popup_border)),
            z + 1,
        );

        // Header background - light gray
        let header_bg = Color::rgba(245, 247, 250, 255);
        let header_rect = Rect {
            x: popup_rect.x,
            y: popup_rect.y,
            w: popup_rect.w,
            h: header_height,
        };
        let header_rrect = RoundedRect {
            rect: header_rect,
            radii: RoundedRadii {
                tl: radius,
                tr: radius,
                br: 0.0,
                bl: 0.0,
            },
        };
        canvas.rounded_rect(header_rrect, Brush::Solid(header_bg), z + 2);

        // Month/Year label (centered)
        let month_year_text = format!(
            "{} {}",
            Self::month_name(self.current_view_month),
            self.current_view_year
        );
        let text_size = 14.0;
        // Estimate text width: ~7px per character for proper centering
        let estimated_text_width = month_year_text.len() as f32 * 7.0;
        let header_text_x = popup_rect.x + (popup_rect.w - estimated_text_width) * 0.5;
        let header_text_y = popup_rect.y + header_height * 0.5 + text_size * 0.35;
        canvas.draw_text_run(
            [header_text_x, header_text_y],
            month_year_text,
            text_size,
            Color::rgba(30, 35, 45, 255), // Dark text for light theme
            z + 3,
        );

        // Navigation arrows (previous/next month)
        let arrow_size = 16.0;
        let prev_arrow_x = popup_rect.x + 12.0;
        let next_arrow_x = popup_rect.x + popup_rect.w - arrow_size - 12.0;
        let arrow_y = popup_rect.y + (header_height - arrow_size) * 0.5;

        let arrow_style = SvgStyle::new()
            .with_stroke(Color::rgba(80, 90, 100, 255)) // Dark gray arrows
            .with_stroke_width(2.5);

        canvas.draw_svg_styled(
            "images/chevron-left.svg",
            [prev_arrow_x, arrow_y],
            [arrow_size, arrow_size],
            arrow_style.clone(),
            z + 4,
        );

        canvas.draw_svg_styled(
            "images/chevron-right.svg",
            [next_arrow_x, arrow_y],
            [arrow_size, arrow_size],
            arrow_style,
            z + 4,
        );

        // Day labels (Sun, Mon, Tue, etc.) - centered
        let day_labels = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
        let label_size = 11.0;
        let labels_y = popup_rect.y + header_height + 20.0;
        let grid_start_x = popup_rect.x + 10.0;

        for (i, label) in day_labels.iter().enumerate() {
            // Center text in cell: estimate 2 chars * 5.5px + center offset
            let label_width = label.len() as f32 * 5.5;
            let label_x =
                grid_start_x + (i as f32 * day_cell_size) + (day_cell_size - label_width) * 0.5;
            canvas.draw_text_run(
                [label_x, labels_y],
                label.to_string(),
                label_size,
                Color::rgba(100, 110, 120, 255), // Darker gray for light theme
                z + 3,
            );
        }

        // Calendar grid
        let grid_start_y = popup_rect.y + header_height + 35.0;
        let days_in_month = Self::days_in_month(self.current_view_year, self.current_view_month);
        let first_day = Self::first_day_of_month(self.current_view_year, self.current_view_month);

        let mut day = 1;
        for week in 0..6 {
            for weekday in 0..7 {
                let cell_index = week * 7 + weekday;

                // Skip cells before the first day of the month
                if cell_index < first_day as usize {
                    continue;
                }

                // Stop if we've rendered all days
                if day > days_in_month {
                    break;
                }

                let cell_x = grid_start_x + (weekday as f32 * day_cell_size);
                let cell_y = grid_start_y + (week as f32 * day_cell_size);

                let cell_rect = Rect {
                    x: cell_x,
                    y: cell_y,
                    w: day_cell_size - 2.0,
                    h: day_cell_size - 2.0,
                };

                // Check if this day is selected
                let is_selected = if let Some((sel_year, sel_month, sel_day)) = self.selected_date {
                    sel_year == self.current_view_year
                        && sel_month == self.current_view_month
                        && sel_day == day
                } else {
                    false
                };

                // Highlight selected day
                if is_selected {
                    let cell_rrect = RoundedRect {
                        rect: cell_rect,
                        radii: RoundedRadii {
                            tl: 4.0,
                            tr: 4.0,
                            br: 4.0,
                            bl: 4.0,
                        },
                    };
                    let selected_bg = Color::rgba(63, 130, 246, 255);
                    canvas.rounded_rect(cell_rrect, Brush::Solid(selected_bg), z + 4);
                }

                // Day number (centered in cell)
                let day_text = day.to_string();
                let day_text_size = 13.0;
                // Estimate text width for centering (1 or 2 digits)
                let text_width = day_text.len() as f32 * 6.5;
                let text_x = cell_x + (day_cell_size - text_width) * 0.5;
                let text_y = cell_y + day_cell_size * 0.5 + day_text_size * 0.35;
                let text_color = if is_selected {
                    Color::rgba(255, 255, 255, 255) // White on blue background
                } else {
                    Color::rgba(30, 35, 45, 255) // Dark text for light theme
                };

                canvas.draw_text_run([text_x, text_y], day_text, day_text_size, text_color, z + 5);

                day += 1;
            }
        }

        // Today and Clear buttons at the bottom
        let buttons_y = popup_rect.y + popup_height - button_height - button_margin;
        let button_width = (popup_width - button_margin * 3.0) * 0.5;

        // Today button (left)
        let today_button_rect = Rect {
            x: popup_rect.x + button_margin,
            y: buttons_y,
            w: button_width,
            h: button_height,
        };
        let today_rrect = RoundedRect {
            rect: today_button_rect,
            radii: RoundedRadii {
                tl: 6.0,
                tr: 6.0,
                br: 6.0,
                bl: 6.0,
            },
        };
        let today_bg = Color::rgba(63, 130, 246, 255);
        canvas.rounded_rect(today_rrect, Brush::Solid(today_bg), z + 6);

        // Today button text (centered)
        let today_text = "Today";
        let today_text_size = 13.0;
        let today_text_width = today_text.len() as f32 * 6.5;
        let today_text_x = today_button_rect.x + (button_width - today_text_width) * 0.5;
        let today_text_y = today_button_rect.y + button_height * 0.5 + today_text_size * 0.35;
        canvas.draw_text_run(
            [today_text_x, today_text_y],
            today_text.to_string(),
            today_text_size,
            Color::rgba(255, 255, 255, 255),
            z + 7,
        );

        // Clear button (right)
        let clear_button_rect = Rect {
            x: popup_rect.x + button_margin * 2.0 + button_width,
            y: buttons_y,
            w: button_width,
            h: button_height,
        };
        let clear_rrect = RoundedRect {
            rect: clear_button_rect,
            radii: RoundedRadii {
                tl: 6.0,
                tr: 6.0,
                br: 6.0,
                bl: 6.0,
            },
        };
        let clear_bg = Color::rgba(99, 104, 118, 255);
        canvas.rounded_rect(clear_rrect, Brush::Solid(clear_bg), z + 6);

        // Clear button text (centered)
        let clear_text = "Clear";
        let clear_text_size = 13.0;
        let clear_text_width = clear_text.len() as f32 * 6.5;
        let clear_text_x = clear_button_rect.x + (button_width - clear_text_width) * 0.5;
        let clear_text_y = clear_button_rect.y + button_height * 0.5 + clear_text_size * 0.35;
        canvas.draw_text_run(
            [clear_text_x, clear_text_y],
            clear_text.to_string(),
            clear_text_size,
            Color::rgba(255, 255, 255, 255),
            z + 7,
        );
    }

    fn render_months_grid(&self, canvas: &mut Canvas, z: i32) {
        let popup_width = 280.0;
        let popup_height = 280.0; // Adjusted for 3x4 grid
        let header_height = 40.0;
        let month_cell_width = (popup_width - 32.0) / 3.0; // 3 columns with padding
        let month_cell_height = 45.0;
        let grid_padding = 16.0;

        // Smart positioning: default down, up if not enough space below
        let popup_rect = Rect {
            x: self.rect.x,
            y: self.calculate_popup_y(popup_height),
            w: popup_width,
            h: popup_height,
        };

        let radius = 8.0;
        let popup_rrect = RoundedRect {
            rect: popup_rect,
            radii: RoundedRadii {
                tl: radius,
                tr: radius,
                br: radius,
                bl: radius,
            },
        };

        // Popup background - white for light theme
        let popup_bg = Color::rgba(255, 255, 255, 255);
        canvas.rounded_rect(popup_rrect, Brush::Solid(popup_bg), z);

        // Border - 1px gray
        let popup_border = Color::rgba(200, 208, 216, 255);
        shapes::draw_rounded_rectangle(
            canvas,
            popup_rrect,
            None,
            Some(1.0),
            Some(Brush::Solid(popup_border)),
            z + 1,
        );

        // Header background - light gray
        let header_bg = Color::rgba(245, 247, 250, 255);
        let header_rect = Rect {
            x: popup_rect.x,
            y: popup_rect.y,
            w: popup_rect.w,
            h: header_height,
        };
        let header_rrect = RoundedRect {
            rect: header_rect,
            radii: RoundedRadii {
                tl: radius,
                tr: radius,
                br: 0.0,
                bl: 0.0,
            },
        };
        canvas.rounded_rect(header_rrect, Brush::Solid(header_bg), z + 2);

        // Year label (centered) - clickable to go to years view
        let year_text = format!("{}", self.current_view_year);
        let text_size = 14.0;
        let estimated_text_width = year_text.len() as f32 * 7.0;
        let header_text_x = popup_rect.x + (popup_rect.w - estimated_text_width) * 0.5;
        let header_text_y = popup_rect.y + header_height * 0.5 + text_size * 0.35;
        canvas.draw_text_run(
            [header_text_x, header_text_y],
            year_text,
            text_size,
            Color::rgba(30, 35, 45, 255), // Dark text for light theme
            z + 3,
        );

        // Navigation arrows (previous/next year)
        let arrow_size = 16.0;
        let prev_arrow_x = popup_rect.x + 12.0;
        let next_arrow_x = popup_rect.x + popup_rect.w - arrow_size - 12.0;
        let arrow_y = popup_rect.y + (header_height - arrow_size) * 0.5;

        let arrow_style = SvgStyle::new()
            .with_stroke(Color::rgba(80, 90, 100, 255)) // Dark gray arrows
            .with_stroke_width(2.5);

        canvas.draw_svg_styled(
            "images/chevron-left.svg",
            [prev_arrow_x, arrow_y],
            [arrow_size, arrow_size],
            arrow_style.clone(),
            z + 4,
        );

        canvas.draw_svg_styled(
            "images/chevron-right.svg",
            [next_arrow_x, arrow_y],
            [arrow_size, arrow_size],
            arrow_style,
            z + 4,
        );

        // Month grid (3x4)
        let grid_start_x = popup_rect.x + grid_padding;
        let grid_start_y = popup_rect.y + header_height + grid_padding;

        let month_names = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];

        for (idx, month_name) in month_names.iter().enumerate() {
            let col = idx % 3;
            let row = idx / 3;

            let cell_x = grid_start_x + (col as f32 * month_cell_width);
            let cell_y = grid_start_y + (row as f32 * month_cell_height);

            let cell_rect = Rect {
                x: cell_x,
                y: cell_y,
                w: month_cell_width - 4.0,
                h: month_cell_height - 4.0,
            };

            // Check if this month is selected
            let is_selected = self.current_view_month == (idx + 1) as u32;

            // Highlight selected month
            if is_selected {
                let cell_rrect = RoundedRect {
                    rect: cell_rect,
                    radii: RoundedRadii {
                        tl: 4.0,
                        tr: 4.0,
                        br: 4.0,
                        bl: 4.0,
                    },
                };
                let selected_bg = Color::rgba(63, 130, 246, 255);
                canvas.rounded_rect(cell_rrect, Brush::Solid(selected_bg), z + 4);
            }

            // Month name (centered in cell)
            let month_text_size = 13.0;
            let text_width = month_name.len() as f32 * 6.5;
            let text_x = cell_x + (month_cell_width - text_width) * 0.5;
            let text_y = cell_y + month_cell_height * 0.5 + month_text_size * 0.35;
            let text_color = if is_selected {
                Color::rgba(255, 255, 255, 255) // White on blue background
            } else {
                Color::rgba(30, 35, 45, 255) // Dark text for light theme
            };

            canvas.draw_text_run(
                [text_x, text_y],
                month_name.to_string(),
                month_text_size,
                text_color,
                z + 5,
            );
        }
    }

    fn render_years_grid(&self, canvas: &mut Canvas, z: i32) {
        let popup_width = 280.0;
        let popup_height = 240.0; // Adjusted for 3x3 grid
        let header_height = 40.0;
        let year_cell_width = (popup_width - 32.0) / 3.0; // 3 columns with padding
        let year_cell_height = (popup_height - header_height - 32.0) / 3.0; // 3 rows
        let grid_padding = 16.0;

        // Smart positioning: default down, up if not enough space below
        let popup_rect = Rect {
            x: self.rect.x,
            y: self.calculate_popup_y(popup_height),
            w: popup_width,
            h: popup_height,
        };

        let radius = 8.0;
        let popup_rrect = RoundedRect {
            rect: popup_rect,
            radii: RoundedRadii {
                tl: radius,
                tr: radius,
                br: radius,
                bl: radius,
            },
        };

        // Popup background - white for light theme
        let popup_bg = Color::rgba(255, 255, 255, 255);
        canvas.rounded_rect(popup_rrect, Brush::Solid(popup_bg), z);

        // Border - 1px gray
        let popup_border = Color::rgba(200, 208, 216, 255);
        shapes::draw_rounded_rectangle(
            canvas,
            popup_rrect,
            None,
            Some(1.0),
            Some(Brush::Solid(popup_border)),
            z + 1,
        );

        // Header background - light gray
        let header_bg = Color::rgba(245, 247, 250, 255);
        let header_rect = Rect {
            x: popup_rect.x,
            y: popup_rect.y,
            w: popup_rect.w,
            h: header_height,
        };
        let header_rrect = RoundedRect {
            rect: header_rect,
            radii: RoundedRadii {
                tl: radius,
                tr: radius,
                br: 0.0,
                bl: 0.0,
            },
        };
        canvas.rounded_rect(header_rrect, Brush::Solid(header_bg), z + 2);

        // Year range label (centered)
        let start_year = self.current_view_year - 4;
        let end_year = self.current_view_year + 4;
        let range_text = format!("{} - {}", start_year, end_year);
        let text_size = 14.0;
        let estimated_text_width = range_text.len() as f32 * 7.0;
        let header_text_x = popup_rect.x + (popup_rect.w - estimated_text_width) * 0.5;
        let header_text_y = popup_rect.y + header_height * 0.5 + text_size * 0.35;
        canvas.draw_text_run(
            [header_text_x, header_text_y],
            range_text,
            text_size,
            Color::rgba(30, 35, 45, 255), // Dark text for light theme
            z + 3,
        );

        // Navigation arrows (previous/next 9 years)
        let arrow_size = 16.0;
        let prev_arrow_x = popup_rect.x + 12.0;
        let next_arrow_x = popup_rect.x + popup_rect.w - arrow_size - 12.0;
        let arrow_y = popup_rect.y + (header_height - arrow_size) * 0.5;

        let arrow_style = SvgStyle::new()
            .with_stroke(Color::rgba(80, 90, 100, 255)) // Dark gray arrows
            .with_stroke_width(2.5);

        canvas.draw_svg_styled(
            "images/chevron-left.svg",
            [prev_arrow_x, arrow_y],
            [arrow_size, arrow_size],
            arrow_style.clone(),
            z + 4,
        );

        canvas.draw_svg_styled(
            "images/chevron-right.svg",
            [next_arrow_x, arrow_y],
            [arrow_size, arrow_size],
            arrow_style,
            z + 4,
        );

        // Year grid (3x3, with selected year in the middle)
        let grid_start_x = popup_rect.x + grid_padding;
        let grid_start_y = popup_rect.y + header_height + grid_padding;

        for row in 0..3 {
            for col in 0..3 {
                let idx = row * 3 + col;
                let year = start_year + idx as u32;

                let cell_x = grid_start_x + (col as f32 * year_cell_width);
                let cell_y = grid_start_y + (row as f32 * year_cell_height);

                let cell_rect = Rect {
                    x: cell_x,
                    y: cell_y,
                    w: year_cell_width - 4.0,
                    h: year_cell_height - 4.0,
                };

                // Check if this year is selected
                let is_selected = year == self.current_view_year;

                // Highlight selected year
                if is_selected {
                    let cell_rrect = RoundedRect {
                        rect: cell_rect,
                        radii: RoundedRadii {
                            tl: 4.0,
                            tr: 4.0,
                            br: 4.0,
                            bl: 4.0,
                        },
                    };
                    let selected_bg = Color::rgba(63, 130, 246, 255);
                    canvas.rounded_rect(cell_rrect, Brush::Solid(selected_bg), z + 4);
                }

                // Year text (centered in cell)
                let year_text = format!("{}", year);
                let year_text_size = 13.0;
                let text_width = year_text.len() as f32 * 6.5;
                let text_x = cell_x + (year_cell_width - text_width) * 0.5;
                let text_y = cell_y + year_cell_height * 0.5 + year_text_size * 0.35;
                let text_color = if is_selected {
                    Color::rgba(255, 255, 255, 255) // White on blue background
                } else {
                    Color::rgba(30, 35, 45, 255) // Dark text for light theme
                };

                canvas.draw_text_run(
                    [text_x, text_y],
                    year_text,
                    year_text_size,
                    text_color,
                    z + 5,
                );
            }
        }
    }

    // =========================================================================
    // EVENT HANDLING METHODS
    // =========================================================================

    /// Get the bounds of the popup calendar
    pub fn get_popup_bounds(&self) -> Option<Rect> {
        if !self.open {
            return None;
        }

        let popup_width = 280.0;
        let popup_height = match self.picker_mode {
            PickerMode::Days => 334.0,
            PickerMode::Months => 280.0,
            PickerMode::Years => 240.0,
        };

        Some(Rect {
            x: self.rect.x,
            y: self.calculate_popup_y(popup_height),
            w: popup_width,
            h: popup_height,
        })
    }

    /// Toggle the picker popup open/closed
    pub fn toggle_popup(&mut self) {
        self.open = !self.open;
        if !self.open {
            // Reset to Days view when closing
            self.picker_mode = PickerMode::Days;
        }
    }

    /// Handle click on the date picker field (not the popup)
    /// Returns true if the click was handled
    pub fn handle_field_click(&mut self, x: f32, y: f32) -> bool {
        // Check if click is on the field
        if x >= self.rect.x
            && x <= self.rect.x + self.rect.w
            && y >= self.rect.y
            && y <= self.rect.y + self.rect.h
        {
            self.toggle_popup();
            true
        } else {
            false
        }
    }

    /// Handle click on the popup calendar
    /// Returns true if the click was handled
    pub fn handle_popup_click(&mut self, x: f32, y: f32) -> bool {
        if !self.open {
            return false;
        }

        let popup_bounds = match self.get_popup_bounds() {
            Some(bounds) => bounds,
            None => return false,
        };

        // Check if click is inside popup
        if x < popup_bounds.x
            || x > popup_bounds.x + popup_bounds.w
            || y < popup_bounds.y
            || y > popup_bounds.y + popup_bounds.h
        {
            return false;
        }

        let header_height = 40.0;
        let header_y_max = popup_bounds.y + header_height;

        // Check header clicks
        if y >= popup_bounds.y && y <= header_y_max {
            return self.handle_header_click(x, y, &popup_bounds, header_height);
        }

        // Check grid clicks based on picker mode
        match self.picker_mode {
            PickerMode::Days => self.handle_days_grid_click(x, y, &popup_bounds, header_height),
            PickerMode::Months => self.handle_months_grid_click(x, y, &popup_bounds, header_height),
            PickerMode::Years => self.handle_years_grid_click(x, y, &popup_bounds, header_height),
        }
    }

    fn handle_header_click(
        &mut self,
        x: f32,
        y: f32,
        popup_bounds: &Rect,
        header_height: f32,
    ) -> bool {
        let arrow_size = 16.0;
        let prev_arrow_x = popup_bounds.x + 12.0;
        let next_arrow_x = popup_bounds.x + popup_bounds.w - arrow_size - 12.0;
        let arrow_y = popup_bounds.y + (header_height - arrow_size) * 0.5;

        // Previous arrow click
        if x >= prev_arrow_x
            && x <= prev_arrow_x + arrow_size
            && y >= arrow_y
            && y <= arrow_y + arrow_size
        {
            self.navigate_previous();
            return true;
        }

        // Next arrow click
        if x >= next_arrow_x
            && x <= next_arrow_x + arrow_size
            && y >= arrow_y
            && y <= arrow_y + arrow_size
        {
            self.navigate_next();
            return true;
        }

        // Middle label click (switch to next level: Days → Months → Years)
        let left_arrow_end = popup_bounds.x + 40.0;
        let right_arrow_start = popup_bounds.x + popup_bounds.w - 40.0;
        if x > left_arrow_end && x < right_arrow_start {
            match self.picker_mode {
                PickerMode::Days => {
                    // Days → Months
                    self.picker_mode = PickerMode::Months;
                    return true;
                }
                PickerMode::Months => {
                    // Months → Years
                    self.picker_mode = PickerMode::Years;
                    return true;
                }
                PickerMode::Years => {
                    // Already in years view, do nothing
                    return true;
                }
            }
        }

        false
    }

    fn handle_days_grid_click(
        &mut self,
        x: f32,
        y: f32,
        popup_bounds: &Rect,
        header_height: f32,
    ) -> bool {
        let day_cell_size = 36.0;
        let button_height = 36.0;
        let button_margin = 8.0;
        let grid_start_x = 10.0;
        let grid_start_y = header_height + 35.0;

        let cell_local_x = x - popup_bounds.x - grid_start_x;
        let cell_local_y = y - popup_bounds.y - grid_start_y;

        // Check day grid clicks
        if cell_local_x >= 0.0 && cell_local_y >= 0.0 {
            let col = (cell_local_x / day_cell_size) as usize;
            let row = (cell_local_y / day_cell_size) as usize;

            if col < 7 && row < 6 {
                let days_in_month =
                    Self::days_in_month(self.current_view_year, self.current_view_month);
                let first_day =
                    Self::first_day_of_month(self.current_view_year, self.current_view_month);
                let cell_index = row * 7 + col;

                if cell_index >= first_day as usize {
                    let day = (cell_index - first_day as usize + 1) as u32;
                    if day <= days_in_month {
                        self.selected_date =
                            Some((self.current_view_year, self.current_view_month, day));
                        self.open = false;
                        self.picker_mode = PickerMode::Days;
                        return true;
                    }
                }
            }
        }

        // Check Today button
        let buttons_y = popup_bounds.y + popup_bounds.h - button_height - button_margin;
        let button_width = (popup_bounds.w - button_margin * 3.0) * 0.5;
        let today_button_x = popup_bounds.x + button_margin;

        if x >= today_button_x
            && x <= today_button_x + button_width
            && y >= buttons_y
            && y <= buttons_y + button_height
        {
            // Set to today's date (hardcoded for demo - in production, use system time)
            self.selected_date = Some((2025, 11, 18));
            self.current_view_month = 11;
            self.current_view_year = 2025;
            self.open = false;
            self.picker_mode = PickerMode::Days;
            return true;
        }

        // Check Clear button
        let clear_button_x = popup_bounds.x + button_margin * 2.0 + button_width;
        if x >= clear_button_x
            && x <= clear_button_x + button_width
            && y >= buttons_y
            && y <= buttons_y + button_height
        {
            self.selected_date = None;
            self.open = false;
            self.picker_mode = PickerMode::Days;
            return true;
        }

        false
    }

    fn handle_months_grid_click(
        &mut self,
        x: f32,
        y: f32,
        popup_bounds: &Rect,
        header_height: f32,
    ) -> bool {
        let month_cell_width = (popup_bounds.w - 32.0) / 3.0;
        let month_cell_height = 45.0;
        let grid_padding = 16.0;
        let grid_start_x = grid_padding;
        let grid_start_y = header_height + grid_padding;

        let cell_local_x = x - popup_bounds.x - grid_start_x;
        let cell_local_y = y - popup_bounds.y - grid_start_y;

        if cell_local_x >= 0.0 && cell_local_y >= 0.0 {
            let col = (cell_local_x / month_cell_width) as usize;
            let row = (cell_local_y / month_cell_height) as usize;

            if col < 3 && row < 4 {
                let month = (row * 3 + col + 1) as u32;
                self.current_view_month = month;
                self.picker_mode = PickerMode::Days;
                return true;
            }
        }

        false
    }

    fn handle_years_grid_click(
        &mut self,
        x: f32,
        y: f32,
        popup_bounds: &Rect,
        header_height: f32,
    ) -> bool {
        let year_cell_size = 70.0;
        let grid_padding = 16.0;
        let grid_start_x = grid_padding;
        let grid_start_y = header_height + grid_padding;

        let cell_local_x = x - popup_bounds.x - grid_start_x;
        let cell_local_y = y - popup_bounds.y - grid_start_y;

        if cell_local_x >= 0.0 && cell_local_y >= 0.0 {
            let col = (cell_local_x / year_cell_size) as usize;
            let row = (cell_local_y / year_cell_size) as usize;

            if col < 3 && row < 3 {
                let idx = row * 3 + col;
                let start_year = self.current_view_year - 4;
                let year = start_year + idx as u32;
                self.current_view_year = year;
                self.picker_mode = PickerMode::Months;
                return true;
            }
        }

        false
    }

    /// Navigate to previous month/year/decade based on current picker mode
    pub fn navigate_previous(&mut self) {
        match self.picker_mode {
            PickerMode::Days => {
                if self.current_view_month == 1 {
                    self.current_view_month = 12;
                    self.current_view_year -= 1;
                } else {
                    self.current_view_month -= 1;
                }
            }
            PickerMode::Months => {
                self.current_view_year -= 1;
            }
            PickerMode::Years => {
                self.current_view_year -= 9;
            }
        }
    }

    /// Navigate to next month/year/decade based on current picker mode
    pub fn navigate_next(&mut self) {
        match self.picker_mode {
            PickerMode::Days => {
                if self.current_view_month == 12 {
                    self.current_view_month = 1;
                    self.current_view_year += 1;
                } else {
                    self.current_view_month += 1;
                }
            }
            PickerMode::Months => {
                self.current_view_year += 1;
            }
            PickerMode::Years => {
                self.current_view_year += 9;
            }
        }
    }

    /// Close the popup and reset to Days view
    pub fn close_popup(&mut self) {
        self.open = false;
        self.picker_mode = PickerMode::Days;
    }

    /// Handle keyboard input for the date picker
    /// Returns DatePickerKeyResult indicating what action was taken
    pub fn handle_keyboard(&mut self, key: DatePickerKey) -> DatePickerKeyResult {
        match key {
            DatePickerKey::ArrowLeft => {
                self.navigate_previous();
                DatePickerKeyResult::Navigated
            }
            DatePickerKey::ArrowRight => {
                self.navigate_next();
                DatePickerKeyResult::Navigated
            }
            DatePickerKey::Escape => {
                self.close_popup();
                DatePickerKeyResult::Closed
            }
            DatePickerKey::Other => DatePickerKeyResult::Ignored,
        }
    }

    // ===== Utility Methods =====

    /// Check if this date picker is focused
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state for this date picker
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Check if a point is inside this date picker (field or popup)
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        // Check if click is on the field
        if x >= self.rect.x
            && x <= self.rect.x + self.rect.w
            && y >= self.rect.y
            && y <= self.rect.y + self.rect.h
        {
            return true;
        }

        // Check if click is on the popup (if open)
        if let Some(popup_bounds) = self.get_popup_bounds() {
            if x >= popup_bounds.x
                && x <= popup_bounds.x + popup_bounds.w
                && y >= popup_bounds.y
                && y <= popup_bounds.y + popup_bounds.h
            {
                return true;
            }
        }

        false
    }

    /// Check if the popup is currently open
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Get the selected date
    pub fn get_selected_date(&self) -> Option<(u32, u32, u32)> {
        self.selected_date
    }

    /// Set the selected date
    pub fn set_selected_date(&mut self, date: Option<(u32, u32, u32)>) {
        self.selected_date = date;
        if let Some((year, month, _)) = date {
            self.current_view_year = year;
            self.current_view_month = month;
        }
    }
}

/// Keys that the date picker can handle
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DatePickerKey {
    ArrowLeft,
    ArrowRight,
    Escape,
    Other,
}

/// Result of date picker keyboard handling
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DatePickerKeyResult {
    /// Navigation occurred (previous/next)
    Navigated,
    /// Picker was closed
    Closed,
    /// Key was not handled
    Ignored,
}

// ===== EventHandler Trait Implementation =====

impl crate::event_handler::EventHandler for DatePicker {
    /// Handle mouse click event
    ///
    /// Handles clicks on the field (to toggle popup) and popup elements.
    fn handle_mouse_click(
        &mut self,
        event: crate::event_handler::MouseClickEvent,
    ) -> crate::event_handler::EventResult {
        use winit::event::ElementState;

        // Only handle left mouse button press
        if event.button != winit::event::MouseButton::Left || event.state != ElementState::Pressed {
            return crate::event_handler::EventResult::Ignored;
        }

        // Try handling popup click first (if open)
        if self.open && self.handle_popup_click(event.x, event.y) {
            return crate::event_handler::EventResult::Handled;
        }

        // Try handling field click (to toggle popup)
        if self.handle_field_click(event.x, event.y) {
            return crate::event_handler::EventResult::Handled;
        }

        crate::event_handler::EventResult::Ignored
    }

    /// Handle keyboard input event
    ///
    /// Arrow keys navigate, Escape closes the popup.
    fn handle_keyboard(
        &mut self,
        event: crate::event_handler::KeyboardEvent,
    ) -> crate::event_handler::EventResult {
        use winit::event::ElementState;
        use winit::keyboard::KeyCode;

        // Only handle key press, not release
        if event.state != ElementState::Pressed {
            return crate::event_handler::EventResult::Ignored;
        }

        // Only handle keyboard events if focused or popup is open
        if !self.focused && !self.open {
            return crate::event_handler::EventResult::Ignored;
        }

        // Map KeyCode to DatePickerKey
        let picker_key = match event.key {
            KeyCode::ArrowLeft => DatePickerKey::ArrowLeft,
            KeyCode::ArrowRight => DatePickerKey::ArrowRight,
            KeyCode::Escape => DatePickerKey::Escape,
            _ => DatePickerKey::Other,
        };

        // Handle the key
        match self.handle_keyboard(picker_key) {
            DatePickerKeyResult::Navigated => crate::event_handler::EventResult::Handled,
            DatePickerKeyResult::Closed => crate::event_handler::EventResult::Handled,
            DatePickerKeyResult::Ignored => crate::event_handler::EventResult::Ignored,
        }
    }

    /// Check if this date picker is focused
    fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state for this date picker
    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Check if the point is inside this date picker
    fn contains_point(&self, x: f32, y: f32) -> bool {
        self.contains_point(x, y)
    }
}
