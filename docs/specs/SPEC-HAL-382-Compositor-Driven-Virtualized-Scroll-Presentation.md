# SPEC-HAL-382 — Compositor-driven virtualized scroll presentation

**PROPOSED / INACTIVE / NON-BINDING** · 2026-11-10

JC commissioned this design record and the first internal reuse slice. Halcyon
owns scrolling, input projection, realization, retained scenes, and compositor
execution. The delegated technical reviewer owns independent architectural review. Any new runtime
authority requires separate JC adoption. Consumers supply their existing data and
widgets through the owning APIs.

## 1. Current authority and execution boundary

| Owner | Applicable contract |
|---|---|
| Frame, geometry, and phases | [HAL-002](SPEC-HAL-002-Constitution-and-Architectural-Invariants.md) and [HAL-002A](SPEC-HAL-002A-Constitution-Amendment-Guidelines.md) govern ownership. [HAL-024](SPEC-HAL-024-Coordinate-Model.md), [HAL-042](SPEC-HAL-042-Frame-Loop-and-Engine-Ticker.md), [HAL-141](SPEC-HAL-141-Rect-Mutation-Authority.md), and [HAL-146](SPEC-HAL-146-Temporal-Ownership-of-Geometry.md) govern current transforms and their frame cutoffs. HAL-158 is Draft. |
| Scroll intent and restoration | [HAL-086](SPEC-HAL-086-Scroll-and-Viewport-System.md) documents the partial dual-offset implementation. [HAL-131A](SPEC-HAL-131A-Scroll-Substrate-and-Input-Arbitration.md) is Binding / Locked. [HAL-366](SPEC-HAL-366-Sequential-Horizontal-Scroll-Continuity.md) and [HAL-367](SPEC-HAL-367-Pre-Realization-Scroll-Initialization.md) are Accepted / Binding for the existing horizontal publication and first-layout restoration. |
| Realization, identity, and hit projection | [HAL-131B](SPEC-HAL-131B-Realization-and-Virtualization.md) is Binding / Locked. [HAL-131C](SPEC-HAL-131C-Input-Projection-Contract.md), [HAL-134](SPEC-HAL-134-Variable-Item-Height-Virtualization.md), [HAL-189](SPEC-HAL-189-Row-and-Cell-Reuse-and-Dirty-Flag-Law.md), [HAL-313](SPEC-HAL-313-Tabular-Row-Expansion.md), and [HAL-353](SPEC-HAL-353-Provider-Authoritative-ListView-Sorting-and-Order-Projection.md) preserve layout and input ownership, row and cell reuse, expansion, and provider order. |
| Retained composition and output | [HAL-150](SPEC-HAL-150-Compositor-Authority-and-Limits.md), [HAL-354](SPEC-HAL-354-Retained-Value-Updates-and-Scene-Fragments.md), and [HAL-359](SPEC-HAL-359-Cross-Pipeline-Z-Order-Execution.md) bind composition to admitted transforms, clips, fragments, and ordering. [HAL-266A](SPEC-HAL-266A-Per-Target-Custom-Render-Preparation.md) and [HAL-295A](SPEC-HAL-295A-Custom-Render-Pass-Authority-Reconciliation.md) continue to govern target compatibility, registered Image lifetime, and Queue custody. |

`ScrollArea` owns the requested offset and current visual offset. The existing
engine update advances the visual offset with the current 0.25 normal and 0.5
drag factors and existing snap. Its `_dt` argument is unused by this calculation.
`SceneNode::Scroll` projects a supplied offset inside the ordinary retained frame.
There is no independently ticking scroll sampler in this path.

HAL-131A §L.2 names the target in its early enforcement description and requires
layout invalidation when the visible range changes. The current pre- and
post-layout sync use `ScrollArea::visual_scroll_y()` so realization can cover the
offset shown during motion. HAL-042 §7.3 describes the engine visual update.
HAL-131A §L.6 and HAL-367 separately govern immediate restoration with a final
clamp before first realization. This proposal records those distinct cases.
It does not amend the locked contract or select a new sampling rule.

The first internal slice may reuse an unchanged authoritative window at the
existing scroll-sync cutoff. It may request paint-only invalidation through
`Widget::scroll_sync_invalidation` if the existing layout result, child geometry,
and current source are still valid. A changed window follows ordinary layout and
realization. Paint continues to consume only prepared row state. This optimization
does not depend on adoption of §§2–4.

The first source slice is the fixed-height TreeView body in ScrollArea's existing
content-owned vertical translation mode. The immediate content receives that
mode in its final LayoutContext. A nonempty completed window with unchanged HTS
source, geometry, topology, and TreeView layout configuration qualifies if it has
no live cell source, editor, or row child geometry to reconcile. HTS compares the
exact existing f64 range for the next visual offset. It can reuse that numeric
calculation in a subsequent layout and never uses it as row or source identity.
The body retains its realized rows and repaints them with the existing
viewport-local f64 subtraction. Horizontal composition and the existing hit
projection remain in place. This slice saves layout and realization. It still
does local Paint and ordinary scene composition.

The participating owners are the existing [frame scroll sync](../../src/core/app.rs),
[ScrollArea layout](../../src/widgets/scroll_area.rs), [TreeView body](../../src/widgets/tree_view/body.rs),
and [HTS virtualization](../../src/engines/hierarchical/virtualization.rs).

Generic ListView remains on its layout path because that path drains provider
invalidations and realizes current bindings. Equal ordinals alone cannot replace
that source work. Standalone List has a separate projected-input path that reads
the logical scroll offset. Its coordination with the visual transform requires
its own proof. This slice introduces no new List or ListView contract.

## 2. Proposed presentation and readiness contract

**Requested offset** is the current ScrollArea intent after the existing input,
anchor, linked-axis, restore, and clamp decisions. **Presented offset** is the
exact logical X and Y coordinate used by a given coherent target frame for its
scene transform and corresponding input projection. It is separate from physical
scanout. A successor request alone has no presented geometry or hit authority.

**Visible range** is the half-open ordered interval produced by the existing
virtualization owner from its authorized viewport offset and geometry.
**Realized range** is that owner's current bounded set of prepared resident
slots, including only the prefetch and pins admitted by its contract. A resident
slot or matching ordinal range alone does not prove that current pixels, input,
and resources are ready.

**Ready presentation region** is the exact region of content for which the
current realization has prepared complete scene and hit coverage. The owning
layout/realization result supplies it. Its identity includes the mounted
ScrollArea and content owner, surface and compatible target, viewport and clip,
scale, authoritative order and row identity domain, geometry and binding
revisions, and the actual registered Image generations used by its fragments.
A proposed scroll sample qualifies only if its whole clipped viewport is covered
in both axes and those dependencies are still current. Coverage includes partial
rows, overlapping content and all live interaction pins required by the existing
owner. The compositor consumes that result directly. No secondary range
expansion, provider query, row reconstruction, or layout occurs in composition.

An eligible within-region sample changes only the existing presentation transform
and any already-admitted dependent projection. Drawing and hit resolution use the
same presented coordinate, transform origin, clip, and generation. The existing
input owner keeps pointer capture, selection, and keyboard identity across row
recycling. For tabular content, identity comes from the admitted row identity
domain and RowId, with current ordinal association from the authoritative order
revision. Slot index, retained node address, and equal row counts do not prove
identity. Deep-content owners preserve the existing f64 subtraction or rebasing
before narrowing to viewport-local f32 under [PM-017](SPEC-PM-017-Content-Space-F32-Mantissa-Exhaustion.md).
Each axis is translated exactly once.

## 3. Boundary crossings and invalidation

When requested coverage leaves the ready region, the existing owner computes and
prepares the required range at its normal realization boundary. Only completed
geometry, row bindings, and resources for the exact current generation can
qualify the next sample. A clamp, resize, restore, order replacement, source
update, layout/clip change, or target change retires any incompatible readiness.
A stale or superseded completion cannot qualify a new presentation even if the
offset, ordinal range, or resource ID repeats. Removal and inactive coverage
follow the existing owner lifecycle and custody.

Keep at most one current presented region and one current replacement request
per scroll owner. Their resources obey existing bounded residency, retention,
and retirement. Failure and insufficient coverage return control to the existing
frame and realization path. The compositor gains no new authority to extend an
offset, delay input, block a surface, show missing rows, or sample an incompatible
front. HAL-266A's separately adopted Plot-specific rule remains confined to Plot.

## 4. Separate admission required for independent composition

The proposed handoff requires an adopted frame contract for when ScrollArea may
publish a newer offset to composition without a new UI realization pass, when the
corresponding presented input snapshot becomes observable, and which generation
owns rejection at a coverage boundary. The handoff must be qualified against
HAL-042 frame order, HAL-150 compositor limits, and locked HAL-131A/B plus
HAL-131C. It must preserve the existing coordinate, phase, provider, retained
fragment, z-order, and Queue owners in §1. A compositor may consume a newer
immutable authorized offset under such a contract. Scroll intent and sampling
authority remain with the existing ScrollArea and frame owners.

Independent compositor ticks, asynchronous pan/zoom, scheduling/threading, and
latency guarantees have no implementation or admission under this record. There
is no new public widget or consumer-facing readiness API. The first source slice
keeps the current frame cadence and ordinary wheel and thumb motion.

## 5. Proof obligations

For the internal slice, compare identical scroll sequences before and after at
the existing frame and sync cutoffs. Count body layout, realization and provider
reads, local paint, and retained composition separately. Check fractional forward
and reverse motion in one window, exact boundary entry, pointer and keyboard
identity, clipping, resize and clamp, retained X/Y, and deep f64 precision. Check
dirty layout/child roles and source changes on the existing path. Cite the
actual participating owners and separate a retained-scene test from a native
presentation measurement.

Independent composition additionally requires GPU compile/readback equivalence,
native motion and input observations, exact resource and target qualification,
and proof under source replacement, interaction pins, superseded work, and
coverage loss. No performance or native acceptance follows from a scene-only
test. independent technical review and JC adoption precede any use of the proposed handoff.

### First-slice evidence

The [retained-scene regression](../../src/tests/treeview_smoke.rs) applied the
same three visual offsets of 1.25, 2.1875, and 1.640625 logical pixels with the
same realized row IDs. It measured these changes against base `e8a718ce`.

| Work in the three frames | Base | First slice |
|---|---:|---:|
| Frames with TreeView body layout | 3 | 0 |
| HTS realization passes | 6 | 0 |
| TreeView body local paints | 3 | 3 |

The test also exercises the routed pointer and keyboard identity, clip exit,
boundary realization, and resize and clamp. The [HTS precision regression](../../src/tests/hts_f64_scroll_core.rs)
checks a same-window move beyond 2²⁴ and refusal of a different substrate at the
same revision. The evidence is from the retained-scene and source owners.
