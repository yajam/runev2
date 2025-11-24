# Animation Module Implementation Checklist

This document outlines the implementation plan for adding CSS-like animations to the rune-scene IR runtime.

## Overview

The animation module will provide CSS-like animation capabilities for IR nodes, supporting:
- **Transitions**: Smooth interpolation between property values on state change
- **Keyframe Animations**: Multi-step animations with defined keyframes
- **Easing Functions**: Standard CSS timing functions
- **Animation Events**: Callbacks for animation lifecycle

---

## Phase 1: Core Animation Infrastructure

### 1.1 Animation Types & Data Structures

- [ ] Create `crates/rune-scene/src/animation/mod.rs` module entry point
- [ ] Define `AnimatableValue` enum for all animatable property types:
  - [ ] `F64(f64)` - for numeric properties (width, height, opacity, etc.)
  - [ ] `Color([f32; 4])` - for RGBA color values
  - [ ] `EdgeInsets { top, right, bottom, left }` - for padding/margin
  - [ ] `Transform { translate_x, translate_y, scale_x, scale_y, rotate }` - for transforms
  - [ ] `BoxShadow { offset_x, offset_y, blur, color }` - for shadows
- [ ] Define `AnimatableProperty` enum mapping to IR node properties:
  - [ ] Geometry: `Width`, `Height`, `MinWidth`, `MinHeight`, `MaxWidth`, `MaxHeight`
  - [ ] Spacing: `PaddingTop`, `PaddingRight`, `PaddingBottom`, `PaddingLeft`, `MarginTop`, `MarginRight`, `MarginBottom`, `MarginLeft`
  - [ ] Visual: `Opacity`, `CornerRadius`, `BorderWidth`, `BorderColor`
  - [ ] Background: `BackgroundColor`
  - [ ] Text: `FontSize`, `TextColor`
  - [ ] Transform: `TranslateX`, `TranslateY`, `ScaleX`, `ScaleY`, `Rotate`
  - [ ] Shadow: `BoxShadowOffsetX`, `BoxShadowOffsetY`, `BoxShadowBlur`, `BoxShadowColor`
- [ ] Create `AnimationId` type (u64 or UUID)
- [ ] Create `AnimationState` enum: `Pending`, `Running`, `Paused`, `Finished`, `Cancelled`

### 1.2 Easing Functions

- [ ] Create `crates/rune-scene/src/animation/easing.rs`
- [ ] Implement `EasingFunction` enum:
  - [ ] `Linear`
  - [ ] `Ease` (default CSS ease)
  - [ ] `EaseIn`
  - [ ] `EaseOut`
  - [ ] `EaseInOut`
  - [ ] `CubicBezier(f32, f32, f32, f32)` - custom bezier curves
  - [ ] `Steps(u32, StepPosition)` - stepped animations
  - [ ] `Spring { stiffness, damping, mass }` - physics-based (stretch goal)
- [ ] Implement `StepPosition` enum: `Start`, `End`, `Both`, `None`
- [ ] Implement `fn evaluate(&self, t: f32) -> f32` for each easing function
- [ ] Add unit tests for all easing functions

### 1.3 Interpolation System

- [ ] Create `crates/rune-scene/src/animation/interpolate.rs`
- [ ] Implement `Interpolate` trait:
  ```rust
  pub trait Interpolate {
      fn interpolate(&self, to: &Self, t: f32) -> Self;
  }
  ```
- [ ] Implement `Interpolate` for:
  - [ ] `f64` (linear interpolation)
  - [ ] `[f32; 4]` (color - consider color space for smooth blending)
  - [ ] `EdgeInsets` (per-component interpolation)
  - [ ] `Transform` (decomposed interpolation)
  - [ ] `BoxShadowSpec` (per-component interpolation)
- [ ] Add color space handling (sRGB vs linear for smooth gradients)
- [ ] Add unit tests for interpolation

---

## Phase 2: Transition System

### 2.1 Transition Definition

- [ ] Create `crates/rune-scene/src/animation/transition.rs`
- [ ] Define `TransitionSpec` struct:
  ```rust
  pub struct TransitionSpec {
      pub property: AnimatableProperty,    // or "all"
      pub duration_ms: f32,
      pub delay_ms: f32,
      pub easing: EasingFunction,
  }
  ```
- [ ] Define `TransitionGroup` for multiple property transitions
- [ ] Add `transition` field to relevant IR spec structs (FlexContainerSpec, etc.)
- [ ] Support shorthand `transition: all 300ms ease` style definitions

### 2.2 Active Transition Tracking

- [ ] Create `ActiveTransition` struct:
  ```rust
  pub struct ActiveTransition {
      pub id: AnimationId,
      pub node_id: ViewNodeId,
      pub property: AnimatableProperty,
      pub from_value: AnimatableValue,
      pub to_value: AnimatableValue,
      pub duration_ms: f32,
      pub delay_ms: f32,
      pub elapsed_ms: f32,
      pub easing: EasingFunction,
      pub state: AnimationState,
  }
  ```
- [ ] Implement `fn current_value(&self) -> AnimatableValue`
- [ ] Implement `fn update(&mut self, delta_ms: f32) -> bool` (returns true if still running)

### 2.3 Transition Manager

- [ ] Create `TransitionManager` struct in `crates/rune-scene/src/animation/manager.rs`
- [ ] Implement `fn start_transition(node_id, property, from, to, spec) -> AnimationId`
- [ ] Implement `fn update(&mut self, delta_ms: f32)` - advance all active transitions
- [ ] Implement `fn get_animated_value(node_id, property) -> Option<AnimatableValue>`
- [ ] Implement `fn cancel_transition(id)` and `fn cancel_all_for_node(node_id)`
- [ ] Handle transition interruption (new transition starts while one is running)

---

## Phase 3: Keyframe Animation System

### 3.1 Keyframe Definition

- [ ] Create `crates/rune-scene/src/animation/keyframes.rs`
- [ ] Define `Keyframe` struct:
  ```rust
  pub struct Keyframe {
      pub offset: f32,  // 0.0 to 1.0 (0% to 100%)
      pub values: HashMap<AnimatableProperty, AnimatableValue>,
      pub easing: Option<EasingFunction>,  // easing TO this keyframe
  }
  ```
- [ ] Define `KeyframeAnimation` struct:
  ```rust
  pub struct KeyframeAnimation {
      pub name: String,
      pub keyframes: Vec<Keyframe>,
      pub duration_ms: f32,
      pub delay_ms: f32,
      pub iteration_count: IterationCount,  // Number or Infinite
      pub direction: AnimationDirection,
      pub fill_mode: AnimationFillMode,
      pub play_state: AnimationPlayState,
  }
  ```
- [ ] Define `IterationCount` enum: `Count(f32)`, `Infinite`
- [ ] Define `AnimationDirection`: `Normal`, `Reverse`, `Alternate`, `AlternateReverse`
- [ ] Define `AnimationFillMode`: `None`, `Forwards`, `Backwards`, `Both`
- [ ] Define `AnimationPlayState`: `Running`, `Paused`

### 3.2 Animation Registry

- [ ] Create global/scoped animation registry for named keyframe animations
- [ ] Support defining animations in IR schema (ViewDocument level)
- [ ] Support referencing animations by name from node specs

### 3.3 Active Animation Tracking

- [ ] Create `ActiveKeyframeAnimation` struct:
  ```rust
  pub struct ActiveKeyframeAnimation {
      pub id: AnimationId,
      pub node_id: ViewNodeId,
      pub animation: KeyframeAnimation,
      pub elapsed_ms: f32,
      pub current_iteration: f32,
      pub state: AnimationState,
  }
  ```
- [ ] Implement keyframe interpolation between adjacent keyframes
- [ ] Handle iteration counting and direction changes
- [ ] Implement fill mode behavior

### 3.4 Animation Manager Extension

- [ ] Extend manager to handle keyframe animations
- [ ] Implement `fn start_animation(node_id, animation_name) -> AnimationId`
- [ ] Implement `fn pause_animation(id)` and `fn resume_animation(id)`
- [ ] Handle animation completion and iteration events

---

## Phase 4: IR Schema Integration

### 4.1 Spec Extensions

- [ ] Add `animations` field to `ViewDocument`:
  ```rust
  pub struct ViewDocument {
      // ... existing fields
      pub animations: HashMap<String, KeyframeAnimation>,
  }
  ```
- [ ] Add animation properties to container specs (FlexContainerSpec, etc.):
  ```rust
  pub struct FlexContainerSpec {
      // ... existing fields
      pub transition: Option<TransitionGroup>,
      pub animation: Option<AnimationRef>,
  }
  ```
- [ ] Define `AnimationRef` struct:
  ```rust
  pub struct AnimationRef {
      pub name: String,
      pub duration_ms: Option<f32>,   // override
      pub delay_ms: Option<f32>,      // override
      pub iteration_count: Option<IterationCount>,
      pub direction: Option<AnimationDirection>,
      pub fill_mode: Option<AnimationFillMode>,
  }
  ```

### 4.2 Serde Support

- [ ] Implement Serialize/Deserialize for all animation types
- [ ] Support CSS-like shorthand syntax parsing where appropriate
- [ ] Add JSON schema documentation for animation properties

---

## Phase 5: Rendering Integration

### 5.1 Animated Property Resolution

- [ ] Create `crates/rune-scene/src/animation/resolver.rs`
- [ ] Implement `AnimatedPropertyResolver`:
  ```rust
  pub struct AnimatedPropertyResolver<'a> {
      pub animation_manager: &'a AnimationManager,
  }

  impl AnimatedPropertyResolver {
      pub fn resolve_f64(&self, node_id: &ViewNodeId, property: AnimatableProperty, base: f64) -> f64;
      pub fn resolve_color(&self, node_id: &ViewNodeId, property: AnimatableProperty, base: [f32; 4]) -> [f32; 4];
      // ... etc
  }
  ```
- [ ] Integrate resolver into `IrRenderer` rendering path

### 5.2 Layout Integration

- [ ] Handle animated layout properties (width, height, padding, margin)
- [ ] Determine when to trigger Taffy relayout during animations
- [ ] Optimize: batch layout updates, skip when only visual properties change

### 5.3 Rendering Loop Integration

- [ ] Add animation update to frame loop in `runner.rs`:
  ```rust
  // In RedrawRequested handler:
  animation_manager.update(delta_time * 1000.0);
  let needs_animation_redraw = animation_manager.has_active_animations();
  ```
- [ ] Request continuous redraw when animations are active
- [ ] Integrate animated values into element rendering functions

---

## Phase 6: State Change Detection

### 6.1 Property Change Tracking

- [ ] Implement property value snapshot for transition triggers
- [ ] Detect property changes between frames/updates
- [ ] Auto-start transitions when transitioned properties change

### 6.2 Trigger Mechanisms

- [ ] Support explicit animation triggers via intents/events
- [ ] Support hover/focus state transitions (`:hover`, `:focus` equivalents)
- [ ] Support class/variant-based state changes

---

## Phase 7: Animation Events & Callbacks

### 7.1 Event Types

- [ ] Define `AnimationEvent` enum:
  - [ ] `Started { animation_id, node_id }`
  - [ ] `Ended { animation_id, node_id }`
  - [ ] `Cancelled { animation_id, node_id }`
  - [ ] `Iteration { animation_id, node_id, iteration }`
- [ ] Define `TransitionEvent` enum:
  - [ ] `Started { node_id, property }`
  - [ ] `Ended { node_id, property }`
  - [ ] `Cancelled { node_id, property }`

### 7.2 Event Dispatch

- [ ] Add event queue to animation manager
- [ ] Implement event polling/callback system
- [ ] Integrate with IR intent system for animation-triggered actions

---

## Phase 8: Transform Support

### 8.1 Transform Stack

- [ ] Create `Transform2D` struct with decomposed components
- [ ] Implement transform composition and decomposition
- [ ] Add `transform` property to relevant IR specs

### 8.2 Transform Rendering

- [ ] Integrate transform into rendering pipeline
- [ ] Handle transform origin point
- [ ] Apply transforms during display list building

---

## Phase 9: Opacity & Visibility

### 9.1 Opacity Support

- [ ] Add `opacity: Option<f64>` to container specs
- [ ] Implement opacity in rendering (multiply alpha)
- [ ] Handle opacity inheritance for nested elements

### 9.2 Visibility Transitions

- [ ] Support `visibility` property with transition
- [ ] Handle `display: none` equivalent in IR

---

## Phase 10: Testing & Demo

### 10.1 Unit Tests

- [ ] Test all easing functions
- [ ] Test interpolation for all animatable types
- [ ] Test transition lifecycle (start, update, complete, cancel)
- [ ] Test keyframe animation with multiple iterations
- [ ] Test animation direction and fill modes

### 10.2 Integration Tests

- [ ] Test animation with layout changes
- [ ] Test multiple simultaneous animations on same node
- [ ] Test animation interruption scenarios
- [ ] Test performance with many active animations

### 10.3 Demo Scene

- [ ] Create `animation` demo scene in demo-app
- [ ] Showcase transitions on hover/focus
- [ ] Showcase keyframe animations
- [ ] Showcase various easing functions
- [ ] Add to `cargo run -p demo-app -- --scene=animation`

---

## Phase 11: Documentation

- [ ] Document animation module architecture in `docs/animation.md`
- [ ] Add animation examples to IR schema documentation
- [ ] Document performance considerations
- [ ] Add migration guide for existing IR documents
- [ ] Update `CLAUDE.md` with animation-related information

---

## Phase 12: Performance Optimization

### 12.1 Efficiency

- [ ] Batch animation updates
- [ ] Skip rendering unchanged frames
- [ ] Use dirty flags for animated properties
- [ ] Consider GPU-accelerated animations for transforms/opacity

### 12.2 Memory

- [ ] Pool animation objects
- [ ] Clean up completed animations promptly
- [ ] Limit maximum concurrent animations per node

---

## File Structure

```
crates/rune-scene/src/animation/
├── mod.rs              # Module exports and AnimationManager
├── types.rs            # AnimatableValue, AnimatableProperty, AnimationState
├── easing.rs           # EasingFunction implementations
├── interpolate.rs      # Interpolate trait and implementations
├── transition.rs       # TransitionSpec, ActiveTransition
├── keyframes.rs        # Keyframe, KeyframeAnimation
├── manager.rs          # AnimationManager (transitions + keyframes)
├── resolver.rs         # AnimatedPropertyResolver for rendering
└── events.rs           # AnimationEvent, TransitionEvent
```

---

## Dependencies

- No new external crates required for core functionality
- Consider `interpolation` crate for additional easing curves (optional)
- Consider `palette` crate for better color interpolation (optional)

---

## Priority Order

1. **Core Infrastructure** (Phase 1) - Foundation for all animation work
2. **Transitions** (Phase 2) - Most common use case, immediate value
3. **Rendering Integration** (Phase 5) - Make transitions visible
4. **State Change Detection** (Phase 6) - Automatic transition triggering
5. **Keyframe Animations** (Phase 3-4) - Extended animation capabilities
6. **Transform Support** (Phase 8) - Enable transform animations
7. **Opacity** (Phase 9) - Common animation property
8. **Events** (Phase 7) - Animation lifecycle hooks
9. **Testing & Demo** (Phase 10) - Validation and showcase
10. **Documentation** (Phase 11) - Knowledge sharing
11. **Optimization** (Phase 12) - Performance tuning

---

## CSS Animation Parity Reference

This implementation aims for parity with these CSS features:
- `transition` property
- `@keyframes` rule
- `animation` property
- `animation-timing-function`
- `animation-duration`, `animation-delay`
- `animation-iteration-count`
- `animation-direction`
- `animation-fill-mode`
- `animation-play-state`
- `transform` property (2D subset)
- `opacity` property

---

## Notes

- **Thread Safety**: AnimationManager should be `Send` for potential future async usage
- **Determinism**: Animations should produce identical results given same inputs (no RNG)
- **Frame Independence**: Use delta time, not frame count, for timing
- **Cancellation**: Always support clean cancellation without visual glitches
