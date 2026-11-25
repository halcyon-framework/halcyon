# SPEC-HAL-357: Shared 3D Spatial-Domain Composition

## Pre-Flight

- SPEC: SPEC-HAL-000 §11, SPEC-HAL-002, SPEC-HAL-002A, SPEC-HAL-024,
  SPEC-HAL-309, SPEC-HAL-343, SPEC-HAL-354, SPEC-HAL-355, and SPEC-HAL-356
  read before writing this task: YES
- Policy: N/A — checked immutable 3D render-description composition and
  mechanical coordinate conversion
- Interaction added: NO — this SPEC adds committed-association and ray
  conversion mechanics, but no gesture, selection, hit-resolution, or command
  policy
- Forbidden behaviors checked: FORBID-357-SECOND-RENDERER,
  FORBID-357-RETAINED-HIERARCHY, FORBID-357-SEMANTIC-AUTHORITY,
  FORBID-357-INFERRED-RELATION,
  FORBID-357-IMPLICIT-ROOT, FORBID-357-INADMISSIBLE-EMBEDDING,
  FORBID-357-ORDINAL-IDENTITY, FORBID-357-CONSUMER-FLATTENING,
  FORBID-357-PARTIAL-DOMAIN-STATE, FORBID-357-PARTIAL-FRONT,
  FORBID-357-UNCOMMITTED-ASSOCIATION, FORBID-357-PICKING-CLAIM,
  FORBID-357-CAMERA-COUPLING, FORBID-357-UNBOUNDED-METADATA,
  FORBID-357-2D-CONTAMINATION, and FORBID-357-SCOPE-SMUGGLING
- Existing infrastructure: `Context3D`; immutable `RenderSnapshot` and complete
  `ModelState`; checked `ModelPlacement3`, `Orientation3`, positive scale, and
  world/local ray math; `Viewport3D` requested/committed tuples and last-good
  publication; target-local committed rays; retained invalidation; typed resource
  leases; reverse-Z depth; existing 2D SceneGraph isolation
- Reinvention risk: Extends the existing `Viewport3D` render-description boundary
  with one immutable one-level composition and deterministic lowering. It does
  not create another renderer, retained scene graph, ECS, document model,
  selection system, input router, scheduler, texture bridge, label system, or
  semantic coordinate framework.

**Author:** JC
**Status:** Adopted / Binding
**Date:** 2026-08-28
**Layer:** Public 3D render-description composition / checked local-domain math /
committed interaction association
**Owner:** 3D Substrate Architecture
**Depends on:** SPEC-HAL-002, SPEC-HAL-002A, SPEC-HAL-024, SPEC-HAL-343,
SPEC-HAL-354, SPEC-HAL-355, SPEC-HAL-356
**Extends without amending:** SPEC-HAL-355 flat immutable snapshot/model input with an
optional checked one-level domain-composition producer
**Implementation status:** Not implemented by this adoption; conformance requires
live source and executed §11 proof

Under SPEC-HAL-309, this document carries binding authority. It authorizes the
bounded public substrate and implementation described here, extends binding
SPEC-HAL-355 without changing its flat renderer contract, and preserves
SPEC-HAL-356 camera-only ownership. Adoption does not establish implemented or
conforming truth without live source and executed §11 proof.

---

## 1. Problem and Authority Decision

SPEC-HAL-355 correctly presents the renderer with one flat immutable structural
snapshot and one complete flat model state. A consumer that owns several local
2D or 3D coordinate domains must currently perform every local-to-world
composition itself and retain a second parallel identity/placement record if it
later needs to interpret a committed pointer ray in the same local domain.

That duplication is not semantic consumer work. The multiplication of checked
placements, deterministic packet lowering, association with the exact committed
frame, and inverse world/domain conversion are reusable 3D mechanics. Leaving
them to each consumer creates two avoidable failure classes:

1. the rendered flat placement and the consumer's retained domain placement can
   disagree; and
2. a consumer can accidentally interpret a committed ray through requested or
   stale domain state.

The authority decision is:

```text
consumer-owned semantic/domain snapshots
    -> explicit one-level local-domain/content descriptors and immutable states
    -> Context3D checked admission
    -> deterministic Halcyon-owned lowering
       -> existing flat RenderSnapshot
       -> existing complete flat ModelState
    -> Viewport3D atomic requested/committed tuple
    -> existing SPEC-HAL-355 raster/publication path
```

The common carrier is one right-handed SPEC-HAL-355 world. Every local domain is
embedded into that world by one checked similarity transform: positive uniform
scale, proper rotation, and translation. Camera state changes only the view of
that world. It never changes a domain embedding, content placement, or
consumer-owned semantic value.

**INV-357-AUTHORITY:** Halcyon owns only explicit domain/content identity,
checked similarity algebra, immutable admission, deterministic flat lowering,
committed mechanical association, and pure coordinate conversion. The consumer
owns every mathematical meaning, axis label, unit, valid semantic region,
selection, command, and successor-state decision.

**INV-357-NO-SEMANTIC-RELATION:** `DomainComposition3` expresses mechanical
spatial co-presence only. Co-membership in one world, equal local coordinate
tuples, equal or identity embeddings, proximity, overlapping geometry, a shared
world position, or successful `A local -> world -> B local` conversion establishes
no semantic equality, mapping, dependency, directionality, inverse, shared value,
or update propagation. The public model contains no domain edge, relation
registry, semantic token, callback, or dependency traversal. A consumer that
owns a relation evaluates it explicitly and publishes derived successor content;
Halcyon never updates one domain because another changed.

**INV-357-ONE-LEVEL:** Composition has exactly one domain/content level:

```text
DomainComposition3
    -> LocalDomainId3
        -> DomainContentId3
            -> one or more existing DrawPacketDescriptor values
```

Domains do not contain domains. Content does not contain content. Draw packets
remain the renderer's flat structural unit.

**INV-357-FLAT-RENDERER:** The renderer, raster path, dynamic storage, and
publication state machine continue to consume exactly the SPEC-HAL-355 flat
`RenderSnapshot` and complete `ModelState`. No domain traversal, domain branch,
or hierarchy reaches GPU preparation or the 2D renderer.

---

## 2. Scope and Ownership

This SPEC requires that Halcyon own:

1. opaque non-semantic local-domain and content IDs;
2. the closed `Planar2` / `Spatial3` domain-kind distinction;
3. one checked domain similarity embedding;
4. one checked domain-local content placement per content identity;
5. immutable structural composition, independently admitted immutable domain
   state, and one complete immutable domain frame;
6. deterministic lowering to existing snapshot/model values;
7. requested/committed retention of exact domain metadata beside the lowered
   values;
8. pure world/domain point, vector, and ray conversion;
9. a committed Planar2 plane-intersection helper with explicit side access; and
10. revision, lifecycle, invalidation, budget, and proof rules for that boundary.

It imports without reinterpretation:

- SPEC-HAL-024 for retained UI/event/logical/physical coordinate ownership;
- SPEC-HAL-343 for the separate affine 2D spatial substrate and its external
  semantic-domain authority;
- SPEC-HAL-354 for update-owned immutable publication and proportional
  invalidation;
- SPEC-HAL-355 for world/local precision, geometry, appearance, flat snapshot,
  complete model state, resources, rasterization, target lifecycle, atomic
  publication, and last-good behavior; and
- SPEC-HAL-356 for camera-only navigation and EventRouter-owned input lifecycle.

| Concern | Sole owner | Proposed 357 role |
|---|---|---|
| Consumer mathematics, coordinate meaning, units, categories, dates, logarithms, valid semantic region | Consumer/domain authority | Receives no semantic value and performs no semantic mapping |
| Local-domain/content identity in one 3D render description | Halcyon checked composition | Retains opaque IDs and exact associations only |
| World/domain similarity math and inverse | Halcyon 3D substrate | Validates and converts explicit immutable values |
| Geometry, appearance, packet leases, raster and depth | SPEC-HAL-355 | Reuses existing admitted resources and flat packets |
| Camera pose/projection/navigation | SPEC-HAL-355/356 view and controller owners | Consumes an independent immutable view revision only |
| Input routing, focus, capture, gesture | EventRouter and consumer policy | Adds no input binding or interaction state |
| Selection, semantic hit, transform target, command | Consumer/application | May use an explicit committed association as evidence; Halcyon never resolves meaning |
| Cross-domain meaning, chart, relation, dependency, and semantic freshness | Consumer/application | Receives no edge or semantic revision; offers only independent domain/world conversion |
| Final 2D placement and composition | Existing retained SceneGraph/compositor | Continues to receive one ordinary Viewport3D Image plane |
| Frame cadence and invalidation | Existing Halcyon update/frame owners | Publishes immutable revisions; owns no clock or scheduler |

The local-domain spaces defined here are private 3D render-description spaces
inside the already admitted Viewport3D producer. They do not add UI layout,
paint, event, hit-test, physical, or NDC spaces to SPEC-HAL-024. `Planar2` is
not the SPEC-HAL-343 affine pixel-space viewport and does not replace it.

---

## 3. Closed Public Model

The conceptual public units are:

```text
LocalDomainId3
DomainContentId3
DomainStateRevision3
LocalDomainKind3::{Planar2, Spatial3}
DomainPoint2
DomainPoint3
DomainVector3
DomainEmbedding3
DomainContentPlacement3
LocalDomain3Descriptor
DomainContent3Descriptor
DomainComposition3Descriptor
DomainComposition3
DomainState3Descriptor
DomainState3
DomainFrame3Descriptor
DomainFrame3
DomainRetentionLimits3D
DomainRetentionUsage3D
DomainRetentionBudget3D
DomainRetentionError3D
CommittedDomain3
DomainRay3
PlanarRayAccess3::{FrontOnly, BackOnly, TwoSided}
DomainComposition3Error
DomainState3Error
DomainFrame3Error
DomainAssociation3Error
```

Exact Rust layout is implementation work, but these ownership units, closed
kinds, immutable boundaries, and error distinctions are normative under this
binding authority.

### 3.1 IDs and Identity Scope

`LocalDomainId3` and `DomainContentId3` are opaque value IDs. They expose
equality and hashing.
They expose no consumer noun, semantic key, pointer, callback, resource handle,
or packet ordinal.

IDs are supplied explicitly in a descriptor and scoped by the immutable
`DomainComposition3` identity:

- every domain ID is unique in one composition;
- every content ID is unique within its owning domain;
- the complete content identity is `(LocalDomainId3, DomainContentId3)`;
- the same ID may remain stable across structural successors only when the
  consumer deliberately declares continuity; and
- every committed association also carries the exact composition identity and
  revision, so an equal numeric/value ID cannot make an old association current.

No global ID registry, singleton allocator, semantic lookup table, or persistent
reference is introduced.

### 3.2 Domain Kinds

`LocalDomainKind3` is closed:

```text
Planar2
Spatial3
```

`Planar2` defines an ordered local plane:

- local `+X` is the first in-plane basis direction;
- local `+Y` is the second in-plane basis direction;
- local `+Z = +X cross +Y` is its declared front normal; and
- a planar point `(u, v)` embeds mechanically as `(u, v, 0)` before the domain
  similarity transform.

`Spatial3` exposes the full ordered local `+X`, `+Y`, `+Z` basis.

The domain kind declares available mechanical coordinates. It does not declare
what an axis means, require axes/grid/background geometry, constrain content to
one artistic representation, or create a separate render mode. Both kinds use
the same SPEC-HAL-355 geometry, appearance, depth, target, and publication path.

Planar2 packet geometry and content placement remain mechanically 3D so a
consumer may render finite thickness, a normal indicator, or other geometry
attached to the plane. Only the Planar2 point mapping itself is constrained to
local `z = 0`; Halcyon does not reinterpret off-plane geometry as a third
semantic coordinate.

`Planar2` is exactly this canonical affine plane. An abstract two-dimensional
manifold, a nonlinear chart, or a chart painted on a curved surface does not
thereby become a Halcyon `Planar2`. A consumer maps such values itself and submits
the resulting ordinary geometry in `Spatial3` unless separately accepted
cross-consumer evidence establishes reusable substrate mechanics.

The f64 mechanical coordinate roles are closed and named:

```text
DomainPoint2 { u: finite f64, v: finite f64 }
DomainPoint3 { x: finite f64, y: finite f64, z: finite f64 }
DomainVector3 { x: finite f64, y: finite f64, z: finite f64 }
```

`DomainPoint2` is a point on canonical `Planar2` and embeds as `(u, v, 0)`.
`DomainPoint3` is the full mechanical coordinate used by `Spatial3`, by
domain-local content translation, and by inverse `Planar2` point decomposition;
for the last role, `z` is the signed offset along declared local `+Z`, not a
third semantic axis. `DomainVector3` is a free vector and never receives point
translation. These types are distinct from SPEC-HAL-355 f32 mesh-local
`LocalPoint3`/`LocalVector3` and existing f64 world roles. They expose no generic
scalar family or third-party math type.

### 3.3 Immutable Structural Composition

`DomainComposition3Descriptor` contains:

```text
one existing non-wrapping StructuralRevision
one nonempty ordered array of local-domain descriptors
```

Each local-domain descriptor contains:

```text
LocalDomainId3
LocalDomainKind3
one nonempty ordered array of content descriptors
```

Each content descriptor contains:

```text
DomainContentId3
one nonempty ordered array of existing DrawPacketDescriptor values
```

Admission validates all IDs, packets, resource generations, context identity,
ordinals, checked count products, and existing SPEC-HAL-355 budgets before it
creates `DomainComposition3`.

Lowered packet order is exactly:

```text
domain descriptor order
    -> content descriptor order within that domain
        -> packet descriptor order within that content
```

The existing packet stable ordinal is preserved verbatim and remains only a
render-order identity. It never names a domain or content.

Let `D`, `C`, and `P` be admitted domain, content, and packet counts. The closed
nonempty model guarantees:

```text
1 <= D <= C <= P <= SPEC-HAL-355 effective draw-packet budget
```

The same packet budget therefore bounds domain/content metadata. An
implementation may expose stricter finite policy limits, but it cannot expose an
unbounded mode.

`DomainComposition3` is immutable and owns or shares the exact underlying
`RenderSnapshot`, resource leases, normalized identity tables, and packet ranges
for its domains and contents. Equal structural revision requires identical
composition identity and content. Membership, domain kind, domain/content order,
packet membership/order, resource generation, raster class, or stable ordinal
change requires a new structural revision and immutable composition.

### 3.3A Context-Local Aggregate Retention Accounting

The per-value packet bound does not by itself bound the number of simultaneously
retained immutable domain values. If adopted, each `Context3D` effective
configuration therefore adds finite, queryable limits for at least:

```text
live unique DomainComposition3 identities
live unique DomainState3 identities
live unique DomainFrame3 identities
aggregate retained domain-metadata bytes
```

The same context exposes current usage and high-water usage for those four
classes. Counts and byte products use checked arithmetic. Admission computes and
reserves the complete increase transactionally before publishing an immutable
value; exhaustion returns a distinguishable policy-budget error from the
corresponding composition/state/frame admission and creates no partial value,
counter change, lowered model, or viewport request.

This aggregate boundary is additive and domain-specific:

```text
DomainRetentionLimits3D
DomainRetentionUsage3D
DomainRetentionBudget3D
DomainRetentionError3D

Context3D::new_with_domain_retention_limits(...)
Context3D::with_policy_and_domain_retention_limits(...)
Context3D::domain_retention_limits(...)
Context3D::domain_retention_usage(...)
```

`DomainRetentionLimits3D` is constructed only through a checked finite-positive
constructor or its conservative default; its fields are not added to the
pre-existing exhaustive `PolicyLimits3D`. Domain exhaustion and accounting
failure use `DomainRetentionBudget3D` / `DomainRetentionError3D`, not new variants
of the pre-existing exhaustive `PolicyBudget3D` / `Context3DError`.
`Context3D::new` and `Context3D::with_policy` select conservative domain-retention
limits while retaining their exact prior signatures and error surfaces. The
existing `EffectiveLimits3D` and `Context3D::limits` result remain unchanged;
domain-retention limits and usage are queried only through the additive methods
above.

Accounting is by unique context-admitted immutable identity. Clones and
requested, committed, compatible last-good, or in-flight owners share one
context-local retention lease and do not double-count the underlying value. A
frame counts its own association/index storage; it references rather than
recounts its admitted composition and states. Usage is released only when the
last live context-local lease for that identity ends. A compatible last-good
front therefore keeps its exact domain metadata charged until replacement or
withdrawal releases that ownership. No revision history or removed-value cache
is retained, and no global registry or cross-`Context3D` governor is introduced.

### 3.4 Independently Admitted Immutable Domain State

`DomainState3Descriptor` is admitted against one exact
`DomainComposition3` and one exact `LocalDomainId3`. It contains:

```text
one non-wrapping DomainStateRevision3 scoped to that domain and composition
one DomainEmbedding3
one complete set of (DomainContentId3, DomainContentPlacement3)
```

The content set must contain every content ID in that domain exactly once and no
other ID. Input order is not identity; admission normalizes by the structural
composition's content order.

`DomainState3` is an independently admitted immutable value. A failed candidate
admission changes no existing domain state, sibling state, resource lease,
consumer value, or viewport request. The consumer may explicitly retain and use
the prior valid state for that domain while advancing valid sibling states only
when its own semantic relation contract permits that combination. This is not a
hidden fallback: the complete next frame names every state it uses.

Within one exact composition identity, an equal domain-state revision requires
the same immutable state identity/content. A lower revision cannot replace a
higher requested state for that domain. A structural successor creates a new
revision scope; committed composition identity still prevents cross-structure
confusion.

### 3.5 Complete Atomic Domain Frame

`DomainFrame3Descriptor` is admitted against one exact
`DomainComposition3`. It contains:

```text
one existing non-wrapping ModelRevision
one complete set of DomainState3 values
```

The set contains exactly one compatible state for every domain in the
composition and no extra state. Frame admission rejects duplicate, missing,
foreign, or structurally incompatible values before constructing a frame;
Viewport3D successor admission rejects stale or illegally reused domain/frame
revisions against its current requested state.

`DomainFrame3` is immutable. It owns or shares:

- the complete per-domain states and their revisions;
- the deterministic domain/content association table; and
- the exact complete flat `ModelState` produced by §4.

The frame is the atomic mechanical world-presentation input. There is no
partial-domain patch, implicit merge, in-place placement mutation, or per-domain
GPU front. Shared depth and one published Image require one mechanically complete
frame. This does not claim semantic coherence, a cross-domain transaction, a
dependency graph, or a multi-view lifecycle contract. The consumer alone decides
whether a combination of independently admitted domain states is semantically
valid; if its own relation contract forbids a mixture, it retains or withdraws
the appropriate whole semantic presentation rather than asking Halcyon to infer
or propagate sibling state. Candidate failure preserves the preceding compatible
last-good committed mechanical frame under SPEC-HAL-355; it never publishes a
partially admitted mechanical mixture.

### 3.6 Explicit Global-World Content

There is no implicit root or privileged world domain. Content authored directly
in global world coordinates uses an explicit `Spatial3` domain whose embedding
is identity. It participates in the same IDs, revisions, lowering, lifecycle,
and proof as every other domain.

---

## 4. Admitted Transform Algebra

### 4.1 Domain Embedding

`DomainEmbedding3` contains exactly:

```text
origin_world: finite f64 WorldPoint3
orientation_world_from_domain: checked proper Orientation3
uniform_scale_world_per_domain: finite positive f32
```

For a domain-space point `x_d` represented in f64 arithmetic:

```text
x_world = origin_world + R_d * (s_d * x_d)
```

For a world point:

```text
x_domain = R_d^-1 * (x_world - origin_world) / s_d
```

Here `x_d` is `DomainPoint3`; a canonical `Planar2` point first expands its
`DomainPoint2` to `(u, v, 0)`. Vectors use `DomainVector3` and omit translation:

```text
v_world = R_d * (s_d * v_domain)
v_domain = R_d^-1 * v_world / s_d
```

Orientations are proper rotations, `s_d > 0`, and all forward/inverse results
must be finite. Domain scale is uniform, preserving angles, aspect ratio,
handedness, and the Planar2 normal.

### 4.2 Content Placement and Flat Lowering

`DomainContentPlacement3` contains exactly:

```text
translation_domain: DomainPoint3
orientation_domain_from_content: checked proper Orientation3
scale_content: existing finite positive PositiveScale3
```

For content-local mesh point `x_c`:

```text
x_domain = translation_domain + R_c * diag(scale_content) * x_c
```

The final SPEC-HAL-355 `ModelPlacement3` repeated for every packet of that
content is:

```text
translation_world = origin_world + R_d * (s_d * translation_domain)
orientation_world_from_content = q_d * q_c
scale_world = s_d * scale_content
```

Quaternion multiplication uses the same checked Hamilton composition semantics
as SPEC-HAL-355. Uniform scale multiplication is checked component-wise before
constructing the existing positive scale. Translation composition remains f64;
orientation and scale remain checked f32. Admission rejects any overflow,
non-finite result, non-representable required conversion, non-positive scale, or
failed normalized orientation without partial output.

Because both rotations are proper and every scale component is positive:

```text
det(world_from_content.linear)
    = s_d^3 * scale_content.x * scale_content.y * scale_content.z
    > 0
```

The lowering cannot introduce reflection or reverse declared mesh winding.

### 4.3 Deliberately Excluded Algebra

The admitted transform is a similarity at the domain/world boundary. It has no:

- negative or zero scale;
- anisotropic domain scale;
- reflection;
- shear;
- caller-supplied basis matrix;
- general affine or projective transform;
- nested parent transform; or
- nonlinear/curved chart map.

Content retains the existing positive possibly non-uniform local scale because
SPEC-HAL-355 already owns that final mesh transform. This does not widen the
domain embedding beyond similarity.

---

## 5. Pure Coordinate and Committed-Ray Boundary

### 5.1 Pure Conversion

The public boundary supplies allocation-free checked conversion for:

- `DomainPoint2` on Planar2 local `z = 0` to world;
- world point to Planar2 `DomainPoint3 { x: u, y: v, z:
  signed_normal_offset }` without
  projecting or inventing an on-plane tolerance;
- Spatial3 `DomainPoint3` to/from world;
- Planar2 and Spatial3 `DomainVector3` to/from world; and
- normalized world ray to normalized domain-local ray with the exact positive
  world-units-per-local-parameter factor.

For `Planar2`, a vector's `DomainVector3::z` is exactly its signed component
along declared local `+Z`. Both conversion directions preserve that component;
world-to-domain conversion never projects it away, and domain-to-world
conversion never silently forces it to zero. A consumer that requires an
in-plane tangent vector must supply or admit `z = 0` explicitly.

The inverse uses the exact admitted `DomainEmbedding3`. It does not query
geometry, render resources, a camera controller, or consumer state.

Composing the two independent checked conversions may mechanically map
`A local -> world -> B local`; the result remains coordinate arithmetic only and
creates none of the semantic relations forbidden by `INV-357-NO-SEMANTIC-RELATION`.
No direct cross-domain mapping helper or registry is added.

### 5.2 Committed Domain Association

The domain-aware Viewport3D request retains the exact
`DomainComposition3` and `DomainFrame3` metadata beside their lowered
snapshot/model values in requested and committed tuples. Existing flat requests
retain `None` for this metadata and remain unchanged.

For an active target in a compatible `Committed` or `FailedWithLastGood`
lifecycle, the target status may return one `CommittedDomain3` value containing
at least:

```text
target compatibility key
committed request serial
committed view revision
committed composition identity and structural revision
committed frame identity and model revision
LocalDomainId3 and LocalDomainKind3
committed DomainStateRevision3 and DomainEmbedding3
```

A committed domain ray is constructed in one mechanical chain:

```text
committed target extent + committed ViewDescriptor3
    -> existing committed world Ray3
    -> exact committed DomainEmbedding3 inverse
    -> DomainRay3 plus the complete CommittedDomain3 association
```

An active compatible `FailedWithLastGood` returns the old committed association
and ray exactly as binding SPEC-HAL-355 §11.4 requires; the failed requested tuple
remains distinct and is never reported as committed. Missing/inactive target, a
lifecycle without a compatible committed front, flat request, absent domain ID,
invalid normalized viewport coordinate, or arithmetic failure returns `None` or
the exact typed error. No requested domain state, caller-parallel placement, or
fabricated fallback is used.

An association is current only while every key/revision above still equals the
target's current committed tuple. A newer target, view, composition, frame, or
domain-state commit makes the prior association stale. A consumer must validate
that equality immediately before publishing a semantic action derived from the
association. Pure historical arithmetic may remain reproducible, but it is not
current interaction authority.

Mechanical currentness is necessary and never sufficient for semantic
publication. The consumer must externally pair the submitted frame with its own
exact immutable semantic/representation association, including whatever
definition, value, parameters, chart, valid-region, and revision its accepted
contract requires, and validate that association immediately before publishing
an action. Halcyon neither accepts nor stores that semantic revision. A
semantic-only consumer change may leave every Halcyon value and placement
byte-identical; it still makes the consumer's older semantic association
non-current. Likewise, the consumer-owned record paired with a mechanically
compatible last-good front remains associated with its older consumer snapshot
and cannot be relabeled current for newer mathematics merely because
`CommittedDomain3` remains mechanically valid.

`Viewport3DTargetStatus::is_current_domain_association` performs that exact
value comparison against the current committed tuple. It returns false when no
active compatible committed front exists or any target key/revision changed, and
has no mutation or semantic side effect. A `Committed` to compatible
`FailedWithLastGood` transition alone does not make the old committed association
stale because the committed tuple remains unchanged.

### 5.3 Planar2 Side Access

`DomainRay3` for `Planar2` may intersect the domain's canonical `z = 0` plane
only under an explicit caller-supplied `PlanarRayAccess3`:

```text
FrontOnly  -> admit only direction dot +Z < 0
BackOnly   -> admit only direction dot +Z > 0
TwoSided   -> admit either non-parallel direction
```

The result reports finite local `(u, v)`, whether the approached side was front
or back, nonnegative ray parameter, and corresponding world distance. A parallel
ray or disallowed side is a mechanical miss. Halcyon does not infer side policy
from culling, visibility, camera location, material, or consumer meaning.

This helper is not generic picking. For `Spatial3`, and for geometry on a
Planar2 domain, the consumer performs its own analytic or explicit-geometry hit
against the returned local ray and then applies its own semantic mapping.

---

## 6. Viewport and Compatibility Boundary

The public integration is additive:

```text
DomainRetentionLimits3D::try_new(...)
Context3D::new_with_domain_retention_limits(...)
Context3D::with_policy_and_domain_retention_limits(...)
Context3D::domain_retention_limits(...)
Context3D::domain_retention_usage(...)
Context3D::admit_domain_composition(...)
Context3D::admit_domain_state(...)
Context3D::admit_domain_frame(...)

Viewport3D::new_domain_composition(...)
Viewport3D::set_domain_composition_request(...)
Viewport3DTargetStatus::committed_domain(...)
Viewport3DTargetStatus::committed_domain_ray(...)
Viewport3DTargetStatus::is_current_domain_association(...)
```

The domain-aware constructor/request accepts the same view revision,
`ViewDescriptor3`, sample count, and target lifecycle as the flat path. It
extracts the exact lowered `RenderSnapshot`/`ModelState`, validates their common
composition identity, and enters the existing atomic request path. A caller
cannot pair metadata from one composition/frame with flat values from another.

Existing `Viewport3D::new`, `Viewport3D::set_request`, `RenderSnapshot`,
`ModelState`, `PolicyLimits3D`, `PolicyBudget3D`, `Context3DError`,
`EffectiveLimits3D`, and every existing consumer remain source-compatible. In
particular, the pre-domain exhaustive policy struct and enum shapes acquire no
domain field or variant. The flat API does not synthesize a fake identity domain
and cannot answer domain-association queries.

Compatibility is implemented at the `Viewport3D` participant owner with a closed
flat-versus-domain-aware request-kind discriminator. Existing flat constructor
and request calls enter the flat branch directly and retain no composition,
state, frame, association table, or domain-budget lease. Domain-aware calls
finish admission and lowering before the unchanged SPEC-HAL-355 flat
snapshot/model renderer boundary. A flat Viewport3D may therefore perform one
bounded O(1) request-kind/metadata-absence discrimination in its participant
state; this SPEC does not falsely claim zero CPU instructions there. It requires
zero domain admission, lowering, table allocation, traversal, association lookup,
budget acquisition, resource work, or GPU branch for a flat request, and the
renderer still sees no domain kind at all. Ordinary retained 2D, SPEC-HAL-343
affine spatial, and zero-3D paths remain structurally branch-free and work-free
with respect to this capability under SPEC-HAL-355.

Once an adopted implementation offers this boundary, a consumer using local
domains must use the domain-aware request rather than flattening the same domain
state independently and retaining a parallel interaction map. Legacy consumers
with genuinely flat content need not migrate.

---

## 7. Revisions, Invalidation, and Work

The revision mapping is exact:

| Change | Required revision/domain | Existing work consequence |
|---|---|---|
| Domain membership, kind, order, content membership/order, packet membership/order, resource generation, raster class, ordinal | `StructuralRevision` / new `DomainComposition3` | Existing structural admission/preparation |
| One domain embedding or any placement in that domain | that `DomainStateRevision3` and full frame `ModelRevision` | Existing model-only path |
| Another domain unchanged | same immutable `DomainState3` identity/revision | No domain-state or lowering work for that domain; Halcyon makes no semantic-work claim |
| Camera pose/projection only | existing view revision | No composition/domain/frame change |
| Target extent/format/sample/activity | existing target key/lifecycle | No fake structural/model/view revision |
| No effective change | no revision change | Zero write/encode/submit/publication under SPEC-HAL-355 |

Domain-frame lowering may compare final placements by structural packet range.
With stable view, target, render origin, and warmed capacity, a one-domain
placement change writes only that domain's changed object blocks. It reuses the
same structural snapshot, resource leases, geometry/appearance materialization,
pipelines, buffers, bind groups, and unaffected placements. A necessary
render-origin rebase remains governed by SPEC-HAL-355 and may require wider
object-block writes without changing domain authority.

Camera-only changes leave every composition, domain-state, frame, and consumer
semantic value byte-identical/identity-identical. Domain placement changes leave
camera state and view revision unchanged.

No domain value owns a Timeline, clock, worker, frame request, or invalidation
channel. A retained owner publishes a complete immutable successor during
existing event/update authority and requests the narrowest existing invalidation.

---

## 8. Lifecycle, Failure, and Independent Domains

Admission is transactional at every layer:

1. a failed composition candidate creates no new admitted composition;
2. a failed state candidate changes no existing state or sibling;
3. a failed frame candidate changes no viewport request; and
4. a failed viewport preparation preserves the compatible last-good complete
   frame/front under SPEC-HAL-355.

Typed admission/conversion errors distinguish at least identity
duplication/absence, wrong domain kind, foreign composition/context,
structural/state/frame mismatch, stale or reused successor revision,
invalid/non-finite embedding or placement, unrepresentable lowering, and exact
domain-retention budget exhaustion. As §5.2 specifies, a flat/inactive target,
missing domain, or lifecycle without a compatible committed front returns
`None`; committed-association staleness is the non-fallible result
`is_current_domain_association(...) == false`. Neither condition fabricates a
typed error for an operation that did not fail conversion or admission.
Consumer-controlled invalid input does not panic or silently substitute another
domain, state, transform, or hit.

One common-world front is always mechanically atomic. This SPEC deliberately
rejects a partially committed front containing an arbitrary mixture of old and
new domain states. It does not declare those sibling states semantically
coherent. Independent progress is expressed before publication only when the
consumer's own accepted relation contract permits it: after one domain candidate
fails, the consumer may explicitly assemble a successor frame from that domain's
prior admitted state and new admitted sibling states, or retain/withdraw the
whole semantic presentation when such a mixture is forbidden.

Domain suspension/removal is explicit structural absence, not a hidden visibility
bit or zero-scale trick. The consumer publishes a new `DomainComposition3` that
omits that domain and a matching complete frame for the remaining domains.
Unchanged sibling geometry/resource generations remain reusable. Reintroducing a
domain occurs through another explicit structural composition and cannot revive
a stale committed association because composition identity changes.

The final domain cannot be replaced by an empty composition. Removing the final
domain withdraws or unmounts the domain-aware viewport through the existing
target/widget lifecycle; no empty packet set or invisible placeholder is
fabricated.

Dropping a composition/state/frame releases only its immutable CPU ownership.
Geometry, appearance, target, dynamic-storage, and front leases retire exactly
under SPEC-HAL-355 after requested, committed, last-good, and in-flight ownership
ends. No domain registry, revision history, or removed-state cache may grow
without a finite live owner.

Target inactivity, zero extent, incompatible resize, unmount, destruction, and
terminal device failure retain their existing SPEC-HAL-355 behavior. Domain
metadata neither keeps a withdrawn front sampleable nor creates a private replay
or recovery epoch.

---

## 9. Geometry-Native First Vertical

The first implementation vertical is geometry-native
composition through existing SPEC-HAL-355 packets only. Its reference fixture
contains at least:

- one explicitly placed `Planar2` domain with opaque triangle geometry; and
- one independently placed `Spatial3` domain with opaque closed or volumetric
  triangle geometry.

Both render inside one Viewport3D world and one depth field. The fixture moves or
rotates one domain through a model-only successor while the other domain,
camera, structural resources, and consumer test state remain unchanged. It uses
opaque non-semantic IDs and no Locus type or mathematical noun.

Axes, grids, vectors, points, projections, plane surfaces, and similar content
are optional ordinary geometry supplied by a consumer. The substrate creates
none automatically. A plane is not a compulsory background, and a Planar2
domain can exist without visible plane geometry.

---

## 10. Forbidden Behaviors

**FORBID-357-SECOND-RENDERER:** A domain renderer, domain render pass, alternate
depth system, domain target, or compositor path beside SPEC-HAL-355.

**FORBID-357-RETAINED-HIERARCHY:** Nested domains, nested content, parent/child
mutation, retained scene graph, ECS, generic transform graph, or hierarchy
traversal in renderer preparation.

**FORBID-357-SEMANTIC-AUTHORITY:** Domain/axis names, dates, logarithms,
categories, units, mathematical values, chart maps, valid semantic regions,
selection, document state, or commands inside Halcyon domain-composition types.

**FORBID-357-INFERRED-RELATION:** A domain edge, relation registry, semantic
token, callback, dependency traversal, sibling update, or inferred equality/
mapping from ID equality, coordinate equality, identity embedding, proximity,
overlap, shared world position, or domain/world/domain conversion.

**FORBID-357-IMPLICIT-ROOT:** Content outside an explicit domain, a privileged
unnamed world root, or inference that identity embedding means semantic root.

**FORBID-357-INADMISSIBLE-EMBEDDING:** Negative/zero/anisotropic domain scale,
reflection, shear, arbitrary basis matrix, affine/projective transform, nested
transform, nonlinear chart, or silent repair of invalid math.

**FORBID-357-ORDINAL-IDENTITY:** Treating packet order, stable ordinal, geometry,
appearance, raster result, or visual equivalence as domain/content identity or a
transform target.

**FORBID-357-CONSUMER-FLATTENING:** After adoption/implementation, a domain-aware
consumer recomputing the same lowering and keeping an independent placement map
instead of using the checked domain request. Genuinely flat legacy consumers are
not domain-aware and remain valid.

**FORBID-357-PARTIAL-DOMAIN-STATE:** Missing/extra content placement, implicit
merge, placement patch, in-place domain mutation, or a frame missing one admitted
domain state.

**FORBID-357-PARTIAL-FRONT:** Publishing sibling changes while one domain in that
same candidate frame is invalid, mixing domain revisions after admission, or
maintaining one hidden GPU front per domain.

**FORBID-357-UNCOMMITTED-ASSOCIATION:** Building an interaction mapping from a
requested view, requested frame, consumer-parallel placement, stale target,
inactive target, or association superseded by a later commit.

**FORBID-357-PICKING-CLAIM:** Calling domain-ray conversion, plane intersection,
or consumer analytic geometry rendered-visible picking, selection, scene
traversal, or transform-target resolution.

**FORBID-357-CAMERA-COUPLING:** Camera navigation mutating a domain/content
placement, a domain following a camera implicitly, or a camera controller owning
domain IDs, semantic targets, or placement revisions.

**FORBID-357-UNBOUNDED-METADATA:** Unbounded retained composition/state/frame
identities or metadata bytes, unchecked aggregate products, clone-counted
duplicates, a leaked last-good retention charge, revision history, or reliance
on the per-value packet bound as the complete aggregate `Context3D` policy.

**FORBID-357-2D-CONTAMINATION:** Domain types, traversal, resources, state, or
branches entering the retained 2D SceneGraph, Batcher, DrawPlan, RenderGraph hot
path, SPEC-HAL-343 spatial API, or ordinary application with no domain-aware
Viewport3D.

**FORBID-357-SCOPE-SMUGGLING:** Adding a live retained-2D texture bridge, 3D
labels/billboards, generic picking, fractional translucency, nested/affine
domains, semantic-axis framework, selection/gizmo policy, or Locus-specific
type as a helper for this vertical.

---

## 11. Conformance Proof Obligations

Adoption defines authority only. Implementation conformance requires executable
proof for every applicable item below.

### 11.1 Identity and Admission

1. Duplicate domain IDs, duplicate content IDs within a domain, empty domains,
   empty content, missing/extra/foreign state, context mismatch, packet-budget
   overflow, stale/reused revision, and structural mismatch fail transactionally.
2. Domain and content IDs remain independent from packet ordinal, geometry,
   appearance, and input target policy.
3. One content with multiple packets lowers contiguously and receives the same
   exact final placement for every packet.
4. Equal revisions require equal immutable identity/content; lower revisions
   cannot move requested state backward.
5. At each configured aggregate composition/state/frame/metadata limit, the
   final admitted value succeeds and the next unique value fails transactionally
   with its exact `DomainRetentionBudget3D` category; invalid zero limits fail in
   `DomainRetentionLimits3D::try_new`; clones do not increase usage,
   requested/committed/last-good/in-flight retention keeps the exact charge, the
   final lease releases it, and current/high-water queries report the transition.
6. Two co-located identity-embedded domains with equal local tuples remain
   distinct, and changing either causes no automatic sibling update, semantic
   equality, mapping, or dependency traversal.

### 11.2 Math

7. Identity, translated, rotated, and scaled Planar2 and Spatial3 embeddings
   prove exact anchor cases and forward/inverse point/vector round trips within
   declared f64/f32 tolerance; Planar2 world conversion preserves explicit
   signed normal offset and never silently projects an off-plane point.
8. Composition proves `q_d * q_c`, uniform-times-content scale, f64 translation,
   positive determinant, inverse-transpose compatibility, and unchanged CCW
   winding.
9. Non-finite values, zero/negative scale, overflow, and every excluded transform
   representation are impossible to construct or fail with typed results.
10. Planar2 proves ordered `+X/+Y`, derived `+Z` normal, aspect-preserving uniform
    scale, exact signed-normal preservation for `DomainVector3` in both
    directions, and explicit FrontOnly/BackOnly/TwoSided local-plane ray behavior.
11. Spatial3 committed-ray conversion preserves world distance/parameter scale and
   supports consumer-owned analytic hits without semantic code in Halcyon.

### 11.3 Lowering, Rendering, and Invalidation

12. At least two independently placed domains lower in deterministic
    domain/content/packet order and render with correct shared reverse-Z depth.
13. Camera-only changes leave composition, domain states, frame, and a sentinel
    consumer source snapshot unchanged.
14. One-domain placement change advances only that domain state and the complete
    model frame, preserves structure/resources/siblings, and under stable render
    origin writes only affected object blocks after warm-up.
15. Structural domain removal leaves sibling IDs, resource generations, and
    placements unchanged while retiring only no-longer-leased state.
16. Equal composition/frame/view/target input performs zero domain lowering work,
    dynamic write, encode, submission, swap, or publication.

### 11.4 Committed Association and Failure

17. Domain-ray construction succeeds only from an active compatible `Committed`
    or `FailedWithLastGood` target and carries exact committed target/view/
    composition/frame/domain revisions.
18. After a failed successor request, the compatible last-good path exposes the
    old committed domain association and ray, keeps requested and committed
    revisions distinct, and never substitutes failed requested metadata. A later
    placement or structural commit makes that preceding association stale;
    `is_current_domain_association` returns false and current semantic publication
    through that old association is rejected.
19. Changing only an external consumer semantic/representation revision leaves
    Halcyon domain state unchanged but cannot make the older consumer association
    current; the consumer keeps the mechanically last-good frame paired with its
    exact old external association and never presents it as current for newer
    source state.
20. Flat Viewport3D requests, inactive/suspended targets, missing IDs, invalid
    coordinates, and arithmetic failure never fabricate domain association.
21. A failed domain-state candidate leaves every prior/sibling state unchanged;
    an explicit frame using the prior valid state plus a new sibling state can
    commit normally only when the consumer's semantic relation contract admits
    that combination.
22. A failed complete candidate preserves one compatible last-good whole-world
    frame/front and never exposes a partial domain mixture.
23. Target resize, inactivity, unmount, and terminal failure keep domain metadata,
    front truth, and lease retirement aligned with SPEC-HAL-355.

### 11.5 Isolation

24. Compiled-but-unused domain composition adds no branch or work to ordinary 2D,
    SPEC-HAL-343 affine spatial, or zero-3D paths. A flat Viewport3D performs at
    most the declared O(1) owner-side request-kind/metadata-absence discrimination
    and performs zero domain admission, lowering, allocation, traversal,
    association lookup, budget acquisition, resource work, or GPU branching;
    its flat output and renderer path remain unchanged.
25. The geometry-native reference fixture contains no semantic axis framework,
    Locus type, generic picking, retained 2D texture, 3D label, fractional
    translucency, nested domain, or affine-domain escape.
26. Public API-shape proof contains no domain relation/edge, semantic token,
    callback, chart, mapping, propagation, or dependency traversal; independent
    `domain -> world` and `world -> domain` conversion is the only admitted
    cross-domain arithmetic composition.
27. A black-box compile-time compatibility guard constructs the exact pre-domain
    exhaustive `PolicyLimits3D` literal and exhaustively matches every
    `PolicyBudget3D` and `Context3DError` variant. Domain retention configuration,
    usage, exhaustion, and accounting failure are reachable only through the new
    domain-specific types and cannot widen those legacy exhaustive surfaces.

CPU-only math cannot substitute for the actual Viewport3D/shared-depth and
committed-target proof where this section requires the production path.

---

## 12. Explicit Exclusions and Separate Authority Units

The following are deliberately outside this SPEC and require independently
accepted authority before implementation:

| Excluded capability | Why separate |
|---|---|
| Live retained-2D texture bridge into a domain | Requires cross-renderer texture production, color/alpha, target sizing, synchronization, update, failure, and lifecycle authority |
| 3D labels, billboards, world-to-screen text | Requires text measurement, occlusion, facing, scaling, overlap, accessibility, and retained overlay policy |
| Generic picking or selection | Requires geometry visibility semantics, acceleration/readback, IDs, hit ordering, session state, and command policy beyond a mechanical domain ray |
| Fractional translucent objects or planes | SPEC-HAL-355 rejects it; correct depth/order/OIT, alpha, budgets, and backend proof need their own rendering authority |
| Nested domains or transform hierarchy | Changes identity, update propagation, cycle prevention, flattening, budgets, and committed association |
| Anisotropic/general affine domain embedding | Changes normal/winding/singularity and Planar2 aspect semantics; no current consumer evidence requires it |
| Curved or nonlinear surface charts | Requires a chart map, inverse multiplicity, validity region, derivatives, tessellation, and failure semantics |
| Semantic axes/dates/log/categories framework | Meanings and mappings remain consumer-owned until concrete cross-consumer demand proves a shared contract |
| Cross-domain semantic relation, mapping, or dependency graph | Spatial co-presence and coordinate conversion provide no semantic edge; consumers own explicit typed relations and derived successor publication |
| Consumer semantic/representation revision payload | Mechanical committed association is intentionally insufficient; the consumer pairs and validates its own immutable association externally |
| Selection, gizmos, transform bindings, scene authoring | Input and semantic mutation policy are not render-description mechanics |

These exclusions delimit only SPEC-HAL-357's current authority. They are not
permanent prohibitions on future Halcyon capability: separately accepted,
owner-correct authority may introduce one of them, and existing mechanisms
already authorized by another binding SPEC remain unaffected. Until such
authority exists, this SPEC still forbids introducing, emulating, or claiming
the excluded capability through this domain-composition boundary. This
clarification does not weaken any present exclusion.

---

## 13. Constitutional and Authority Consistency

| Authority | Result |
|---|---|
| SPEC-HAL-002 Pillar A | Preserved: immutable values contain IDs and data, never Widgets or cross-tree references |
| SPEC-HAL-002 Pillar B | Preserved: event/update owner publishes complete values; layout/paint/preparation consume without semantic mutation |
| SPEC-HAL-002 Pillar C | Strengthened: domain/content association is explicit ID-based, never packet/reference inference |
| SPEC-HAL-002 Pillar D | Preserved: pure checked descriptors in, immutable values/errors out; semantic actions remain external |
| SPEC-HAL-002 Pillar E | Preserved: one deterministic flat GPU command stream is produced before rendering |
| SPEC-HAL-002 Pillar F | Preserved: no OS input, focus, capture, composition, or frame authority is added |
| SPEC-HAL-024 | Preserved: domain/world are private 3D render-description spaces after the retained viewport boundary, not UI coordinate spaces |
| SPEC-HAL-343 | Preserved: its affine 2D pixel-space mechanics and domain adapters are neither reused incorrectly nor generalized into perspective/3D |
| SPEC-HAL-354 | Preserved: complete immutable revisions and existing invalidation/cadence remain the only publication path |
| SPEC-HAL-355 | Extended: lowering ends in the exact flat snapshot/model pair and inherits all resource/raster/lifecycle proof |
| SPEC-HAL-356 | Preserved: camera and navigation remain view-only and own no domain/content placement or semantic target |

This SPEC does not amend SPEC-HAL-002. It establishes a subordinate checked data
boundary that preserves every permanent invariant.

---

## 14. Non-Normative Implementation Decomposition

This order is guidance only. Implementation authority comes from the binding
normative body, and no decomposition step may weaken it:

1. checked domain similarity math, named f64 domain point/vector roles, inverse
   conversions, IDs, kinds, typed errors, and context-local aggregate retention
   accounting;
2. structural composition admission and deterministic RenderSnapshot lowering;
3. independent domain-state admission and complete DomainFrame3/ModelState
   lowering;
4. additive domain-aware Viewport3D request and committed metadata retention;
5. committed domain ray and explicit Planar2 side helper;
6. revision/lifecycle/resource-reuse proof; and
7. one domain-neutral geometry-only shared-depth reference fixture.

No step may introduce an excluded capability to make an earlier step appear
complete.

---

## 15. Binding Decision and Adoption Record

This SPEC chooses one authority rather than an artificial family. Identity,
similarity algebra, immutable composition/state/frame, flat lowering, committed
association, lifecycle, and proof are one owner boundary; splitting any of them
would leave an unusable or unverifiable partial contract.

The bounded long-term decisions are:

1. one global right-handed SPEC-HAL-355 world;
2. one-level immutable domain/content composition;
3. closed Planar2 and Spatial3 mechanical domain kinds;
4. positive-uniform-scale similarity embedding;
5. independently admitted immutable domain states plus one complete atomic
   mechanical world frame;
6. geometry-native first implementation vertical;
7. spatial co-presence and mechanical currentness carry no semantic relation or
   consumer revision;
8. aggregate domain metadata is context-budgeted for its complete lease lifetime
   through additive domain-specific limits, usage, budget, and error types while
   the pre-existing exhaustive Context3D policy/error surfaces remain unchanged;
9. flat Viewport3D compatibility permits only the declared bounded owner-side
   discriminator and zero domain work; and
10. all §12 systems remain separate authority units.

JC adopted this SPEC on 2026-08-28 as one bounded Halcyon-owned substrate
authority. Adoption performed exactly these authority changes:

1. changed SPEC-HAL-357 from Draft to Adopted / Binding;
2. reconciled SPEC-HAL-355 to admit the optional domain-aware producer path only
   when it lowers completely before the unchanged flat renderer boundary;
3. reconciled SPEC-HAL-356 to keep camera/navigation state independent of every
   domain/content placement and semantic association;
4. retained every §12 capability outside this SPEC's current authority while
   clarifying that separately accepted owner-correct authority and already
   authorized mechanisms remain possible and unaffected; and
5. left implementation and conformance claims gated on live source and the
   complete applicable §11 proof.

Locus remains consumer evidence, not substrate authority. Existing flat
Viewport3D remains binding and source-compatible; domain-composition support may
be claimed only by a live implementation that satisfies this SPEC.

The first binding implementation record is Halcyon `0.197.0-beta`. It adds the
closed public model and checked algebra in `src/three_d/domain.rs`, context-local
aggregate retention accounting in `src/three_d/context.rs`, the closed
flat/domain-aware request discriminator and committed-domain query boundary in
`src/three_d/viewport.rs`, and the applicable executable §11 conformance proof in
`src/tests/three_d_domain.rs`. The aggregate boundary uses additive
`DomainRetentionLimits3D`, `DomainRetentionUsage3D`,
`DomainRetentionBudget3D`, and `DomainRetentionError3D` types; the black-box
`tests/three_d_context_source_compatibility.rs` guard preserves the exact
pre-domain exhaustive Context3D policy/error shapes. Committed-association
staleness is reported only by `is_current_domain_association == false`, while
`DomainAssociation3Error` remains limited to actual fallible conversion.
`examples/spatial_domains_3d.rs` is the domain-neutral visible proof: a finite
Planar2 lattice with mechanical local-basis cues and a closed Spatial3 cube share
depth, while only the complete Planar2 embedding moves and rotates and the camera
and Spatial3 state remain fixed. The renderer still consumes only the lowered
flat view/model/snapshot tuple; this record adds no semantic relation, graph,
callback, selection, generic picking, chart, retained scene hierarchy, or new 2D
path.
