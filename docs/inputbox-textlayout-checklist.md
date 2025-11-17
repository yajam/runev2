# InputBox + TextLayout Integration Checklist

High-level plan to converge `InputBox` on `rune-text`’s `TextLayout` while keeping the current Canvas + `TextProvider` rendering and UX.

The goal: a single, well-factored editing engine (selection, undo/redo, clipboard, hit-testing) that drives both single-line `InputBox` and future multi-line controls, without duplicating logic across `rune-scene` and `rune-text`.

---

## Phase 0: Baseline + Switch

- [x] Freeze current behavior
  - [x] Confirm `viewport_ir` input boxes work as desired (scrolling, caret blink, empty caret, placeholder).
  - [x] Note current editing paths in `crates/rune-scene/src/elements/input_box.rs` (manual string edits) and `crates/rune-scene/src/lib.rs` (keyboard wiring).
- [x] Introduce a minimal guard / toggle
  - [x] Add a simple internal flag or cfg (e.g. `use_textlayout_backend: bool`) inside `InputBox` so you can turn `TextLayout`-driven editing on/off during the migration.
  - [x] Keep default behavior matching today (no surprises for other callers).

---

## Phase 1: Single Source of Truth for Text

**Goal:** `rune-text::layout::TextLayout` becomes the authoritative text/offset model; `InputBox.text` mirrors it (or is removed later).

- [x] Align fonts between layout and rendering
  - [x] Ensure `TextLayout::with_system_font` and `engine_core::RuneTextProvider::from_system_fonts` both select the _same_ system font family.
  - [x] Document the assumption: “TextLayout and TextProvider share the same font bytes, so cursor/selection metrics match Canvas rendering.”
- [x] Make `TextLayout` authoritative
  - [x] In `InputBox::new`, always attempt to build a `TextLayout` from the initial `text` (using system font).
  - [x] Replace direct manipulation of `self.text` with a small helper that:
    - [x] Calls the appropriate `TextLayout` editing API.
    - [x] Syncs `self.text` from `layout.text()` (or eventually removes `self.text` entirely).
  - [x] Add a small helper to clamp and synchronize `cursor_position` with the layout (e.g. ensure it never exceeds `layout.text().len()`).

---

## Phase 2: Keyboard Editing via TextLayout

**Goal:** All keyboard edits go through `TextLayout`, but the visible rendering can still be the simple string path for now.

- [x] Wire basic editing
  - [x] Backspace → `TextLayout::delete_backward`.
  - [x] Delete → `TextLayout::delete_forward`.
  - [x] Character insertion → `TextLayout::insert_char`.
  - [x] Space/newline/tab → `insert_char` / `insert_newline` / `insert_tab` as appropriate.
  - [x] Home/End → move cursor to start/end using `0`/`layout.text().len()` (or a dedicated API if needed).
- [x] Wire cursor movement
  - [x] Left/Right → `move_cursor_left` / `move_cursor_right`.
  - [x] Cmd+Left/Right (macOS) or Ctrl+Left/Right (Windows) → `move_cursor_line_start` / `move_cursor_line_end` (go to start/end of line).
  - [x] Option+Left/Right (macOS) or Alt+Left/Right (Windows) → `move_cursor_left_word` / `move_cursor_right_word` (word movement).
- [x] Respect selection in editing
  - [x] When a selection is active, use `insert_str_with_undo` / `replace_selection` so edits replace the selected range rather than ignoring it.
  - [x] Ensure `cursor_position` is always set to `selection.active()` after each edit.

---

## Phase 3: Selection Model in InputBox

**Goal:** Track full selection state in `InputBox` and keep it in sync with `TextLayout`.

- [x] Add selection fields
  - [x] Add `rt_selection: rune_text::layout::Selection` to `InputBox`.
  - [x] Initialize it as `Selection::collapsed(initial_cursor)` in `InputBox::new`.
- [x] Keyboard selection
  - [x] Shift+Left/Right → use `TextLayout::extend_selection` with `move_cursor_left/right`.
  - [x] Cmd+Shift+Left/Right (macOS) or Ctrl+Shift+Left/Right (Windows) → `extend_selection_to_line_start` / `extend_selection_to_line_end` (select to start/end of line).
  - [x] Shift+Home/End → `extend_selection` with offsets `0` / `text.len()`.
  - [x] Keep `cursor_position` equal to `rt_selection.active()` after each update.
- [x] Mouse selection skeleton
  - [x] Add `mouse_selecting: bool` and `last_mouse_pos: Option<Point>` to `InputBox` (or to `viewport_ir` wrapper).
  - [x] On mouse down inside the input:
    - [x] Convert screen coordinates → local text coordinates (respecting viewport transform, scroll, padding).
    - [x] Call `TextLayout::start_mouse_selection(point)` and store the resulting selection.
  - [x] On mouse move with button held:
    - [x] Call `extend_mouse_selection` to update `rt_selection`.
  - [x] On mouse up:
    - [x] Clear `mouse_selecting`.

---

## Phase 4: Rendering Selection & Caret from TextLayout

**Goal:** Use `TextLayout` for visual selection rectangles and caret position, while still rendering glyphs via Canvas + `TextProvider`.

- [x] Compute cursor x from layout
  - [x] Replace `cursor_x`'s width calculation with:
    - [x] Ask `TextLayout::cursor_rect_at_position` for the current cursor position.
    - [x] Use `cursor_rect.x` as the logical cursor x; keep your existing scroll logic.
  - [x] Keep the `space_width` approximation as a fallback if layout is unavailable.
- [x] Draw selection highlight
  - [x] Call `TextLayout::selection_rects(&rt_selection)` to get `SelectionRect`s.
  - [x] For each rect:
    - [x] Map layout-local x/y → input-local coordinates (accounting for padding and scroll).
    - [x] Draw a filled rectangle behind the text (semi-transparent blue at 80 alpha).
- [x] Caret rendering
  - [x] Use `cursor_rect`'s x-position for the caret line instead of measuring `text[..cursor_position]`.
  - [x] Keep `CaretBlink` for visibility (blink timing) and use `caret.visible` to gate drawing.

---

## Phase 5: Clipboard, Undo/Redo, and Word/Line Selection

**Goal:** Expose rune-text's rich editing features through keyboard shortcuts and mouse gestures.

- [x] Clipboard shortcuts
  - [x] Ctrl/Cmd+C → `TextLayout::copy_to_clipboard(&rt_selection)`.
  - [x] Ctrl/Cmd+X → `cut_to_clipboard(&rt_selection, ...)` then sync text + cursor.
  - [x] Ctrl/Cmd+V → `paste_from_clipboard(cursor_position, ...)` or `paste_replace_selection(&rt_selection, ...)`.
- [x] Undo/Redo
  - [x] Ctrl/Cmd+Z → `TextLayout::undo(&rt_selection, ...)`, then:
    - [x] Update `rt_selection` and `cursor_position` from returned `(offset, selection)`.
    - [x] Sync `text` from `layout.text()`.
  - [x] Ctrl/Cmd+Shift+Z / Ctrl+Y → `TextLayout::redo(...)` with same sync.
- [x] Word/line selections (mouse)
  - [x] Double-click → `start_word_selection(point)` and update `rt_selection`.
  - [x] Double-click + drag → `extend_word_selection(&rt_selection, point)`.
  - [x] Triple-click → `start_line_selection(point)`.
  - [x] Triple-click + drag → `extend_line_selection(&rt_selection, point)`.

---

## Phase 6: Horizontal Scroll + Layout Integration Cleanup

**Goal:** Make scrolling and text metrics rely entirely on `TextLayout` for consistency, and remove redundant measurement logic.

- [x] Scroll based on layout metrics
  - [x] Replace `measure_run(provider, ..)` usage in `cursor_x` / `update_scroll` with:
    - [x] `cursor_rect.x` for caret position.
    - [x] Line width or `line.width` for total text width.
  - [x] Ensure scroll clamping still respects margins and viewport width.
- [x] Remove redundant metrics helpers
  - [x] Once `TextLayout` drives x-positions, evaluate and remove:
    - [x] `space_advance` and `space_width` approximations (if no longer needed).
    - [x] Any duplicated grapheme iteration that's now covered by `TextLayout` APIs.

---

## Phase 7: API, Redundancy, and Code Cleanup

**Goal:** Simplify `InputBox` and centralize all editing concerns in rune-text so there’s a single place to maintain behavior.

- [x] Simplify `InputBox` data model
  - [x] Keep:
    - [x] Visual parameters (`rect`, colors, padding, text_size, placeholder, focused).
    - [x] `rt_layout`, `rt_selection`, `cursor_position`, `scroll_x`, `CaretBlink`.
  - [x] Remove:
    - [x] Direct string editing helpers that duplicate `TextLayout` (once fully migrated).
    - [x] Manual grapheme logic that is now redundant.
- [x] Document responsibilities
  - [x] In `input_box.rs` module docs, clearly state:
    - [x] "All editing is delegated to rune-text `TextLayout`; InputBox is a thin visual wrapper over layout + Canvas."
  - [x] Add a short note in `docs/text-rendering-checklist.md` pointing to this file for editing-related work.

---

## Phase 8: Testing, Edge Cases, and Future Sharing

- [ ] Manual testing scenarios
  - [ ] Long ASCII strings with spaces (scrolling + selection).
  - [ ] Unicode graphemes (emoji, combining marks) to verify cursor/selection are grapheme-aware.
  - [ ] Undo/redo chains (typing, deletions, selection edits).
  - [ ] Clipboard operations across apps.
- [ ] TextArea + future controls
  - [ ] After `InputBox` is stable, plan how to:
    - [ ] Reuse the same `TextLayout` + `Selection` + `CaretBlink` pattern in `TextArea`.
    - [ ] Share selection/caret rendering helpers between single-line and multi-line widgets.

Use this checklist as the “source of truth” for migrating away from the ad‑hoc string-based input box toward a unified rune-text editing stack, while keeping the good behavior you’ve already tuned in `viewport_ir` (formerly `sample_ui`).
