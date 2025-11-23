wasm_guest (example)
====================

A minimal Rust WASM guest that handles form submit events in Rune and demonstrates runtime computation, augmentation, and cancellation.

Exports

- `start()`: optional signal; emits `{type:"wasm_guest_ready"}` via host dispatch.
- `on_form_submit(ptr,len)`: global listener; cancels the `intercept` form.
- `submit_contact_form(ptr,len)`: named handler for `onsubmit_intent`; computes a token and overwrites `first-name`, then augments the pending native submit.

Build

- Install target: `rustup target add wasm32-unknown-unknown`
- Build: `cargo build --release --target wasm32-unknown-unknown --manifest-path examples/wasm_guest/Cargo.toml`
- Stage into sample package (overwrites the demo WAT):
  `cp target/wasm32-unknown-unknown/release/wasm_guest.wasm examples/sample_package/logic/hello.wasm`

Run

- `cargo run -p rune`
- Try the three forms in the sample:
  - Contact: uses `onsubmit_intent = "submit_contact_form"`; named handler augments data, native submit proceeds.
  - Plain: no intent; global listener does nothing special; native submit proceeds.
  - Intercept: global listener cancels native submit.
