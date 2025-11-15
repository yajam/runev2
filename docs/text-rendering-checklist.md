# Text Rendering Implementation Checklist

A comprehensive guide for implementing robust multiline text rendering in rune-draw.

## Phase 1: Foundation (Essential)

### 1.1 Dependencies Setup
- [x] Add `rustybuzz` or `harfbuzz_rs` for text shaping
- [x] Add `unicode-segmentation` for grapheme cluster handling
- [x] Add `unicode-linebreak` for UAX-14 line breaking
- [x] Add `unicode-bidi` for bidirectional text support
- [x] Add `swash` for font parsing and glyph rasterization ( pairs well with rustybuzz)

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
- [ ] Integrate unicode-linebreak algorithm
- [ ] Implement break opportunity detection
- [ ] Handle mandatory breaks (newlines, paragraphs)
- [ ] Support soft hyphens and zero-width spaces
- [ ] Add word boundary detection

### 2.2 Line Box Builder
- [ ] Create line box data structure (width, height, baseline)
- [ ] Implement single-line layout
- [ ] Build multi-line layout engine
- [ ] Calculate line metrics (ascent, descent, leading)
- [ ] Handle empty lines and whitespace-only lines

### 2.3 Text Wrapping
- [ ] Implement greedy line breaking
- [ ] Add width constraint handling
- [ ] Support break-word vs break-all modes
- [ ] Handle overflow behavior (clip, ellipsis, scroll)
- [ ] Test with various container widths

### 2.4 Prefix Sums Optimization
- [ ] Build prefix sum array for line offsets
- [ ] Implement O(1) line-to-character mapping
- [ ] Add character-to-line lookup
- [ ] Support efficient range queries
- [ ] Benchmark performance on large documents

## Phase 3: Bidirectional Text (BiDi)

### 3.1 BiDi Algorithm (UAX-9)
- [ ] Integrate unicode-bidi crate
- [ ] Implement paragraph-level direction detection
- [ ] Apply BiDi reordering algorithm
- [ ] Handle embedding levels
- [ ] Support explicit directional overrides (LRO, RLO, LRE, RLE, PDF)

### 3.2 Mixed Directionality
- [ ] Test LTR text with RTL segments
- [ ] Test RTL text with LTR segments (numbers, Latin)
- [ ] Handle neutral characters correctly
- [ ] Support mirrored characters (brackets, parentheses)
- [ ] Verify cursor movement in BiDi text

### 3.3 BiDi + Line Breaking
- [ ] Reorder text after line breaking
- [ ] Handle line breaks within BiDi runs
- [ ] Test complex mixed-direction paragraphs
- [ ] Verify visual vs logical order consistency

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

### 6.1 Cursor Management
- [ ] Implement cursor position tracking (byte offset)
- [ ] Add cursor rendering (blinking animation)
- [ ] Support cursor visibility toggle
- [ ] Handle cursor positioning at grapheme boundaries
- [ ] Implement cursor affinity (left/right of character)
- [ ] Support multiple cursors (optional)

### 6.2 Hit Testing & Positioning
- [ ] Implement hit testing (point to character offset)
- [ ] Add character offset to screen position mapping
- [ ] Handle hit testing in BiDi text
- [ ] Support hit testing with ligatures
- [ ] Calculate cursor rectangle for rendering
- [ ] Handle hit testing at line boundaries

### 6.3 Cursor Movement
- [ ] Left/right by character (grapheme cluster)
- [ ] Left/right by word boundary
- [ ] Up/down by line (maintain column position)
- [ ] Home/End (line start/end)
- [ ] Ctrl+Home/End (document start/end)
- [ ] Handle BiDi cursor movement (visual vs logical)
- [ ] Support cursor movement with combining marks
- [ ] Handle cursor movement across ligatures

### 6.4 Selection Management
- [ ] Implement selection range (start, end offsets)
- [ ] Add selection rendering (background highlight)
- [ ] Support selection extension (Shift+movement)
- [ ] Handle selection in BiDi text (visual order)
- [ ] Implement word selection (double-click)
- [ ] Implement line selection (triple-click)
- [ ] Support paragraph selection
- [ ] Handle selection across multiple lines
- [ ] Calculate selection rectangles for rendering

### 6.5 Text Insertion
- [ ] Insert character at cursor position
- [ ] Insert string at cursor position
- [ ] Replace selection with inserted text
- [ ] Handle newline insertion
- [ ] Support tab insertion
- [ ] Validate inserted text (grapheme clusters)
- [ ] Trigger layout invalidation after insert
- [ ] Update cursor position after insert

### 6.6 Text Deletion
- [ ] Delete character before cursor (backspace)
- [ ] Delete character after cursor (delete)
- [ ] Delete word before cursor (Ctrl+Backspace)
- [ ] Delete word after cursor (Ctrl+Delete)
- [ ] Delete selection range
- [ ] Delete line
- [ ] Handle deletion at grapheme boundaries
- [ ] Handle deletion with combining marks
- [ ] Trigger layout invalidation after delete

### 6.7 Clipboard Operations
- [ ] Copy selection to clipboard
- [ ] Cut selection to clipboard
- [ ] Paste from clipboard at cursor
- [ ] Replace selection with pasted text
- [ ] Handle clipboard text normalization
- [ ] Support rich text clipboard (optional)
- [ ] Handle large clipboard content efficiently

### 6.8 Undo/Redo System
- [ ] Implement undo stack data structure
- [ ] Record text insertion operations
- [ ] Record text deletion operations
- [ ] Record selection changes (optional)
- [ ] Implement undo operation
- [ ] Implement redo operation
- [ ] Group consecutive operations (typing)
- [ ] Set undo stack size limit
- [ ] Clear undo stack on major changes

### 6.9 Text Measurement for Editing
- [ ] Measure single-line text width
- [ ] Calculate multi-line text bounds
- [ ] Implement character width queries
- [ ] Add line height calculations
- [ ] Support baseline queries
- [ ] Calculate visible text range (viewport culling)
- [ ] Measure text range width

### 6.10 Scrolling & Viewport
- [ ] Implement scroll offset tracking
- [ ] Calculate visible line range
- [ ] Auto-scroll to cursor on movement
- [ ] Auto-scroll on selection
- [ ] Handle smooth scrolling
- [ ] Support horizontal scrolling (long lines)
- [ ] Implement scroll to position API
- [ ] Handle mouse wheel scrolling

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
- [ ] Add SDF/MSDF rendering (optional)
- [ ] Support subpixel positioning
- [ ] Implement gamma-correct blending
- [ ] Add color emoji support (COLR/CPAL, SBIX, CBDT)

### 9.2 Text Effects
- [ ] Implement text color
- [ ] Add background color
- [ ] Support underline (single, double, wavy)
- [ ] Implement strikethrough
- [ ] Add text shadow/outline (optional)

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
# Text shaping (choose one)
rustybuzz = "0.14"  # Pure Rust, recommended
# harfbuzz_rs = "2.0"  # Alternative with C bindings

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
