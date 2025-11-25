# Video Element Integration Checklist

Plan for adding a first-class `Video` element to **rune-ir** and **rune-scene**, including rendering, playback control, and IR-driven configuration, while keeping the media backend pluggable.

---

## Phase 0: Scope & Constraints

### 0.1 Product/Runtime Decisions

- [x] Initial target runtimes: native desktop Rune (`rune-app` macOS/Windows/Linux) plus future mobile (iOS/Android); no browser-hosted target.
- [x] Primary decode backend strategy:
  - Desktop (macOS/Windows/Linux): `gstreamer-rs`
  - iOS: AVFoundation-based wrapper
  - Android: ExoPlayer/MediaCodec-based wrapper
- [ ] Decide if audio is mandatory for v1 or if muted/visual-only playback is acceptable
- [ ] Define maximum supported resolution / frame rate targets (e.g. 1080p60) and memory budget
- [ ] Decide on first class of sources: local file paths, packaged assets, HTTP URLs, or IR-bound data fields

### 0.2 UX / Feature Scope

- [ ] Define UX for inline video playback (always-visible controls vs. hover/overlay)
- [ ] Decide which controls are required for v1 (play/pause, seek bar, mute, volume, full-screen toggle)
- [ ] Decide whether `Video` is:
  - [ ] A leaf widget with built-in controls, or
  - [ ] A pure media surface with separate IR elements for controls (recommended for flexibility)
- [ ] Define expected behavior for resize (maintain aspect ratio vs. stretch/fill)
- [ ] Define behavior for autoplay, loop, muted, and poster image

---

## Phase 1: IR Schema & View Model

Goal: Introduce a `Video` node type in **rune-ir** that is fully serializable, schema-validated, and ready for IR renderer mapping.

### 1.1 `ViewNodeKind` Extension

- [ ] Add `Video(VideoSpec)` variant to `ViewNodeKind` in `crates/rune-ir/src/view/mod.rs`
- [ ] Ensure `Video` is tagged as `type: "video"` via Serde (`#[serde(tag = "type", rename_all = "snake_case")]`)
- [ ] Place `Video` in the **Content** section of docs and mapping tables (near `Image`)
- [ ] Regenerate / update any JSON schema references in `crates/rune-ir/src/schema.rs` to include `video`

### 1.2 `VideoSpec` Definition

- [ ] Create `VideoSpec` struct in `crates/rune-ir/src/view/mod.rs` (or dedicated `video.rs` submodule if the file becomes too large)
- [ ] Include core fields:
  - [ ] `src: String` (or data binding compatible type)
  - [ ] `poster: Option<String>` (image/thumbnail shown before playback)
  - [ ] `auto_play: bool`
  - [ ] `loop_playback: bool`
  - [ ] `muted: bool`
  - [ ] `controls: bool` (whether default inline controls should appear)
  - [ ] `playback_rate: Option<f64>`
  - [ ] `start_time_sec: Option<f64>`
  - [ ] `end_time_sec: Option<f64>`
  - [ ] `fit: Option<ContentFit>` (e.g. contain, cover, fill, scale_down)
- [ ] Reuse or introduce a simple `ContentFit` enum (sharing semantics with `Image` if possible)
- [ ] Add optional visual styling fields if needed (border radius, background) or document that container elements provide styling
- [ ] Add `serde(default)` and `skip_serializing_if` annotations for backwards-compatible defaults

### 1.3 Event & Interaction Hooks

- [ ] Define `VideoEventsSpec` (or similar) with optional intent names:
  - [ ] `on_play: Option<String>`
  - [ ] `on_pause: Option<String>`
  - [ ] `on_ended: Option<String>`
  - [ ] `on_error: Option<String>`
  - [ ] `on_time_update: Option<String>` (rate-limited)
- [ ] Embed `events: Option<VideoEventsSpec>` field inside `VideoSpec`
- [ ] Decide how time-update frequency is exposed/configurable (e.g. max Hz, min delta seconds)
- [ ] Wire these event intent names to the existing IR logic/mutation system (e.g. by extending event routing docs; implementation comes later)

### 1.4 Data Binding Integration

- [ ] Decide how `src`/`poster` can be bound to `DataDocument` (reuse pattern from `Image` / `Text`)
- [ ] If needed, add explicit `Binding` fields so video source can be changed by mutations
- [ ] Document expected mutation patterns (e.g. change `src`, seek to a timestamp via mutation vs. direct host call)

### 1.5 Schema & Tooling

- [ ] Update JSON schemas in `crates/rune-ir/src/schema.rs` to include `VideoSpec` shape
- [ ] Regenerate any published schema artifacts or test fixtures that assume a closed set of `type` values
- [ ] Add at least one sample `ViewDocument` snippet using `type: "video"` for schema validation tests

---

## Phase 2: rune-scene Element & Rendering Surface

Goal: Implement a `Video` element in **rune-scene** that can render a video frame (or poster) into the Canvas and expose the surface for media backend updates.

### 2.1 Element Skeleton

- [ ] Create `crates/rune-scene/src/elements/video.rs` with a `Video` struct
- [ ] Decide whether `Video` owns its own playback state or delegates to a shared `MediaManager`
- [ ] Add module exports in `crates/rune-scene/src/elements/mod.rs`:
  - [ ] `pub mod video;`
  - [ ] `pub use video::Video;`
- [ ] Mirror API ergonomics from `ImageBox` for basic construction and rendering

### 2.2 Layout & Sizing

- [ ] Decide how `Video` derives its rect:
  - [ ] Accept an `engine_core::Rect` provided by layout (IR renderer / app state)
  - [ ] Maintain intrinsic aspect ratio if only width or height is specified
- [ ] Implement `fit` behavior (contain/cover/fill/scale_down) consistent with `ImageBox`
- [ ] Decide whether `Video` can be clipped by container scroll regions (rely on Canvas clipping rules)

### 2.3 Rendering Path

- [ ] Define an internal type for the current video frame (e.g. `VideoFrameHandle` or texture ID)
- [ ] Reuse the existing image/texture pipeline from `engine-core` for sampling frames
- [ ] Implement `Video::render(&self, canvas: &mut Canvas, z: i32)`:
  - [ ] If a decoded frame is available, draw it into the provided rect
  - [ ] If no frame is available yet, draw `poster` image if provided
  - [ ] If neither is available, draw a placeholder (e.g. gray rect + play icon)
- [ ] Ensure color space and premultiplied alpha assumptions match image pipeline

### 2.4 Controls & Hit Testing (Optional for v1)

- [ ] Decide minimal built-in controls for v1 (e.g. overlay play/pause, simple scrub bar)
- [ ] Implement internal hit testing for controls if `controls: true`
- [ ] Expose simple events (e.g. `VideoClickResult`) for integration with the event router, mirroring patterns from `Button` / `FileInput`
- [ ] Ensure `Video` can be used in both:
  - [ ] Stateless/IR-driven rendering (render only), and
  - [ ] Stateful widget usage with full `EventHandler` integration

---

## Phase 3: Media Runtime / Backend Integration

Goal: Introduce a backend-agnostic media layer that feeds decoded video frames into `Video` elements without blocking the render loop.

### 3.1 Media Abstraction Layer

- [ ] Define a `MediaBackend` trait in an appropriate crate (`rune-scene` or new `rune-media`):
  - [ ] Methods to open/close media by id or URL
  - [ ] Play/pause/seek/set_rate/mute/volume controls
  - [ ] Query duration, current time, playback state
  - [ ] Retrieve latest frame handle/texture for a given media id
- [ ] Define a `MediaId` newtype for stable references from `Video` elements
- [ ] Decide threading model (decode on background threads, render on main thread)

### 3.2 Backend Implementation (Per-Platform)

- [ ] Implement `MediaBackend` using `gstreamer-rs` for desktop (macOS/Windows/Linux)
- [ ] Implement `MediaBackend` using an AVFoundation wrapper on iOS
- [ ] Implement `MediaBackend` using an ExoPlayer/MediaCodec wrapper on Android
- [ ] For each backend, implement a non-blocking decode loop that:
  - [ ] Pulls frames at target FPS or when timestamps demand
  - [ ] Uploads frames to GPU textures compatible with `engine-core`
- [ ] Ensure clean teardown when a media resource is no longer referenced (drop frames, free textures)
- [ ] Provide a simple error channel for decode failures (surfaced to `Video` and IR via events)

### 3.3 Time & Frame Synchronization

- [ ] Decide on the clock source (vsync/frame time vs. media clock)
- [ ] Implement logic for mapping `current_time_sec` → frame selection
- [ ] Handle pause/resume without drifting (store paused time, resume from same timestamp)
- [ ] Handle seek operations efficiently (flush old frames, request new position)
- [ ] Ensure the renderer marks affected `Video` nodes as dirty when frames advance

### 3.4 Audio Handling (Optional / Later Phase)

- [ ] Decide whether audio output is part of this layer or delegated to host app
- [ ] If included, ensure sync between audio clock and video frames
- [ ] Expose mute/volume controls and audio device selection as needed

---

## Phase 4: IR Renderer & rune-ir Integration

Goal: Wire `VideoSpec` from **rune-ir** into **rune-scene**’s IR renderer and layout system.

### 4.1 IR Adapter Mapping

- [ ] Extend `crates/rune-scene/src/ir_adapter.rs` (or equivalent) to handle `ViewNodeKind::Video`
- [ ] Map `VideoSpec` fields into a `Video` element instance:
  - [ ] Resolve `src` and `poster` from `DataDocument` bindings if applicable
  - [ ] Propagate `auto_play`, `loop_playback`, `muted`, `controls`, `fit`
  - [ ] Pass event intent names or keep them attached for event routing
- [ ] Decide how media IDs are allocated (e.g. `node_id` → `MediaId` mapping)
- [ ] Ensure `Video` participates correctly in layout and z-ordering

### 4.2 IR Renderer Implementation

- [ ] Add a dedicated `render_video_element` helper in `crates/rune-scene/src/ir_renderer/elements.rs`
- [ ] Reuse sizing rules from `Image` where possible to keep behavior predictable
- [ ] Ensure scrollable containers clip video content correctly
- [ ] Implement fallback behavior when media backend is unavailable (poster/placeholder)

### 4.3 Documentation & Mapping Tables

- [ ] Update `docs/ir-element-mapping.md` to include `Video` row under **Content**
- [ ] Document expected IR fields and defaults for `Video` in `docs/rune-scene.md` or a dedicated section
- [ ] Add a short “how to author a video element in IR” snippet to `docs/usage.md` or another appropriate guide

---

## Phase 5: WASM / Logic Integration (Optional but Recommended)

Goal: Allow WASM logic (via **rune-wasm**) to control video playback and respond to video events through IR mutations or host calls.

### 5.1 Host API Design

- [ ] Decide between:
  - [ ] Pure IR mutation control (e.g. `SetVideoState { node_id, playing, time }`), or
  - [ ] Direct host calls (e.g. `rune.video_play(node_id)`)
  - [ ] Hybrid approach (mutations for declarative state, host calls for immediate control)
- [ ] Design a minimal set of operations:
  - [ ] play / pause
  - [ ] seek_to(time_sec)
  - [ ] set_muted(bool) / set_volume(f32)
  - [ ] set_playback_rate(f32)

### 5.2 Mutation Types & Handlers

- [ ] If using mutations, add new mutation types in `crates/rune-ir/src/logic/mutation.rs` for video control
- [ ] Implement handlers in the rune-scene mutation processor to translate mutations into `MediaBackend` operations
- [ ] Ensure idempotent behavior when mutations arrive out-of-order or duplicate

### 5.3 Event Propagation to WASM

- [ ] Wire `Video` element events (`on_play`, `on_pause`, `on_ended`, `on_error`, `on_time_update`) into the existing event → mutation → WASM pipeline
- [ ] Implement throttling for `on_time_update` to avoid flooding WASM with events
- [ ] Add example WASM module that reacts to video events (e.g. progress bar, analytics ping)

---

## Phase 6: Testing, Samples & Validation

### 6.1 Unit & Integration Tests

- [ ] Add unit tests for `VideoSpec` serialization/deserialization (JSON roundtrips)
- [ ] Add schema validation tests to ensure `type: "video"` documents validate successfully
- [ ] Add tests for IR renderer mapping from `VideoSpec` to `Video` element with various field combinations

### 6.2 Sample Packages & Demos

- [ ] Create a minimal IR package with a single `Video` element and poster image
- [ ] Add a sample with multiple video instances (muted autoplay grid)
- [ ] Add a sample demonstrating WASM-controlled playback (if Phase 5 is implemented)
- [ ] Document how to run these samples via `USE_IR=1 cargo run -p rune-scene -- <path>`

### 6.3 Performance & Quality Validation

- [ ] Measure CPU and GPU utilization for typical video resolutions
- [ ] Validate frame pacing and smoothness under resize + other scene activity
- [ ] Test behavior under slow I/O or network conditions (if HTTP sources are supported)
- [ ] Verify memory is released when videos are removed from the scene or documents are swapped

---

## Phase 7: Future Enhancements (Non-Blocking)

- [ ] Subtitle / closed caption track support (e.g. WebVTT integration)
- [ ] Multiple audio tracks and language selection
- [ ] Picture-in-picture or floating video overlay modes
- [ ] Advanced controls theming via IR (compose controls from standard widgets instead of built-ins)
- [ ] DRM-protected streams support (if product requirements demand it)
- [ ] Integration with telemetry/tracing for playback analytics
