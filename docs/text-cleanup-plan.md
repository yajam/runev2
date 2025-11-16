# Text Rendering Code Cleanup Plan

**Status**: Ready for execution  
**Priority**: Medium (code works, but cleanup improves maintainability)  
**Estimated Effort**: 2-4 hours

## Goals

1. Remove unused/legacy text provider code
2. Clean up dead code and unused features
3. Simplify configuration and remove unnecessary options
4. Improve code documentation
5. Consolidate text rendering to single approach (harfrust + swash)

## What to Keep

### ✅ Core Components (DO NOT REMOVE)

1. **`RuneTextProvider`** (`engine-core/src/text.rs`)
   - harfrust integration
   - swash rasterization
   - Subpixel RGB rendering
   - Font fallback via fontdb

2. **`TextLayoutCache`** (`engine-core/src/text_layout.rs`)
   - Fast wrapping algorithm
   - Cache implementation
   - All public APIs

3. **`Canvas::draw_text_run()`** (`rune-surface/src/canvas.rs`)
   - Direct rasterization path
   - Transform stack support
   - DPI scaling

4. **`MultilineText`** (`rune-scene/src/elements/multiline_text.rs`)
   - `render_cached()` method
   - Paragraph handling
   - All rendering methods

5. **`PassManager` glyph rendering** (`engine-core/src/pass_manager.rs`)
   - Glyph atlas management
   - GPU upload and rendering
   - Batch rendering

## What to Remove/Clean Up

### ❌ Legacy Text Providers

#### 1. Remove fontdue-based providers (if present)

**Files to check**:
- `engine-core/src/text.rs`

**Code to remove**:
```rust
// Remove these if they exist:
- SimpleFontdueProvider
- GrayscaleFontdueProvider  
- PatchedFontdueProvider
```

**Reason**: Replaced by swash (better quality, subpixel support)

#### 2. Simplify cosmic-text provider

**File**: `engine-core/src/text.rs`

**Current**: cosmic-text provider exists for compatibility  
**Action**: Keep but document as "legacy compatibility only"

**Add comment**:
```rust
/// Legacy cosmic-text provider for compatibility.
/// NOT RECOMMENDED: Use RuneTextProvider (harfrust + swash) instead.
/// Only kept for testing/comparison purposes.
#[cfg(feature = "cosmic-text")]
impl TextProvider for CosmicTextProvider {
    // ... existing code ...
}
```

### ❌ Unused Display List Caching

#### 3. Remove complex text caching in PassManager (if present)

**File**: `engine-core/src/pass_manager.rs`

**Check for**:
- Text run caching beyond simple atlas
- Complex shape caching
- Display list text optimization

**Keep**:
- Glyph atlas (texture cache)
- Basic batch rendering

**Remove**:
- Any complex per-run caching
- Shape result caching (harfrust is fast enough)

### ❌ Unused Configuration Options

#### 4. Simplify text provider selection

**File**: `crates/rune-scene/src/lib.rs` (lines ~102-141)

**Current code**:
```rust
let provider: std::sync::Arc<dyn engine_core::TextProvider> =
    match std::env::var("RUNE_TEXT_PROVIDER") {
        Ok(ref v) if v.eq_ignore_ascii_case("cosmic") => {
            std::sync::Arc::new(
                engine_core::CosmicTextProvider::from_system_fonts(SubpixelOrientation::RGB),
            )
        }
        _ => {
            // Default: rune-text + harfrust
            // ... complex font loading logic ...
        }
    };
```

**Simplified version**:
```rust
// Default to harfrust + swash (RuneTextProvider)
let provider: std::sync::Arc<dyn engine_core::TextProvider> = {
    // Check for custom font path
    if let Ok(path) = std::env::var("RUNE_TEXT_FONT") {
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(p) = engine_core::RuneTextProvider::from_bytes(
                &bytes,
                SubpixelOrientation::RGB,
            ) {
                std::sync::Arc::new(p)
            } else {
                eprintln!("Failed to load font from {}, using system fonts", path);
                create_default_provider()
            }
        } else {
            eprintln!("Failed to read font file {}, using system fonts", path);
            create_default_provider()
        }
    } else {
        create_default_provider()
    }
};

fn create_default_provider() -> std::sync::Arc<dyn engine_core::TextProvider> {
    std::sync::Arc::new(
        engine_core::RuneTextProvider::from_system_fonts(SubpixelOrientation::RGB)
            .expect("Failed to load system fonts")
    )
}
```

**Remove**:
- `RUNE_TEXT_PROVIDER=cosmic` option
- Complex nested if/else chains

### ❌ Unused Rendering Methods

#### 5. Remove unused multiline text methods

**File**: `crates/rune-scene/src/elements/multiline_text.rs`

**Keep**:
- `render_cached()` - Primary method (uses cache)

**Consider removing** (or mark as deprecated):
- `render()` - Uses provider directly, no caching
- `render_fast()` - Redundant with render_cached
- `render_simple()` - Only splits on newlines, no wrapping

**Recommendation**: Keep all for now, but add deprecation warnings:
```rust
/// DEPRECATED: Use render_cached() instead for better performance
#[deprecated(since = "0.2.0", note = "Use render_cached() for better performance")]
pub fn render_simple(&self, canvas: &mut Canvas, z: i32) {
    // ... existing code ...
}
```

### ❌ Dead Code and Warnings

#### 6. Fix compiler warnings

**File**: `engine-core/src/pass_manager.rs:118`

**Current warning**:
```
warning: field `text_mask_atlas_view` is never read
```

**Action**: Either use it or remove it. Check if it's needed for text rendering.

#### 7. Remove unused imports and features

**Check all text-related files for**:
- Unused `use` statements
- Unused feature flags
- Commented-out code
- TODO comments that are done

## Documentation Improvements

### 📝 Add Module-Level Documentation

#### 8. Document text.rs

**File**: `engine-core/src/text.rs`

**Add at top**:
```rust
//! Text rendering providers for rune-draw.
//!
//! The primary provider is [`RuneTextProvider`] which uses:
//! - `harfrust` for text shaping (HarfBuzz implementation)
//! - `swash` for glyph rasterization
//! - `fontdb` for font discovery and fallback
//!
//! This provides high-quality text rendering with:
//! - Proper kerning and ligatures
//! - Subpixel RGB rendering
//! - BiDi support
//! - Complex script support
//!
//! # Example
//! ```no_run
//! use engine_core::{RuneTextProvider, SubpixelOrientation, TextRun};
//!
//! let provider = RuneTextProvider::from_system_fonts(SubpixelOrientation::RGB)
//!     .expect("Failed to load fonts");
//!
//! let run = TextRun {
//!     text: "Hello, world!".to_string(),
//!     pos: [0.0, 0.0],
//!     size: 16.0,
//!     color: ColorLinPremul::rgba(255, 255, 255, 255),
//! };
//!
//! let glyphs = provider.rasterize_run(&run);
//! ```
```

#### 9. Document text_layout.rs

**File**: `engine-core/src/text_layout.rs`

**Improve existing docs**:
```rust
//! Fast text layout and wrapping utilities with caching support.
//!
//! This module provides efficient text wrapping that can be cached between frames
//! to avoid expensive per-frame string allocations and processing.
//!
//! # Performance
//! - Uses character-count approximation for fast wrapping
//! - Cache provides ~95% hit rate during window resize
//! - Mutex-protected for thread safety
//! - Lazy eviction (only when 2x capacity)
//!
//! # Example
//! ```no_run
//! use engine_core::TextLayoutCache;
//!
//! let cache = TextLayoutCache::new(200);
//! let wrapped = cache.get_or_wrap(
//!     "Long text that needs wrapping...",
//!     400.0,  // max_width
//!     16.0,   // font_size
//!     1.2,    // line_height_factor
//! );
//!
//! for line in wrapped.lines {
//!     // Render each line...
//! }
//! ```
```

#### 10. Document canvas.rs text methods

**File**: `rune-surface/src/canvas.rs`

**Improve draw_text_run docs**:
```rust
/// Draw text using direct rasterization (recommended).
///
/// This method rasterizes glyphs immediately using the text provider,
/// bypassing complex display list paths. This is simpler and more
/// reliable than deferred rendering.
///
/// # Performance
/// - Glyphs are shaped and rasterized on each call
/// - Use [`TextLayoutCache`] to cache wrapping computations
/// - Debounce resize events to avoid excessive rasterization
///
/// # Transform Stack
/// The current transform is applied to position text correctly
/// within zones (viewport, toolbar, etc.).
///
/// # DPI Scaling
/// Both position and size are automatically scaled by `self.dpi_scale`.
///
/// # Example
/// ```no_run
/// canvas.draw_text_run(
///     [10.0, 20.0],
///     "Hello, world!".to_string(),
///     16.0,
///     Color::rgba(255, 255, 255, 255),
///     10,  // z-index
/// );
/// ```
pub fn draw_text_run(&mut self, origin: [f32; 2], text: String, 
                     size_px: f32, color: ColorLinPremul, z: i32) {
    // ... existing code ...
}
```

## Testing After Cleanup

### ✅ Verification Checklist

After making changes, verify:

1. **Build succeeds**
   ```bash
   cargo build --package rune-scene
   cargo build --package engine-core
   ```

2. **Tests pass**
   ```bash
   cargo test --package engine-core
   cargo test --package rune-scene
   ```

3. **Demo runs correctly**
   ```bash
   cargo run --package rune-scene
   ```

4. **Text renders correctly**
   - [ ] Multiple paragraphs visible
   - [ ] Word wrapping works
   - [ ] Paragraph breaks preserved
   - [ ] Colors and sizes correct
   - [ ] No visual regressions

5. **Resize works smoothly**
   - [ ] No hanging or stuttering
   - [ ] Text reflows after 200ms
   - [ ] No crashes or panics

6. **Custom font loading works**
   ```bash
   RUNE_TEXT_FONT=/path/to/font.ttf cargo run --package rune-scene
   ```

## Implementation Order

### Phase 1: Safe Cleanup (Low Risk)
1. Add documentation to text.rs, text_layout.rs, canvas.rs
2. Fix compiler warnings (text_mask_atlas_view)
3. Remove unused imports
4. Add deprecation warnings to old methods

### Phase 2: Provider Cleanup (Medium Risk)
5. Remove fontdue providers (if present)
6. Simplify provider selection logic
7. Add comments to cosmic-text provider

### Phase 3: Code Removal (Higher Risk)
8. Remove unused display list caching (if present)
9. Consider removing deprecated render methods
10. Final testing and verification

## Rollback Plan

If anything breaks:

1. **Git revert** to last working commit
2. **Identify** which change caused the issue
3. **Fix** the specific issue
4. **Test** thoroughly before proceeding

## Success Criteria

Cleanup is successful when:

✅ All compiler warnings resolved  
✅ Code builds without errors  
✅ All tests pass  
✅ Demo runs and text renders correctly  
✅ Resize performance maintained  
✅ Documentation improved  
✅ Unused code removed  
✅ Single clear text rendering path (harfrust + swash)  

## Future Maintenance

### Guidelines for Future Changes

1. **Always use RuneTextProvider** (harfrust + swash)
2. **Always use render_cached()** for multiline text
3. **Always use TextLayoutCache** for wrapping
4. **Always debounce resize events** (200ms)
5. **Never reintroduce** fontdue or complex caching

### When to Revisit

Only revisit this architecture if:
- Performance degrades significantly (>100ms render times)
- New requirements need features not in harfrust
- Memory usage becomes problematic (>100MB for text)

Otherwise, keep the current simple, working approach.
