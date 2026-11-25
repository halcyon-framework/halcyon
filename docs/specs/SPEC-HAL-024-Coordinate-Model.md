# **SPEC-HAL-024 — Coordinate Model (Authoritative)**

> **Paired amendment status — Proposed (2026-10-05):** The scoped
> SPEC-HAL-363/364 text in this document is part of the proposed paired adoption
> package, not an independent adoption or implementation claim. It becomes
> effective only with joint adoption of
> [SPEC-HAL-363](SPEC-HAL-363-Transient-Presentation-Coordination-and-Lifetime.md)
> and [SPEC-HAL-364](SPEC-HAL-364-Presentation-Occurrences-Coverage-and-Execution-Admission.md)
> and their required scoped amendments. The existing declared status and
> implementation evidence apply to the pre-existing contract. All unlisted
> clauses and ordinary-owner boundaries remain unchanged.

**Status:** Stable
**Implementation:** Complete
**Supersedes:** All coordinate-related content in SPEC-HAL-001…011
**Enforced by:** Halcyon Core
**Scope:** Layout → Paint → Scene → Renderer → Events → Hit-Test

> **App-local UI Scale — Accepted / Binding (2026-10-16):** JC's direct mandate
> records independent architectural acceptance of draft `ec307c53` and authorizes
> scoped reconciliation and implementation. Layer E and the expressly reconciled
> coordinate clauses below are binding; implementation proof remains pending.
> The pre-existing coordinate implementation status does not certify UI Scale.
> Separate 363/364 proposals remain inactive. Constitution SPEC-HAL-002 is supreme.

This SPEC defines the **only allowed coordinate model** for Halcyon.
It overrides and replaces all older definitions.

## Implementation Status

| Stage | Description | Status |
|-------|-------------|--------|
| Stage 2 | `absolute_xy` layout plumbing | Complete |
| Stage 3 | `absolute_paint_rect` stored per node | Complete |
| Stage 3B | Scroll-aware `absolute_paint_rect` for hit-testing | Complete |
| Stage 4 | Hit-test uses `absolute_paint_rect` | Complete |
| Stage 5 | Platform boundary normalizes pointer coords | Complete |
| Stage 6 | Event router uses `absolute_paint_rect` | Complete |
| Stage 7 | Renderer uses `absolute_paint_rect` as canonical geometry | Complete |

---

# ============================================================

# **LAYER A — THE HAMMER (NON-NEGOTIABLE LAWS)**

# ============================================================

These laws override *all* other documentation.
Implementations must treat them as **binding coordinate rules**.

Short. Brutal. Final.

---

## **1. Only three coordinate spaces exist. No others.**

1. **Relative Logical Space**
   Output of layout. Rect.x/y are **relative to parent**.

2. **Absolute Logical Space**
   Computed **only during traversal** (paint + hit-test).
   `abs = parent_abs + rect`.

3. **Physical Space**
   Exists **only inside the renderer**.
   Used for scissor rects, glyph rasterization, GPU operations.

Nothing else is allowed.
No “root UI”, no “window space”, no “widget local”, no hybrids.

---

## **2. Layout produces ONLY relative logical rects**

Widgets MUST store:

* `rect.x` and `rect.y` relative to parent
* `rect.width` and `rect.height` in logical units

Widgets must NOT store nor compute:

* absolute coordinates
* physical coordinates
* DPI-scaled values
* global positions

---

## **3. Absolute logical coordinates are computed ONLY by traversal**

Only two functions are allowed to compute absolute coordinates:

1. **paint_node()**
2. **hit_test_node()**

Both MUST compute:

```
abs_x = parent_abs_x + rect.x
abs_y = parent_abs_y + rect.y
```

Widgets must NEVER do this.
SceneNodes must NEVER do this.
Renderer must NEVER do this.

Traversal owns absolute space.

**363/364 scoped amendment:** Traversal remains sole producer of canonical absolute logical
coordinates, authoritative clip/scroll scope and hit-test geometry. The engine
may additionally publish a SPEC-HAL-364-certified presentation occurrence with
an explicit source-to-host mapping. The renderer MAY execute that already-resolved
mapping only under its admitted profile; it SHALL NOT infer parent geometry,
recompute layout or choose a logical transform.

---

## **4. Widgets NEVER:**

* compute absolute coordinates
* compute physical coordinates
* apply scroll offsets
* apply clip rects
* paint children
* produce scissor rects
* derive platform/App scale or perform device conversion; the existing 266
  read-only paint-time target projection remains its narrowly admitted exception
* read window size directly
* read DPI or physical resolution
* do “+ padding” in global/screen space
* apply transformations

Widgets operate ONLY in relative logical coordinates.

---

## **5. Clipping is applied ONLY by traversal**

* Widgets must NOT clip.
* ScrollArea must NOT call begin_clip / end_clip.
* No widget may generate its own ClipNode.
* Only traversal may push ClipNodes.

Clips are axis-aligned rectangles in **absolute logical** space.

**363/364 scoped amendment:** Canonical clips retain their owner-generated geometry and ordering.
SPEC-HAL-364 presentation masks and certified transported historical internal
coverage are distinct engine-authored paint data, not widget-generated canonical
ClipNodes. No widget gains child-paint, absolute-coordinate, scroll or clipping
authority. Ordinary axis-aligned/scissor behavior is unchanged; affine or
soft-mask execution needs an explicit pipeline profile.

---

## **6. Scroll offsets are applied ONLY by traversal**

ScrollArea stores:

* `scroll_x`, `scroll_y`
* `viewport_size`
* `content_size`

Traversal applies:

```
content_abs_x = parent_abs_x + rect.x - scroll_x
content_abs_y = parent_abs_y + rect.y - scroll_y
```

Widgets do NOT offset their children.
Widgets do NOT compute clipped child rects.

**363/364 scoped amendment:** An occurrence records actual pre/applied-scroll provenance and
applies each owner mapping once. It cannot change the scroll owner's state or
invent a second scroll transform.

---

## **7. Logical → Physical conversion happens ONLY inside renderer**

Allowed conversion operations:

* logical → physical scissor
* logical → physical text bounds
* logical → physical glyph positions
* logical → NDC (width/height)

Widgets NEVER touch physical space.

**363/364 scoped amendment:** SPEC-HAL-364 source/host mappings remain logical presentation
values. Exactly-once DPR conversion still belongs to the existing renderer
owner; a profile must preserve each pipeline's admitted conversion path.

---

## **8. Clip bounds propagate to BOTH rects and text**

* Traversal creates ClipNodes with absolute logical bounds
* Renderer converts those bounds to physical
* Rects MUST respect scissor
* Text MUST respect TextBounds derived from clip

If rect clips but text doesn’t → it violates this SPEC.

---

## **9. Event coordinates transform in exactly this order**

1. **Physical (raw winit)**
2. **Logical**
   (`/ E`, with `E = D × S` under Layer E; `S = 1.0` preserves DPI-only behavior)
3. **Absolute logical**
4. **Local logical = absolute − widget_abs**

Widgets receive ONLY local logical coordinates.

---

## **10. Hit-testing must mirror paint traversal EXACTLY**

Absolute logical computation MUST be identical to paint.

Clip boundaries MUST be respected.

Scroll offsets MUST be respected.

Same formula, same ordering, same rules.

**363/364 scoped amendment:** Paint and hit testing SHALL share canonical traversal geometry,
authoritative clips and scroll computation. Additional admitted presentation
mapping affects paint only and SHALL NOT be inverted into input geometry.
“Exactly” in this section refers to canonical traversal, not projected pixels.

---

## **11. No ad-hoc math anywhere**

Forbidden:

* rect.x + some_global_offset
* mixing logical & physical
* mixing relative & absolute
* doing layout using physical
* computing clip rects in widgets
* pushing ClipNodes from widgets
* widget-specific coordinate hacks

---

## **12. Renderer cannot infer logical coordinates**

Renderer receives:

* absolute logical positions from scene graph
* logical clip rects
* conversion rules
* scale_factor

Renderer must not:

* recompute layout
* interpret child-parent relationships
* reorder clips
* invent logical → logical transforms; only upstream-certified SPEC-HAL-364 mapping execution is permitted within the paired scope
* compute absolute logical

Renderer = pure translation layer.

For an App-owned target, `scale_factor` is the admitted effective physical/logical
ratio `E`, not an instruction to multiply by App scale again. Raw platform `D`
remains separate transport metadata. Layer E owns App admission/readback;
266/266A retain target projection/resource authority.

**363/364 scoped amendment:** Renderer-invented logical transforms are forbidden. Mechanical
execution of an upstream-certified SPEC-HAL-364 transform is permitted only
under its admitted profile; coordinate bases, clip classes, mapping order and
exactly-once DPR conversion SHALL be explicit. This is no canonical coordinate
system or permission to infer absolute positions.

---

# ============================================================

# **LAYER B — FORMAL STRUCTURED SPEC (PROFESSIONAL, COMPLETE)**

# ============================================================

This section expands Layer A into a full professional specification.
Layer A always overrides ambiguities.

---

# **1. Coordinate Spaces**

## **1.1 Relative Logical Space (layout space)**

* Origin: top-left of widget’s parent
* Units: logical pixels
* Output of `layout()`
* Stored in widget tree (`rect.x`, `rect.y`, `rect.w`, `rect.h`)

## **1.2 Absolute Logical Space**

* Origin: root widget’s top-left
* Units: logical pixels
* Computed during traversal (paint + hit-test)
* Defines the scene graph nodes fed to renderer

## **1.3 Physical Space (GPU space)**

* Units: physical pixels
* Only renderer uses it
* Renderer receives logical values and converts them using `scale_factor`

---

# **2. Transform Graph**

Allowed conversions:

```
physical → logical (platform boundary, using admitted E once)
logical → physical (renderer, using the same target E once)
relative logical → absolute logical (paint + hit-test only)
absolute logical → local logical (event routing only)
```

All other conversions forbidden.

**363/364 scoped amendment:** The Layer A §12 exception is mechanical execution of a resolved
presentation mapping, not an additional canonical conversion or inferred layout.
“All other conversions” does not prohibit that expressly admitted execution.

---

# **3. Layout Model**

### **3.1 Layout produces relative logical rects**

Layout must:

* not compute absolute
* not apply DPI
* not apply scroll
* not apply clip
* not read physical sizes
* not access scale_factor

### **3.2 Constraints**

Constraints are always logical.

### **3.3 ScrollArea layout**

ScrollArea:

* determines viewport_size
* lays out content with width constrained to viewport width
* content_size stored
* no clip applied
* no scroll applied
* no child offset applied

---

# **4. Paint Traversal**

Traversal is the **canonical owner** of:

* absolute logical coords
* scroll offsets
* clip propagation
* z-order
* scene graph construction

### **4.1 Absolute coordinate computation**

```
abs_x = parent_abs_x + rect.x
abs_y = parent_abs_y + rect.y
```

Scroll offset applied when descending into content.

### **4.2 Clip propagation**

Traversal creates:

```
SceneNode::Clip { rect: abs_clip_rect, children: [...] }
```

**363/364 scoped amendment:** Layer A §§5/12 and SPEC-HAL-364 C1–C10 distinguish current
authoritative clips, certified historical InternalCoverage, live barriers and
additional presentation masks. Their scope, geometry and coverage multiplicity
cannot be collapsed into one root crop.

### **4.3 Scroll propagation**

When entering content node of ScrollArea:

```
content_abs_x -= scroll_x
content_abs_y -= scroll_y
```

### **4.4 Scene Graph**

Traversal builds a scene of:

* Rects
* Text
* Clips
* Layers

All in **absolute logical** coordinates.

---

# **5. Renderer Model**

Renderer receives a scene graph of **absolute logical** nodes.

### **5.1 Logical → Physical Conversion**

Renderer converts:

* scissor rects
* text bounds
* glyph positions
* NDC coordinates

Only renderer may use scale_factor.

Here and in TextBounds below, the target `scale_factor` is `E = D × S` for
App-owned targets. Existing explicitly configured non-App targets supply their
admitted physical/logical ratio directly; they do not acquire an OS dependency.

### **5.2 Clip → Scissor**

`SceneNode::Clip` → push scissor rectangle

### **5.3 Text Bounds**

Every text node must receive **clip bounds** propagated from traversal.

Renderer resolves:

```
TextBounds {
  left   = logical_left * scale_factor
  top    = logical_top * scale_factor
  right  = logical_right * scale_factor
  bottom = logical_bottom * scale_factor
}
```

If this does not match clip, violation.

**363/364 scoped amendment:** The Layer A §12 exception permits only mechanical execution of
upstream-certified presentation values. It transfers no logical-geometry,
clip-policy, layout, timing or source-admission authority to the renderer.

---

# **6. Hit-Testing Model**

Must mirror paint traversal exactly:

* same absolute computation
* same scroll offset logic
* clip boundaries enforced
* depth-first recursion
* returns deepest hit

Conversions:

```
physical → logical (platform)
logical → absolute (unchanged)
local = absolute − widget_abs
```

**363/364 scoped amendment:** The canonical traversal equivalence in Layer A §10 remains;
presentation projection never changes these hit-test conversions or regions.

---

# **7. Event Routing Model**

### **7.1 Physical → Logical**

Platform converts:

```
logical = physical / E
```

### **7.2 Logical → Local**

Routers subtract widget absolute coordinates computed from hit-testing.

### **7.3 Widgets receive only local logical**

Widgets must never see absolute or physical coordinates.

---

# **8. DPI Model**

### **8.1 Core NEVER sees DPI**

Layout, widgets and canonical core geometry remain purely logical. The App-owned
boundary admits platform density `D` and App scale `S`, composes `E = D × S`, and
derives logical root constraints from obtained physical extent `/ E`. This grants
neither LayoutContext nor widgets DPI/App-scale interpretation authority.

### **8.2 Renderer ONLY uses scale_factor**

Renderer converts logical → physical using the admitted target ratio `E`.
No second per-element or scene-wide App multiplier is permitted.

---

# **9. Clip & Scroll Invariants**

1. Clip applies to rects **and** text
2. Clip propagated as absolute logical
3. Scroll offset applied only once in traversal
4. Nested ScrollAreas allowed
5. Widgets never manipulate scroll of parent

**363/364 scoped amendment:** For occurrences, the Layer A §§5–7/12 scoped distinction applies.
Canonical clips and scroll mappings remain owner-generated; transported
historical coverage and presentation masks require SPEC-HAL-364 certification.

---

# **10. Forbidden Operations (Formal)**

Widgets may NOT:

* compute absolute coords
* compute physical coords
* compute clip rects
* offset children manually
* clip manually
* do any geometry in physical space
* use DPI or scale_factor
* read window size
* paint children
* apply transformations
* generate ClipNodes

Renderer may NOT:

* compute absolute logical
* interpret widget structure
* reorder Clips
* generate logical coords

LayoutContext may NOT:

* access renderer
* use scale_factor

**363/364 scoped amendment:** The logical-transform prohibition excludes only mechanical
execution expressly admitted by Layer A §12. All widget-specific coordinate,
layout, clip, scroll and DPR prohibitions remain.

---

# **11. Invariants**

The following MUST hold at all times:

1. All widget rects in tree = relative logical
2. Scene graph = absolute logical
3. GPU scissor = physical
4. TextBounds = physical from logical clip
5. Paint traversal = only producer of absolute & clip
6. Event routing uses the same absolute model as paint
7. No coordinate math exists outside these rules

**363/364 scoped amendment:** Canonical absolute positions remain traversal-produced.
The separate immutable presentation mapping described in Layer A §12 may be
executed downstream; it is not another source of canonical coordinates.

---

# ============================================================

# **LAYER C — COORDINATE SPACES (CANONICAL)**

# ============================================================

This section defines the canonical coordinate space names used throughout
the codebase and documentation. All code and comments must use these names.

---

## **Canonical Coordinate Spaces**

### **1. Local**

Widget-local coordinates; origin at the widget's own top-left.
Used inside widgets during layout and painting.

- Origin: top-left of the widget itself
- Units: logical pixels
- Usage: internal widget calculations, event handlers receiving local positions

### **2. Layout Logical**

Coordinates in logical units, relative to the parent widget, with DPI stripped out.
This is what the layout engine works in.

- Origin: top-left of parent widget
- Units: logical pixels
- Usage: layout output (`rect.x`, `rect.y` stored in widget tree)
- Equivalent to "Relative Logical Space" in Layer A/B

### **3. Absolute Logical**

Coordinates in logical units, relative to the root window.
This is what hit testing and scene graph use as the "global" space.

- Origin: top-left of root widget / window content area
- Units: logical pixels
- Usage: scene graph nodes, hit-testing, paint traversal output
- Computed only during traversal: `abs = parent_abs + rect`

### **4. Physical**

Pixel / device units, after applying DPI scale.
This is what the OS / windowing system and GPU surfaces operate in.

- Origin: top-left of window surface
- Units: physical pixels (device pixels)
- Usage: scissor rects, glyph rasterization, GPU operations, winit events
- Conversion: `physical = logical * E`; for App-owned targets `E = D × S`

### **5. NDC (Normalized Device Coordinates)**

Normalized device coordinates in clip space ([-1, 1]²).
GPU-only, produced just before the vertex shader writes positions.

- Origin: center of viewport
- Range: x ∈ [-1, 1], y ∈ [-1, 1]
- Usage: vertex shader output, GPU clip space
- Conversion: done in vertex shader from physical coordinates

---

## **Current Code Terms → Canonical Names**

| Current Code Location / Term | Canonical Name |
|------------------------------|----------------|
| Widget rects in `widget_tree` (`rect.x`, `rect.y`) | **Layout Logical** |
| Window/event positions from winit | **Physical** |
| `RectF` used by renderer batching | **Absolute Logical** (target model) |
| Scene graph node positions | **Absolute Logical** |
| Hit-test input coordinates (after `/scale_factor`) | **Absolute Logical** |
| Event coordinates delivered to widget handlers | **Local** |
| GPU scissor rect values | **Physical** |
| Vertex shader position output | **NDC** |

---

## **Conversion Rules**

**Rule:** All layout and widget logic work in **Logical** space (Local, Layout Logical, or Absolute Logical depending on context).

**Rule:** All platform/window/event APIs talk **Physical**.

**Rule:** Conversion between Logical and Physical goes through a **single helper module** (`utils::coords`), not hand-rolled `* scale` math.

**Rule:** Only traversal (paint/hit-test) may convert Layout Logical → Absolute Logical.

**Rule:** Only the renderer may convert Absolute Logical → Physical → NDC.

---

## **Stage 5: Platform Boundary Normalization**

All inbound pointer positions from winit (`CursorMoved`, `MouseInput`, `MouseWheel`, `Touch`)
are in **Physical** coordinates.

**Conversion requirement:** These must be converted to **Absolute Logical** using
`utils::coords::physical_xy_to_logical_point()` **once** at the platform boundary
(in `platform/window.rs`), before being passed to any core systems.

**Consumers:** `WidgetTree::hit_test(x, y)` and widget pointer handlers must assume
all incoming coordinates are **Absolute Logical**. No ad-hoc `/scale_factor` math
is allowed outside of `utils::coords`.

The App supplies the admitted effective ratio `E` to this boundary. Raw OS scale
`D` is retained separately; the adapter neither discovers App preferences nor
divides by `D` and then repeats device conversion with `E`. Layer E's ordered
batch settlement and final geometry publication govern a runtime basis change.

---

# ============================================================

# **LAYER D — SCROLL GEOMETRY INVARIANTS**

# ============================================================

**Added**: 2025-12-23 (virtualization black gap fix)

This section formalizes geometry invariants for ScrollArea content coverage.

---

## **ScrollArea Content Coverage**

**INV-COORD-SCROLL-1 — Content Rect Coverage**

ScrollArea MUST assign its content child a rect height >= viewport_height.

```
child_rect_height = max(content_layout_result.size.height, viewport_height)
child_rect_width  = max(content_layout_result.size.width, viewport_width)
```

**Rationale:**
In virtualized mode, the realized content height may be smaller than the
viewport (e.g., only 25 items realized when viewport shows 30). The child
rect MUST still cover the viewport to prevent black gaps.

**Clarification:**
- This is **visual coverage**, not scroll extent
- `content_size` still represents full logical content (e.g., 100k items)
- `child_rect` is allowed to be larger than realized content

**INV-COORD-SCROLL-2 — Scroll Transform Application**

Scroll transform (`scene.begin_scroll(offset)`) is applied by traversal
AFTER layout completes. The transform subtracts scroll offset from
rendered positions:

```
rendered_y = layout_y - scroll_offset_y
```

Child widgets MUST be laid out at their **logical positions** (not scrolled).
Scroll transform is applied during paint traversal.

**INV-COORD-SCROLL-3 — Layout-Paint Ordering**

For virtualized content:
1. Pre-layout sync updates visible_range from current scroll_y
2. Layout positions children at logical positions
3. Paint applies scroll transform

Reordering is forbidden. See SPEC-HAL-131A Section L.4.

---

See: SPEC-HAL-131A Section L.3 (INV-SA-GEOM-1)

---

# LAYER E — App-local UI Scale (accepted scoped addition)

**Author:** JC. **Status:** Accepted / Binding, 2026-10-16.
JC owns final authority. **Acceptance:** JC's direct continuation mandate records
independent architectural review and approval of signed draft
`ec307c537b0a430875ee0bb769a6971dfe903387`, following initial draft `1259519c`.
Only this App-scale contract and its narrow dependent clauses are adopted.
**Implementation:** Canonical `App::run` and per-target owners implemented;
bounded Windows proof is recorded in E.7. Dedicated runtime remains at `S = 1`.
Independent implementation review remains required.
**Superseding owner disposition, 2026-10-16:** HAL-081B INV-P2-007 requires base
presence and Visible state, not successful GPU output at a zero physical extent.
Keep the base registered and Visible; transport actual `P = 0` as inactive through
the existing target eligibility/Compile boundary. No reopening is required for
this case. [HAL-368](SPEC-HAL-368-Zero-Area-Target-Output-Eligibility.md) remains
Proposed/inactive under the subsequent owner review, with no reopening recommended.
HAL-081B and its invariant text remain unchanged.

## Pre-Flight

This Pre-Flight records the preceding authority-only reconciliation unit.
The runtime unit and its executed proof are recorded separately in E.7.

- Objective: reconcile accepted App-local UI Scale into its exact existing
  coordinate/App/target owners, then implement and verify that bounded contract.
- Mandatory startup and authority entry chain completed: full AGENTS,
  PRE-FLIGHT, ENGINEERING, PROJECT-ENTRY, Constitution/002A previously read;
  all six SHA-256 values verified unchanged for this assignment. Conditional
  DESIGN-INTELLIGENCE read fully as advisory material.
- Governing authority and status: supreme Foundational 002, Permanent 002A,
  Stable 024 and the status-bound owners in E.1; JC's direct mandate records
  scoped architectural acceptance and authorizes implementation.
- Authority change, if any: accepted Layer E, exact 024 coordinate clauses,
  scoped 018 App entry and 266/266A effective-density semantics only.
- Target layer and existing state verified: source baseline
  `ec307c537b0a430875ee0bb769a6971dfe903387`, `0.215.0-beta`.
- Owned mechanisms and files: this authority unit owns 024/018/266/266A only;
  implementation follows through existing App/input/target/retained owners.
- Named invariants: logical-only layout; traversal-owned placement/clips;
  exactly-once conversion; coherent per-target metrics; retained identity.
- Forbidden behavior: constitutional weakening, widget DPI math, a second
  scaling/rendering engine, implicit input/editor commits or competing SPECs.
- Existing mechanisms inspected and reuse decision: E.1 records the existing
  configuration, coordinate, target, text, image, input and retained owners.
- Relevant external or dependent boundaries: product sizing/persistence remain
  product-owned; no asset or application adoption.
- Test ownership and placement: authority unit uses document checks; following
  implementation uses nearest existing functional owners under E.6.
- Exact runnable verification commands and expected proof:
  from the repository root, `git diff --check -- docs/specs/SPEC-HAL-024-Coordinate-Model.md`
  must report no errors; review the complete owned diff and resolve its links
  and source references against the inspected baseline.
- Output or artifact completeness boundary: reconciled binding clauses with
  implementation status distinct from accepted design and recorded proof.
- SemVer and changelog impact: no runtime release from the authority-only commit;
  implementation must update the affected package/lock/changelog for the API.
- Git, history-rewrite, destructive, and external effects: one local signed
  documentation commit under standing authorization; no push, branch/worktree,
  history rewrite, deletion, installation or writer activation.
- Out of scope: unrequested UI/IME, pixel-test families, new widget
  interactions, density adoption and unrelated SPEC rewrites.

## Halcyon Pre-Flight

- Governing SPEC and read status: Constitution/002A and 024 fully read;
  relevant status-bound clauses in E.1 inspected.
- Widget policy or N/A: N/A; no widget behavior changes in this unit.
- Interaction added and policy authority: explicit App scale setting only;
  widget interaction and native IME remain unchanged.
- Applicable FORBID rules: 002 permanent prohibitions; 024 widget coordinate
  prohibitions; 266 no DPR in layout; 354 no stale fragment/fresh-world rebuild.
- Existing framework mechanism inspected: existing owners listed in E.1.
- Reinvention risk: extend those boundaries; no parallel scale engine or SPEC.
- Demo/example compliance: no demo, asset, UI or runtime edits/launches.

## E.1 Existing authority and source evidence

024 already owns coordinate conversion, DPI and input/paint equivalence. It is
the contract's owner; 266 owns propagation, not application preference policy.
No constitutional amendment is proposed. Any conflict must be resolved under
002A before adoption; this scoped addition cannot relax permanent law.

| Existing authority | Exact declared status and boundary |
|---|---|
| [002](SPEC-HAL-002-Constitution-and-Architectural-Invariants.md), [002A](SPEC-HAL-002A-Constitution-Amendment-Guidelines.md) | Foundational; Permanent Appendix. Sovereignty, IDs, phases and deferred mutation are supreme. |
| This 024; [026](SPEC-HAL-026-Dirty-Flag-Layout-Invalidation.md) | Stable / Implementation Complete; Final, extended by 354. Logical geometry and lawful reflow. |
| [018](SPEC-HAL-018-App-Framework.md) §§15–16 | Document-wide Draft; only the scoped startup fullscreen/first-frame additions are binding. Obtained physical extent and first-visible-frame ordering remain binding. |
| [266](SPEC-HAL-266-Device-Pixel-Ratio-Scene-Propagation.md), [266A](SPEC-HAL-266A-Per-Target-Custom-Render-Preparation.md) §§3–6 | Active, amended by Adopted / Binding 266A. Immutable target density/metrics; no DPR in layout. Their proposed 363/364 clauses remain inactive. |
| [048](SPEC-HAL-048-Text-Layout-Engine.md) scoped typography contract | Body status Implemented; scoped 2026-10-10 acceptance retains its own proof/review history. Shared logical shaping/measurement and complete text realization, not font-size multiplication in widgets. |
| [012](SPEC-HAL-012-Theming.md), [R-019](SPEC-R-019-Nine-Patch-and-Border-Image-Rendering.md), [263](SPEC-HAL-263-Image-Pipeline-and-Texture-Resource-Architecture.md) | 012 Adopted / Binding, not whole-contract implemented; R-019 Adopted / Binding, Implemented; 263 Implemented, amended by binding 263A/263B. Appearance, logical NinePatch destinations and Image resource owners remain separate. |
| [128](SPEC-HAL-128-Pointer-Capture-and-Gesture-Arbitration.md) geometry-dependent cancellation; [362](SPEC-HAL-362-Relative-Locked-Pointer-Motion-and-Cursor-Restoration.md) §8.2 | Implemented; Adopted / Binding. Changed geometry/normalization can require terminal cancellation. |
| [151](SPEC-HAL-151-Multi-Window-Focus-and-Interaction-Domains.md), [176](SPEC-HAL-176-Explicit-Keyboard-Authority-Modes.md), [354](SPEC-HAL-354-Retained-Value-Updates-and-Scene-Fragments.md) §§2–3 | IMPLEMENTED (engine-level); Design / Binding; Binding. Window-domain isolation, keyboard authority and exact retained dependencies. |
| [020D](SPEC-HAL-020D-Action-Execution-and-Scope-Lifecycle.md) §§B,C,F; [360](SPEC-HAL-360-Complete-Text-Realization.md) §§2,4 | Design / Binding; Adopted / Binding. Explicit App action owner/outcome; complete text admission per surface, without cross-surface atomic presentation. |

[004](SPEC-HAL-004-WGPU-Winit-Integration.md),
[008](SPEC-HAL-008-Layout-Engine.md) and
[104](SPEC-HAL-104-Pixel-Stable-Text-Metrics-and-Glyph-Quad-Normalization.md)
declare Draft; [091](SPEC-HAL-091-Adaptive-Density-Modes.md) and the general
[092](SPEC-HAL-092-Unified-Typography-Rules.md) declare PROPOSED. Their descriptive
text or implementation references do not adopt their remaining scope.

Source anchors below are bound to the baseline above, not permanent line IDs:

| Actual owner | Verified implementation boundary |
|---|---|
| `src/utils/coords.rs:20–105`; `src/platform/window.rs:367–460` | Existing multiply/divide helpers; OS pointer/pixel-wheel conversion uses window DPI. Helpers do not validate factors. Line-wheel units are distinct. |
| `src/core/app.rs:11010–11046,11203–11214,13219–13240` | AppConfig has no UI Scale; requested window dimensions are OS logical units. Startup normalizes obtained physical extent before Screen construction. |
| `src/renderer/render_target.rs:161–194,430–442`; `src/renderer/wgpu_renderer.rs:5477–5517,6265–6293` | Per-target physical extent and scale; existing density invalidation and geometry/path projection. |
| `src/renderer/text_realization.rs:147–225,258–282`; `src/renderer/wgpu_renderer.rs:6974–6999` | Target-specific shaping/rasterization and physical text origin, with primitive clip retained. |
| `src/core/paint.rs:989–1018,1141–1219`; `src/renderer/batcher.rs:1412–1450` | Normalized source splits, logical NinePatch borders, ordinary Image lowering and shared clip/scroll state. |
| `src/renderer/render_graph/executor.rs:57–79`; `src/renderer/pixel_coverage.rs:87–164` | Existing target-metrics coverage/scissor conversion and finite/extent rejection. |
| `src/core/app.rs:3137–3202,3671–3688,4216–4269,7310–7476` | Clean-layout reuse; retained scale dependency invalidates paint, not layout; Screen rebuild removes mounted nodes. |
| `src/core/widget_tree.rs:405–427,2387–2422,2790–2911`; `src/core/event_router.rs:4679–4716` | Geometry-basis observation, hit geometry/clips and existing SystemCancel owner. |
| `src/core/app.rs:13651–13698,15613–15666,15860–15888`; `src/core/input_fusion.rs:922–1047` | Resize/DPI settlement, child-target isolation and relative-normalization cancellation. |
| `src/platform/window.rs:467–627`; `src/core/event_router.rs:3051–3083,3213–3247` | Keyboard/text ingress and keyboard Capture lifecycle. Audited native event conversion has no explicit winit IME arm; full native composition preservation is not established. |
| `src/core/app.rs:1443–1469,6011–6043,6073–6082,6131–6144,14427–14434,14770–14790`; `src/core/input_fusion.rs:74–135,649–667` | App flags/navigation and widget-owned intent drain are distinct. No App-scale request/result checkpoint exists. Fusion stores already-logical absolute motion without a scale epoch; relative disposal is not an absolute-input rebaser. |
| `src/renderer/custom_render_prepare.rs:39–72`; `src/core/widget_tree.rs:906–948`; `src/plot/mod.rs:2329–2452` | Custom preparation reports publication/binding, not aggregate asynchronous readiness. Async resource/front generations remain participant-owned. |
| `src/renderer/wgpu_renderer.rs:5894–5902,7194–7257`; `src/core/app.rs:15498–15517`; `src/core/event/mod.rs:2732–2795` | Zero-size target configuration is skipped; acquisition/text failures can skip one output; outputs submit independently. RawInputEvent has no preedit/composition lifecycle variants. |

## E.2 Accepted scale meaning

**JC-approved direction:** platform DPI and App-local user-selected UI Scale are
independent; UI Scale defaults to `1.0`. Product element sizing remains separate.
The following details are accepted by the scoped decision above.

For one App and one target, let `D` be the accepted platform density, `S` the
App's uniform dimensionless UI Scale, `E = D × S` the effective physical pixels
per canonical logical unit, and `P` the obtained physical client extent.
These are boundary quantities, not new coordinate spaces:

- logical target extent = `P / E`;
- physical output = canonical absolute logical geometry × `E`;
- incoming physical positions and pixel deltas = physical value / `E`;
- event-local position = normalized absolute logical position − canonical
  widget absolute position, under the existing router.

Each conversion occurs once at its existing boundary through the canonical
coordinate owners. Do not first divide input by `D` and then treat it as raw
physical input, or multiply scene geometry by `S` before a renderer already
using `E`. Semantic line-wheel steps and keyboard/text values are not lengths.
No rounded logical intermediate may become a second geometry authority.

Changing `S` does not request a different physical window size, alter OS DPI,
rewrite AppConfig's requested OS-logical window size, or change fullscreen mode.
At `S = 1.0` the existing behavior is preserved. Application controls and panels
keep product-owned logical sizes and optional local size choices. Do not
rewrite/multiply those stored element settings by `S`, apply them globally,
infer them from OS DPI, or adopt density modes. Screenshots do not establish
another application's internal scaling implementation.

## E.3 Configuration, change and persistence

### E.3.1 Bounded public surface

These are the accepted public API shapes; E.7 records implemented scope and proof:

| Accepted surface | Bounded meaning |
|---|---|
| `UiScale::try_from(f32) -> Result<UiScale, UiScaleValueError>` | Opaque finite positive scalar; default `1.0`. Nonfinite/nonpositive raw values return a typed value error before configuration/request mutation. |
| `AppConfig.ui_scale: UiScale` | Initial App value, settled before first Screen build/layout. This field does not become a live mutable widget or process-global setting. |
| `AppContext::request_ui_scale(UiScale) -> Result<UiScaleReceipt, UiScaleRejection>` | Explicit App-owned request. Receipt is `Queued { request_id }` or `Unchanged { admitted_revision }`; failure carries the reason. A callback may enqueue; it cannot apply scale inline. |
| `AppContext::ui_scale_status(Option<SurfaceId>) -> UiScaleStatus` | Immutable readback of the latest queued request/outcome, admitted App value/revision, and the selected target's effective snapshot/output status. No IO, mutation, layout or GPU wait. |

**Scoped runtime disposition:** canonical `App::run` owns scale admission. The
dedicated `run_ui_loop` retains `S = 1`: a non-1 request returns
`UnsupportedRuntime` without mutation; `1` is an unchanged no-op. Its existing
scene/input transport has no admitted scale/revision handshake with independent
target registration and Compile. HAL-044 remains Draft and does not authorize
such a transport extension. No universal runtime support is claimed.

One App-local pending slot is sufficient: a second request while occupied returns
`PendingRequest` without replacing the first. A queued receipt has a checked
App-local request identity; its terminal outcome is exactly one `Admitted` or
`Rejected(reason)`. After settlement the slot is free; retain only the latest
bounded outcome. Equal value with no pending request returns `Unchanged` without
a scale revision, invalidation or gesture cancellation. No separate job queue,
callback service or scheduler is admitted.

Readback distinguishes **requested** (queued value/receipt), **admitted** (the
App value accepted at E.3.2, with revision) and **effective** (one target's
published canonical snapshot: App revision, SurfaceId, target-metrics identity,
`D`, `P`, `E`). An admitted request can have targets still pending/unavailable.
Effective metrics do not certify resource readiness, submission or visible
pixels; report those outcomes separately under E.5. Missing/retired targets
produce an explicit target-unavailable result, never a primary-target default.

The product owns preference loading/saving, storage key/schema and explicit
save/apply policy. Persist selected `S`, never `D`, `E` or a pending receipt.
Halcyon performs no automatic global preference IO or OS synchronization.

### E.3.2 Settlement and rejection

The checkpoint is an **App-owned Phase-2 substep**: after the complete old-basis
InputFusion batch and its post-route Screen actions, with routing borrows ended,
before affected rebuild/layout/scene work or the next new-basis input batch.
Every admitted scale change must obey this ordering; the dedicated loop rejects
non-default requests under E.3.1. Existing widget-ID intent draining and
`set_theme` are not this checkpoint.
An action's synchronous execution only enqueues the explicit deferred request;
the existing engine frame wake is its resumption trigger under 020D. A request
arriving after the checkpoint waits for the next ordinary admitted frame.

Before any scale-caused mutation, validate the candidate and current admitted
target metrics. Inactive/retired targets supply no fabricated usable extent;
their next active snapshot must validate before effective publication.
Typed reasons distinguish nonfinite/nonpositive input,
`PendingRequest`, nonrepresentable effective/logical metrics and unavailable or
unsupported required target metrics. Include the affected target where relevant.
Constructor/immediate rejection returns its reason; deferred rejection appears
on the same receipt's readback. There is no silent drop, clamp or `1.0` fallback.
Rejection leaves the admitted value/revision, effective snapshots, retained
state and Capture untouched by the scale request. Independent OS/lifecycle
events retain their own authority; rejection cannot undo a real resize/loss.

For a valid change, consume the pending request once, perform required 128/362
cancellation before any affected pending commit, and admit one App revision.
All already-normalized old-basis events must finish routing under the old basis;
do not discard button/key edges, renormalize logical values or replay them.
Close the existing Fusion batch before admission; relative pending motion is
disposed by its cancellation owner. This does not pretend that current Fusion
can reproject arbitrary pending absolute motion. Retain physical cursor
provenance at ingress for subsequent normalization; an old logical point is not
physical provenance. DPI/resize is sampled before this checkpoint; a later
change belongs to the next target snapshot.

Recompute allocation when `P / E` changes, then final geometry, hit regions,
clips and viewports. Refresh scale-dependent retained/text/target dependencies
under 026/354/266A even when allocation is equal. Publish effective geometry
before accepting coordinate input under its new basis; deferred layout cannot
route that input through stale bounds. This is an App/input ordering obligation,
not a GPU-readiness gate or second event scheduler. Preserve widget/mount IDs;
Screen rebuild/navigation, a fresh tree and a repair frame are not scale tools.

Compare the newly admitted effective scale of the root target with the shaping
scale installed for the mounted tree. A change invalidates layout through its
descendants, including children with equal logical constraints. Auxiliary
targets keep their own output metrics and leave the mounted tree's shaping
input in place.

## E.4 Geometry, text, images and interaction compatibility

Layout/text style inputs remain logical and do not read `D`, `S` or `E`.
Measurement, wrapping, selection/caret geometry and paint consume the existing
shared logical text result. Rasterization and existing physical snapping use
the same target `E` as geometry, paths, Image, text bounds and scissor conversion.
No per-widget DPI workaround, new renderer, glyph-quad scaling shortcut or
duplicate rounding policy is admitted.

Image UVs/source texels, identity and binding lifetime remain unchanged.
NinePatch retains normalized source splits, logical destination borders and its
existing small-destination compression; lower its logical cells once through
the ordinary Image path. App scale does not modify texture metadata, invent
intrinsic sizing/padding, or multiply destination borders a second time.

Scale alone does not change focus, keyboard authority mode, editor value,
selection/caret indices, undo history, modal/Screen lifetime or navigation.
Preserve actual keyboard Capture without synthetic leave/re-enter, commit,
cancel or focus-loss events. Pointer capture remains governed by 128: ordinary
capture must not be retargeted; geometry-dependent sessions retire with the
existing SystemCancel before new-basis input or the first affected scene.
Relative-pointer normalization changes retain 362's forced cancellation and
pending-motion disposal. Blanket preservation of all Capture is forbidden.
Actual unrelated focus loss/unmount retains its existing teardown authority;
scale cannot synthesize that lifecycle.

The current audited canonical native path supplies committed text input but no
preedit/composition lifecycle vocabulary; 176 explicitly excludes IME. This
contract therefore guarantees only lawful existing text/editor and selection
continuity, without synthetic commit/cancel. It neither implements native IME
nor certifies preservation of an active native composition, candidate window or
IME session. Do not disable IME, fabricate composition state, or treat keyboard
Capture as proof of IME capability. A consumer requiring certified native IME
continuity needs a separate accepted owner/proof decision; this scale contract
must not advertise that capability. If such a path is separately admitted, its
owner must preserve preedit/selection/session identity and project placement
through the same coordinate boundary without scale-triggered commit/cancel.

## E.5 Targets, resize and failure bounds

No process-global `S` or primary-window DPI may supply another App/target.
All supported targets of one App use that App's `S` with their own `D` and `P`;
independent Apps may choose different values. Native and offscreen targets use
their existing admitted target metrics; no OS query is fabricated for an
offscreen target. This does not activate 151's deferred per-window widget trees.
Windows/macOS/Unix/WASM must share the same contract; source parity alone is not
observed target acceptance.

### E.5.1 Per-target atomicity

App scalar admission is one CPU-state transition. For each target, the effective
revision and final logical geometry/hit/clip/retained sidecars are published as
one canonical basis; preparation, paint, text Compile, Image/NinePatch lowering
and Execute must consume that basis and compatible target/resource generations.
No new-scale geometry may sample an incompatible old-scale custom front, glyph
preparation, clip or retained fragment. Effective readback names the basis; an
output result names the attempt actually submitted, pending or unavailable.

The **first successor output for a target** is its first actual successful
submission after admission, not the next attempted frame. It must be wholly
coherent for that target's then-current admitted scale/metrics. An unavailable
target may retain old OS-displayed pixels without producing a fresh output;
those pixels do not become canonical input geometry or an effective-scale
receipt. No bounded presentation latency or scanout synchronization is claimed.

Targets are independently eligible under binding 360 §4 and 266A. One may submit
while another is pending, lost or unavailable; a shared Compile/device failure
may still abort the entire attempt. There is no all-window readiness
barrier, cross-window atomic GPU Present, rollback of another target's submitted
output, or fabricated global renderer gate. A post-admission resource/acquisition
failure is a target output failure, not retroactive rejection of an already
admitted App request. Keep the admitted `S`; report the target reason, withdraw
incompatible output and use its existing recovery/wake owner. Never silently
revert `S` or resubmit a fresh output using the prior scale revision.

### E.5.2 Asynchronous resources and required readiness

266A custom preparation currently returns publication/binding evidence, not a
general asynchronous-ready transaction. Participant owners retain their pending
work, completion generations and target keys. A late completion is usable only
for current SurfaceId/metrics/resource generations; otherwise it cannot be
published as the successor front. The producer's existing Update/wake policy
drives later ordinary attempts: Plot consumes completion during Update and
reports pending work through `needs_update`. No new polling loop or second
scheduler is admitted.

Preserve actual owner admission: complete text is required by 360; every sampled
Image/NinePatch binding must be current under 263/263B/R-019. For an ordinary
custom participant lacking a compatible front, 266A §6 permits omission or a
separately authorized placeholder; scale does not promote it to a required front
or invent an implicit replacement. If an admitted consumer explicitly requires
that producer's front, its owner must provide the readiness evidence; absent
evidence cannot count as Ready. This contract supplies no universal new
required-resource API and does not silently adopt proposed 363/364 completion.

Pending readiness withholds only the output that actually requires it. Preserve
018 §16's stricter initial primary reveal gate: required producers and an actual
qualifying primary output, GPU completion, reveal and presentation; secondary
success cannot satisfy it. Runtime target readiness does not turn that startup
gate into a global all-surface transaction.

### E.5.3 Resize, loss and bounded failure

Resize changes `P`; accepted DPI change changes `D`; neither changes persisted
`S`. Snapshot coalesced changes at the App boundary. A later change cannot mutate
an in-flight snapshot; the next attempt uses the newer target metrics and
invalidates incompatible preparation. Target destruction retires its identity;
new targets derive the currently admitted `S`, not a retired target's receipt.
Apply 128/362 cancellation when their watched basis/normalization changes, even
if a different `D`/`S` pair produces equal `E`.

Require finite `S > 0`, finite positive representable `E`, finite representable
logical extents and existing physical/resource limits before activation. Reject
NaN, infinity, zero/negative scale, arithmetic overflow and unrepresentable or
unsupported target extents explicitly in release as well as debug builds.
An invalid initial configuration fails before reveal; 018 retains its fatal
startup conventions. E.3.2's runtime rejection preserves the prior admitted
state; failures discovered only after admission follow per-target output rules.

For a genuinely zero-area/minimized target, record zero-area inactivity: do not
divide to invent positive geometry, allocate zero-sized resources or call its
old configured extent a current viewport. Preserve `S` and resume only on an
admitted nonzero target snapshot. The E.1 source baseline skipped zero-size
configuration and could retain an older registry extent; that was not proof of
a complete suspension protocol. Lost/Outdated/Timeout or device/resource failure records
unavailable output under the existing target owner, without promising recovery
that source does not implement. A later valid snapshot/front may resume ordinary
attempts. Existing fatal owner errors remain fatal; readback is not a recovery
promise. No scale-specific recovery engine, silent retry loop or cross-target
compensation is admitted.

## E.6 Accepted decisions and compact functional acceptance

Independent architectural review and JC's continuation mandate accept the following:

| Scoped accepted decision | Rationale and boundary |
|---|---|
| Public API and overload policy | E.3.1's typed initial value, App request/readback and one pending slot with explicit PendingRequest rejection supply correlation/observability without a general settings job system. |
| Settlement/rejection | Adopt E.3.2's single App-owned Phase-2 checkpoint and pre-mutation rejection. Batch closure preserves normalization and once-only effects without repurposing widget intents or inventing an event scheduler. |
| Target atomicity/readiness | Adopt E.5's per-target coherent basis and independently admitted outputs. It matches binding 266A/360 and async owner limits; cross-window atomic presentation is outside this proposal. |
| IME capability | Limit acceptance to the audited existing text/editor path; exclude certified native IME continuity until separate owner/proof admission. This avoids turning scaling into new IME implementation. |
| Scale policy and integration | Use `E = D × S` and mathematical/resource bounds without an invented fixed numeric interval. Reconcile only 024 A§9/B§§5,7,8/C normalization, the scoped 018 App entry, and 266/266A target-density wording. No unrelated Draft or 363/364 adoption. |

These decisions adopt only the named scope. All unrelated Draft/Proposed
authority, including 363/364, retains its existing status.

Functional acceptance is bounded:

- `S = 1` preserves current startup/window behavior; a non-default initial value
  is coherent in the first visible frame, with product dimensions unchanged.
- At a fractional `D` and non-default `S`, one representative retained view
  keeps geometry/text/Image/NinePatch, nested clip/scroll and pointer hit/local
  coordinates consistent; text remains measured/rasterized through its owner.
- Runtime change preserves focus/keyboard Capture/editor identity and data,
  uses required gesture cancellation, and has no stale first-successor geometry,
  hit sidecars or target resources. Readback distinguishes Queued, Admitted,
  Rejected and per-target output outcomes; equal requests cause no invalidation.
- One pending-request conflict and one invalid-metrics request prove typed
  rejection with unchanged admitted state, no Capture interruption and one
  terminal receipt. Close an old-basis coalesced batch without replaying edges.
- Two targets, one async front/acquisition refusal, a resize/DPI race and a
  zero-area/resume prove per-target coherence, stale-completion exclusion and
  independent outputs. Two Apps prove isolated `S`; no simultaneous-Present,
  deferred-tree or general suspension capability is inferred.
- Existing editor/selection proof includes no synthetic commit/cancel; the
  native IME capability exclusion and initial primary reveal remain explicit.

Use nearest existing owner tests and a compact native functional observation
where required. No pixel-test families, unsupported-space enumeration or
unrelated refactors. Executed proof is limited to E.7; the acceptance list is not
a claim of an exhaustively verified platform or target matrix. Report verified
units for independent review; surface a real owner/constitutional conflict
without substituting a weaker mechanism.

## E.7 Scoped implementation and executed proof — 2026-10-16

**Superseding owner disposition:** the subsequent architectural decision
withdraws the earlier reopening instruction and zero-area hold. INV-P2-007 is
preserved by retaining base registration, identity, presence and Visible state;
zero output at an unusable physical extent does not violate it. No new contract
or reinterpretation of unrelated HAL-081B invariants is adopted here.

Current source transports OS resize `P` from App's coalesced size to
`WgpuRenderer::resize_render_target`, then `RenderTargetRegistry::observe_size_px`.
The last configured extent remains separate. Ordered Compile eligibility removes
the inactive target before positive-size assertions, preparation and acquisition;
other valid child targets remain independently eligible. The exact seam exists
in `src/core/app.rs:13771–13778,14660–14665`,
`src/renderer/render_target.rs:371–391` and
`src/renderer/wgpu_renderer.rs:5909–5913,6029–6038,7236–7253` at runtime commit
`5a2d832d`. No second transport or renderer architecture is required for canonical
App. This finding does not extend the dedicated loop's scale capability.

The runtime unit is based on authority reconciliation
`d8504b5ce01de6046a836593cb7397bf60bb746a` and JC's explicit architectural disposition
for the dedicated-loop boundary. Owners are `core/app` request/state/checkpoint and
input ingress, `utils/coords` checked conversion, and the existing render-target
registry/Compile/submission path. Package version is `0.216.0-beta`; no dependency,
application, asset, widget or native IME implementation was changed.

Canonical App implements E.3's typed initial scale, one deferred request slot and
request/admission/target readback. The existing input checkpoint closes the old
batch, validates before mutation, retires affected pointer gestures, preserves
keyboard Capture/editing, and invalidates retained paint and changed layout
constraints. Existing effective-density consumers own text, Image/NinePatch,
projection and clipping. No per-element scale multiplication was added.

Observed zero target extent is distinct from the last configured GPU extent.
Resize retires prepared plans; inactive targets have no Compile descriptor or
acquisition. App geometry admission can exclude a deferred primary while prepared
secondary targets continue in compositor order. This does not change base-surface
visibility, promise cross-window Present atomicity, or add a global renderer gate.
The dedicated loop accepts only the unchanged `1` no-op and rejects non-1 with
`UnsupportedRuntime` without state mutation; its transport is not extended.

Executed bounded verification on Windows:

- `grip x "verify package halcyon check timeout 300" --output raw`: passed,
  zero errors, 85 warnings reported.
- `grip x "verify package halcyon test ui_scale_ timeout 240" --output raw`:
  11 passed, zero failed. Includes real TextField/Capture/selection preservation,
  SystemCancel and retained preview retirement, pre-mutation rejection, isolated
  App state, exact conversion, inactive/deferred primary and secondary Compile.
- `grip x "verify package halcyon test frame_realization_preserves_exact_multi_surface_compile_without_fabricating_output timeout 60" --output raw`:
  one existing functional proof passed, zero failed.
- Built `app_frame_realization_native` fixture, `--run-ui-scale-native`: passed
  at actual `D = 1.5` on both targets. Initial `S = 1.25`, `E = 1.875`, revision
  `0`; deferred `S = 1.6`, `E = 2.4`, revision `1`. Primary physical extent stayed
  `(480, 270)`, secondary `(240, 150)`. Primary text refusal suppressed its output
  while secondary submitted; the following primary attempt submitted successfully.
  This observes submit/Present calls, not GPU completion or display scanout.

The native fixture was launched directly because Grip's typed execution backend
reported no accepted backend on this Windows host; Cargo checks/tests used Grip.
Formatting used exact-file rustfmt with child traversal disabled after Grip's
file formatter produced child-module output in the root-file preview. No new
pixel-test family was added. Other OS execution, universal runtime support and
native IME continuity are not certified by this proof. Independent implementation review remains required; a dedicated-loop transport contract requires separate owner
authority and is not adopted here.

### E.7.1 Native primary inactivity and restoration — 2026-10-16

The existing `tests/app_frame_realization_native.rs` scale mode now extends its
positive-target proof with actual Windows primary minimize/restore. It observes
App's physical metrics and HAL-361 receipts; it does not inject zero metrics or
infer them from the minimized flag. Runtime source remains at `5a2d832d`.

`grip x "verify package halcyon test app_frame_realization_native timeout 240" --output raw`
passed and built the fixture; its zero matched ordinary tests are build evidence
only. Direct invocation of that built fixture with `--run-ui-scale-native` passed:

| Observed attempt | Primary physical extent | Primary output | Positive child | App scale / admitted revision | Primary metrics revision |
|---|---|---|---|---|---|
| 5, inactive | `(0, 0)` | No text result/output/submit/Present call | `(240, 150)`, Complete and submitted | `1.6 / 1` | `5` |
| 7, restored | `(480, 270)` | Complete and submitted | `(240, 150)`, Complete and submitted | `1.6 / 1` | `6` |

The same primary SurfaceId and compositor receipt order survive the transition.
The inactive primary has no new custom-preparation attempt; restored output
matches a fresh preparation attempt and newer metrics revision. Restored targets
report `D = 1.5`, `E = 2.4`; inactive metrics are checked against each target's
own `D × 1.6`. The existing
`--run-native` default-scale mode also passed its three ordinary output/refusal/
success cases. These are actual canonical App submit/Present calls, without a
GPU-completion or display-scanout claim.

This follow-up changes only the existing proof fixture and this evidence record;
no production API/behavior, package version, compositor invariant, transport,
dedicated-loop capability or pixel-test family is changed.

# **END OF SPEC-HAL-024**
