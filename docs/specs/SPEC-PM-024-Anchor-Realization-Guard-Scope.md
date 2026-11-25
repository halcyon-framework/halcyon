# SPEC-PM-024 — Anchor realization guard applied before resolver qualification

- **Status** Recorded postmortem under existing authority
- **Date** 2026-11-10
- **Owner** Core Architecture
- **Scope** App post-layout anchor resolution for wired ScrollArea content

## Verified failure

`post_layout_resolve_scroll_anchors` iterates the typed ScrollArea index. The old
pass asserted positive content geometry before checking the live pair and asking
the child for an anchor resolver. A mounted nonresolver with a completed zero
viewport layout tripped `INV-REALIZE-ORDER-1`. The zero fallback for an absent
child also reported a resolver realization fault. A positive-sized slot reused
under a different parent escaped structural validation.

The index identifies scroll containers. It conveys no resolver capability.
Positive-size fixtures hide this mistake because their rectangles satisfy the
assertion even when their content never supplies a resolver.

## Prevention under existing authority

Validate the live ScrollArea, sealed content reference and child relationship
before using the pair. Missing or mismatched wiring is a structural failure
under [HAL-154](SPEC-HAL-154-Sealed-After-Layout-Deferred-Wiring-Contract.md).
For a valid child, only `as_anchor_resolver() == Some` qualifies the existing
realization assertion and anchor call under
[HAL-131A Stage 15](SPEC-HAL-131A-Scroll-Substrate-and-Input-Arbitration.md).
Use the final Layout rect and keep the original positive-width-or-height
predicate and diagnostic. Direct scroll requests retain their existing phase.

Do not use index membership or rectangle dimensions as resolver evidence. Do not
convert absent wiring to a zero rectangle or suppress a true resolver failure
by treating a zero viewport as an exemption.

## Regression and evidence boundary

The [scroll layout anchor tests](../../src/tests/scroll_layout_anchor.rs) cover
zero and positive nonresolver geometry, direct scroll requests, the genuine List
guard, successful List resolution, missing content and wrong-parent slot reuse.
The isolated consumer replay exercised all 21 real resize extents with positive
indexed content geometry. The archival crash did not establish its failing
mount, resolver membership or final content rect. This record adds no feature
authority under [PM-000](SPEC-PM-000-Pathology-and-Mistake-Prevention-Series.md).
