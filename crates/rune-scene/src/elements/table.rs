use engine_core::{Brush, ColorLinPremul, Rect};
use rune_surface::Canvas;

/// A single cell in the table
#[derive(Debug, Clone)]
pub struct TableCell {
    /// Cell text content
    pub text: String,
    /// Optional custom text color for this cell
    pub color: Option<ColorLinPremul>,
    /// Optional custom text size for this cell
    pub size: Option<f32>,
}

impl TableCell {
    /// Create a new table cell with text
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: None,
            size: None,
        }
    }

    /// Set custom color for this cell
    pub fn with_color(mut self, color: ColorLinPremul) -> Self {
        self.color = Some(color);
        self
    }

    /// Set custom size for this cell
    pub fn with_size(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }
}

impl<S: Into<String>> From<S> for TableCell {
    fn from(s: S) -> Self {
        TableCell::new(s)
    }
}

/// A row in the table (can be header, data, or footer)
#[derive(Debug, Clone)]
pub struct TableRow {
    /// Cells in this row
    pub cells: Vec<TableCell>,
    /// Optional background color for this row
    pub bg_color: Option<ColorLinPremul>,
}

impl TableRow {
    /// Create a new table row
    pub fn new(cells: Vec<TableCell>) -> Self {
        Self {
            cells,
            bg_color: None,
        }
    }

    /// Set background color for this row
    pub fn with_bg_color(mut self, color: ColorLinPremul) -> Self {
        self.bg_color = Some(color);
        self
    }
}

/// Column alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Left,
    Center,
    Right,
}

/// Column configuration
#[derive(Debug, Clone)]
pub struct Column {
    /// Column header text
    pub header: String,
    /// Column width (None = auto-width, equal distribution)
    pub width: Option<f32>,
    /// Text alignment for this column
    pub alignment: Alignment,
}

impl Column {
    /// Create a new column with default settings
    pub fn new(header: impl Into<String>) -> Self {
        Self {
            header: header.into(),
            width: None,
            alignment: Alignment::Left,
        }
    }

    /// Set column width
    pub fn with_width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Set column alignment
    pub fn with_alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
}

/// Table element for displaying structured tabular data
///
/// Displays data in rows and columns with optional caption, header, and footer.
pub struct Table {
    /// Position and size of the table
    pub rect: Rect,
    /// Optional caption displayed above the table
    pub caption: Option<String>,
    /// Column definitions
    pub columns: Vec<Column>,
    /// Data rows
    pub rows: Vec<TableRow>,
    /// Optional footer row
    pub footer: Option<TableRow>,
    /// Table background color
    pub bg_color: ColorLinPremul,
    /// Header background color
    pub header_bg_color: ColorLinPremul,
    /// Header text color
    pub header_text_color: ColorLinPremul,
    /// Header text size
    pub header_text_size: f32,
    /// Default cell text color
    pub cell_text_color: ColorLinPremul,
    /// Default cell text size
    pub cell_text_size: f32,
    /// Footer background color
    pub footer_bg_color: ColorLinPremul,
    /// Footer text color
    pub footer_text_color: ColorLinPremul,
    /// Footer text size
    pub footer_text_size: f32,
    /// Caption text color
    pub caption_color: ColorLinPremul,
    /// Caption text size
    pub caption_size: f32,
    /// Border color
    pub border_color: ColorLinPremul,
    /// Border width
    pub border_width: f32,
    /// Enable zebra striping (alternating row colors)
    pub zebra_striping: bool,
    /// Zebra stripe color (for odd rows)
    pub zebra_color: ColorLinPremul,
    /// Corner radius for outer border/background
    pub corner_radius: f32,
    /// Row height
    pub row_height: f32,
    /// Cell padding (horizontal)
    pub cell_padding_x: f32,
    /// Cell padding (vertical)
    pub cell_padding_y: f32,
    /// Show horizontal grid lines
    pub show_horizontal_lines: bool,
    /// Show vertical grid lines
    pub show_vertical_lines: bool,
}

impl Table {
    /// Create a new table with default styling
    pub fn new(rect: Rect, columns: Vec<Column>) -> Self {
        Self {
            rect,
            caption: None,
            columns,
            rows: Vec::new(),
            footer: None,
            bg_color: ColorLinPremul::from_srgba_u8([255, 255, 255, 255]),
            header_bg_color: ColorLinPremul::from_srgba_u8([248, 248, 248, 255]),
            header_text_color: ColorLinPremul::from_srgba_u8([60, 60, 60, 255]),
            header_text_size: 14.0,
            cell_text_color: ColorLinPremul::from_srgba_u8([80, 80, 80, 255]),
            cell_text_size: 14.0,
            footer_bg_color: ColorLinPremul::from_srgba_u8([248, 248, 248, 255]),
            footer_text_color: ColorLinPremul::from_srgba_u8([60, 60, 60, 255]),
            footer_text_size: 14.0,
            caption_color: ColorLinPremul::from_srgba_u8([40, 40, 40, 255]),
            caption_size: 16.0,
            border_color: ColorLinPremul::from_srgba_u8([224, 224, 224, 255]),
            border_width: 1.0,
            zebra_striping: false,
            zebra_color: ColorLinPremul::from_srgba_u8([249, 249, 249, 255]),
            corner_radius: 0.0,
            row_height: 44.0,
            cell_padding_x: 16.0,
            cell_padding_y: 12.0,
            show_horizontal_lines: true,
            show_vertical_lines: true,
        }
    }

    /// Set the table caption
    pub fn with_caption(mut self, caption: String) -> Self {
        self.caption = Some(caption);
        self
    }

    /// Set the table rows
    pub fn with_rows(mut self, rows: Vec<TableRow>) -> Self {
        self.rows = rows;
        self
    }

    /// Add a single row
    pub fn add_row(&mut self, row: TableRow) {
        self.rows.push(row);
    }

    /// Set the footer row
    pub fn with_footer(mut self, footer: TableRow) -> Self {
        self.footer = Some(footer);
        self
    }

    /// Enable zebra striping
    pub fn with_zebra_striping(mut self, enabled: bool) -> Self {
        self.zebra_striping = enabled;
        self
    }

    /// Calculate column widths based on configuration
    fn calculate_column_widths(&self) -> Vec<f32> {
        let total_width = self.rect.w - self.border_width * 2.0;
        let num_columns = self.columns.len();

        if num_columns == 0 {
            return vec![];
        }

        // Check if any columns have fixed widths
        let fixed_width_sum: f32 = self.columns.iter().filter_map(|col| col.width).sum();

        let num_auto_columns = self
            .columns
            .iter()
            .filter(|col| col.width.is_none())
            .count();

        let auto_width = if num_auto_columns > 0 {
            (total_width - fixed_width_sum) / num_auto_columns as f32
        } else {
            0.0
        };

        self.columns
            .iter()
            .map(|col| col.width.unwrap_or(auto_width))
            .collect()
    }

    /// Calculate text X position based on alignment
    fn calculate_text_x(
        &self,
        x_start: f32,
        col_width: f32,
        text: &str,
        text_size: f32,
        alignment: Alignment,
    ) -> f32 {
        match alignment {
            Alignment::Left => x_start + self.cell_padding_x,
            Alignment::Center => {
                let approx_text_width = text.len() as f32 * text_size * 0.5;
                x_start + (col_width - approx_text_width) * 0.5
            }
            Alignment::Right => {
                let approx_text_width = text.len() as f32 * text_size * 0.5;
                x_start + col_width - approx_text_width - self.cell_padding_x
            }
        }
    }

    /// Render the table
    pub fn render(&self, canvas: &mut Canvas, z: i32) {
        let mut current_y = self.rect.y;

        // Render caption if present
        if let Some(ref caption_text) = self.caption {
            let caption_y = current_y + self.caption_size;
            canvas.draw_text_run(
                [self.rect.x, caption_y],
                caption_text.clone(),
                self.caption_size,
                self.caption_color,
                z,
            );
            current_y += self.caption_size + self.cell_padding_y;
        }

        let table_start_y = current_y;
        let column_widths = self.calculate_column_widths();

        // Render table background and border (rounded if requested)
        if self.corner_radius > 0.0 {
            let rrect = engine_core::RoundedRect {
                rect: Rect {
                    x: self.rect.x,
                    y: table_start_y,
                    w: self.rect.w,
                    h: self.rect.h - (table_start_y - self.rect.y),
                },
                radii: engine_core::RoundedRadii {
                    tl: self.corner_radius,
                    tr: self.corner_radius,
                    br: self.corner_radius,
                    bl: self.corner_radius,
                },
            };
            rune_surface::shapes::draw_rounded_rectangle(
                canvas,
                rrect,
                Some(Brush::Solid(self.bg_color)),
                Some(self.border_width),
                Some(Brush::Solid(self.border_color)),
                z,
            );
        } else {
            canvas.fill_rect(
                self.rect.x,
                table_start_y,
                self.rect.w,
                self.rect.h - (table_start_y - self.rect.y),
                Brush::Solid(self.bg_color),
                z,
            );

            // Render outer border (top, right, bottom, left)
            if self.border_width > 0.0 {
                let border_rect = Rect {
                    x: self.rect.x,
                    y: table_start_y,
                    w: self.rect.w,
                    h: self.rect.h - (table_start_y - self.rect.y),
                };

                let border_style = rune_surface::shapes::RectStyle {
                    fill: None,
                    border: Some(rune_surface::shapes::BorderStyle {
                        widths: rune_surface::shapes::BorderWidths {
                            top: self.border_width,
                            right: self.border_width,
                            bottom: self.border_width,
                            left: self.border_width,
                        },
                        brush: Brush::Solid(self.border_color),
                    }),
                };

                rune_surface::shapes::draw_rectangle(
                    canvas,
                    border_rect.x,
                    border_rect.y,
                    border_rect.w,
                    border_rect.h,
                    &border_style,
                    z + 1,
                );
            }
        }

        // Render header row
        if !self.columns.is_empty() {
            let header_rect = Rect {
                x: self.rect.x + self.border_width,
                y: current_y,
                w: self.rect.w - self.border_width * 2.0,
                h: self.row_height,
            };

            // Use rounded top corners for the header fill when corner radius is set.
            if self.corner_radius > 0.0 {
                let header_radius = engine_core::RoundedRadii {
                    tl: self.corner_radius,
                    tr: self.corner_radius,
                    br: 0.0,
                    bl: 0.0,
                };
                rune_surface::shapes::draw_rounded_rectangle(
                    canvas,
                    engine_core::RoundedRect {
                        rect: Rect {
                            x: self.rect.x,
                            y: header_rect.y,
                            w: self.rect.w,
                            h: header_rect.h,
                        },
                        radii: header_radius,
                    },
                    Some(Brush::Solid(self.header_bg_color)),
                    None,
                    None,
                    z + 2,
                );
            } else {
                canvas.fill_rect(
                    header_rect.x,
                    header_rect.y,
                    header_rect.w,
                    header_rect.h,
                    Brush::Solid(self.header_bg_color),
                    z + 2,
                );
            }

            let mut x_offset = self.rect.x + self.border_width;
            for (col_idx, column) in self.columns.iter().enumerate() {
                let col_width = column_widths[col_idx];
                let text_x = self.calculate_text_x(
                    x_offset,
                    col_width,
                    &column.header,
                    self.header_text_size,
                    column.alignment,
                );
                let text_y = current_y + self.row_height * 0.5 + self.header_text_size * 0.35;

                canvas.draw_text_run(
                    [text_x, text_y],
                    column.header.clone(),
                    self.header_text_size,
                    self.header_text_color,
                    z + 3,
                );

                // Render vertical grid line
                if self.show_vertical_lines && col_idx < self.columns.len() - 1 {
                    canvas.fill_rect(
                        x_offset + col_width,
                        current_y,
                        self.border_width,
                        self.row_height,
                        Brush::Solid(self.border_color),
                        z + 3,
                    );
                }

                x_offset += col_width;
            }

            // Horizontal line after header
            if self.show_horizontal_lines {
                canvas.fill_rect(
                    self.rect.x + self.border_width,
                    current_y + self.row_height,
                    self.rect.w - self.border_width * 2.0,
                    self.border_width,
                    Brush::Solid(self.border_color),
                    z + 3,
                );
            }

            current_y += self.row_height;
        }

        // Render data rows
        for (row_idx, row) in self.rows.iter().enumerate() {
            // Zebra striping
            if self.zebra_striping && row_idx % 2 == 1 {
                canvas.fill_rect(
                    self.rect.x + self.border_width,
                    current_y,
                    self.rect.w - self.border_width * 2.0,
                    self.row_height,
                    Brush::Solid(self.zebra_color),
                    z + 2,
                );
            }

            // Custom row background
            if let Some(row_bg) = row.bg_color {
                canvas.fill_rect(
                    self.rect.x + self.border_width,
                    current_y,
                    self.rect.w - self.border_width * 2.0,
                    self.row_height,
                    Brush::Solid(row_bg),
                    z + 2,
                );
            }

            let mut x_offset = self.rect.x + self.border_width;
            for (col_idx, cell) in row.cells.iter().enumerate() {
                if col_idx >= self.columns.len() {
                    break;
                }

                let col_width = column_widths[col_idx];
                let alignment = self.columns[col_idx].alignment;
                let text_size = cell.size.unwrap_or(self.cell_text_size);
                let text_color = cell.color.unwrap_or(self.cell_text_color);

                let text_x =
                    self.calculate_text_x(x_offset, col_width, &cell.text, text_size, alignment);
                let text_y = current_y + self.row_height * 0.5 + text_size * 0.35;

                canvas.draw_text_run(
                    [text_x, text_y],
                    cell.text.clone(),
                    text_size,
                    text_color,
                    z + 3,
                );

                // Render vertical grid line
                if self.show_vertical_lines && col_idx < self.columns.len() - 1 {
                    canvas.fill_rect(
                        x_offset + col_width,
                        current_y,
                        self.border_width,
                        self.row_height,
                        Brush::Solid(self.border_color),
                        z + 3,
                    );
                }

                x_offset += col_width;
            }

            // Horizontal line after row
            if self.show_horizontal_lines && row_idx < self.rows.len() - 1 {
                canvas.fill_rect(
                    self.rect.x + self.border_width,
                    current_y + self.row_height,
                    self.rect.w - self.border_width * 2.0,
                    self.border_width,
                    Brush::Solid(self.border_color),
                    z + 3,
                );
            }

            current_y += self.row_height;

            // Stop rendering if we exceed the table height
            if current_y > self.rect.y + self.rect.h {
                break;
            }
        }

        // Render footer if present
        if let Some(ref footer_row) = self.footer {
            // Horizontal line before footer
            if self.show_horizontal_lines {
                canvas.fill_rect(
                    self.rect.x + self.border_width,
                    current_y,
                    self.rect.w - self.border_width * 2.0,
                    self.border_width,
                    Brush::Solid(self.border_color),
                    z + 3,
                );
            }

            canvas.fill_rect(
                self.rect.x + self.border_width,
                current_y,
                self.rect.w - self.border_width * 2.0,
                self.row_height,
                Brush::Solid(self.footer_bg_color),
                z + 2,
            );

            let mut x_offset = self.rect.x + self.border_width;
            for (col_idx, cell) in footer_row.cells.iter().enumerate() {
                if col_idx >= self.columns.len() {
                    break;
                }

                let col_width = column_widths[col_idx];
                let alignment = self.columns[col_idx].alignment;
                let text_size = cell.size.unwrap_or(self.footer_text_size);
                let text_color = cell.color.unwrap_or(self.footer_text_color);

                let text_x =
                    self.calculate_text_x(x_offset, col_width, &cell.text, text_size, alignment);
                let text_y = current_y + self.row_height * 0.5 + text_size * 0.35;

                canvas.draw_text_run(
                    [text_x, text_y],
                    cell.text.clone(),
                    text_size,
                    text_color,
                    z + 3,
                );

                // Render vertical grid line
                if self.show_vertical_lines && col_idx < self.columns.len() - 1 {
                    canvas.fill_rect(
                        x_offset + col_width,
                        current_y,
                        self.border_width,
                        self.row_height,
                        Brush::Solid(self.border_color),
                        z + 3,
                    );
                }

                x_offset += col_width;
            }
        }
    }
}
