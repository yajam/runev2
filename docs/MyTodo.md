Features

- [ ] Text selection and highlight
- [x] Datepicker
- [x] Hyperlink element
- [NP] Box in elements - use canvas.draw_rect
- [x] File input was not added
- [x] All elements should take dynamic parameters for styling
- [x] Add Rune-ir
- [x] Rune-layout using taffy
- [ ] Address input, forward, backward, refresh implementation in toolbar
- [x] Port persistence.rs
- [ ] Bookmark, Tabs
- [ ] Port Boa, csslightning (canonical)
- [ ] Test gradients in layers
- [x] Full svg vector support
- [ ] Multi select
- [ ] Custom widgets through IR blocks using primitives
- [ ] wire to wasm, fetch, form submission
- [ ] Pdf export
- [ ] Elements, Console and Network implementation in devtools
- [ ] Light mode
- [ ] Root level font definition inherit
- [ ] Theming support dark mode, light mode system mode
- [ ] Native menus support
- [ ] Web page rendering
- [ ] Fallback theough headless CEF

IR Porting

- [x] Input, text, textarea, buttons, label, hyperlinks, Table
- [x] Image, Select, datepicker, fileinput
- [x] Alert, modal, confirm
- [x] Checkboxes, Radio buttons
- [~] Scroll horizontal and vertical

Bugs

- [ ] Text is still not crisp, specs and dust flickering
- [x] Textarea caret down is not working correctly
- [ ] Edge visbility fix using background expansion
- [x] Caret blink is not consistent
- [x] Change SVG to true vector
- [ ] Text editing textarea line selection multiple with keyboard doesnt work
- [ ] Double click triple click doesnt work right only highlights till cursor position
- [ ] Hyperlink underline mismatch
- [ ] Load delay for elements

Improvements

- [ ] Select needs a placeholder
- [ ] Implement a scrollbar
- [ ] Refactor and move viewport_ir as a second demo-app
- [ ] Text must ALWAYS be rendered in a linear (unorm) format, with premultiplied alpha, into a linear color attachment.
- [ ] Add style parameter for all elements and make it robust

Testing

- BIDI testing
