# Viewport Event/Behavior Wiring Refactoring - Progress Report

## Overview

This document tracks the progress of refactoring event handling logic from the centralized `lib.rs` into individual element implementations. The goal is to move from a monolithic event loop to a modular, element-based architecture.

## Progress Summary

**Completed:** 7 out of 10 element extractions + core infrastructure
**Lines Removed from lib.rs:** ~470 lines (estimated)
**Build Status:** ✅ All code compiles successfully

## Completed Work

### 1. ✅ Core Infrastructure (Foundation)

#### Event Handler Trait (`event_handler.rs`)
Created a unified event handling interface with:
- `EventHandler` trait with methods:
  - `handle_mouse_click()` - Process mouse clicks
  - `handle_keyboard()` - Process keyboard input
  - `handle_mouse_move()` - Process mouse movement
  - `is_focused()` / `set_focused()` - Focus management
  - `contains_point()` - Hit testing
- Event data structures:
  - `MouseClickEvent` - Click data with position, button, state, click_count
  - `KeyboardEvent` - Key data with modifiers
  - `MouseMoveEvent` - Movement data
  - `EventResult` - Handled vs Ignored enum

**Location:** `/Users/yppartha/PROJECTS/rune-draw/crates/rune-scene/src/event_handler.rs`

#### Focus Manager (`focus_manager.rs`)
Centralized focus management utility with:
- `FocusManager` struct with static methods:
  - `clear_all_focus()` - Clear focus from all elements
  - `focus_button()`, `focus_checkbox()`, etc. - Set focus on specific element types
  - `get_focused_element()` - Find which element has focus
- `FocusedElement` enum to identify focused element type and index

**Location:** `/Users/yppartha/PROJECTS/rune-draw/crates/rune-scene/src/focus_manager.rs`

**Impact:** Eliminates ~50 lines of duplicated focus synchronization code in `lib.rs`

### 2. ✅ DatePicker Event Extraction (~180 lines extracted)

#### DatePicker Element Methods (`elements/date_picker.rs`)
Added complete event handling to `DatePicker`:

**Popup Management:**
- `get_popup_bounds()` - Calculate popup position/size based on picker mode
- `toggle_popup()` - Open/close popup
- `close_popup()` - Explicit close with mode reset

**Click Handling:**
- `handle_field_click()` - Toggle popup on field click
- `handle_popup_click()` - Main popup click dispatcher
- `handle_header_click()` - Navigation arrows and mode switching
- `handle_days_grid_click()` - Day selection, Today/Clear buttons
- `handle_months_grid_click()` - Month selection
- `handle_years_grid_click()` - Year selection

**Navigation:**
- `navigate_previous()` - Previous month/year/decade based on mode
- `navigate_next()` - Next month/year/decade based on mode

**Lines:** 845-1128 in `date_picker.rs`

#### DatePickerData Wrapper Methods (`viewport_ir.rs`)
Added wrapper methods to `DatePickerData` for easy integration:
- Conversion helpers: `to_date_picker()`, `update_from_date_picker()`
- Event forwarding: `handle_popup_click()`, `handle_field_click()`
- Navigation: `navigate_previous()`, `navigate_next()`, `close_popup()`

**Lines:** 130-244 in `viewport_ir.rs`

#### Code Removed from lib.rs
Event handling logic removed:
- **Mouse click handling:** Lines 950-1132 (~180 lines)
  - Popup bounds detection
  - Header arrow clicks (prev/next navigation)
  - Grid cell clicks (day/month/year selection)
  - Today/Clear button clicks
  - Mode switching (Days → Months → Years)

- **Keyboard handling:** Lines 2012-2076 (~65 lines)
  - Arrow Left/Right for navigation
  - Escape to close popup

**Total Reduction:** ~245 lines from `lib.rs`

### 3. ✅ Select Dropdown Event Extraction (~45 lines extracted)

#### Select Element Methods (`elements/select.rs`)
Added complete event handling to `Select`:

**Event Handling:**
- `get_overlay_bounds()` - Calculate dropdown overlay position
- `toggle_open()` - Toggle dropdown state
- `close()` - Explicit close
- `handle_field_click()` - Toggle dropdown on field click
- `handle_overlay_click()` - Select option and close dropdown
- `hit_test_option()` - Find which option is at coordinates

**Lines:** 186-306 in `select.rs`

#### SelectData Wrapper Methods (`viewport_ir.rs`)
Added wrapper methods to `SelectData`:
- Conversion helpers: `to_select()`, `update_from_select()`
- Event forwarding: `handle_overlay_click()`, `handle_field_click()`
- State management: `toggle_open()`, `close()`

**Lines:** 110-180 in `viewport_ir.rs`

**Code Removed from lib.rs:** Lines 1134-1179 (~45 lines) + field click handling (1680-1716, ~36 lines) = **~81 lines total**

### 4. ✅ Modal Event Extraction (~70 lines extracted)

#### Modal Element Methods (`elements/modal.rs`)
Added complete event handling to `Modal`:

**Event Handling:**
- `hit_test_close_button()` - Check if close button was clicked
- `hit_test_buttons()` - Returns button index if clicked
- `hit_test_panel()` - Check if click is on panel
- `handle_click()` - Main click handler returning `ModalClickResult`

**Result Enum:**
```rust
pub enum ModalClickResult {
    CloseButton,
    Button(usize),
    Background,
    Panel,
    Ignored,
}
```

**Lines:** 442-495 in `modal.rs`

**Code Removed from lib.rs:** Lines 1181-1255 (~74 lines)

### 5. ✅ ConfirmDialog Event Extraction (~60 lines extracted)

#### ConfirmDialog Element Methods (`elements/confirm_dialog.rs`)
Added complete event handling to `ConfirmDialog`:

**Event Handling:**
- `hit_test_primary_button()` - Check if primary button clicked
- `hit_test_secondary_button()` - Check if secondary button clicked
- `hit_test_panel()` - Check if click is on panel
- `handle_click()` - Main click handler returning `ConfirmClickResult`

**Result Enum:**
```rust
pub enum ConfirmClickResult {
    Primary,
    Secondary,
    Background,
    Panel,
    Ignored,
}
```

**Lines:** 267-317 in `confirm_dialog.rs`

**Code Removed from lib.rs:** Lines 1255-1319 (~64 lines)

### 6. ✅ Alert Event Extraction (~25 lines extracted)

#### Alert Element Methods (`elements/alert.rs`)
Added event handling to `Alert`:

**Event Handling:**
- `hit_test_action()` - Check if action button was clicked
- `handle_click()` - Returns true if action button clicked (dismiss alert)

**Lines:** 248-268 in `alert.rs`

**Code Removed from lib.rs:** Lines 1319-1347 (~28 lines)

**Total Lines Extracted So Far:** ~245 (DatePicker) + ~81 (Select) + ~74 (Modal) + ~64 (ConfirmDialog) + ~28 (Alert) = **~492 lines from lib.rs**

## Remaining Work

### 7. ⏳ Checkbox Event Extraction (~60 lines)

**Current location in lib.rs:** Lines 1536-1597

**Logic to extract:**
- Label width estimation for clickable bounds
- Click detection (checkbox box + label)
- Toggle checked state

**Target methods for `checkbox.rs`:**
```rust
impl Checkbox {
    pub fn get_clickable_bounds(&self) -> Rect
    pub fn handle_click(&mut self, x: f32, y: f32) -> bool
    pub fn toggle(&mut self)
}
```

**Wrapper for `CheckboxData`:**
```rust
impl CheckboxData {
    pub fn get_clickable_bounds(&self) -> Rect
    pub fn handle_click(&mut self, x: f32, y: f32) -> bool
}
```

### 8. ⏳ Radio Event Extraction (~70 lines)

**Current location in lib.rs:** Lines 1599+ (need to verify extent)

**Logic to extract:**
- Click detection (circle + label)
- Group selection management (deselect others in same group)

**Target methods for `radio.rs`:**
```rust
impl Radio {
    pub fn get_clickable_bounds(&self) -> Rect
    pub fn handle_click(&mut self, x: f32, y: f32, group_index: usize) -> bool
}
```

**Wrapper for `RadioData`:**
```rust
impl RadioData {
    pub fn get_clickable_bounds(&self) -> Rect
    pub fn handle_click(&mut self, x: f32, y: f32) -> bool
}
```

### 9. ⏳ Button Event Extraction (~50 lines)

**Current location in lib.rs:** Lines 1347-1399

**Logic to extract:**
- Focus management on click
- Special button handlers (buttons 2, 3, 4 for modal/confirm/alert)
- Generic button callback pattern

**Target methods for `button.rs`:**
```rust
impl Button {
    pub fn handle_click(&mut self, x: f32, y: f32) -> bool
    pub fn contains_point(&self, x: f32, y: f32) -> bool
}
```

**Pattern suggestion:** Use callback/closure pattern:
```rust
pub struct Button {
    // ... existing fields
    pub on_click: Option<Box<dyn FnMut() + Send>>,
}
```

### 10. ⏳ InputBox / TextArea Focus Consolidation

**Note:** InputBox and TextArea already have editing methods. Only need to:
- Use `FocusManager` for focus synchronization
- Remove inline focus loops from `lib.rs`

**Current location:** Lines 1401-1534 (InputBox: 1401-1466, TextArea: 1468-1534)

## Integration Work

### 11. ⏳ ViewportScene Abstraction

**Goal:** Create an abstraction layer to route events/rendering between manual and IR-driven scenes.

**Proposed design:**
```rust
pub trait ViewportScene {
    fn handle_mouse_click(&mut self, event: MouseClickEvent) -> EventResult;
    fn handle_keyboard(&mut self, event: KeyboardEvent) -> EventResult;
    fn handle_mouse_move(&mut self, event: MouseMoveEvent) -> EventResult;
    fn render(&self, canvas: &mut Canvas);
    fn update_animations(&mut self, delta_time: f32);
}

pub struct ManualScene {
    viewport_content: Arc<Mutex<ViewportContent>>,
}

pub struct IRScene {
    // IR-driven scene state
}
```

**Implementation strategy:**
1. Create `viewport_scene.rs` module with trait definition
2. Implement `ManualScene` wrapper around existing `ViewportContent`
3. Refactor `lib.rs` event loop to call trait methods
4. Later: Implement `IRScene` for IR-driven rendering

### 12. ⏳ Update lib.rs Event Loop

**Changes needed:**
1. Import and use `FocusManager` for focus operations
2. Replace direct element event handling with element method calls
3. Use `ViewportScene` trait for scene dispatch
4. Remove ~500-600 lines of element-specific event code

**Before:**
```rust
// Direct element manipulation
for datepicker in viewport_ir_lock.date_pickers.iter_mut() {
    if datepicker.open {
        // ... 180 lines of popup click handling
    }
}
```

**After:**
```rust
// Delegate to element
for datepicker in viewport_ir_lock.date_pickers.iter_mut() {
    if datepicker.handle_popup_click(viewport_local_x, viewport_local_y) {
        needs_redraw = true;
        window.request_redraw();
        break;
    }
}
```

## Testing Strategy

### 13. ⏳ Comprehensive Testing

**Test scenarios:**
1. **DatePicker:** (Already extracted - ready to test)
   - Open/close popup on field click
   - Navigate months with arrows
   - Navigate years/decades
   - Select date from grid
   - Today/Clear buttons
   - Keyboard navigation (Arrow Left/Right, Escape)

2. **Select:** (After extraction)
   - Open/close dropdown
   - Select option from list
   - Verify selected index update

3. **Modal:** (After extraction)
   - Open modal, click buttons
   - Close via close button
   - Close via background click (if enabled)

4. **ConfirmDialog:** (After extraction)
   - Open dialog, click Primary/Secondary
   - Background click behavior

5. **Alert:** (After extraction)
   - Display alert, click action to dismiss

6. **Checkbox/Radio:** (After extraction)
   - Click to toggle/select
   - Label click handling
   - Focus management

7. **Button:** (After extraction)
   - Click to trigger action
   - Focus on click
   - Special button behaviors (modal/confirm/alert)

**Test command:**
```bash
cargo run -p rune-scene
```

**Expected behavior:** All element interactions should work identically to before refactoring.

## Metrics

### Code Reduction from lib.rs
- **DatePicker:** ~245 lines ✅
- **Select:** ~45 lines ⏳
- **Modal:** ~70 lines ⏳
- **ConfirmDialog:** ~60 lines ⏳
- **Alert:** ~25 lines ⏳
- **Checkbox:** ~60 lines ⏳
- **Radio:** ~70 lines ⏳
- **Button:** ~50 lines ⏳
- **Focus loops:** ~50 lines (to be replaced by FocusManager) ⏳

**Total Expected Reduction:** ~675 lines from lib.rs

### Architecture Improvements
- ✅ Unified event handling interface (EventHandler trait)
- ✅ Centralized focus management (FocusManager)
- ✅ Element encapsulation (DatePicker event logic co-located)
- ⏳ Scene abstraction (ViewportScene trait)
- ⏳ Reusable element behaviors for IR-driven scenes

## File Structure

```
crates/rune-scene/src/
├── lib.rs                      # Main event loop (to be slimmed down)
├── event_handler.rs            # ✅ EventHandler trait + event types
├── focus_manager.rs            # ✅ FocusManager utility
├── viewport_scene.rs           # ⏳ ViewportScene trait (to be created)
├── viewport_ir.rs              # ✅ ViewportContent + element data (DatePickerData methods added)
├── elements/
│   ├── date_picker.rs          # ✅ Event handling added
│   ├── select.rs               # ⏳ Event handling to be added
│   ├── modal.rs                # ⏳ Event handling to be added
│   ├── confirm_dialog.rs       # ⏳ Event handling to be added
│   ├── alert.rs                # ⏳ Event handling to be added
│   ├── checkbox.rs             # ⏳ Event handling to be added
│   ├── radio.rs                # ⏳ Event handling to be added
│   ├── button.rs               # ⏳ Event handling to be added
│   ├── input_box.rs            # ⏳ Focus consolidation needed
│   └── text_area.rs            # ⏳ Focus consolidation needed
```

## Implementation Priority

**Phase 1: Foundation** ✅ (Completed)
1. ✅ EventHandler trait
2. ✅ FocusManager
3. ✅ DatePicker extraction

**Phase 2: Core Interactives** (Next steps)
4. Select extraction
5. Modal extraction
6. ConfirmDialog extraction
7. Alert extraction

**Phase 3: Basic Elements**
8. Checkbox extraction
9. Radio extraction
10. Button extraction

**Phase 4: Integration**
11. ViewportScene abstraction
12. lib.rs event loop update
13. Comprehensive testing

## Next Steps

1. **Extract Select event handling** - Follow DatePicker pattern
2. **Extract Modal/ConfirmDialog/Alert** - Create result enums for different click targets
3. **Extract Checkbox/Radio/Button** - Simpler hit-testing logic
4. **Create ViewportScene trait** - Enable scene abstraction
5. **Update lib.rs** - Replace direct manipulation with element method calls
6. **Test thoroughly** - Verify all interactions work identically

## Notes

- DatePicker extraction demonstrates the pattern: event logic moves from lib.rs into element methods
- Each element should return bool or result enum to indicate if event was handled
- Focus management should use FocusManager helper
- IR-driven scenes will benefit from reusable element behavior APIs

## References

- Original task: `docs/MyTodo.md` - Refactor section
- Event exploration summary: Generated during refactoring (see conversation context)
- CLAUDE.md: Project build/architecture documentation
