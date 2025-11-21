# IR-WASM-IO Integration Assessment
**Date**: 2025-11-19
**Branch**: feature/integrate-ir-wasm
**Goal**: Enable dynamic rendering in rune-scene via declarative IR + WASM logic

---

## Executive Summary

Three crates have been imported from the legacy codebase:
- **rune-wasm**: WASM runtime for logic modules (clean, production-ready)
- **rune-ir**: Intermediate representation for data/view/logic (mixed quality, needs refactoring)
- **rune-io**: I/O utilities for file dialogs and HTTP (simple, stable)

**Overall Assessment**: The core IR concepts (data/view documents, mutations, packages) are solid and well-designed. However, significant cleanup is needed in the CSS/HTML parsing layers and layout integration points. The taffy dependency (referenced in `css/taffy_mapper.rs`) should be removed or replaced as it was intentionally excluded due to bugs.

**Recommended Approach**: Adopt the IR data model and mutation system, refactor the CSS layer to work without Taffy, integrate WASM runtime for dynamic logic, and wire up I/O services for interactive features.

---

## 1. Crate-by-Crate Analysis

### 1.1 rune-wasm (✅ Production-Ready)

**Purpose**: Wasmtime-based runtime for executing WASM logic modules with host callbacks.

**Architecture**:
- Wasmtime engine with linker for host imports
- Host API surface:
  - `rune.core_dispatch_mutation` / `rune.execute_mutation` / `rune.rune_execute_mutation` (backward-compatible aliases)
  - `rune.assets_fetch` (stubbed, gated by `network` capability)
- `MutationHandler` trait for receiving mutations from guest code
- Module lifecycle: `register_package_modules()` → `execute_module()` → `tick()`
- Supports both raw WASM and WAT (WebAssembly Text) formats

**Code Quality**: ✅ Excellent
- Clean separation of concerns
- Well-tested (see `tests::invokes_start_and_dispatches_mutation`)
- Proper error handling with `WasmRuntimeError`
- Capability system for security (e.g., `network` cap for fetch)
- No dependencies on problematic legacy code

**Integration Actions**:
- ✅ **Keep as-is** - No cleanup needed
- Wire `MutationHandler` to a new mutation processor in rune-scene
- Add to workspace `Cargo.toml` members
- Consider exposing `WasmRuntime` through a rune-scene service layer

**Risks**: None. This crate is stable.

---

### 1.2 rune-ir (⚠️ Requires Significant Refactoring)

**Purpose**: Intermediate representation for Rune packages (data model, view layout, logic modules, HTML/CSS parsing).

**Architecture**:
```
rune-ir/
├── data/document.rs      - DataDocument: semantic data nodes (text, actions, images, tables)
├── view/mod.rs           - ViewDocument: layout specs (flex, grid, overlays, widgets)
├── logic/
│   ├── mutation.rs       - IrMutation: runtime commands (ReplaceText, IrDiff, OpenOverlay, HttpFetch)
│   ├── diff.rs           - IrDiffOp: granular document patching
│   └── mod.rs            - LogicEngine enum (Wasm, Js)
├── package/mod.rs        - RunePackage: manifest, TOC, integrity checking, document loading
├── schema.rs             - JSON Schema validation for data/view documents
├── css/                  - ⚠️ Servo CSS parser + taffy_mapper (4800+ lines, complex, buggy)
└── html/                 - HTML parsing (29k+ tokens, large)
```

**Code Quality**: 🟡 Mixed

**✅ Strong Components** (keep with minor cleanup):
1. **Data Model** (`data/document.rs`):
   - Clean node types: `Group`, `Text`, `Action`, `Image`, `Table`
   - Binding system for data-view connections
   - Channel system for communication
   - **Action**: Adopt as-is, document in usage guide

2. **View Model** (`view/mod.rs`):
   - Comprehensive layout specs: `FlexContainer`, `GridContainer`, `FormContainer`
   - Rich widget set: `Text`, `Button`, `Image`, `InputBox`, `TextArea`, `Checkbox`, `Radio`, `Select`, `FileInput`, `Table`
   - Overlay system: `Alert`, `Modal`, `Confirm` with positioning
   - Backgrounds: `Solid`, `LinearGradient`, `RadialGradient`
   - **Action**: Adopt with minor simplifications (e.g., collapse `background` + `backgrounds` fields)

3. **Mutation System** (`logic/mutation.rs`, `logic/diff.rs`):
   - Well-designed mutation types: `ReplaceText`, `IrDiff`, `OpenOverlay`, `CloseOverlay`, `HttpFetch`
   - Serde-based JSON serialization for WASM boundary
   - **Action**: Adopt as-is, implement handlers in rune-scene

4. **Package System** (`package/mod.rs`):
   - Manifest + TOC format for distributable packages
   - SHA256 integrity checking
   - Identifier normalization (8-char alphanumeric IDs)
   - **Action**: Adopt, test with sample packages

5. **Schema Validation** (`schema.rs`):
   - JSON Schema Draft 2020-12 validation
   - Embedded schemas for data/view documents
   - **Action**: Keep, integrate with package loading

**⚠️ Problematic Components** (requires cleanup/removal):
1. **CSS Module** (`css/*` - 4800 lines):
   - **Problem**: References `taffy_mapper.rs` which was intentionally excluded due to Taffy bugs
   - Dependencies: Servo's `cssparser`, `selectors`, LightningCSS (feature-gated)
   - Includes UA defaults, cascade resolution, Servo DOM adapters
   - **Action**:
     - **Option A** (Recommended): Remove CSS parsing entirely, use view IR directly for styling
     - **Option B**: Refactor to lightweight property extraction without Taffy integration
     - **Decision Point**: Does rune-scene need CSS input, or can users author view JSON directly?

2. **HTML Module** (`html/mod.rs` - 29k tokens):
   - **Problem**: Large, complex, likely tightly coupled to CSS layer
   - Uses `scraper` crate and `ego-tree` for DOM traversal
   - **Action**:
     - If CSS is removed, evaluate if HTML parsing is still needed
     - May be useful for importing HTML → IR, but not critical for initial integration
     - **Defer**: Low priority, evaluate after core IR integration

**Integration Actions**:
1. ✅ **Adopt**: data, view, logic, package, schema modules
2. ⚠️ **Refactor**: CSS module - remove Taffy dependencies
3. ⚠️ **Defer**: HTML module - evaluate after core integration
4. Add feature flag to disable CSS/HTML modules initially: `default = []` (no cssv2/servo_selectors)
5. Update dependencies to match workspace versions

**Risks**:
- CSS/HTML refactoring may uncover hidden dependencies
- Schema validation may be too strict for iterative development (consider making optional)
- Taffy removal may break CSS property mapping (needs replacement strategy)

---

### 1.3 rune-io (✅ Stable, Simple)

**Purpose**: Non-blocking file dialogs and HTTP requests using OS threads.

**Architecture**:
- **FileDialogService**: `rfd`-based file pickers with async polling
  - `request()` → spawns thread → `poll()` returns results
  - Supports single/multiple file selection
- **HttpService**: `reqwest`-based HTTP client with origin allowlisting
  - `request()` → spawns thread → `poll()` returns results
  - Security: origin allowlist, scheme guards (http/https only)
  - Default 15-second timeout

**Code Quality**: ✅ Good
- Simple, focused API
- Non-blocking design suitable for event loop integration
- Proper error handling
- Security-conscious (origin filtering, scheme validation)

**Integration Actions**:
- ✅ **Keep as-is** - No cleanup needed
- Wire `FileDialogService` to `IrMutation::HttpFetch` and file input widgets
- Wire `HttpService` to `IrMutation::HttpFetch` mutations
- Add to workspace `Cargo.toml` members
- Consider moving to `rune-scene` as an internal service (not exposed as public API)

**Risks**: None. This crate is stable.

---

## 2. Integration Architecture

### 2.1 High-Level Flow

```
User Interaction (rune-scene)
    ↓
Hit-test identifies widget/node
    ↓
Generate Event → WASM Logic Module (rune-wasm)
    ↓
Logic emits IrMutation (e.g., ReplaceText, IrDiff)
    ↓
Mutation Handler updates IR (DataDocument / ViewDocument)
    ↓
IR-to-DisplayList Renderer (NEW: to be implemented)
    ↓
Painter builds display list with z-indices
    ↓
Upload to GPU → PassManager renders with depth buffer
```

### 2.2 New Components Needed

1. **IR Renderer** (`crates/rune-scene/src/ir_renderer.rs`):
   - Translates `ViewDocument` + `DataDocument` → `Painter` calls
   - Maps `FlexContainer` / `GridContainer` to positioned rects
   - Renders widgets (text, buttons, inputs) using existing engine-core primitives
   - Assigns z-indices based on document order + overlay stacking
   - **Note**: Layout engine required (see Section 2.3)

2. **Mutation Processor** (`crates/rune-scene/src/mutation_handler.rs`):
   - Implements `rune_wasm::MutationHandler`
   - Handles `IrMutation` types:
     - `ReplaceText`: Update `DataDocument` nodes, trigger re-render
     - `IrDiff`: Apply batch updates
     - `OpenOverlay` / `CloseOverlay`: Manage overlay stack
     - `HttpFetch`: Delegate to `rune_io::HttpService`, invoke WASM callback on completion
   - Maintains mutable `RunePackage` state

3. **Layout Engine** (TBD - see Section 2.3):
   - Converts `FlexLayout` / `GridLayout` specs → positioned rects
   - Handles padding, margin, sizing constraints
   - **Critical Decision Point**: How to avoid Taffy bugs?

4. **Event Bridge** (`crates/rune-scene/src/event_bridge.rs`):
   - Maps winit events → IR events (clicks, text input, form submit)
   - Serializes events to JSON for WASM `tick()` or event handlers
   - Handles form state management (form inputs, validation)

5. **Service Integrations**:
   - `FileDialogService`: Wire to file input widgets
   - `HttpService`: Wire to `HttpFetch` mutations, manage request lifecycle

### 2.3 Layout Strategy (Critical Decision)

**Problem**: `rune-ir` references Taffy via `css/taffy_mapper.rs`, but Taffy was intentionally excluded due to bugs.

**Options**:

**A. Manual Layout Engine** (Recommended for MVP)
- Implement simple flexbox-like layout in rune-scene
- Support basic flex properties: direction, align, justify, gap
- Defer grid layout to later phase
- **Pros**: Full control, no Taffy bugs, aligns with GPU-native philosophy
- **Cons**: Development time, feature parity with CSS

**B. Taffy Integration (with caution)**
- Re-evaluate Taffy: Has it improved since last assessment?
- Sandbox Taffy in isolated module with extensive testing
- Fallback to manual layout if bugs reappear
- **Pros**: Comprehensive layout engine, CSS alignment
- **Cons**: Risk of reintroducing bugs, external dependency

**C. Alternative Layout Library**
- Evaluate: `morphorm`, `yoga-rs`, custom solution
- **Pros**: May avoid Taffy-specific issues
- **Cons**: Unfamiliar APIs, different bugs

**Recommendation**: **Option A (Manual Layout)** for initial integration. Implement flex layout only, defer grid. Measure performance and complexity before committing to external layout library.

---

## 3. Integration Roadmap

### Phase 1: Core IR Integration (Week 1-2)
- [ ] Add crates to workspace `Cargo.toml` members
- [ ] Run `cargo build --workspace` to identify missing dependencies
- [ ] Update dependency versions to match workspace
- [ ] Disable CSS/HTML modules via feature flags (set `default = []` in rune-ir)
- [ ] Write integration tests: `RunePackage::from_directory()` → parse sample package
- [ ] Document IR data model in `docs/ir-data-model.md`

### Phase 2: Manual Layout Engine (Week 2-3)
- [ ] Implement `FlexLayoutEngine` in rune-scene
  - [ ] Direction, align, justify, wrap, gap
  - [ ] Padding, margin, sizing (width, height, min/max)
  - [ ] Child positioning with depth-based z-indices
- [ ] Write unit tests with known layouts
- [ ] Defer grid layout, overlays to Phase 3

### Phase 3: IR Renderer (Week 3-4)
- [ ] Implement `IrRenderer::render(data, view) → DisplayList`
- [ ] Widget rendering:
  - [ ] `FlexContainer` → positioned rects with backgrounds
  - [ ] `Text` → text runs with styling
  - [ ] `Button` → interactive rects + text
  - [ ] `Image` → image draws (integrate with existing image system)
- [ ] Z-index assignment: document order + overlay stacking
- [ ] Test with sample package

### Phase 4: WASM Runtime Integration (Week 4-5)
- [ ] Implement `MutationProcessor` in rune-scene
- [ ] Wire `WasmRuntime` to package loading
- [ ] Handle `ReplaceText`, `IrDiff` mutations
- [ ] Test with sample WASM module (e.g., counter, form validation)

### Phase 5: Event System (Week 5-6)
- [ ] Implement `EventBridge`: winit → IR events
- [ ] Hit-testing for widgets (map display list IDs → view nodes)
- [ ] Form state management (input boxes, checkboxes, selects)
- [ ] Test interactive scenarios

### Phase 6: I/O Services (Week 6)
- [ ] Wire `HttpService` to `HttpFetch` mutations
- [ ] Wire `FileDialogService` to file input widgets
- [ ] Test file upload, HTTP requests

### Phase 7: Overlays & Advanced Features (Week 7+)
- [ ] Implement overlay stack (`OpenOverlay`, `CloseOverlay`)
- [ ] Modal positioning (center, corners, absolute)
- [ ] Grid layout support
- [ ] Advanced widgets (table rendering, select dropdowns)

---

## 4. Refactoring Checklist

### rune-ir Cleanup
- [ ] **Remove Taffy dependency**:
  - [ ] Delete or stub out `css/taffy_mapper.rs`
  - [ ] Remove Taffy references from `Cargo.toml` (if present)
- [ ] **Disable CSS/HTML by default**:
  - [ ] Update `Cargo.toml`: `default = []`
  - [ ] Add feature flag guards: `#[cfg(feature = "cssv2")]` around CSS modules
- [ ] **Simplify ViewDocument**:
  - [ ] Collapse `background` + `backgrounds` fields (choose one pattern)
  - [ ] Remove unused fields (e.g., `box_shadow` if not implemented)
- [ ] **Dependency updates**:
  - [ ] Match workspace versions for `serde`, `anyhow`, `tracing`
  - [ ] Remove unused deps (check with `cargo-udeps`)

### rune-wasm Cleanup
- [ ] **Dependency check**: Ensure wasmtime version is compatible with workspace Rust version
- [ ] **No code changes needed** - crate is clean

### rune-io Cleanup
- [ ] **No code changes needed** - crate is clean
- [ ] Consider making this a private module in rune-scene rather than public crate

---

## 5. Risks & Mitigation

### Risk 1: Taffy Dependency in CSS Module
**Impact**: High - CSS module is 4800 lines and tightly coupled to Taffy
**Likelihood**: High - Already confirmed in `css/taffy_mapper.rs`
**Mitigation**:
- Disable CSS module initially (feature flag off)
- Evaluate if CSS parsing is needed or if view JSON is sufficient
- If needed, write minimal CSS property extractor without layout engine

### Risk 2: Hidden Dependencies in HTML Module
**Impact**: Medium - HTML module is 29k tokens, may reference CSS/Taffy
**Likelihood**: Medium
**Mitigation**:
- Defer HTML integration to later phase
- Build core IR renderer first, validate that view JSON is expressive enough
- Only enable HTML if user feedback demands HTML-to-IR translation

### Risk 3: Layout Engine Complexity
**Impact**: High - Manual layout engine is significant dev effort
**Likelihood**: High
**Mitigation**:
- Start with MVP flexbox (direction, align, justify only)
- Defer grid, scrolling, complex constraints
- Test with simple layouts before adding features

### Risk 4: Performance of IR-to-DisplayList Translation
**Impact**: Medium - Could add frame time overhead
**Likelihood**: Low-Medium
**Mitigation**:
- Cache layout results, only recompute on mutation
- Use dirty tracking: only re-render changed subtrees
- Profile with large documents (100+ nodes)

### Risk 5: Schema Validation Strictness
**Impact**: Low - May block iteration if schemas are too strict
**Likelihood**: Low
**Mitigation**:
- Make schema validation optional (feature flag or runtime flag)
- Provide detailed error messages with hints for fixing

---

## 6. Technical Debt & Cleanup Opportunities

### Immediate (Do During Integration)
1. **Remove Taffy references** from rune-ir
2. **Disable CSS/HTML modules** by default
3. **Update dependencies** to workspace versions
4. **Write integration tests** for package loading

### Short-Term (Do in Phase 1-3)
1. **Simplify ViewDocument**: Consolidate background fields, remove unused options
2. **Document IR schemas**: Create usage guide with examples
3. **Add validation helpers**: Better error messages for malformed documents

### Long-Term (Post-MVP)
1. **Evaluate CSS need**: Do users want CSS input, or is view JSON sufficient?
2. **HTML-to-IR translator**: If needed, implement lightweight HTML parser
3. **Grid layout**: Add support for `GridContainer` (currently spec'd but not implemented)
4. **Advanced widgets**: Table rendering, rich text, custom components

---

## 7. Success Criteria

### Phase 1 Complete When:
- [ ] All crates build in workspace
- [ ] Sample package loads without errors
- [ ] IR schemas validate against test documents

### MVP Complete When:
- [ ] Static IR document renders to display list
- [ ] WASM module can emit mutations
- [ ] `ReplaceText` mutation updates rendered text
- [ ] Simple interactive demo works (e.g., button increments counter)

### Full Integration Complete When:
- [ ] All mutation types implemented
- [ ] Form inputs work with state management
- [ ] HTTP requests complete and update UI
- [ ] Overlays render and dismiss correctly
- [ ] Layout engine handles 90% of real-world use cases

---

## 8. Recommendations

### Adopt Immediately
1. ✅ **rune-wasm** - Production-ready, no changes needed
2. ✅ **rune-io** - Stable, simple, works as-is
3. ✅ **rune-ir core** (data, view, logic, package, schema) - Well-designed, minor cleanup only

### Refactor Before Use
1. ⚠️ **rune-ir CSS module** - Remove Taffy, make optional
2. ⚠️ **rune-ir HTML module** - Defer, evaluate need

### Build New
1. 🔨 **Manual layout engine** - Critical path, start immediately
2. 🔨 **IR renderer** - Core integration component
3. 🔨 **Mutation processor** - Bridges WASM to IR updates
4. 🔨 **Event bridge** - User interaction → WASM events

### Key Decisions Needed
1. **Layout Strategy**: Manual vs. Taffy vs. Alternative library?
2. **CSS Support**: Remove entirely or minimal property extraction?
3. **HTML Support**: Defer to later phase or implement translator?
4. **Schema Strictness**: Strict validation or permissive mode for iteration?

**Recommended Answers**:
1. **Manual layout** (MVP flexbox only)
2. **Remove CSS** (view JSON is sufficient)
3. **Defer HTML** (evaluate post-MVP)
4. **Permissive mode** (optional validation, helpful errors)

---

## Conclusion

The IR/WASM/IO crates provide a solid foundation for dynamic rendering in rune-scene. The core data model, mutation system, and WASM runtime are production-ready. Main challenges are:
1. Removing Taffy dependency from CSS module
2. Implementing manual layout engine
3. Building IR-to-DisplayList renderer

With disciplined scoping (MVP flexbox, defer CSS/HTML), integration is achievable in 6-7 weeks. The architecture aligns well with rune-draw's GPU-native philosophy and will enable declarative UI authoring + WASM-driven interactivity.

**Next Steps**:
1. Review this assessment with team
2. Make key decisions (layout strategy, CSS support)
3. Begin Phase 1: Core IR integration
4. Set up weekly integration reviews to track progress

---

**Document Version**: 1.0
**Last Updated**: 2025-11-19
**Author**: Claude Code Integration Analysis
