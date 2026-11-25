# SPEC-HAL-150: Compositor Authority and Limits

> **Paired amendment status — Proposed (2026-10-05):** The scoped
> SPEC-HAL-363/364 text in this document is part of the proposed paired adoption
> package, not an independent adoption or implementation claim. It becomes
> effective only with joint adoption of
> [SPEC-HAL-363](SPEC-HAL-363-Transient-Presentation-Coordination-and-Lifetime.md)
> and [SPEC-HAL-364](SPEC-HAL-364-Presentation-Occurrences-Coverage-and-Execution-Admission.md)
> and their required scoped amendments. The existing declared status and
> implementation evidence apply to the pre-existing contract. All unlisted
> clauses and ordinary-owner boundaries remain unchanged.

**Status:** Binding
**Layer:** Rendering & Composition
**Scope:** Authority boundaries of the compositor and its interaction with layout, animation, and input
**Depends on:** SPEC-HAL-040, SPEC-HAL-145, SPEC-HAL-146, SPEC-HAL-147
**Constrains:** Engine-level semantics only (no widget or OS policy invention)

---

## SECTION A — Problem Statement

In many UI systems, the compositor becomes a **silent superpower**:

* it moves things “just visually”
* it applies transforms that layout doesn’t know about
* it animates geometry independently
* it masks state inconsistencies instead of exposing them

This leads to:

* hit-testing desync
* input mismatch
* animation/layout fights
* debugging by coincidence
* visual truth diverging from logical truth

Halcyon requires the compositor to be **powerful but obedient**.

This SPEC defines **what the compositor is allowed to do — and what it is absolutely forbidden to do**.

---

## SECTION B — Design Principles

**PRINCIPLE-1: Compositor is a projection layer**
It renders authorized state; it does not invent it.

**PRINCIPLE-2: No semantic authority**
The compositor owns no meaning, intent, or interaction.

**PRINCIPLE-3: Geometry truth is singular**
There is exactly one authoritative geometry source.

**PRINCIPLE-4: Visual tricks must be reversible**
Anything the compositor does must be explainable and unwindable.

**PRINCIPLE-5: Desync is a bug, not a feature**
The compositor must never “paper over” inconsistencies.

---

## SECTION C — Compositor Definition

The **Compositor** is the stage that:

* receives finalized render primitives
* applies GPU transforms
* batches, layers, and blends
* submits frames to the GPU

It does **not**:

* compute layout
* decide interaction
* mutate state
* resolve input

---

## SECTION D — Authority Boundaries

**INV-COMP-NO-STATE**
The compositor MUST NOT mutate engine state.

**INV-COMP-NO-INTENT**
The compositor MUST NOT generate or reinterpret user intent.

**INV-COMP-NO-GEOM-OWNERSHIP**
The compositor MUST NOT own geometry.

All geometry must originate from:

* layout engine (SPEC-HAL-145)
* animation interpolation (SPEC-HAL-147)

**363/364 scoped amendment:** The origin list additionally admits SPEC-HAL-364-certified
presentation values produced after SPEC-HAL-145/147 ownership resolution.
This grants the compositor no geometry ownership.

**HAL375 scoped adoption, 2026-11-07.** The existing Viewport3D layout and final-target preparation may supply the [first passive PBR HUD](SPEC-HAL-375-3D-Anchored-Spatial-HUD-and-Information-Connectors.md#9-adopted-first-passive-pbr-hud-profile--2026-11-07-s1)'s bounded derived label and leader geometry. The existing surface compiler/executor consumes it in the HAL359/360 scope with its matching private depth visibility result. The compositor owns neither projection, placement, source facts nor input.

---

## SECTION E — Allowed Operations

**363/364 scoped amendment:** For SPEC-HAL-363/364 occurrences, the compositor MAY execute only
the exact admitted operation/pipeline/resource/target profile under
SPEC-HAL-364. Translation, finite affine, mask, isolated group opacity and
scoped occurrence order are vocabulary, not blanket execution admission.
Animation sampling and policy remain upstream.

Outside this scoped path, existing separately admitted visual transforms,
clipping, masking, draw-layer order and batching retain their owning contracts.
The compositor consumes frames already resolved by animation; it never chooses
or samples the motion.

**INV-COMP-VISUAL-ONLY:** All compositor operations MUST be visual-only.

---

## SECTION F — Forbidden Operations

The compositor MUST NOT:

* move hit-test regions independently
* animate geometry without authorization
* override layout results
* invent visual positions not supplied by an admitted upstream presentation owner
* correct logical bugs visually

**INV-COMP-NO-LIES**
**363/364 scoped amendment:** The compositor SHALL NOT invent visual positions, correct canonical
errors through paint, mutate hit geometry or override layout. Rendered geometry
SHALL equal the upstream-authorized presentation projection subject to
authoritative coverage. Where no projection is admitted, it SHALL equal ordinary
canonical presentation. Canonical input/layout truth remains unchanged.

---

## SECTION G — Animation Interaction

Animations are resolved **before** the compositor.

**INV-COMP-ANIM-BOUNDARY**
The compositor receives animation output; it does not run animations.

**INV-COMP-ANIM-CONSISTENT**
Animation frames MUST be hit-test consistent.

**363/364 scoped amendment:** Input consistency means canonical routing plus SPEC-HAL-078's
existing contradiction/settlement rule. It does not require every pixel to occupy
a hit-test position during an authorized transition. The compositor SHALL NOT
perform hit testing or transform input regions.

---

## SECTION H — Input and Hit-Testing

Input resolution is upstream of the compositor.

**INV-COMP-NO-HITTEST**
The compositor MUST NOT perform hit-testing.

Rendered transforms MUST NOT invalidate hit-testing results.

**363/364 scoped amendment:** Rendered transforms cannot revise canonical hit-test results.
The Section G distinction governs admitted projection, not visual-to-input
feedback.

---

## SECTION I — Debugging and Inspection

**INV-COMP-INSPECTABLE**
All compositor inputs MUST be inspectable and replayable.

**INV-COMP-NO-HIDDEN-STATE**
No internal compositor state may affect interaction semantics.

---

## SECTION J — Failure Modes (Explicit)

If the compositor receives invalid or conflicting data:

* it MUST render exactly what it was given
* it MUST NOT “fix” it
* errors must surface upstream

**INV-COMP-FAIL-LOUD**
Silently correcting invalid state is forbidden.

**363/364 scoped amendment:** For SPEC-HAL-363/364's complete required composition, the instruction
to render exactly the supplied data means faithful execution only after complete
admission; it does not require execution of a known invalid group. The specific
SPEC-HAL-364 B-9.8/B-9.9 scene-choice cutoff and complete surface-unavailability
rule governs known admission failure, with B-9.18 for unforeseen later failure.
No compositor correction or alternative scene is authorized. Ordinary non-group
failure ownership is unchanged.

---

## SECTION K — Performance Guarantees

**INV-COMP-O1**
Compositor operations MUST be linear or better relative to primitives.

**INV-COMP-DETERMINISTIC**
Given identical inputs, output MUST be identical.

---

## SECTION L — Interaction with Existing SPECs

* Geometry authority: SPEC-HAL-145, SPEC-HAL-146
* Animation authority: SPEC-HAL-147
* Rendering substrate: SPEC-HAL-040

SPEC-HAL-150 introduces **no new rendering features**, only constraints.

---

## SECTION M — Acceptance Criteria

SPEC-HAL-150 is satisfied when:

* [ ] Compositor never mutates state
* [ ] Visual output matches logical geometry
* [ ] Hit-testing never desyncs from visuals
* [ ] Animations are fully authorized upstream
* [ ] Debugging reveals exact compositor inputs

**363/364 scoped amendment:** The logical-geometry and hit-test criteria use Sections F–H's
canonical/projection distinction. They do not forbid admitted visual projection
or transfer canonical hit testing to it.

---

## SECTION N — Authoritative Statements

**AUTH-150-1:** “The compositor paints truth; it does not create it.”
**AUTH-150-2:** “If it moves, someone authorized it.”
**AUTH-150-3:** “Visual correctness without logical correctness is a lie.”
## 2026-11-18 scoped HAL-395 Bloom composition authority acknowledgment [S1 Binding]

An explicitly authored HAL-395 Bloom is an optical projection of an original checked material in its existing compositor SurfaceId and HAL-359 source position. HAL-150 owns no Bloom user intent, geometry, source selection, time, focus, capture, layout or input. The renderer executes only the accepted HAL-084 same-surface source/filter/SDR screen-composite graph after original material and before higher-Z content, retaining one original current target and terminal sink. An emitted halo can expand visual raster/damage coverage but not hit/scroll/accessibility geometry. No cross-surface read, neighboring physical light response, independent compositor overlay, arbitrary per-window HDR scanout or new semantic authority is admitted. Existing compositor guarantees and full original source/clip/provenance checks remain unchanged.

