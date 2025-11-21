# IR Renderer Guardrails

Keep the IR layer (Taffy/layout + mapper) thin and push visuals/interaction into self-contained elements.

## Core Rules
- Mapper only maps IR specs to element APIs (creates/updates element instances, passes data/flags). It must not paint UI or invent geometry.
- Each element module owns its visuals and hit geometry. Missing pieces (scrim, close buttons, hit regions) are added to element APIs, not reimplemented in the mapper.
- Overlay orchestration lives in one place (core): choose which overlay to show; call the element’s render/hit API. No ad-hoc drawing in core.
- No hardcoded strings or layout constants in the mapper except explicit defaults already defined in element APIs.

## Checklist Before Merging
1) Are all `fill_rect`/`hit_region_rect` calls inside element methods (or their helpers), not in the IR mapper? If found in the mapper, move them to the element.
2) Do overlays use element-provided scrim/layout/hit APIs? Core should just call those with IR data.
3) Does each element expose the props it needs (e.g., `render_scrim`, `register_hit_regions`, `render_chrome`, etc.) so the mapper stays orchestration-only?
4) Are IR props passed through directly? No silent overrides unless documented defaults exist in the element.
5) Are new behaviors unit-bound to the element module (tests/checks) rather than spread across mapper code?

## When Adding/Editing an Element
- Extend the element API to cover the needed visuals/hit regions.
- Keep mapper changes to wiring: create/update element, set props, call render/hit methods.
- Verify hit regions register via the element, not via mapper-side `hit_region_rect`.

## Troubleshooting Flow
- Layout/data issues: check IR mapper/core.
- Visuals/hit/scrim issues: check the element module for the overlay/control.
- Overlay activation flow: core + state; overlay visuals/hits: element API.
