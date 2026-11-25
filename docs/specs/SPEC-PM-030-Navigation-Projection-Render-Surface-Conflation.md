# SPEC-PM-030: Navigation Projection and Render Surface Conflation

**Status:** Active  
**Series:** SPEC-PM (Pathology & Mistake Prevention)  
**Layer:** Navigation / Runtime Projection  
**Owner:** Core Architecture  
**Applies To:** Navigation context projection when several route entries share a render surface

## 1. Observed failure

On 2026-11-23, source review of the ordinary Web App history integration found
a projection-contract violation. The new same-window route mode kept a complete
entry-to-surface map, with every route entry mapped to the window's existing
surface. After constructing the ordered projection, it truncated
`navigation_surfaces` to one element.

The native fixture explicitly expected that one-element projection, including
when the committed path was Home followed by Confirm. The fixture separately
proved that the compositor retained one actual surface. Passing both assertions
did not establish the binding per-entry projection contract in HAL-246.

This incident was established from source and the existing assertion. It is
not a claim that the retained browser run failed or rendered the route twice.

## 2. Cause

Navigation testimony was reduced according to render-resource uniqueness.
Those are different quantities.

HAL-246 sources one surface identifier from each complete committed path entry,
in path order. Several entries may resolve to the same identifier. Repetition
preserves the route projection; it does not allocate additional surfaces.

HAL-248 preserves that testimony through the frame projection. Its resolved
view terminates at ScreenHost. Rendering continues to use the separate
compositor snapshot. A constraint on actual render targets must not silently
change the navigation projection's cardinality.

## 3. Prevention rules

These rules enforce the existing HAL-246/HAL-248 contracts; they introduce no
new navigation policy or rendering authority.

1. Produce one mapped identifier for every committed projection-path entry,
   preserving order. Reject incomplete or inconsistent mappings. Do not
   truncate, deduplicate, or filter the result because identifiers repeat.
2. Read and validate the active surface from its own authoritative field.
   Repeated identifiers do not authorize reconstruction of active state.
3. Prove resource reuse at the compositor allocation/snapshot boundary.
   Prove complete route testimony at the navigation projection boundary.
   Neither assertion substitutes for the other.
4. Preserve the complete frame projection mechanically. Do not compensate
   for an upstream truncation by reconstructing missing entries downstream.
5. Any different projection shape requires the explicit authority change
   required by HAL-246 section 8 before implementation.

## 4. Regression boundary

The existing `web_history_` fixture in
[`web_navigation_tests.rs`](../../src/core/app/web_navigation_tests.rs)
must verify both facts through the real prepared standalone transaction:

- a committed Home/Confirm path projects two ordered identifiers, both naming
  the same window surface;
- a root-only path projects one identifier;
- the active surface remains the window surface and the compositor still has
  one actual render surface.

Keep the existing history, guard, frame-participation and structural-Up
assertions. Do not replace their evidence with a projection-only helper.

## 5. Governing references

- [HAL-246 sections 3 and 8](SPEC-HAL-246-SurfaceStack-Navigation-Projection-Contract.md):
  per-entry field sourcing and the closed projection contract.
- [HAL-248 section 3](SPEC-HAL-248-Runtime-Navigation-Projection-Pull.md):
  exact frame composition and the projection/rendering boundary.
- [PM-000](SPEC-PM-000-Pathology-and-Mistake-Prevention-Series.md):
  recording a violated projection boundary.
