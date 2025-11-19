# SVG Rendering Architecture

## Overview

The engine supports **two SVG rendering paths** with **automatic detection** to choose the best approach:

### 1. Vector Tessellation (Preferred)
- **What**: Converts SVG paths to GPU geometry using `import_svg_geometry_to_painter()`
- **Advantages**:
  - ✅ Resolution-independent (perfect at any scale)
  - ✅ No scale bucketing needed
  - ✅ Lower memory usage (geometry vs textures)
  - ✅ True vector sharpness
  - ✅ Can be styled dynamically

### 2. Rasterization (Fallback)
- **What**: Rasterizes SVG to texture using `usvg` + `resvg`
- **Advantages**:
  - ✅ Supports all SVG features (gradients, filters, patterns, masks)
  - ✅ Consistent appearance for complex SVGs

## Automatic Detection

The `painter.svg()` API **automatically detects** which path to use:

```rust
painter.svg("images/search.svg", origin, max_size, z);
// → Auto-detects: Simple Lucide icon → Vector rendering
// → Perfect at any scale with no bucketing

painter.svg("images/weather-animated-icons.svg", origin, max_size, z);
// → Auto-detects: Complex gradients → Rasterization
// → Handles advanced SVG features
```

### Detection Logic

An SVG requires rasterization if it uses:
- Gradients (linear/radial)
- Patterns
- Filters (blur, drop-shadow, etc.)
- Embedded images
- Text-as-graphics
- Masks with alpha channels

Otherwise, it uses vector tessellation (paths with solid fills/strokes).

## Implementation Details

### Vector Path (Simple SVGs)
```rust
// In painter.svg():
1. Call svg_requires_rasterization() to check features
2. If false:
   - Get intrinsic size from SVG
   - Calculate scale to fit within max_size
   - Call import_svg_geometry_to_painter()
   - SVG paths → PathCmd → Lyon tessellation → GPU geometry
```

### Raster Path (Complex SVGs)
```rust
// In painter.svg():
1. Call svg_requires_rasterization() to check features
2. If true:
   - Add Command::DrawSvg to display list
   - During rendering:
     - Get intrinsic size
     - Calculate scale (bucketed to 13 levels)
     - Call rasterize_svg_to_view()
     - usvg parses → resvg rasterizes → GPU texture
```

## Scale Bucketing (Raster Path Only)

When rasterization is needed, textures are cached at bucketed scales to balance quality vs memory:

- 0.25x, 0.5x, 0.75x, 1.0x, 1.25x, 1.5x, 2.0x, 2.5x, 3.0x, 4.0x, 5.0x, 6.0x, 8.0x

**Example**: Requesting a 24×24 icon at 96px:
- Calculated scale: 4.0
- Bucketed to: 4.0x
- Rendered size: 96×96 ✓

## Performance Characteristics

| Aspect | Vector | Raster |
|--------|--------|--------|
| Resolution | Infinite | Limited by bucket |
| Memory | ~10KB geometry | ~100KB+ textures |
| Feature Support | Solid fills/strokes only | Full SVG |
| Scaling | Free | Re-rasterize on bucket change |
| Best For | Icons, simple graphics | Illustrations, complex art |

## Examples

### Lucide Icons (24×24 SVG)
```xml
<svg stroke="currentColor" fill="none">
  <path d="M20 6 9 17l-5-5"/>
</svg>
```
→ **Vector rendering**: Perfect at 16px, 24px, 48px, 96px, 192px, etc.

### Complex Weather Icon
```xml
<svg>
  <defs>
    <linearGradient id="sky">...</linearGradient>
    <filter id="blur">...</filter>
  </defs>
  <path fill="url(#sky)" filter="url(#blur)"/>
</svg>
```
→ **Rasterization**: Bucketed scales handle gradients/filters

## Migration Notes

**No API changes required!** Existing code continues to work:

```rust
// Before: Always rasterized
painter.svg("images/icon.svg", pos, size, z);

// After: Auto-detects
// - Simple icons → Vector (better!)
// - Complex SVGs → Raster (same as before)
```

## Manual Control (Advanced)

If you need explicit control:

```rust
// Force vector rendering (may skip unsupported features):
import_svg_geometry_to_painter(&mut painter, Path::new("icon.svg"));

// Force rasterization (always works):
// [Add DrawSvg command manually]
```

## Future Enhancements

Possible improvements:
1. Gradient support in vector path (via GPU shaders)
2. Pattern/filter translation to GPU effects
3. Adaptive bucketing based on viewport DPI
4. SVG fragment caching for partial updates
