# Incremental Testing Plan

## ✅ Simplified Test Case Created

### Backups
- `welcome-full.vizr.backup` - Original complex layout (all elements)
- `RUNE.MANIFEST.json.backup` - Original manifest
- `welcome-simple.vizr` - Simple version (kept for reference)

### Active Test: `welcome.vizr` (simplified)

**Only uses implemented elements:**
- ✅ FlexContainer with gradient background
- ✅ Text elements (2x)
- ✅ Button element (1x)

**Structure:**
```
root_view (FlexContainer - purple gradient)
  └─ card_view (FlexContainer - white card)
       ├─ title_text (Text - "Hello from the first card")
       ├─ body_text (Text - "Hello from the second card")
       └─ action_button (Button - "Tap me")
```

## Testing Phases

### Phase 1: Basic Rendering ✅ READY TO TEST
```bash
USE_IR=1 cargo run -p rune-scene -- crates/rune-scene/examples/sample_first_node
```

**Expected:**
- Purple gradient background (linear gradient)
- White rounded card
- Two text elements
- Blue button

**Verifies:**
- Taffy layout works
- FlexContainer rendering
- Text element rendering
- Button element rendering
- Data binding (DataDocument → ViewDocument)
- Gradient backgrounds

### Phase 2: Add Image (Next)
Once Phase 1 works, add:
```json
{
  "id": "hero_image",
  "node_id": "IMGFRST1",
  "type": "image",
  "width": 240
}
```

### Phase 3: Add Interactive Elements
Incrementally add (as implemented):
1. Checkbox
2. Radio buttons
3. Select dropdown
4. InputBox
5. TextArea

### Phase 4: Restore Full Layout
Once all elements work, restore from `welcome-full.vizr.backup`

## Element Implementation Status

| Element | Status | Priority |
|---------|--------|----------|
| FlexContainer | ✅ Done | - |
| Text | ✅ Done | - |
| Button | ✅ Done | - |
| Image | ✅ Basic | Test Phase 2 |
| Checkbox | ❌ TODO | High |
| Radio | ❌ TODO | High |
| Select | ❌ TODO | Medium |
| InputBox | ❌ TODO | Medium |
| TextArea | ❌ TODO | Medium |

## Troubleshooting Guide

### If nothing renders:
1. Check console for errors
2. Verify package loads: "Loaded IR package:"
3. Check Taffy layout builds successfully
4. Add debug prints in render_view_node_with_elements()

### If layout is wrong:
1. Check Taffy style conversion
2. Verify padding/margin values
3. Check flex direction/justify/align

### If colors are wrong:
1. Verify parse_color() works
2. Check ColorLinPremul vs Color conversion
3. Verify gradient interpolation

### If text doesn't show:
1. Check data binding (node_id resolution)
2. Verify font size calculation
3. Check baseline positioning

## Success Metrics

### Phase 1 Success Criteria:
- [ ] Window opens without crash
- [ ] Gradient background visible
- [ ] White card renders
- [ ] Text elements render with correct content
- [ ] Button renders with label
- [ ] Layout matches flex specification

## Rollback Instructions

To restore original complex layout:
```bash
cp crates/rune-scene/examples/sample_first_node/views/layout/welcome-full.vizr.backup \
   crates/rune-scene/examples/sample_first_node/views/layout/welcome.vizr

cp crates/rune-scene/examples/sample_first_node/RUNE.MANIFEST.json.backup \
   crates/rune-scene/examples/sample_first_node/RUNE.MANIFEST.json
```

## Next Steps After Phase 1

1. ✅ Verify basic rendering works
2. 🔧 Fix any layout/rendering issues
3. 📸 Screenshot for documentation
4. ➕ Add one element type at a time
5. 🎯 Test each addition
6. 📝 Document findings
7. 🔁 Iterate until all elements work
