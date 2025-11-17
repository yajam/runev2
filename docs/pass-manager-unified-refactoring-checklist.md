# Pass Manager Unified Rendering Refactoring Checklist

## 🚨 CRITICAL BUG - IMMEDIATE ACTION REQUIRED

**Status**: viewport_ir renders **blank black screen** with `use_unified_rendering = true`

**Root Cause**: `render_unified()` exits early after rendering only solids (line 2958), skipping all text, images, and SVGs.

**Fix Required**: Complete **Phase 0** below - implement full unified pass rendering (2-4 hours)

**Workaround**: Set `surf.set_use_unified_rendering(false)` to use legacy multi-pass rendering

---

## Overview

This document outlines a phased approach to refactor `pass_manager.rs` to fully support unified pass rendering for images, text, and overlays. The goal is to eliminate multi-pass rendering and achieve optimal z-ordering performance with a single render pass.

**Current Status**: Phase 1-2 of depth buffer implementation complete, but unified rendering (Phase 3) is incomplete. Text, images, and overlays still render in separate passes after solids.

**Target Architecture**: Single unified render pass where all element types (solids, text, images, SVGs) are rendered together with proper z-ordering via depth testing.

---

## Phase 0: Fix Critical Blank Screen Bug 🚨 URGENT - BLOCKING ISSUE

### Problem Statement

**Current Status**: `viewport_ir` renders a blank black screen when `use_unified_rendering = true`.

**Root Cause**: The `render_unified()` method in `pass_manager.rs` is incomplete. It only renders solids and then immediately returns (lines 2937-2958), completely skipping text, images, and SVGs. This causes all non-solid content to disappear.

**Code Location**: `crates/engine-core/src/pass_manager.rs`, lines 2944-2958

```rust
// Render solids first (they're already sorted by z-index in the scene)
self.solid_direct.record(&mut pass, &vp_bg_direct, &z_bg, scene);

// Drop the pass to end it
drop(pass);

// TODO: Integrate text, images, and SVGs into the unified pass
// Currently these are rendered separately and don't participate in unified z-ordering
// For now, unified rendering only handles solids from the display list
// ...
let _ = (glyph_draws, svg_draws, image_draws); // Silence unused warnings

return; // ⚠️ EXITS EARLY - TEXT/IMAGES/SVGS NEVER RENDERED
```

### Impact

- **Severity**: Critical - Renders application unusable
- **Affected**: All code using `set_use_unified_rendering(true)` (including viewport_ir)
- **Workaround**: Set `use_unified_rendering = false` to use legacy multi-pass rendering

### Objectives

1. Make `render_unified()` render text, images, and SVGs after solids
2. Restore full functionality to unified rendering path
3. Maintain proper z-ordering via depth testing

### Tasks

#### 0.1 Full Unified Pass Fix: Render All Elements Within Single Pass

**Strategy**: Keep the render pass alive and render text, images, and SVGs within the same pass as solids. This achieves true unified rendering with optimal z-ordering and minimal state changes.

**Key Insight**: We already have the infrastructure - `TextRenderer`, `ImageRenderer` all have `record()` methods that accept a `RenderPass`. We just need to call them before dropping the pass.

**Action Items**:

- [ ] **Remove early return and TODO comments** (lines 2941-2958)
- [ ] **Keep render pass alive** - Don't drop it after solids
- [ ] **Render text within the same pass**:
  ```rust
  // Render solids first (they're already sorted by z-index in the scene)
  self.solid_direct.record(&mut pass, &vp_bg_direct, &z_bg, scene);
  
  // Render text glyphs within same pass
  if !glyph_draws.is_empty() {
      // Upload all glyph masks to atlas before rendering
      let mut atlas_cursor_x = 0u32;
      let mut atlas_cursor_y = 0u32;
      let mut next_row_height = 0u32;
      let mut vertices: Vec<TextQuadVtx> = Vec::new();
      
      for (origin, glyph, color) in glyph_draws.iter() {
          let mask = &glyph.mask;
          let w = mask.width;
          let h = mask.height;
          
          // Row wrap logic
          if atlas_cursor_x + w >= 4096 {
              atlas_cursor_x = 0;
              atlas_cursor_y += next_row_height;
              next_row_height = 0;
          }
          next_row_height = next_row_height.max(h);
          
          // Upload mask to atlas
          queue.write_texture(
              wgpu::ImageCopyTexture {
                  texture: &self.text_mask_atlas,
                  mip_level: 0,
                  origin: wgpu::Origin3d { x: atlas_cursor_x, y: atlas_cursor_y, z: 0 },
                  aspect: wgpu::TextureAspect::All,
              },
              &mask.data,
              wgpu::ImageDataLayout {
                  offset: 0,
                  bytes_per_row: Some(w * mask.bytes_per_pixel() as u32),
                  rows_per_image: Some(h),
              },
              wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
          );
          
          // Compute UVs and create quad vertices
          let u0 = atlas_cursor_x as f32 / 4096.0;
          let v0 = atlas_cursor_y as f32 / 4096.0;
          let u1 = (atlas_cursor_x + w) as f32 / 4096.0;
          let v1 = (atlas_cursor_y + h) as f32 / 4096.0;
          
          vertices.extend_from_slice(&[
              TextQuadVtx { pos: [origin[0], origin[1]], uv: [u0, v0], color: [color.r, color.g, color.b, color.a] },
              TextQuadVtx { pos: [origin[0] + w as f32, origin[1]], uv: [u1, v0], color: [color.r, color.g, color.b, color.a] },
              TextQuadVtx { pos: [origin[0] + w as f32, origin[1] + h as f32], uv: [u1, v1], color: [color.r, color.g, color.b, color.a] },
              TextQuadVtx { pos: [origin[0], origin[1] + h as f32], uv: [u0, v1], color: [color.r, color.g, color.b, color.a] },
          ]);
          
          atlas_cursor_x += w;
      }
      
      // Upload vertices and indices
      queue.write_buffer(&self.text_vertex_buffer, 0, bytemuck::cast_slice(&vertices));
      let quad_count = vertices.len() / 4;
      let mut indices: Vec<u16> = Vec::with_capacity(quad_count * 6);
      for i in 0..quad_count {
          let base = (i * 4) as u16;
          indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
      }
      queue.write_buffer(&self.text_index_buffer, 0, bytemuck::cast_slice(&indices));
      
      // Render text within the same pass
      let vp_bg_text = self.text.vp_bind_group(&self.device, &self.vp_buffer);
      let z_bg_text = self.create_z_bind_group(0.0, queue);
      pass.set_pipeline(&self.text.pipeline);
      pass.set_bind_group(0, &vp_bg_text, &[]);
      pass.set_bind_group(1, &z_bg_text, &[]);
      pass.set_bind_group(2, &self.text_bind_group, &[]);
      pass.set_vertex_buffer(0, self.text_vertex_buffer.slice(..));
      pass.set_index_buffer(self.text_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
      pass.draw_indexed(0..(quad_count * 6) as u32, 0, 0..1);
  }
  ```

- [ ] **Render images within the same pass**:
  ```rust
  // Render images within same pass
  for (path, origin, size, _z) in image_draws.iter() {
      if let Some((tex_view, _w, _h)) = self.try_get_image_view(std::path::Path::new(path)) {
          // Create image quad geometry
          let verts = [
              ImageQuadVtx { pos: [origin[0], origin[1]], uv: [0.0, 0.0] },
              ImageQuadVtx { pos: [origin[0] + size[0], origin[1]], uv: [1.0, 0.0] },
              ImageQuadVtx { pos: [origin[0] + size[0], origin[1] + size[1]], uv: [1.0, 1.0] },
              ImageQuadVtx { pos: [origin[0], origin[1] + size[1]], uv: [0.0, 1.0] },
          ];
          let idx: [u16; 6] = [0, 1, 2, 0, 2, 3];
          
          // Create temporary buffers (TODO: optimize with pooled buffers)
          let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
              label: Some("image-vbuf-unified"),
              contents: bytemuck::cast_slice(&verts),
              usage: wgpu::BufferUsages::VERTEX,
          });
          let ibuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
              label: Some("image-ibuf-unified"),
              contents: bytemuck::cast_slice(&idx),
              usage: wgpu::BufferUsages::INDEX,
          });
          
          // Render image within the same pass
          let vp_bg_img = self.image.vp_bind_group(&self.device, &self.vp_buffer);
          let z_bg_img = self.create_z_bind_group(0.0, queue);
          let tex_bg = self.image.tex_bind_group(&self.device, &tex_view);
          self.image.record(&mut pass, &vp_bg_img, &z_bg_img, &tex_bg, &vbuf, &ibuf, 6);
      }
  }
  ```

- [ ] **Render SVGs within the same pass** (similar to images):
  ```rust
  // Render SVGs within same pass
  for (path, origin, max_size, style, _z, _transform) in svg_draws.iter() {
      if let Some((view, w, h)) = self.rasterize_svg_to_view(
          std::path::Path::new(path),
          1.0,
          *style,
          queue,
      ) {
          let base_w = w.max(1) as f32;
          let base_h = h.max(1) as f32;
          let scale = (max_size[0] / base_w).min(max_size[1] / base_h).max(0.0);
          
          if let Some((view_scaled, sw, sh)) = self.rasterize_svg_to_view(
              std::path::Path::new(path),
              scale,
              *style,
              queue,
          ) {
              // Create SVG quad geometry and render (same as image rendering)
              let size = [sw as f32, sh as f32];
              let verts = [
                  ImageQuadVtx { pos: [origin[0], origin[1]], uv: [0.0, 0.0] },
                  ImageQuadVtx { pos: [origin[0] + size[0], origin[1]], uv: [1.0, 0.0] },
                  ImageQuadVtx { pos: [origin[0] + size[0], origin[1] + size[1]], uv: [1.0, 1.0] },
                  ImageQuadVtx { pos: [origin[0], origin[1] + size[1]], uv: [0.0, 1.0] },
              ];
              let idx: [u16; 6] = [0, 1, 2, 0, 2, 3];
              
              let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                  label: Some("svg-vbuf-unified"),
                  contents: bytemuck::cast_slice(&verts),
                  usage: wgpu::BufferUsages::VERTEX,
              });
              let ibuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                  label: Some("svg-ibuf-unified"),
                  contents: bytemuck::cast_slice(&idx),
                  usage: wgpu::BufferUsages::INDEX,
              });
              
              let vp_bg_svg = self.image.vp_bind_group(&self.device, &self.vp_buffer);
              let z_bg_svg = self.create_z_bind_group(0.0, queue);
              let tex_bg = self.image.tex_bind_group(&self.device, &view_scaled);
              self.image.record(&mut pass, &vp_bg_svg, &z_bg_svg, &tex_bg, &vbuf, &ibuf, 6);
          }
      }
  }
  
  // NOW drop the pass - all rendering complete
  drop(pass);
  ```

**Why This Is The Proper Fix**:
- ✅ True unified rendering - all elements in single pass
- ✅ Minimal state changes - one render pass instead of 4+
- ✅ Optimal z-ordering - depth test handles everything in hardware
- ✅ No depth buffer load/store overhead between passes
- ✅ Matches the original Phase 3 vision

**Vertex Structure Definitions** (add to pass_manager.rs if not present):
```rust
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TextQuadVtx {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ImageQuadVtx {
    pos: [f32; 2],
    uv: [f32; 2],
}
```

**Files to Modify**:
- `crates/engine-core/src/pass_manager.rs`: Lines 2937-2958 (replace early return with full rendering)
- Add `wgpu::util::DeviceExt` import for `create_buffer_init`

#### 0.2 Handle Offscreen Path

**Current Issue**: Line 2961 shows offscreen path falls back to separate passes without rendering text/images/SVGs.

**Action Items**:
- [ ] Implement unified rendering for offscreen path (similar to direct path)
- [ ] Render to offscreen target instead of surface_view
- [ ] Keep same single-pass pattern: solids → text → images → SVGs
- [ ] Test with `use_intermediate = true` configuration

#### 0.3 Known Issues and Optimizations

**Performance Considerations**:
- ⚠️ **Buffer creation per image/SVG**: Current approach creates new vertex/index buffers for each image/SVG quad
  - **Impact**: Allocations on hot path, potential performance issue with many images
  - **Fix**: Pre-allocate pooled buffers or use ring buffer strategy (Phase 6 optimization)
  - **Acceptable for now**: Restores functionality, can optimize later

**Texture Atlas Upload**:
- ⚠️ **Text atlas upload happens during render pass creation**: `queue.write_texture()` calls before pass
  - **Impact**: Correct - texture uploads must happen before render pass
  - **Note**: This is the same pattern as `draw_text_mask()`, proven to work

**Bind Group Creation**:
- ⚠️ **Creating bind groups per element**: `vp_bind_group()`, `create_z_bind_group()` called in loop
  - **Impact**: Allocations on hot path
  - **Fix**: Cache bind groups (Phase 2.2 - Z-index bind group cache)
  - **Acceptable for now**: Matches existing pattern in separate pass rendering

**Z-Index Handling**:
- ⚠️ **Currently using z-index 0.0 for all elements**: `create_z_bind_group(0.0, queue)`
  - **Impact**: All elements at same depth layer, rely on draw order
  - **Fix**: Pass actual z-index from glyph_draws/image_draws/svg_draws
  - **TODO**: Extract z-index from draw data and pass to bind group creation
  - **Critical**: This needs to be fixed for proper z-ordering!

**Correct Z-Index Implementation**:
```rust
// For text - extract z from glyph data (need to add z to RasterizedGlyph)
let z_bg_text = self.create_z_bind_group(glyph.z_index as f32, queue);

// For images - use z from image_draws tuple
let z_bg_img = self.create_z_bind_group(*_z as f32, queue);

// For SVGs - use z from svg_draws tuple  
let z_bg_svg = self.create_z_bind_group(*_z as f32, queue);
```

**Action Items**:
- [ ] Update `RasterizedGlyph` to include z-index field (or pass separately)
- [ ] Use actual z-index values instead of hardcoded 0.0
- [ ] Test z-ordering: text over solids, images over text, etc.

#### 0.4 Testing

**Critical Test Cases**:
- [ ] Verify viewport_ir renders correctly with text visible
- [ ] Test that images appear when loaded
- [ ] Test SVG rendering in unified mode
- [ ] Verify z-ordering: text over solids, images over text
- [ ] Test with `use_intermediate = true` and `false`
- [ ] Test with `direct = true` and `false`

**Expected Outcome**: viewport_ir should render normally with all content visible, matching the legacy rendering path.

#### 0.5 Update Documentation

**Action Items**:
- [ ] Remove TODO comments in `render_unified()` (lines 2944-2954)
- [ ] Add comments explaining unified pass rendering flow
- [ ] Update `z-layering-depth-buffer-implementation.md` to mark Phase 3 as complete
- [ ] Document that unified rendering now handles all element types in single pass
- [ ] Add performance notes about buffer pooling optimization (deferred to Phase 6)

### Success Criteria

- ✅ viewport_ir renders without blank screen
- ✅ All element types (solids, text, images, SVGs) are visible
- ✅ Z-ordering works correctly via depth testing
- ✅ No performance regression compared to legacy path
- ✅ Works with all configuration combinations (direct/offscreen, intermediate/no-intermediate)

### Timeline

**Estimated Time**: 2-4 hours (urgent fix)

**Priority**: CRITICAL - Must be completed before any other refactoring work

---

## Phase 1: Audit and Document Current Multi-Pass Architecture ✅ PARTIALLY COMPLETE

### Objectives
- Understand current rendering flow
- Identify all render pass creation sites
- Document dependencies between passes
- Map out data flow for each element type

### Tasks

#### 1.1 Document Current Rendering Methods
- [x] `render_frame()` - Direct solid rendering with MSAA (lines 2655-2779)
- [x] `render_frame_internal()` - Internal solid rendering (lines 2527-2652)
- [x] `render_frame_and_text()` - Solids + text in separate passes (lines 2781-2812)
- [x] `render_frame_with_intermediate()` - Vello-style with intermediate texture (lines 2431-2477)
- [x] `render_frame_with_intermediate_and_text()` - Intermediate + text (lines 2479-2525)
- [x] `render_unified()` - Incomplete unified rendering (lines 2814-2959)
- [x] `draw_text_mask()` - Separate text rendering pass (lines 490-696)
- [x] `draw_image_quad()` - Separate image rendering pass (lines 318-424)
- [ ] Document SVG rendering flow (likely in surface.rs or scene-level code)

#### 1.2 Identify Multi-Pass Bottlenecks
- [ ] Count render pass creations per frame in typical usage
- [ ] Measure overhead of pass transitions (state changes, barriers)
- [ ] Profile depth buffer clear/load operations
- [ ] Identify redundant viewport/uniform updates

#### 1.3 Map Element Type Dependencies
- [x] Solids: Independent, can render first
- [x] Text: Depends on viewport, uses separate atlas texture
- [x] Images: Depends on viewport, uses cached textures
- [ ] SVGs: Document rasterization and rendering flow
- [ ] Overlays: Identify what constitutes an "overlay" in the codebase

**Deliverable**: Architecture diagram showing current multi-pass flow with timing/dependency annotations.

---

## Phase 2: Consolidate Viewport and Uniform Management 🚧 IN PROGRESS

### Objectives
- Eliminate redundant uniform buffer updates
- Create unified viewport management
- Prepare for single-pass rendering

### Tasks

#### 2.1 Unify Viewport Buffer Usage
- [x] Current: `vp_buffer` and `vp_buffer_text` (separate buffers)
- [ ] **Action**: Merge into single `vp_buffer` used by all pipelines
- [ ] Update all bind group creations to use unified buffer
- [ ] Remove `vp_buffer_text` field from `PassManager`
- [ ] Test: Verify text rendering still works with unified viewport

**Files to modify**:
- `pass_manager.rs`: Lines 99-100 (buffer fields), 187-192 (buffer creation)
- `pass_manager.rs`: Line 510 (text rendering viewport usage)

#### 2.2 Consolidate Z-Index Bind Group Creation
- [x] Current: `create_z_bind_group()` creates bind groups on-demand
- [ ] **Action**: Create reusable z-index bind groups for common values (0, ±100, ±1000)
- [ ] Cache bind groups in `PassManager` to avoid per-frame allocations
- [ ] Add `z_bind_group_cache: HashMap<i32, wgpu::BindGroup>` field

**Rationale**: Reduces allocations when most elements use default z-index values.

#### 2.3 Batch Uniform Updates
- [ ] **Action**: Collect all uniform updates at frame start
- [ ] Write viewport, z-index, and other uniforms in single batch
- [ ] Avoid mid-frame uniform buffer writes where possible

**Deliverable**: Unified uniform management system with reduced per-frame allocations.

---

## Phase 3: Refactor Element Rendering to Support Unified Pass 🎯 CRITICAL

### Objectives
- Make text, image, and SVG rendering compatible with unified pass
- Enable depth testing for all element types in single pass
- Eliminate separate render pass requirements

### Tasks

#### 3.1 Refactor Text Rendering for Unified Pass

**Current Issue**: `draw_text_mask()` creates its own render pass (lines 671-684).

**Action Items**:
- [ ] Extract text rendering logic from `draw_text_mask()` into separate method
- [ ] Create `record_text_batch()` method that accepts existing `RenderPass`
- [ ] Move atlas upload logic before render pass creation
- [ ] Update signature:
  ```rust
  pub fn record_text_batch<'a>(
      &'a self,
      pass: &mut wgpu::RenderPass<'a>,
      glyphs: &[(SubpixelMask, [f32; 2], ColorLinPremul)],
      z_bind_group: &'a wgpu::BindGroup,
  )
  ```
- [ ] Keep `draw_text_mask()` as convenience wrapper for legacy code
- [ ] Test: Verify text renders correctly in unified pass

**Files to modify**:
- `pass_manager.rs`: Lines 490-696 (text rendering)

#### 3.2 Refactor Image Rendering for Unified Pass

**Current Issue**: `draw_image_quad()` creates its own render pass (lines 408-421).

**Action Items**:
- [ ] Extract image rendering logic into `record_image_quad()` method
- [ ] Accept existing `RenderPass` as parameter
- [ ] Move vertex/index buffer creation outside render pass
- [ ] Update signature:
  ```rust
  pub fn record_image_quad<'a>(
      &'a self,
      pass: &mut wgpu::RenderPass<'a>,
      origin: [f32; 2],
      size: [f32; 2],
      tex_view: &'a wgpu::TextureView,
      z_bind_group: &'a wgpu::BindGroup,
  )
  ```
- [ ] Keep `draw_image_quad()` as convenience wrapper
- [ ] Test: Verify images render correctly in unified pass

**Files to modify**:
- `pass_manager.rs`: Lines 318-424 (image rendering)

#### 3.3 Refactor SVG Rendering for Unified Pass

**Action Items**:
- [ ] Locate SVG rendering code (likely in surface.rs or higher-level API)
- [ ] Follow same pattern as text/image refactoring
- [ ] Create `record_svg_quad()` method accepting existing render pass
- [ ] Document SVG texture caching behavior

**Files to modify**: TBD after locating SVG rendering code

#### 3.4 Define "Overlay" Rendering Strategy

**Action Items**:
- [ ] Clarify what constitutes an "overlay" in the codebase
- [ ] Determine if overlays are just high-z-index elements or special rendering
- [ ] If special: Create `record_overlay()` method
- [ ] If not special: Document that overlays use standard unified rendering

**Deliverable**: All element types can render within a single `RenderPass` without creating their own passes.

---

## Phase 4: Implement Complete Unified Rendering 🎯 CRITICAL

### Objectives
- Complete the `render_unified()` implementation
- Integrate text, images, and SVGs into unified pass
- Achieve single-pass rendering for all element types

### Tasks

#### 4.1 Complete `render_unified()` Implementation

**Current Issue**: Lines 2929-2941 show TODOs - text/images/SVGs are skipped.

**Action Items**:
- [ ] Remove TODO comments and unused parameter silencing (line 2941)
- [ ] After render pass creation (line 2903), keep pass alive for all rendering
- [ ] Call `record_text_batch()` for glyph draws within the same pass
- [ ] Call `record_image_quad()` for image draws within the same pass
- [ ] Call `record_svg_quad()` for SVG draws within the same pass
- [ ] Ensure proper z-index bind groups are passed for each element type

**Pseudocode**:
```rust
let mut pass = encoder.begin_render_pass(...);

// Render solids (already implemented)
self.solid_direct.record(&mut pass, &vp_bg_direct, &z_bg, scene);

// Render text glyphs
for (origin, glyph, color) in glyph_draws {
    let z_bg = self.create_z_bind_group(glyph.z_index, queue);
    self.record_text_batch(&mut pass, &[(glyph.mask, *origin, *color)], &z_bg);
}

// Render images
for (path, origin, size, z) in image_draws {
    if let Some((view, _, _)) = self.try_get_image_view(path) {
        let z_bg = self.create_z_bind_group(*z as f32, queue);
        self.record_image_quad(&mut pass, *origin, *size, &view, &z_bg);
    }
}

// Render SVGs
for (path, origin, size, style, z, transform) in svg_draws {
    // Similar pattern
}

drop(pass); // End unified render pass
```

**Files to modify**:
- `pass_manager.rs`: Lines 2814-2959 (`render_unified()`)

#### 4.2 Handle Offscreen Unified Rendering

**Current Issue**: Line 2946 shows offscreen path falls back to separate passes.

**Action Items**:
- [ ] Implement unified rendering for offscreen path (non-direct mode)
- [ ] Allocate offscreen target with depth attachment
- [ ] Follow same unified rendering pattern as direct mode
- [ ] Test with intermediate texture workflow

#### 4.3 Optimize Z-Index Sorting

**Action Items**:
- [ ] Create unified draw command enum:
  ```rust
  enum UnifiedDrawCommand<'a> {
      Solid { /* scene data */ },
      Text { origin: [f32; 2], glyph: &'a RasterizedGlyph, color: ColorLinPremul },
      Image { path: &'a Path, origin: [f32; 2], size: [f32; 2] },
      Svg { path: &'a Path, origin: [f32; 2], size: [f32; 2], style: Option<SvgStyle> },
  }
  ```
- [ ] Sort all draw commands by z-index before rendering
- [ ] Batch consecutive commands of same type to reduce state changes
- [ ] Profile: Measure performance improvement from sorting

**Deliverable**: Fully functional `render_unified()` that handles all element types in single pass.

---

## Phase 5: Deprecate Legacy Multi-Pass Methods 🧹 CLEANUP

### Objectives
- Remove or deprecate old multi-pass rendering methods
- Update all call sites to use unified rendering
- Maintain backward compatibility where needed

### Tasks

#### 5.1 Identify Call Sites
- [ ] Search codebase for calls to:
  - `render_frame_and_text()`
  - `render_frame_with_intermediate_and_text()`
  - `draw_text_mask()` (direct calls)
  - `draw_image_quad()` (direct calls)
- [ ] Document which call sites are in public API vs internal

#### 5.2 Update Call Sites to Use Unified Rendering
- [ ] Replace multi-pass calls with `render_unified()`
- [ ] Update surface.rs `end_frame()` to always use unified path
- [ ] Remove `use_unified_rendering` flag (make it always-on)
- [ ] Test: Ensure all existing demos/examples still work

**Files to modify**:
- `crates/rune-scene/src/surface.rs` (or similar surface implementation)
- Any demo/example code using old API

#### 5.3 Deprecate or Remove Legacy Methods
- [ ] Add `#[deprecated]` attributes to old methods
- [ ] Update documentation to recommend unified rendering
- [ ] Consider removing methods entirely if no external users
- [ ] Keep convenience wrappers if they're part of public API

**Methods to deprecate**:
- `render_frame_and_text()` - Use `render_unified()` instead
- `render_frame_with_intermediate_and_text()` - Use `render_unified()` with intermediate texture
- Direct `draw_text_mask()` calls - Use `record_text_batch()` within unified pass
- Direct `draw_image_quad()` calls - Use `record_image_quad()` within unified pass

**Deliverable**: Clean API with unified rendering as the primary path.

---

## Phase 6: Optimize Unified Rendering Performance 🚀 OPTIMIZATION

### Objectives
- Minimize state changes within unified pass
- Optimize draw call batching
- Reduce GPU pipeline switches

### Tasks

#### 6.1 Implement Material Batching
- [ ] Group draw commands by pipeline type (solid/text/image/svg)
- [ ] Sort within each group by z-index
- [ ] Render in batches to minimize pipeline switches
- [ ] Profile: Measure reduction in pipeline binds

**Strategy**:
```
1. Sort all commands by z-index
2. Identify "batch boundaries" where pipeline must change
3. Within each batch, render all commands of same type
4. Use depth testing to maintain correct z-order
```

#### 6.2 Optimize Bind Group Management
- [ ] Implement bind group caching (from Phase 2.2)
- [ ] Reuse viewport bind groups across frames
- [ ] Pool z-index bind groups for common values
- [ ] Profile: Measure reduction in bind group allocations

#### 6.3 Reduce Vertex/Index Buffer Allocations
- [ ] Pre-allocate large vertex/index buffers for dynamic geometry
- [ ] Use ring buffer strategy for per-frame data
- [ ] Batch multiple quads into single buffer write
- [ ] Profile: Measure reduction in buffer allocations

**Current Issue**: `draw_image_quad()` creates new buffers per call (lines 374-385).

#### 6.4 Optimize Depth Buffer Usage
- [ ] Verify depth buffer format (Depth32Float vs Depth24Plus)
- [ ] Test if depth buffer can be shared across frames (clear vs load)
- [ ] Measure depth test performance impact
- [ ] Consider depth buffer compression if supported

**Deliverable**: Optimized unified rendering with minimal overhead.

---

## Phase 7: Testing and Validation ✅ VERIFICATION

### Objectives
- Ensure unified rendering produces correct output
- Verify z-ordering works across all element types
- Validate performance improvements

### Tasks

#### 7.1 Correctness Testing
- [ ] Create test scene with mixed element types at various z-indices
- [ ] Test cases:
  - Text over solid backgrounds
  - Images behind/in front of UI elements
  - SVGs interleaved with other elements
  - Overlapping elements at same z-index (verify tie-breaking)
  - Negative and positive z-indices
  - Extreme z-index values (±10000)
- [ ] Visual regression testing against legacy multi-pass rendering

#### 7.2 Performance Testing
- [ ] Benchmark frame time: unified vs legacy multi-pass
- [ ] Measure GPU time per pass (use timestamp queries)
- [ ] Profile CPU overhead (uniform updates, bind group creation)
- [ ] Test with varying scene complexity (10, 100, 1000+ elements)
- [ ] Expected improvement: 10-30% reduction in frame time

#### 7.3 Stress Testing
- [ ] Test with maximum glyph count (atlas capacity limits)
- [ ] Test with many images (texture cache limits)
- [ ] Test rapid z-index changes (bind group cache thrashing)
- [ ] Test window resizing with unified rendering
- [ ] Test MSAA on/off configurations

#### 7.4 Compatibility Testing
- [ ] Test on different GPU vendors (Metal, Vulkan, DX12)
- [ ] Verify depth buffer support on all platforms
- [ ] Test with different texture formats (sRGB, linear)
- [ ] Validate MSAA + depth buffer interaction

**Deliverable**: Comprehensive test suite validating unified rendering correctness and performance.

---

## Phase 8: Documentation and Migration Guide 📚 DOCUMENTATION

### Objectives
- Document new unified rendering API
- Provide migration guide for existing code
- Update architecture documentation

### Tasks

#### 8.1 Update API Documentation
- [ ] Document `render_unified()` with usage examples
- [ ] Document `record_text_batch()`, `record_image_quad()`, etc.
- [ ] Add rustdoc examples showing unified rendering
- [ ] Document z-index behavior and depth mapping

#### 8.2 Create Migration Guide
- [ ] Document breaking changes from multi-pass to unified
- [ ] Provide before/after code examples
- [ ] List deprecated methods and their replacements
- [ ] Explain performance benefits of migration

#### 8.3 Update Architecture Documentation
- [ ] Update `z-layering-depth-buffer-implementation.md` with Phase 3 completion
- [ ] Create architecture diagram showing unified rendering flow
- [ ] Document depth buffer management strategy
- [ ] Explain MSAA + depth buffer interaction

#### 8.4 Add Code Comments
- [ ] Comment complex sections of `render_unified()`
- [ ] Document bind group caching strategy
- [ ] Explain z-index to depth conversion
- [ ] Add safety comments for unsafe code (if any)

**Deliverable**: Complete documentation enabling developers to use and maintain unified rendering.

---

## Success Criteria

### Functional Requirements
- ✅ All element types (solids, text, images, SVGs, overlays) render in single pass
- ✅ Correct z-ordering across all element types
- ✅ Depth testing works correctly with MSAA
- ✅ No visual regressions compared to multi-pass rendering

### Performance Requirements
- ✅ 10-30% reduction in frame time for mixed-content scenes
- ✅ Fewer render pass transitions (target: 1 pass vs 4+ passes)
- ✅ Reduced uniform buffer updates (target: 50% reduction)
- ✅ No increase in memory usage

### Code Quality Requirements
- ✅ Clean separation between unified and legacy rendering
- ✅ Comprehensive test coverage
- ✅ Clear documentation and migration guide
- ✅ No unsafe code without justification

---

## Risk Mitigation

### Risk: Breaking Existing Code
**Mitigation**: 
- Keep legacy methods as deprecated wrappers
- Provide comprehensive migration guide
- Test all demos/examples before removing old code

### Risk: Performance Regression on Some GPUs
**Mitigation**:
- Test on multiple GPU vendors early
- Provide fallback to multi-pass if needed
- Profile on low-end hardware

### Risk: Depth Buffer Compatibility Issues
**Mitigation**:
- Test depth buffer support detection
- Fallback to painter's algorithm if depth unsupported
- Document platform-specific limitations

### Risk: Complex Refactoring Introduces Bugs
**Mitigation**:
- Implement in small, testable phases
- Visual regression testing after each phase
- Keep git history clean for easy rollback

---

## Timeline Estimate

- **Phase 0**: 2-4 hours (URGENT - fix blank screen bug) 🚨 **MUST DO FIRST**
- **Phase 1**: 2-3 days (audit and documentation)
- **Phase 2**: 3-4 days (uniform consolidation)
- **Phase 3**: 5-7 days (refactor element rendering) ⚠️ CRITICAL PATH
- **Phase 4**: 5-7 days (complete unified rendering) ⚠️ CRITICAL PATH
- **Phase 5**: 2-3 days (deprecate legacy methods)
- **Phase 6**: 4-5 days (optimization)
- **Phase 7**: 3-4 days (testing)
- **Phase 8**: 2-3 days (documentation)

**Total**: 26-36 days (5-7 weeks) + Phase 0 urgent fix

**Critical Path**: Phase 0 (blocking bug) → Phases 3-4 (refactoring and completing unified rendering)

---

## Next Immediate Actions

### URGENT (Do First)
1. **🚨 Complete Phase 0**: Fix blank screen bug in `render_unified()` - viewport_ir is currently unusable
   - Remove early return at line 2958
   - Add text/image/SVG rendering after solids pass
   - Test with viewport_ir to verify fix

### After Phase 0 Fix
2. **Complete Phase 1.1**: Document SVG and overlay rendering flow
3. **Start Phase 2.1**: Merge viewport buffers into single unified buffer
4. **Begin Phase 3.1**: Refactor `draw_text_mask()` to support unified pass
5. **Prototype Phase 4.1**: Test text rendering within unified pass

**Note**: Phase 0 must be completed immediately to restore basic functionality. All other phases can proceed after the critical bug is fixed.
