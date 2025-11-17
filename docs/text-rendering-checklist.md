# Text Rendering Implementation Checklist

A comprehensive guide for implementing robust multiline text rendering in rune-draw.

## Phase 1: Foundation (Essential)

### 1.1 Dependencies Setup

- [x] Add `harfrust` for text shaping
- [x] Add `unicode-segmentation` for grapheme cluster handling
- [x] Add `unicode-linebreak` for UAX-14 line breaking
- [x] Add `unicode-bidi` for bidirectional text support
- [x] Add `swash` for font parsing and glyph rasterization

### 1.2 Font Management

- [x] Implement font file loading (TTF/OTF)
- [x] Create font face caching system
- [x] Build font metrics extraction (ascent, descent, line gap)
- [x] Implement glyph ID to outline/bitmap conversion
- [x] Add font size and DPI scaling support

### 1.3 Basic Text Shaping

- [x] Integrate Harfbuzz shaping pipeline
- [x] Handle simple LTR text shaping
- [x] Extract glyph positions and advances
- [x] Apply kerning and ligatures
- [ ] Test with complex scripts (Arabic, Devanagari, Thai)

### 1.4 Unicode Handling

- [x] Implement grapheme cluster segmentation
- [x] Handle combining marks correctly
- [x] Support emoji and ZWJ sequences
- [x] Test with various Unicode edge cases

## Phase 2: Line Breaking & Layout

### 2.1 Line Breaking (UAX-14)

- [x] Integrate unicode-linebreak algorithm
- [x] Implement break opportunity detection
- [x] Handle mandatory breaks (newlines, paragraphs)
- [x] Support soft hyphens and zero-width spaces
- [x] Add word boundary detection

### 2.2 Line Box Builder

- [x] Create line box data structure (width, height, baseline)
- [x] Implement single-line layout
- [x] Build multi-line layout engine
- [x] Calculate line metrics (ascent, descent, leading)
- [x] Handle empty lines and whitespace-only lines

### 2.3 Text Wrapping

- [x] Implement greedy line breaking
- [x] Add width constraint handling
- [x] Support break-word vs break-all modes
- [x] Handle overflow behavior (clip, ellipsis, scroll)
- [ ] Test with various container widths

### 2.4 Prefix Sums Optimization

- [x] Build prefix sum array for line offsets
- [x] Implement O(1) line-to-character mapping
- [x] Add character-to-line lookup
- [x] Support efficient range queries
- [ ] Benchmark performance on large documents

## Phase 3: Bidirectional Text (BiDi)

### 3.1 BiDi Algorithm (UAX-9)

- [x] Integrate unicode-bidi crate
- [x] Implement paragraph-level direction detection
- [x] Apply BiDi reordering algorithm
- [x] Handle embedding levels
- [x] Support explicit directional overrides (LRO, RLO, LRE, RLE, PDF)

### 3.2 Mixed Directionality

- [x] Test LTR text with RTL segments
- [x] Test RTL text with LTR segments (numbers, Latin)
- [x] Handle neutral characters correctly
- [x] Support mirrored characters (brackets, parentheses)
- [x] Verify cursor movement in BiDi text

### 3.3 BiDi + Line Breaking

- [x] Reorder text after line breaking
- [x] Handle line breaks within BiDi runs
- [x] Test complex mixed-direction paragraphs
- [x] Verify visual vs logical order consistency

## Phase 4: Advanced Typography

### 4.1 Multi-Font Fallback

- [ ] Design fallback chain architecture
- [ ] Implement font enumeration
- [ ] Add missing glyph detection
- [ ] Build automatic fallback selection
- [ ] Support system font fallbacks
- [ ] Handle emoji font fallback
- [ ] Test with multilingual text (CJK, Arabic, Cyrillic)

### 4.2 OpenType Features

- [ ] Support common ligatures (liga)
- [ ] Add discretionary ligatures (dlig)
- [ ] Implement contextual alternates (calt)
- [ ] Support stylistic sets (ss01-ss20)
- [ ] Add small caps (smcp)
- [ ] Implement tabular figures (tnum)
- [ ] Support old-style figures (onum)

### 4.3 Font Variations

- [ ] Parse variable font axes
- [ ] Support weight variations
- [ ] Handle width variations
- [ ] Implement optical size (opsz)
- [ ] Add custom axis support

## Phase 5: Text Formatting Features

### 5.1 Alignment

- [ ] Implement left alignment
- [ ] Implement right alignment
- [ ] Implement center alignment
- [ ] Add justified alignment (basic)
- [ ] Support start/end alignment (BiDi-aware)

### 5.2 Justification

- [ ] Implement space stretching/shrinking
- [ ] Add inter-character spacing (for CJK)
- [ ] Handle justification with tabs
- [ ] Prevent over-stretching (max ratio)
- [ ] Support kashida for Arabic (advanced)

### 5.3 Indentation & Spacing

- [ ] Implement text-indent (first line)
- [ ] Add paragraph spacing (before/after)
- [ ] Support line height multiplier
- [ ] Handle hanging indents
- [ ] Add block-level margins

### 5.4 Special Characters

- [ ] Implement tab stops (fixed width)
- [ ] Add custom tab stop positions
- [ ] Support ellipsis rendering (…)
- [ ] Handle non-breaking spaces
- [ ] Implement soft hyphens visibility

### 5.5 Hyphenation

- [ ] Integrate `hyphenation` crate
- [ ] Load language-specific dictionaries
- [ ] Implement hyphen insertion
- [ ] Add hyphenation zone (min chars before/after)
- [ ] Support manual hyphenation hints
- [ ] Test with multiple languages

## Phase 6: Text Editing Support

> **Note:** For UI widget integration of text editing features (InputBox, TextArea), see [`inputbox-textlayout-checklist.md`](./inputbox-textlayout-checklist.md) which documents how to build editing widgets on top of `rune-text::TextLayout`.

### 6.1 Cursor Management

- [x] Implement cursor position tracking (byte offset)
- [x] Add cursor rendering (blinking animation)
- [x] Support cursor visibility toggle
- [x] Handle cursor positioning at grapheme boundaries
- [x] Implement cursor affinity (left/right of character)
- [ ] Support multiple cursors (optional)

### 6.2 Hit Testing & Positioning

- [x] Implement hit testing (point to character offset)
- [x] Add character offset to screen position mapping
- [x] Handle hit testing in BiDi text
- [x] Support hit testing with ligatures
- [x] Calculate cursor rectangle for rendering
- [x] Handle hit testing at line boundaries

### 6.3 Cursor Movement

- [x] Left/right by character (grapheme cluster)
- [x] Left/right by word boundary
- [x] Up/down by line (maintain column position)
- [x] Home/End (line start/end)
- [x] Ctrl+Home/End (document start/end)
- [x] Handle BiDi cursor movement (visual vs logical)
- [x] Support cursor movement with combining marks
- [x] Handle cursor movement across ligatures

### 6.4 Selection Management

- [x] Implement selection range (start, end offsets)
- [x] Add selection rendering (background highlight)
- [x] Support selection extension (Shift+movement)
- [x] Handle selection in BiDi text (visual order)
- [x] Implement word selection (double-click)
- [x] Implement line selection (triple-click)
- [x] Support paragraph selection
- [x] Handle selection across multiple lines
- [x] Calculate selection rectangles for rendering
- [x] Mouse click to position cursor
- [x] Mouse drag selection (click + drag)
- [x] Word-wise drag selection (double-click + drag)
- [x] Line-wise drag selection (triple-click + drag)

### 6.5 Text Insertion

- [x] Insert character at cursor position
- [x] Insert string at cursor position
- [x] Replace selection with inserted text
- [x] Handle newline insertion
- [x] Support tab insertion
- [x] Validate inserted text (grapheme clusters)
- [x] Trigger layout invalidation after insert
- [x] Update cursor position after insert

### 6.6 Text Deletion

- [x] Delete character before cursor (backspace)
- [x] Delete character after cursor (delete)
- [x] Delete word before cursor (Ctrl+Backspace)
- [x] Delete word after cursor (Ctrl+Delete)
- [x] Delete selection range
- [x] Delete line
- [x] Handle deletion at grapheme boundaries
- [x] Handle deletion with combining marks
- [x] Trigger layout invalidation after delete

### 6.7 Clipboard Operations

- [x] Copy selection to clipboard
- [x] Cut selection to clipboard
- [x] Paste from clipboard at cursor
- [x] Replace selection with pasted text
- [x] Handle clipboard text normalization
- [ ] Support rich text clipboard (optional)
- [x] Handle large clipboard content efficiently

### 6.8 Undo/Redo System

- [x] Implement undo stack data structure
- [x] Record text insertion operations
- [x] Record text deletion operations
- [x] Record selection changes (optional)
- [x] Implement undo operation
- [x] Implement redo operation
- [x] Group consecutive operations (typing)
- [x] Set undo stack size limit
- [x] Clear undo stack on major changes

### 6.9 Text Measurement for Editing

- [x] Measure single-line text width
- [x] Calculate multi-line text bounds
- [x] Implement character width queries
- [x] Add line height calculations
- [x] Support baseline queries
- [x] Calculate visible text range (viewport culling)
- [x] Measure text range width

### 6.10 Scrolling & Viewport

- [x] Implement scroll offset tracking (handled by rune-scene)
- [x] Calculate visible line range
- [x] Auto-scroll to cursor on movement
- [x] Auto-scroll on selection
- [x] Handle smooth scrolling (helper methods provided)
- [x] Support horizontal scrolling (long lines)
- [x] Implement scroll to position API
- [x] Handle mouse wheel scrolling

### 6.11 Advanced Editing Features

- [ ] Find and replace
- [ ] Incremental search
- [ ] Syntax highlighting (optional)
- [ ] Auto-indentation
- [ ] Bracket matching
- [ ] Line numbers rendering
- [ ] Minimap (optional)
- [ ] Code folding (optional)

## Phase 7: Performance Optimization

### 7.1 Caching

- [ ] Cache shaped text runs
- [ ] Implement glyph atlas/cache
- [ ] Cache line break opportunities
- [ ] Add layout result caching
- [ ] Implement incremental re-layout

### 7.2 Lazy Evaluation

- [ ] Defer shaping until render
- [ ] Implement viewport-based culling
- [ ] Add virtual scrolling for large documents
- [ ] Lazy-load font fallbacks

### 7.3 Parallelization

- [ ] Parallelize line breaking (if beneficial)
- [ ] Concurrent glyph rasterization
- [ ] Multi-threaded font loading

### 7.4 Memory Management

- [ ] Implement glyph cache eviction
- [ ] Add shaped run pooling
- [ ] Optimize string storage
- [ ] Profile memory usage

## Phase 8: IME (Input Method Editor) Support

### 8.1 IME Core

- [ ] Implement `ImeComposition` data structure
- [ ] Add `ImeSegment` with styling attributes
- [ ] Create `ImeEvent` enum (start, update, commit, cancel)
- [ ] Build IME state management in `TextLayout`
- [ ] Handle preedit text insertion/removal

### 8.2 IME Rendering

- [ ] Render preedit text overlay
- [ ] Draw IME underlines (thin/thick based on segment style)
- [ ] Implement selection background for converted segments
- [ ] Support composition cursor rendering
- [ ] Handle preedit text with line wrapping

### 8.3 IME Positioning

- [ ] Calculate IME candidate window position
- [ ] Provide baseline-aligned positioning
- [ ] Handle multi-line preedit positioning
- [ ] Update position on cursor movement
- [ ] Support screen coordinate conversion

### 8.4 IME Testing

- [ ] Test with Japanese IME (Hiragana → Kanji conversion)
- [ ] Test with Chinese IME (Pinyin input)
- [ ] Test with Korean IME (Hangul composition)
- [ ] Verify segment styling (unconverted, converted, selected)
- [ ] Test composition cancellation
- [ ] Verify candidate window positioning

## Phase 9: Rendering Integration

### 9.1 GPU Rendering

- [ ] Implement glyph atlas texture
- [ ] Add SDF/MSDF rendering
- [ ] Support subpixel positioning
- [ ] Implement gamma-correct blending
- [ ] Add color emoji support (COLR/CPAL, SBIX, CBDT)

### 9.2 Text Effects

- [ ] Implement text color
- [ ] Add background color
- [ ] Support underline (single, double, wavy)
- [ ] Implement strikethrough
- [ ] Add text shadow/outline

### 9.3 Decorations

- [ ] Render underline with proper thickness
- [ ] Position underline relative to baseline
- [ ] Handle underline gaps (descenders)
- [ ] Support decoration color (separate from text)

## Phase 10: Testing & Validation

### 10.1 Unit Tests

- [ ] Test grapheme segmentation edge cases
- [ ] Verify line breaking algorithm
- [ ] Test BiDi reordering
- [ ] Validate font fallback logic
- [ ] Test prefix sum calculations
- [ ] Test IME composition state transitions
- [ ] Verify IME segment styling

### 10.2 Integration Tests

- [ ] Test complete layout pipeline
- [ ] Verify rendering output
- [ ] Test with real-world documents
- [ ] Benchmark performance
- [ ] Test IME with line wrapping
- [ ] Verify IME candidate positioning

### 10.3 Visual Tests

- [ ] Create reference renderings
- [ ] Test against browser rendering
- [ ] Verify complex script rendering
- [ ] Compare with native text rendering
- [ ] Verify IME preedit underlines
- [ ] Test IME with various languages

### 10.4 Edge Cases

- [ ] Empty strings
- [ ] Single character
- [ ] Very long lines (10k+ chars)
- [ ] Mixed scripts in single word
- [ ] Zero-width characters
- [ ] Combining mark sequences
- [ ] Emoji with skin tone modifiers
- [ ] Right-to-left with numbers

## Phase 11: Documentation & API

### 11.1 Public API Design

- [ ] Design text layout API
- [ ] Create builder pattern for text styles
- [ ] Add convenience methods
- [ ] Document performance characteristics
- [ ] Provide usage examples

### 11.2 Documentation

- [ ] Write API documentation
- [ ] Create integration guide
- [ ] Add performance tuning guide
- [ ] Document limitations
- [ ] Provide migration guide (if applicable)
- [ ] Document IME integration

### 11.3 Examples

- [ ] Simple text rendering example
- [ ] Multi-line text example
- [ ] BiDi text example
- [ ] Custom font fallback example
- [ ] Text editing example
- [ ] IME integration example

## Recommended Crate Versions

```toml
[dependencies]
# Text shaping
harfrust = "0.3"  # Pure-Rust HarfBuzz port

# Unicode support
unicode-segmentation = "1.11"
unicode-linebreak = "0.1"
unicode-bidi = "0.3"

# Font handling
swash = "0.1"  # Font parsing and rasterization
# ttf-parser = "0.20"  # Alternative, lighter weight

# Optional advanced features
hyphenation = "0.8"  # Hyphenation support
# icu4x = "1.4"  # Comprehensive Unicode support (heavier)
```

## Priority Levels

**P0 (Must Have)**: Phase 1, Phase 2.1-2.3, Phase 5.1, Phase 6.1-6.6 (Core Editing)
**P1 (Should Have)**: Phase 3, Phase 4.1, Phase 6.7-6.9 (Clipboard, Undo, Measurement), Phase 7.1, Phase 8 (IME)
**P2 (Nice to Have)**: Phase 4.2-4.3, Phase 5.2-5.5, Phase 6.10 (Scrolling), Phase 7.2-7.4
**P3 (Future)**: Phase 6.11 (Advanced Editing), Phase 9.2-9.3, Phase 11

## Notes

- Start with simple LTR text and gradually add complexity
- Test continuously with real-world content
- Profile early and often to avoid performance pitfalls
- **Why custom implementation?** Existing solutions like `cosmic-text` don't provide compatible baseline calculation and line measurement APIs needed for precise layout control
- Building from scratch gives full control over line box metrics, baseline alignment, and measurement APIs

## References

- [Unicode Line Breaking Algorithm (UAX-14)](https://www.unicode.org/reports/tr14/)
- [Unicode Bidirectional Algorithm (UAX-9)](https://www.unicode.org/reports/tr9/)
- [HarfBuzz Documentation](https://harfbuzz.github.io/)
- [OpenType Feature Tags](https://docs.microsoft.com/en-us/typography/opentype/spec/featuretags)
