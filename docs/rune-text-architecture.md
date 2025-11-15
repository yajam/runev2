# Rune-Text: Custom Text Layout Engine Architecture

A `cosmic-text` alternative with precise baseline control and line measurement APIs.

## Design Goals

1. **Precise Baseline Control** - Explicit baseline positioning and alignment APIs
2. **Granular Line Metrics** - Direct access to ascent, descent, leading, and line box dimensions
3. **Performance** - Efficient caching, incremental layout, and GPU-friendly output
4. **Correctness** - Full Unicode support (BiDi, complex scripts, grapheme clusters)
5. **Flexibility** - Composable APIs for custom layout algorithms

## Crate Structure

```
crates/rune-text/
├── src/
│   ├── lib.rs                 # Public API exports
│   ├── font/
│   │   ├── mod.rs
│   │   ├── face.rs            # Font face wrapper
│   │   ├── metrics.rs         # Font metrics (ascent, descent, etc.)
│   │   ├── loader.rs          # Font file loading
│   │   └── fallback.rs        # Font fallback chain
│   ├── shaping/
│   │   ├── mod.rs
│   │   ├── shaper.rs          # HarfBuzz integration
│   │   ├── shaped_run.rs      # Shaped text run with glyph positions
│   │   └── cache.rs           # Shape cache
│   ├── layout/
│   │   ├── mod.rs
│   │   ├── line_breaker.rs    # UAX-14 line breaking
│   │   ├── line_box.rs        # Line box with baseline metrics
│   │   ├── paragraph.rs       # Paragraph layout
│   │   ├── text_layout.rs     # Main layout engine
│   │   └── prefix_sums.rs     # Fast line/char lookups
│   ├── bidi/
│   │   ├── mod.rs
│   │   ├── reorder.rs         # BiDi reordering (UAX-9)
│   │   └── levels.rs          # Embedding levels
│   ├── unicode/
│   │   ├── mod.rs
│   │   ├── graphemes.rs       # Grapheme cluster segmentation
│   │   └── properties.rs      # Unicode properties
│   ├── style/
│   │   ├── mod.rs
│   │   ├── text_style.rs      # Text styling (color, weight, etc.)
│   │   ├── paragraph_style.rs # Paragraph styling (alignment, indent)
│   │   └── span.rs            # Styled text spans
│   ├── measurement/
│   │   ├── mod.rs
│   │   ├── metrics.rs         # Text measurement APIs
│   │   ├── hit_test.rs        # Point-to-character mapping
│   │   └── bounds.rs          # Bounding box calculations
│   ├── ime/
│   │   ├── mod.rs
│   │   ├── composition.rs     # IME composition state
│   │   ├── preedit.rs         # Preedit text handling
│   │   └── candidate.rs       # Candidate window positioning
│   ├── editing/
│   │   ├── mod.rs
│   │   ├── editor.rs          # TextEditor main implementation
│   │   ├── cursor.rs          # Cursor management
│   │   ├── selection.rs       # Selection handling
│   │   ├── history.rs         # Undo/redo system
│   │   └── clipboard.rs       # Clipboard operations
│   └── render/
│       ├── mod.rs
│       ├── glyph_cache.rs     # Glyph atlas/cache
│       └── output.rs          # Render-ready output
├── Cargo.toml
└── README.md
```

## Core Data Structures

### 1. Font Metrics

```rust
/// Font-level metrics in font units
#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    /// Ascent above baseline (positive)
    pub ascent: f32,
    /// Descent below baseline (positive)
    pub descent: f32,
    /// Line gap (leading)
    pub line_gap: f32,
    /// Units per em
    pub units_per_em: u16,
    /// Cap height (optional)
    pub cap_height: Option<f32>,
    /// X-height (optional)
    pub x_height: Option<f32>,
}

impl FontMetrics {
    /// Calculate line height (ascent + descent + line_gap)
    pub fn line_height(&self) -> f32 {
        self.ascent + self.descent + self.line_gap
    }
    
    /// Scale metrics to pixel size
    pub fn scale_to_pixels(&self, font_size: f32) -> ScaledFontMetrics {
        let scale = font_size / self.units_per_em as f32;
        ScaledFontMetrics {
            ascent: self.ascent * scale,
            descent: self.descent * scale,
            line_gap: self.line_gap * scale,
            font_size,
        }
    }
}

/// Scaled font metrics in pixels
#[derive(Debug, Clone, Copy)]
pub struct ScaledFontMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub line_gap: f32,
    pub font_size: f32,
}
```

### 2. Line Box with Baseline

```rust
/// A single line of text with precise baseline positioning
#[derive(Debug, Clone)]
pub struct LineBox {
    /// Byte offset in source text
    pub text_range: Range<usize>,
    /// Visual width of the line
    pub width: f32,
    /// Total height of the line box
    pub height: f32,
    /// Distance from line box top to baseline
    pub baseline_offset: f32,
    /// Maximum ascent in this line
    pub ascent: f32,
    /// Maximum descent in this line
    pub descent: f32,
    /// Leading (line gap)
    pub leading: f32,
    /// Shaped runs in visual order
    pub runs: Vec<ShapedRun>,
    /// Y position of line box top (relative to paragraph)
    pub y_offset: f32,
}

impl LineBox {
    /// Get baseline Y position (relative to paragraph)
    pub fn baseline_y(&self) -> f32 {
        self.y_offset + self.baseline_offset
    }
    
    /// Get line box bottom Y position
    pub fn bottom_y(&self) -> f32 {
        self.y_offset + self.height
    }
    
    /// Check if a point is within this line
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        y >= self.y_offset && y < self.bottom_y() && x >= 0.0 && x < self.width
    }
}
```

### 3. Shaped Run

```rust
/// A run of text shaped with a single font
#[derive(Debug, Clone)]
pub struct ShapedRun {
    /// Byte range in source text
    pub text_range: Range<usize>,
    /// Font used for this run
    pub font_id: FontId,
    /// Font size
    pub font_size: f32,
    /// Glyph IDs
    pub glyphs: Vec<GlyphId>,
    /// Glyph positions (x, y offsets from pen position)
    pub positions: Vec<GlyphPosition>,
    /// Glyph advances
    pub advances: Vec<f32>,
    /// Total advance width
    pub width: f32,
    /// X offset within line (for alignment)
    pub x_offset: f32,
    /// BiDi level
    pub bidi_level: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct GlyphPosition {
    pub x_offset: f32,
    pub y_offset: f32,
}
```

### 4. Text Layout

```rust
/// Complete text layout with all lines
#[derive(Debug)]
pub struct TextLayout {
    /// Source text
    text: String,
    /// All line boxes
    lines: Vec<LineBox>,
    /// Prefix sum array for fast lookups
    prefix_sums: PrefixSums,
    /// Layout constraints
    max_width: Option<f32>,
    /// Paragraph style
    style: ParagraphStyle,
}

impl TextLayout {
    /// Get line containing character offset
    pub fn line_at_char(&self, char_offset: usize) -> Option<&LineBox> {
        self.prefix_sums.line_at_char(char_offset)
            .and_then(|line_idx| self.lines.get(line_idx))
    }
    
    /// Get character offset at point
    pub fn hit_test(&self, x: f32, y: f32) -> Option<usize> {
        // Find line containing point
        let line = self.lines.iter().find(|line| {
            y >= line.y_offset && y < line.bottom_y()
        })?;
        
        // Find character within line
        self.hit_test_line(line, x)
    }
    
    /// Measure text bounds
    pub fn bounds(&self) -> Rect {
        let width = self.lines.iter()
            .map(|line| line.width)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);
        
        let height = self.lines.last()
            .map(|line| line.bottom_y())
            .unwrap_or(0.0);
        
        Rect { x: 0.0, y: 0.0, width, height }
    }
    
    /// Get baseline Y for specific line
    pub fn baseline_y(&self, line_index: usize) -> Option<f32> {
        self.lines.get(line_index).map(|line| line.baseline_y())
    }
}
```

### 5. Prefix Sums for Fast Lookups

```rust
/// Prefix sum array for O(1) line/character lookups
#[derive(Debug, Clone)]
pub struct PrefixSums {
    /// Cumulative character counts per line
    char_offsets: Vec<usize>,
    /// Cumulative byte offsets per line
    byte_offsets: Vec<usize>,
}

impl PrefixSums {
    /// Find line index containing character offset
    pub fn line_at_char(&self, char_offset: usize) -> Option<usize> {
        self.char_offsets.binary_search(&char_offset)
            .map(|i| i)
            .or_else(|i| if i > 0 { Some(i - 1) } else { None })
    }
    
    /// Get character offset at start of line
    pub fn char_offset_at_line(&self, line_index: usize) -> Option<usize> {
        self.char_offsets.get(line_index).copied()
    }
}
```

### 6. IME (Input Method Editor) Support

```rust
/// IME composition state
#[derive(Debug, Clone)]
pub struct ImeComposition {
    /// Preedit text (uncommitted text being composed)
    pub preedit_text: String,
    /// Cursor position within preedit text (byte offset)
    pub cursor_offset: usize,
    /// Selection range within preedit text (for conversion selection)
    pub selection: Option<Range<usize>>,
    /// Composition segments with attributes
    pub segments: Vec<ImeSegment>,
}

/// A segment of preedit text with styling attributes
#[derive(Debug, Clone)]
pub struct ImeSegment {
    /// Byte range in preedit text
    pub range: Range<usize>,
    /// Segment style (underline type, thickness)
    pub style: ImeSegmentStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImeSegmentStyle {
    /// Unconverted text (thin underline)
    Unconverted,
    /// Currently selected segment for conversion (thick underline)
    TargetConverted,
    /// Other converted segments (thin underline)
    Converted,
    /// Selected text in candidate list (highlighted background)
    Selected,
}

/// IME candidate window positioning
#[derive(Debug, Clone)]
pub struct ImeCandidatePosition {
    /// Screen position for candidate window (relative to text field)
    pub x: f32,
    pub y: f32,
    /// Line height at cursor position
    pub line_height: f32,
    /// Baseline offset for proper alignment
    pub baseline_offset: f32,
}

/// Text layout with IME composition overlay
impl TextLayout {
    /// Insert IME preedit text at cursor position
    pub fn set_ime_composition(
        &mut self,
        cursor_offset: usize,
        composition: ImeComposition,
    ) -> Result<(), LayoutError> {
        self.ime_state = Some(ImeState {
            cursor_offset,
            composition,
        });
        self.invalidate_layout();
        Ok(())
    }
    
    /// Clear IME composition
    pub fn clear_ime_composition(&mut self) {
        self.ime_state = None;
        self.invalidate_layout();
    }
    
    /// Get IME candidate window position
    pub fn ime_candidate_position(&self) -> Option<ImeCandidatePosition> {
        let ime_state = self.ime_state.as_ref()?;
        let cursor_offset = ime_state.cursor_offset;
        
        // Find line containing cursor
        let line = self.line_at_char(cursor_offset)?;
        
        // Calculate cursor x position within line
        let x = self.cursor_x_in_line(line, cursor_offset)?;
        
        Some(ImeCandidatePosition {
            x,
            y: line.bottom_y(),  // Position below current line
            line_height: line.height,
            baseline_offset: line.baseline_offset,
        })
    }
    
    /// Render preedit text with IME styling
    pub fn render_with_ime(&self) -> Vec<RenderCommand> {
        let mut commands = self.render_base_text();
        
        if let Some(ime_state) = &self.ime_state {
            // Render preedit text overlay
            let preedit_commands = self.render_preedit(
                ime_state.cursor_offset,
                &ime_state.composition,
            );
            commands.extend(preedit_commands);
        }
        
        commands
    }
}

/// IME event handling
#[derive(Debug, Clone)]
pub enum ImeEvent {
    /// IME composition started
    CompositionStart,
    /// IME composition updated
    CompositionUpdate {
        preedit: String,
        cursor_offset: usize,
        selection: Option<Range<usize>>,
    },
    /// IME composition committed (insert final text)
    CompositionCommit {
        text: String,
    },
    /// IME composition cancelled
    CompositionCancel,
}

impl TextLayout {
    /// Handle IME events
    pub fn handle_ime_event(&mut self, event: ImeEvent, cursor_offset: usize) -> Result<(), LayoutError> {
        match event {
            ImeEvent::CompositionStart => {
                self.set_ime_composition(cursor_offset, ImeComposition::default())?;
            }
            ImeEvent::CompositionUpdate { preedit, cursor_offset: preedit_cursor, selection } => {
                let composition = ImeComposition {
                    preedit_text: preedit,
                    cursor_offset: preedit_cursor,
                    selection,
                    segments: self.detect_ime_segments(&preedit, selection),
                };
                self.set_ime_composition(cursor_offset, composition)?;
            }
            ImeEvent::CompositionCommit { text } => {
                // Insert committed text into document
                self.insert_text(cursor_offset, &text)?;
                self.clear_ime_composition();
            }
            ImeEvent::CompositionCancel => {
                self.clear_ime_composition();
            }
        }
        Ok(())
    }
}
```

### 7. Text Editing Support

```rust
/// Text editor state with cursor and selection
#[derive(Debug)]
pub struct TextEditor {
    /// Text layout
    layout: TextLayout,
    /// Cursor position (byte offset)
    cursor: usize,
    /// Selection range (None if no selection)
    selection: Option<Range<usize>>,
    /// Cursor affinity (for BiDi text)
    cursor_affinity: CursorAffinity,
    /// Scroll offset
    scroll_offset: Point,
    /// Undo/redo history
    history: EditHistory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorAffinity {
    /// Cursor prefers left side of character
    Upstream,
    /// Cursor prefers right side of character
    Downstream,
}

impl TextEditor {
    /// Create new text editor
    pub fn new(text: String, font: FontFace, font_size: f32) -> Self {
        let layout = TextLayout::builder()
            .text(text)
            .font(font)
            .font_size(font_size)
            .build()
            .unwrap();
        
        Self {
            layout,
            cursor: 0,
            selection: None,
            cursor_affinity: CursorAffinity::Downstream,
            scroll_offset: Point::ZERO,
            history: EditHistory::new(),
        }
    }
    
    /// Get cursor screen position
    pub fn cursor_position(&self) -> Point {
        self.layout.char_position(self.cursor)
            .unwrap_or(Point::ZERO)
    }
    
    /// Get cursor rectangle for rendering
    pub fn cursor_rect(&self) -> Rect {
        let pos = self.cursor_position();
        let line = self.layout.line_at_char(self.cursor).unwrap();
        
        Rect {
            x: pos.x,
            y: pos.y,
            width: 2.0,  // Cursor width
            height: line.height,
        }
    }
    
    /// Move cursor left by one grapheme
    pub fn move_left(&mut self, extend_selection: bool) {
        if let Some(new_pos) = self.prev_grapheme_offset(self.cursor) {
            self.set_cursor(new_pos, extend_selection);
        }
    }
    
    /// Move cursor right by one grapheme
    pub fn move_right(&mut self, extend_selection: bool) {
        if let Some(new_pos) = self.next_grapheme_offset(self.cursor) {
            self.set_cursor(new_pos, extend_selection);
        }
    }
    
    /// Move cursor up by one line
    pub fn move_up(&mut self, extend_selection: bool) {
        if let Some(new_pos) = self.layout.cursor_up(self.cursor) {
            self.set_cursor(new_pos, extend_selection);
        }
    }
    
    /// Move cursor down by one line
    pub fn move_down(&mut self, extend_selection: bool) {
        if let Some(new_pos) = self.layout.cursor_down(self.cursor) {
            self.set_cursor(new_pos, extend_selection);
        }
    }
    
    /// Move cursor to line start
    pub fn move_to_line_start(&mut self, extend_selection: bool) {
        if let Some(line) = self.layout.line_at_char(self.cursor) {
            self.set_cursor(line.text_range.start, extend_selection);
        }
    }
    
    /// Move cursor to line end
    pub fn move_to_line_end(&mut self, extend_selection: bool) {
        if let Some(line) = self.layout.line_at_char(self.cursor) {
            self.set_cursor(line.text_range.end, extend_selection);
        }
    }
    
    /// Insert text at cursor
    pub fn insert_text(&mut self, text: &str) {
        let insert_pos = if let Some(sel) = self.selection.take() {
            self.delete_range(sel.clone());
            sel.start
        } else {
            self.cursor
        };
        
        self.history.record_insert(insert_pos, text);
        self.layout.insert_text(insert_pos, text).unwrap();
        self.cursor = insert_pos + text.len();
        self.scroll_to_cursor();
    }
    
    /// Delete character before cursor (backspace)
    pub fn delete_backward(&mut self) {
        if let Some(selection) = self.selection.take() {
            self.delete_range(selection);
        } else if let Some(prev) = self.prev_grapheme_offset(self.cursor) {
            let range = prev..self.cursor;
            self.delete_range(range);
        }
    }
    
    /// Delete character after cursor (delete)
    pub fn delete_forward(&mut self) {
        if let Some(selection) = self.selection.take() {
            self.delete_range(selection);
        } else if let Some(next) = self.next_grapheme_offset(self.cursor) {
            let range = self.cursor..next;
            self.delete_range(range);
        }
    }
    
    /// Copy selection to clipboard
    pub fn copy(&self) -> Option<String> {
        self.selection.as_ref().map(|sel| {
            self.layout.text()[sel.clone()].to_string()
        })
    }
    
    /// Cut selection to clipboard
    pub fn cut(&mut self) -> Option<String> {
        let text = self.copy();
        if let Some(sel) = self.selection.take() {
            self.delete_range(sel);
        }
        text
    }
    
    /// Paste text from clipboard
    pub fn paste(&mut self, text: &str) {
        self.insert_text(text);
    }
    
    /// Undo last operation
    pub fn undo(&mut self) {
        if let Some(op) = self.history.undo() {
            self.apply_undo_operation(op);
        }
    }
    
    /// Redo last undone operation
    pub fn redo(&mut self) {
        if let Some(op) = self.history.redo() {
            self.apply_redo_operation(op);
        }
    }
    
    /// Select all text
    pub fn select_all(&mut self) {
        self.selection = Some(0..self.layout.text().len());
        self.cursor = self.layout.text().len();
    }
    
    /// Get selection rectangles for rendering
    pub fn selection_rects(&self) -> Vec<Rect> {
        self.selection.as_ref()
            .map(|sel| self.layout.selection_rects(sel.clone()))
            .unwrap_or_default()
    }
    
    /// Handle mouse click (hit testing)
    pub fn click(&mut self, x: f32, y: f32, extend_selection: bool) {
        if let Some(offset) = self.layout.hit_test(x, y) {
            self.set_cursor(offset, extend_selection);
        }
    }
    
    /// Handle mouse drag (selection)
    pub fn drag(&mut self, x: f32, y: f32) {
        if let Some(offset) = self.layout.hit_test(x, y) {
            if self.selection.is_none() {
                self.selection = Some(self.cursor..self.cursor);
            }
            if let Some(sel) = &mut self.selection {
                sel.end = offset;
                self.cursor = offset;
            }
        }
    }
    
    /// Scroll to make cursor visible
    fn scroll_to_cursor(&mut self) {
        let cursor_rect = self.cursor_rect();
        // Adjust scroll_offset to make cursor_rect visible
        // Implementation depends on viewport size
    }
    
    /// Set cursor position with optional selection extension
    fn set_cursor(&mut self, new_pos: usize, extend_selection: bool) {
        if extend_selection {
            if self.selection.is_none() {
                self.selection = Some(self.cursor..self.cursor);
            }
            if let Some(sel) = &mut self.selection {
                sel.end = new_pos;
            }
        } else {
            self.selection = None;
        }
        self.cursor = new_pos;
        self.scroll_to_cursor();
    }
}

/// Undo/redo history
#[derive(Debug)]
struct EditHistory {
    undo_stack: Vec<EditOperation>,
    redo_stack: Vec<EditOperation>,
    max_size: usize,
}

#[derive(Debug, Clone)]
enum EditOperation {
    Insert { offset: usize, text: String },
    Delete { offset: usize, text: String },
}

impl EditHistory {
    fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size: 1000,
        }
    }
    
    fn record_insert(&mut self, offset: usize, text: &str) {
        self.undo_stack.push(EditOperation::Insert {
            offset,
            text: text.to_string(),
        });
        self.redo_stack.clear();
        self.trim_to_size();
    }
    
    fn record_delete(&mut self, offset: usize, text: &str) {
        self.undo_stack.push(EditOperation::Delete {
            offset,
            text: text.to_string(),
        });
        self.redo_stack.clear();
        self.trim_to_size();
    }
    
    fn undo(&mut self) -> Option<EditOperation> {
        self.undo_stack.pop().map(|op| {
            self.redo_stack.push(op.clone());
            op
        })
    }
    
    fn redo(&mut self) -> Option<EditOperation> {
        self.redo_stack.pop().map(|op| {
            self.undo_stack.push(op.clone());
            op
        })
    }
    
    fn trim_to_size(&mut self) {
        if self.undo_stack.len() > self.max_size {
            self.undo_stack.drain(0..self.undo_stack.len() - self.max_size);
        }
    }
}
```

## Public API Design

### Builder Pattern for Layout

```rust
use rune_text::*;

// Create a text layout
let layout = TextLayout::builder()
    .text("Hello, world!\nThis is a test.")
    .font(font_face)
    .font_size(16.0)
    .max_width(Some(400.0))
    .alignment(Alignment::Left)
    .line_height_multiplier(1.2)
    .build()?;

// Query baseline positions
for (i, line) in layout.lines().iter().enumerate() {
    println!("Line {}: baseline at y={}", i, line.baseline_y());
}

// Hit testing
if let Some(char_offset) = layout.hit_test(100.0, 50.0) {
    println!("Clicked at character offset: {}", char_offset);
}

// Measure bounds
let bounds = layout.bounds();
println!("Text bounds: {}x{}", bounds.width, bounds.height);
```

### Styled Text with Spans

```rust
let mut builder = TextLayout::builder()
    .max_width(Some(500.0));

builder.push_span("Hello ", TextStyle {
    font: font_regular,
    font_size: 16.0,
    color: Color::BLACK,
    ..Default::default()
});

builder.push_span("world", TextStyle {
    font: font_bold,
    font_size: 16.0,
    color: Color::RED,
    ..Default::default()
});

let layout = builder.build()?;
```

### Baseline Alignment

```rust
// Align text to a specific baseline
let layout1 = TextLayout::builder()
    .text("Small text")
    .font_size(12.0)
    .build()?;

let layout2 = TextLayout::builder()
    .text("Large text")
    .font_size(24.0)
    .build()?;

// Align baselines
let baseline1 = layout1.baseline_y(0).unwrap();
let baseline2 = layout2.baseline_y(0).unwrap();
let y_offset = baseline1 - baseline2;

// Render layout2 at y_offset to align baselines
```

### IME (Input Method Editor) Integration

```rust
use rune_text::{TextLayout, ImeEvent, ImeComposition};

// Handle IME events from windowing system
fn on_ime_event(&mut self, event: winit::event::Ime) {
    match event {
        winit::event::Ime::Preedit(text, cursor) => {
            let ime_event = ImeEvent::CompositionUpdate {
                preedit: text,
                cursor_offset: cursor.unwrap_or(0),
                selection: None,
            };
            self.layout.handle_ime_event(ime_event, self.cursor_pos).unwrap();
        }
        winit::event::Ime::Commit(text) => {
            let ime_event = ImeEvent::CompositionCommit { text };
            self.layout.handle_ime_event(ime_event, self.cursor_pos).unwrap();
        }
        _ => {}
    }
}

// Position IME candidate window
fn update_ime_position(&mut self) {
    if let Some(pos) = self.layout.ime_candidate_position() {
        // Convert to screen coordinates
        let screen_x = self.window_x + pos.x;
        let screen_y = self.window_y + pos.y;
        
        // Tell windowing system where to position candidate window
        self.window.set_ime_position(winit::dpi::Position::Physical(
            winit::dpi::PhysicalPosition::new(screen_x as i32, screen_y as i32)
        ));
    }
}

// Render text with IME preedit overlay
fn render_text(&self) {
    let commands = self.layout.render_with_ime();
    
    for cmd in commands {
        match cmd {
            RenderCommand::DrawGlyph { glyph_id, x, y, color } => {
                self.draw_glyph(glyph_id, x, y, color);
            }
            RenderCommand::DrawUnderline { x, y, width, style, color } => {
                // Draw IME underline (thin/thick based on style)
                self.draw_ime_underline(x, y, width, style, color);
            }
            RenderCommand::DrawBackground { rect, color } => {
                // Draw selection background for converted segments
                self.draw_rect(rect, color);
            }
        }
    }
}
```

### Text Editing Integration

```rust
use rune_text::{TextEditor, FontFace};

// Create a text editor
let mut editor = TextEditor::new(
    "Hello, world!".to_string(),
    font_face,
    16.0,
);

// Handle keyboard input
fn on_key_event(&mut self, event: KeyEvent) {
    match event.key {
        Key::Left => self.editor.move_left(event.shift_pressed),
        Key::Right => self.editor.move_right(event.shift_pressed),
        Key::Up => self.editor.move_up(event.shift_pressed),
        Key::Down => self.editor.move_down(event.shift_pressed),
        Key::Home => self.editor.move_to_line_start(event.shift_pressed),
        Key::End => self.editor.move_to_line_end(event.shift_pressed),
        Key::Backspace => self.editor.delete_backward(),
        Key::Delete => self.editor.delete_forward(),
        Key::Char(c) => self.editor.insert_text(&c.to_string()),
        Key::Enter => self.editor.insert_text("\n"),
        _ => {}
    }
    
    // Handle Ctrl/Cmd shortcuts
    if event.ctrl_or_cmd {
        match event.key {
            Key::Char('c') => {
                if let Some(text) = self.editor.copy() {
                    clipboard::set_text(text);
                }
            }
            Key::Char('x') => {
                if let Some(text) = self.editor.cut() {
                    clipboard::set_text(text);
                }
            }
            Key::Char('v') => {
                if let Some(text) = clipboard::get_text() {
                    self.editor.paste(&text);
                }
            }
            Key::Char('z') => self.editor.undo(),
            Key::Char('y') => self.editor.redo(),
            Key::Char('a') => self.editor.select_all(),
            _ => {}
        }
    }
}

// Handle mouse events
fn on_mouse_event(&mut self, event: MouseEvent) {
    match event.kind {
        MouseEventKind::Down => {
            self.editor.click(event.x, event.y, event.shift_pressed);
        }
        MouseEventKind::Drag => {
            self.editor.drag(event.x, event.y);
        }
        _ => {}
    }
}

// Render editor
fn render_editor(&self) {
    // Render selection background
    for rect in self.editor.selection_rects() {
        self.draw_rect(rect, SELECTION_COLOR);
    }
    
    // Render text
    let layout = self.editor.layout();
    for line in layout.lines() {
        for run in &line.runs {
            self.render_shaped_run(run, line.baseline_y());
        }
    }
    
    // Render cursor (with blinking)
    if self.cursor_visible {
        let cursor_rect = self.editor.cursor_rect();
        self.draw_rect(cursor_rect, CURSOR_COLOR);
    }
}
```

## Key Differentiators from cosmic-text

### 1. Explicit Baseline API

```rust
// rune-text: Direct baseline access
let baseline_y = layout.baseline_y(line_index);
let baseline_offset = line.baseline_offset;

// cosmic-text: No direct baseline API
// Must calculate from line metrics manually
```

### 2. Granular Line Metrics

```rust
// rune-text: Full line box metrics
pub struct LineBox {
    pub baseline_offset: f32,  // Distance from top to baseline
    pub ascent: f32,            // Max ascent in line
    pub descent: f32,           // Max descent in line
    pub leading: f32,           // Line gap
    pub height: f32,            // Total box height
}

// cosmic-text: Limited metric access
// Line height is calculated internally
```

### 3. Measurement APIs

```rust
// rune-text: Rich measurement API
impl TextLayout {
    pub fn bounds(&self) -> Rect;
    pub fn line_bounds(&self, line_index: usize) -> Option<Rect>;
    pub fn char_bounds(&self, char_offset: usize) -> Option<Rect>;
    pub fn baseline_y(&self, line_index: usize) -> Option<f32>;
}
```

### 4. Prefix Sums for Performance

```rust
// O(1) line lookup instead of O(n) iteration
let line = layout.line_at_char(char_offset);
```

### 5. Built-in IME Support

```rust
// rune-text: First-class IME support
layout.handle_ime_event(ImeEvent::CompositionUpdate { ... });
let candidate_pos = layout.ime_candidate_position();
let commands = layout.render_with_ime();

// cosmic-text: No built-in IME support
// Must implement preedit overlay manually
```

## Implementation Phases

### Phase 1: Core Foundation (Week 1-2)
- [ ] Set up crate structure
- [ ] Integrate `rustybuzz` for shaping
- [ ] Implement `FontMetrics` and `ScaledFontMetrics`
- [ ] Create `ShapedRun` data structure
- [ ] Build basic single-line shaping

### Phase 2: Line Breaking (Week 2-3)
- [ ] Integrate `unicode-linebreak`
- [ ] Implement `LineBreaker` with UAX-14
- [ ] Create `LineBox` with baseline metrics
- [ ] Build multi-line layout engine
- [ ] Add prefix sums for fast lookups

### Phase 3: Measurement & Hit Testing (Week 3-4)
- [ ] Implement `bounds()` API
- [ ] Add `hit_test()` for point-to-char
- [ ] Create `char_bounds()` for character rectangles
- [ ] Build `baseline_y()` API
- [ ] Add line-level measurement APIs

### Phase 4: BiDi Support (Week 4-5)
- [ ] Integrate `unicode-bidi`
- [ ] Implement BiDi reordering
- [ ] Handle mixed LTR/RTL text
- [ ] Update hit testing for BiDi
- [ ] Test with Arabic/Hebrew

### Phase 5: Font Fallback (Week 5-6)
- [ ] Design fallback chain architecture
- [ ] Implement missing glyph detection
- [ ] Build automatic fallback selection
- [ ] Handle emoji fallback
- [ ] Test with multilingual text

### Phase 6: Text Editing (Week 6-7)
- [ ] Implement `TextEditor` data structure
- [ ] Add cursor position tracking and rendering
- [ ] Build hit testing for cursor positioning
- [ ] Implement cursor movement (left/right/up/down)
- [ ] Add selection management
- [ ] Implement text insertion and deletion
- [ ] Build clipboard operations (copy/cut/paste)
- [ ] Create undo/redo system
- [ ] Add scrolling and viewport management
- [ ] Test with keyboard and mouse input

### Phase 7: Styling & Spans (Week 7-8)
- [ ] Create `TextStyle` and `ParagraphStyle`
- [ ] Implement styled span support
- [ ] Build style-aware shaping
- [ ] Add color and decoration support
- [ ] Create builder API for styled text

### Phase 8: Performance (Week 8-9)
- [ ] Implement shape cache
- [ ] Add layout caching
- [ ] Build incremental re-layout
- [ ] Optimize memory usage
- [ ] Benchmark against cosmic-text

### Phase 9: IME Support (Week 9-10)
- [ ] Implement `ImeComposition` and `ImeSegment` data structures
- [ ] Add `ImeEvent` handling
- [ ] Build preedit text overlay rendering
- [ ] Implement IME candidate window positioning
- [ ] Add IME segment styling (underlines, backgrounds)
- [ ] Test with Japanese, Chinese, Korean IMEs
- [ ] Handle composition cursor positioning
- [ ] Support IME on/off state tracking

### Phase 10: Advanced Features (Week 10+)
- [ ] Add justification support
- [ ] Implement hyphenation
- [ ] Support OpenType features
- [ ] Add tab stops
- [ ] Implement ellipsis

## Dependencies

```toml
[dependencies]
# Text shaping
rustybuzz = "0.14"

# Unicode support
unicode-segmentation = "1.11"
unicode-linebreak = "0.1"
unicode-bidi = "0.3"

# Font parsing
swash = "0.1"

# Optional features
hyphenation = { version = "0.8", optional = true }

[dev-dependencies]
criterion = "0.5"  # Benchmarking
```

## Testing Strategy

### Unit Tests
- Font metrics calculation
- Line breaking edge cases
- BiDi reordering
- Hit testing accuracy
- Prefix sum lookups
- IME composition state management
- IME segment detection and styling
- Candidate window positioning

### Integration Tests
- Complete layout pipeline
- Multi-font fallback
- Styled text rendering
- Baseline alignment
- IME composition with line wrapping
- IME preedit rendering with complex scripts

### Benchmarks
- Layout performance (vs cosmic-text)
- Shape cache hit rate
- Hit testing speed
- Memory usage

### Visual Tests
- Reference images for complex scripts
- Baseline alignment verification
- BiDi rendering correctness
- IME preedit underline styles
- IME candidate window positioning accuracy

## Example Usage in rune-draw

```rust
use rune_text::{TextLayout, TextStyle, Alignment};

// In your UI rendering code
fn render_text_element(&mut self, text: &str, bounds: Rect) {
    let layout = TextLayout::builder()
        .text(text)
        .font(self.default_font)
        .font_size(14.0)
        .max_width(Some(bounds.width))
        .alignment(Alignment::Left)
        .build()
        .unwrap();
    
    // Render each line with precise baseline positioning
    for line in layout.lines() {
        let baseline_y = bounds.y + line.baseline_y();
        
        for run in &line.runs {
            self.render_shaped_run(
                run,
                bounds.x + run.x_offset,
                baseline_y,
            );
        }
    }
}

// Baseline-aligned text boxes
fn render_inline_elements(&mut self, elements: &[InlineElement]) {
    let mut x = 0.0;
    let baseline_y = 100.0;  // Common baseline
    
    for element in elements {
        let layout = element.layout();
        let line = layout.lines().first().unwrap();
        
        // Align to common baseline
        let y_offset = baseline_y - line.baseline_offset;
        
        self.render_layout(layout, x, y_offset);
        x += layout.bounds().width + 10.0;
    }
}
```

## Migration Path

1. **Start with rune-text** for new text rendering code
2. **Keep existing code** using cosmic-text (if any) until rune-text is stable
3. **Benchmark** both implementations
4. **Gradually migrate** once feature parity is reached
5. **Remove cosmic-text** dependency when migration is complete

## Success Metrics

- [ ] Baseline API provides pixel-perfect alignment
- [ ] Line metrics match font specification exactly
- [ ] Performance within 20% of cosmic-text
- [ ] Passes all Unicode test cases
- [ ] Zero visual regressions vs reference renderings
- [ ] API is ergonomic and well-documented

## Future Enhancements

- Variable font support
- Advanced OpenType features (contextual alternates, etc.)
- Vertical text layout
- Ruby text (furigana)
- Text decorations (underline, strikethrough)
- Inline images/widgets
- Rich text editing primitives
