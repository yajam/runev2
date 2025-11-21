# IR/WASM/IO Integration - Quick Start

## Status: ✅ All Crates Compile Successfully

The three imported crates now build in the workspace:
- ✅ **rune-io** - File dialogs and HTTP services
- ✅ **rune-wasm** - Wasmtime runtime for logic modules
- ✅ **rune-ir** - Data/view IR with mutations

## Key Findings

### Excellent (No Changes Needed)
- **rune-wasm**: Production-ready WASM runtime with clean mutation handling
- **rune-io**: Simple, focused I/O utilities with proper async design

### Good (Minor Cleanup)
- **rune-ir core** (data, view, logic, package, schema): Well-designed IR system

### Needs Refactoring
- **rune-ir CSS module**: References Taffy layout engine (intentionally excluded due to bugs)
- **rune-ir HTML module**: Large (29k tokens), defer to later phase

## Critical Path Forward

### Phase 1: Disable CSS/HTML (Immediate)
The CSS module includes `taffy_mapper.rs` which depends on the buggy Taffy layout library you intentionally excluded. Disable it:

```toml
# In crates/rune-ir/Cargo.toml
[features]
default = []  # Remove "cssv2" and "servo_selectors" from default
```

### Phase 2: Build Layout Engine (Priority 1)
You need a layout engine to convert `FlexLayout` / `GridLayout` specs into positioned rects. Options:

1. **Manual Flexbox Engine** (Recommended)
   - Implement basic flex layout in rune-scene
   - Properties: direction, align, justify, gap, padding, margin
   - Avoids Taffy bugs, full control
   - Defer grid layout

2. **Re-evaluate Taffy**
   - Check if recent versions fixed the bugs
   - Sandbox with extensive testing
   - Fallback to manual if issues persist

3. **Alternative Library**
   - Consider: morphorm, yoga-rs
   - Risk: Different bugs, learning curve

### Phase 3: Build IR Renderer (Priority 2)
Implement `ViewDocument` + `DataDocument` → `DisplayList` translator:
- Map containers to rects with backgrounds/borders
- Render text nodes with styling
- Handle widgets (buttons, inputs, etc.)
- Assign z-indices for depth-based rendering

### Phase 4: Wire WASM Runtime (Priority 3)
Connect mutation system:
- Implement `MutationHandler` in rune-scene
- Handle `ReplaceText`, `IrDiff`, `OpenOverlay`, `HttpFetch`
- Update IR documents, trigger re-render

## Next Steps

1. **Read the full assessment**: `docs/ir-wasm-io-integration-assessment.md`
2. **Make key decisions**:
   - Layout strategy: Manual vs. Taffy vs. Alternative?
   - CSS support: Remove entirely or minimal extraction?
   - Timeline: MVP in 6-7 weeks feasible?
3. **Begin Phase 1**: Disable CSS/HTML modules, update feature flags
4. **Start layout engine**: Prototype manual flexbox implementation

## Documentation

- **Full Assessment**: `docs/ir-wasm-io-integration-assessment.md` (comprehensive 7-section analysis)
- **This Quick Start**: `docs/integration-quick-start.md`

## Questions to Answer

1. **Do you want CSS input?** Or is authoring view JSON directly acceptable?
2. **Layout engine preference?** Manual flexbox vs. re-trying Taffy vs. alternative library?
3. **HTML support needed?** For importing HTML → IR or only for runtime rendering?
4. **Timeline?** When do you need dynamic rendering working?

---

**Ready to proceed with Phase 1 (disabling CSS/HTML)?** Or would you like to discuss the layout strategy first?
