# Table Element Usage Examples

The `Table` element provides a structured, customizable table component for displaying tabular data with headers, rows, optional footer, and caption.

## Basic Table

```rust
use rune_scene::elements::{Table, Column, TableRow, TableCell};
use engine_core::Rect;

// Define columns
let columns = vec![
    Column::new("Status"),
    Column::new("Email"),
    Column::new("Amount"),
];

// Create table
let mut table = Table::new(
    Rect {
        x: 100.0,
        y: 100.0,
        w: 800.0,
        h: 400.0,
    },
    columns,
);

// Add data rows
table.add_row(TableRow::new(vec![
    TableCell::new("Success"),
    TableCell::new("ken99@example.com"),
    TableCell::new("$316.00"),
]));

table.add_row(TableRow::new(vec![
    TableCell::new("Success"),
    TableCell::new("abe45@example.com"),
    TableCell::new("$242.00"),
]));

table.add_row(TableRow::new(vec![
    TableCell::new("Processing"),
    TableCell::new("monserrat44@example.com"),
    TableCell::new("$837.00"),
]));

// Render
table.render(&mut canvas, z_index);
```

## Table with Caption

```rust
let table = Table::new(rect, columns)
    .with_caption("Payment Transactions".to_string());
```

## Table with Footer

```rust
use rune_scene::elements::{Table, TableRow, TableCell};

let footer = TableRow::new(vec![
    TableCell::new("Total"),
    TableCell::new(""),
    TableCell::new("$2,190.00"),
]);

let table = Table::new(rect, columns)
    .with_footer(footer);
```

## Column Configuration

### Custom Column Widths

```rust
use rune_scene::elements::Column;

let columns = vec![
    Column::new("Status").with_width(150.0),
    Column::new("Email").with_width(300.0),
    Column::new("Amount").with_width(120.0),
];

// Columns without explicit width will share remaining space equally
```

### Column Alignment

```rust
use rune_scene::elements::{Column, Alignment};

let columns = vec![
    Column::new("Status").with_alignment(Alignment::Left),
    Column::new("Email").with_alignment(Alignment::Left),
    Column::new("Amount").with_alignment(Alignment::Right),
];
```

### Combined Configuration

```rust
let columns = vec![
    Column::new("ID")
        .with_width(80.0)
        .with_alignment(Alignment::Center),
    Column::new("Name")
        .with_width(200.0)
        .with_alignment(Alignment::Left),
    Column::new("Price")
        .with_width(120.0)
        .with_alignment(Alignment::Right),
];
```

## Custom Cell Styling

```rust
use engine_core::ColorLinPremul;

// Cell with custom color
let cell = TableCell::new("Failed")
    .with_color(ColorLinPremul::from_srgba_u8([220, 38, 38, 255])); // Red

// Cell with custom size
let cell = TableCell::new("Important")
    .with_size(16.0);

// Combined
let cell = TableCell::new("Success")
    .with_color(ColorLinPremul::from_srgba_u8([34, 197, 94, 255])) // Green
    .with_size(14.0);
```

## Row Styling

```rust
use engine_core::ColorLinPremul;

// Row with custom background
let row = TableRow::new(vec![
    TableCell::new("Highlighted"),
    TableCell::new("data@example.com"),
    TableCell::new("$500.00"),
])
.with_bg_color(ColorLinPremul::from_srgba_u8([254, 243, 199, 255])); // Yellow tint
```

## Zebra Striping

```rust
let mut table = Table::new(rect, columns);
table.zebra_striping = true;
table.zebra_color = ColorLinPremul::from_srgba_u8([249, 249, 249, 255]);
```

## Grid Lines

```rust
let mut table = Table::new(rect, columns);

// Show/hide horizontal lines
table.show_horizontal_lines = true;

// Show/hide vertical lines
table.show_vertical_lines = true;

// Hide all grid lines for a cleaner look
table.show_horizontal_lines = false;
table.show_vertical_lines = false;
```

## Custom Colors

```rust
use engine_core::ColorLinPremul;

let mut table = Table::new(rect, columns);

// Table background
table.bg_color = ColorLinPremul::from_srgba_u8([255, 255, 255, 255]);

// Header styling
table.header_bg_color = ColorLinPremul::from_srgba_u8([248, 248, 248, 255]);
table.header_text_color = ColorLinPremul::from_srgba_u8([60, 60, 60, 255]);
table.header_text_size = 14.0;

// Cell styling
table.cell_text_color = ColorLinPremul::from_srgba_u8([80, 80, 80, 255]);
table.cell_text_size = 14.0;

// Footer styling
table.footer_bg_color = ColorLinPremul::from_srgba_u8([248, 248, 248, 255]);
table.footer_text_color = ColorLinPremul::from_srgba_u8([60, 60, 60, 255]);
table.footer_text_size = 14.0;

// Border styling
table.border_color = ColorLinPremul::from_srgba_u8([224, 224, 224, 255]);
table.border_width = 1.0;
```

## Spacing and Padding

```rust
let mut table = Table::new(rect, columns);

// Row height
table.row_height = 50.0;

// Cell padding
table.cell_padding_x = 20.0; // Horizontal
table.cell_padding_y = 15.0; // Vertical
```

## Complete Example (Matching Screenshot)

```rust
use rune_scene::elements::{Table, Column, TableRow, TableCell, Alignment};
use engine_core::{ColorLinPremul, Rect};

// Define columns
let columns = vec![
    Column::new("Status").with_alignment(Alignment::Left),
    Column::new("Email").with_alignment(Alignment::Left),
    Column::new("Amount").with_alignment(Alignment::Right),
];

// Create table
let mut table = Table::new(
    Rect {
        x: 30.0,
        y: 30.0,
        w: 1200.0,
        h: 500.0,
    },
    columns,
);

// Add rows
table.add_row(TableRow::new(vec![
    TableCell::new("Success"),
    TableCell::new("ken99@example.com"),
    TableCell::new("$316.00"),
]));

table.add_row(TableRow::new(vec![
    TableCell::new("Success"),
    TableCell::new("abe45@example.com"),
    TableCell::new("$242.00"),
]));

table.add_row(TableRow::new(vec![
    TableCell::new("Processing"),
    TableCell::new("monserrat44@example.com"),
    TableCell::new("$837.00"),
]));

table.add_row(TableRow::new(vec![
    TableCell::new("Success"),
    TableCell::new("silas22@example.com"),
    TableCell::new("$874.00"),
]));

table.add_row(TableRow::new(vec![
    TableCell::new("Failed")
        .with_color(ColorLinPremul::from_srgba_u8([220, 38, 38, 255])),
    TableCell::new("carmella@example.com"),
    TableCell::new("$721.00"),
]));

// Configure styling
table.bg_color = ColorLinPremul::from_srgba_u8([255, 255, 255, 255]);
table.header_bg_color = ColorLinPremul::from_srgba_u8([250, 250, 250, 255]);
table.border_color = ColorLinPremul::from_srgba_u8([224, 224, 224, 255]);
table.row_height = 60.0;

// Render
table.render(&mut canvas, z_index);
```

## Building Rows Dynamically

```rust
// From data structures
struct Transaction {
    status: String,
    email: String,
    amount: f64,
}

let transactions = vec![
    Transaction {
        status: "Success".to_string(),
        email: "user@example.com".to_string(),
        amount: 150.00,
    },
    // ... more transactions
];

let rows: Vec<TableRow> = transactions
    .iter()
    .map(|t| {
        TableRow::new(vec![
            TableCell::new(&t.status),
            TableCell::new(&t.email),
            TableCell::new(format!("${:.2}", t.amount)),
        ])
    })
    .collect();

let table = Table::new(rect, columns).with_rows(rows);
```

## API Reference

### Table

```rust
pub struct Table {
    pub rect: Rect,
    pub caption: Option<String>,
    pub columns: Vec<Column>,
    pub rows: Vec<TableRow>,
    pub footer: Option<TableRow>,
    pub bg_color: ColorLinPremul,
    pub header_bg_color: ColorLinPremul,
    pub header_text_color: ColorLinPremul,
    pub header_text_size: f32,
    pub cell_text_color: ColorLinPremul,
    pub cell_text_size: f32,
    pub footer_bg_color: ColorLinPremul,
    pub footer_text_color: ColorLinPremul,
    pub footer_text_size: f32,
    pub caption_color: ColorLinPremul,
    pub caption_size: f32,
    pub border_color: ColorLinPremul,
    pub border_width: f32,
    pub zebra_striping: bool,
    pub zebra_color: ColorLinPremul,
    pub row_height: f32,
    pub cell_padding_x: f32,
    pub cell_padding_y: f32,
    pub show_horizontal_lines: bool,
    pub show_vertical_lines: bool,
}
```

#### Methods

- `Table::new(rect: Rect, columns: Vec<Column>) -> Self`
- `.with_caption(caption: String) -> Self`
- `.with_rows(rows: Vec<TableRow>) -> Self`
- `.add_row(&mut self, row: TableRow)`
- `.with_footer(footer: TableRow) -> Self`
- `.with_zebra_striping(enabled: bool) -> Self`
- `.render(&self, canvas: &mut Canvas, z: i32)`

### Column

```rust
pub struct Column {
    pub header: String,
    pub width: Option<f32>,
    pub alignment: Alignment,
}
```

#### Methods

- `Column::new(header: impl Into<String>) -> Self`
- `.with_width(width: f32) -> Self`
- `.with_alignment(alignment: Alignment) -> Self`

### TableRow

```rust
pub struct TableRow {
    pub cells: Vec<TableCell>,
    pub bg_color: Option<ColorLinPremul>,
}
```

#### Methods

- `TableRow::new(cells: Vec<TableCell>) -> Self`
- `.with_bg_color(color: ColorLinPremul) -> Self`

### TableCell

```rust
pub struct TableCell {
    pub text: String,
    pub color: Option<ColorLinPremul>,
    pub size: Option<f32>,
}
```

#### Methods

- `TableCell::new(text: impl Into<String>) -> Self`
- `.with_color(color: ColorLinPremul) -> Self`
- `.with_size(size: f32) -> Self`

### Alignment

```rust
pub enum Alignment {
    Left,
    Center,
    Right,
}
```

## Features

✅ **Header Row** - Column headers with customizable styling
✅ **Data Rows** - Multiple rows with cell-level customization
✅ **Footer Row** - Optional footer row for totals/summaries
✅ **Caption** - Optional table caption/title
✅ **Column Widths** - Fixed or auto-width columns
✅ **Text Alignment** - Left, center, or right alignment per column
✅ **Cell Styling** - Custom colors and sizes per cell
✅ **Row Styling** - Custom background colors per row
✅ **Zebra Striping** - Alternating row colors
✅ **Grid Lines** - Show/hide horizontal and vertical lines
✅ **Borders** - Customizable outer border
✅ **Spacing** - Configurable row height and cell padding
✅ **Fully Customizable** - All colors, sizes, and spacing

## Notes

- The table automatically calculates column widths based on configuration
- Columns without explicit width share remaining space equally
- Text is baseline-aligned vertically within cells
- The table renders top-to-bottom and clips at the rect height
- All styling is customizable through public fields
- No interactive features (sorting, selection) - pure rendering element
