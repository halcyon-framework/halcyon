# SPEC-PM-019 — Terminal transition entries as false geometry effects

**Status:** Proposed pathology record; inactive pending Core Architecture / technical acceptance.  
**Date:** 2026-10-20  
**Owner:** Core Architecture  
**Scope:** Existing Timeline → layout-transition bookkeeping → App checkpoint → qualified Absolute-drag boundary.

## Verified failure

Native App advances Timeline without the completed-layout-transition cleanup already present in the dedicated UI loop. The shared geometry checkpoint treats every retained transition entry as a current causal effect, even after Timeline completion. A stable ancestor can therefore acquire a fresh geometry origin on every routed press although it has no remaining transition projection. The opted-in Absolute press is retired before it can commit.

The framework reproduction uses the real ScreenHost, native bootstrap ordering (fresh pre-layout projection, buffer swap, first Layout), actual candidate authorization/binding, real Timeline, qualified InputFusion/EventRouter and the existing retained scene harness. After completion, Down produced one candidate-cancellation callback where none was allowed. That causal RED used the ScreenHost-wide reflow root. The amended regression instead declares the real local ScrollArea canvas as both viewport owner and reflow root, with persistent ScreenHost above that local domain; its separately executed affected proof passed 1/1. The preceding test-premise failure omitted required appearance facts; it is not the causal RED.

Baseline app.rs SHA256: 3b03b49bcf4f96a448672fd5fce4987125e014ae96a54e2965e2abb45071ce6a. Baseline layout_transition.rs SHA256: d4817b7d052422f9ca4ca0ea56dc75edede404a68ecd57d9fd947ae93f6997d9.

## Why local green proofs missed it

The dedicated loop performs cleanup while native App did not. Raw timing tests without a retained completed ancestor cannot exercise that state. Isolated completion tests explicitly cleaned the map. A completed interpolation disappearing from Paint did not prove its map entry stopped influencing the input checkpoint.

## Existing governing boundaries

This record creates no new authority or amendment.

- [HAL-128 §14](SPEC-HAL-128-Pointer-Capture-and-Gesture-Arbitration.md) admits actual affected active transitions, excludes ephemeral layout take/restore and geometry output writes as causes and preserves terminal cancellation for real effects.
- [HAL-145 §J](SPEC-HAL-145-Layout-Transitions-and-Reflow-Authority.md) repeats the active-effect boundary.
- [HAL-078](SPEC-HAL-078-Layout-Transitions-v1.md) separates target geometry from projection and requires bounded/no-op bookkeeping.
- [HAL-021](SPEC-HAL-021-Animations.md) owns Active/Completed/Cancelled/Missing and exact terminal observation. Caller-bookkeeping removal must preserve that Timeline observation window.
- [PM-000 §2](SPEC-PM-000-Pathology-and-Mistake-Prevention-Series.md) requires recording this architectural boundary failure. No previously fixed recurrence is claimed.

## Correction and minimum prevention proof

Native App uses existing completed-entry cleanup after its single Timeline advance. The shared checkpoint consults Timeline activity before admitting a retained entry as an effect, including entries not yet pruned. Routing, capture policy, canonical geometry, interpolation, clocks and Timeline retirement remain unchanged.

The focused existing owner is src/tests/retained_value_updates.rs, test geometry_dependent_absolute_native_completed_transition_preserves_drag: a completed ancestor permits drag/capture and one release commitment; a still-running affected ancestor remains terminal cancellation with no commitment. Existing reflow/clip/paint-only proofs retain their own scope. This is headless framework/native-order evidence, not native OS or product acceptance.

## Forbidden recurrence pattern

Map membership, a scheduled frame or stale presentation bookkeeping must not substitute for the actual lifetime/effect required by accepted authority. Do not bypass a product geometry dependency to hide false framework invalidation.

Independent owner acceptance remains pending; this record is not a Binding SPEC promotion.
