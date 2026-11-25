# SPEC-PM-004: Geometry Authoring Space Contract Drift

**Status:** Active
**Series:** SPEC-PM (Pathology & Mistake Prevention)
**Layer:** Meta-Architecture / Failure Law
**Owner:** Core Architecture
**Audience:** Renderer, Batcher, Pipeline Authors
**Applies To:** Path pipeline, rect pipeline, mesh pipeline, any GPU-driven geometry pipeline, any batching system applying transforms

---

## 0. Pathology Name

**Geometry Authoring Space Contract Drift**

Also known as:
- "Mixed-space mesh authoring"
- "Double-transform from pipeline shift"
- "Pre-transformed vertices under downstream transform"
- "Works at scroll_y=0, drifts at 2× after"
- "Pipeline migration broke scroll"

---

## 1. Summary of the Failure

This pathology occurs when a **geometry pipeline shifts transform responsibility** (e.g., from caller to batching layer) without updating all producers to match the new authoring space contract.

The concrete failure:

1. The path pipeline's `add_path` began applying scroll transform to the affine translation components, so that new unscrolled-mesh producers (guide rect batching) would scroll correctly.
2. Existing producers (`add_rounded_rect`, `add_rounded_border`) were still pre-scrolling vertex coordinates before constructing the PathMesh and calling `add_path`.
3. Result: path-based primitives received scroll **twice** — once baked into mesh vertices, once via the transform in `add_path` — causing 2× displacement.
4. The rect pipeline fallback (zero-radius rects routed through `add_rect`) was unaffected because it never passes through `add_path`.
5. Two incompatible authoring spaces coexisted inside the same batcher: some PathMesh vertices in unscrolled content-local space, others in pre-scrolled space.

Every individual code path was locally correct. The failure was **global inconsistency** between producers and the transform layer.

---

## 2. Observable Symptoms

Any of the following are red flags:

1. Rounded backgrounds/borders scroll at a different rate than text or flat rects
2. Elements align at scroll_y=0 but diverge proportionally to scroll offset
3. Path-rendered primitives move at 2× (or 0×) the expected scroll rate
4. Rect-pipeline fallback (radius=0) scrolls correctly while path-pipeline (radius>0) does not
5. A pipeline refactor "fixes" one producer but breaks existing ones
6. Adding scroll compensation in a producer "fixes" it locally but the fix depends on downstream behavior

If any of these occur, **this pathology is active**.

---

## 3. Root Cause (Architectural)

The failure is caused by **undeclared geometry authoring space contracts**.

Specifically:

- Geometry pipelines must declare what coordinate space their mesh vertices are authored in
- Scroll (or any spatial transform) ownership must be **singular** — applied at exactly one defined layer
- Mixed authoring spaces within the same batcher are forbidden
- When transform responsibility moves (e.g., from producer to batching layer), all producers must be audited and migrated

This is **not a math bug**.

The scroll subtraction in `add_rounded_rect` was correct under the old contract (add_path did not apply scroll). The scroll subtraction in `add_path` was correct under the new contract (mesh vertices are unscrolled). Both were individually valid. The failure occurred because the old contract and new contract coexisted.

---

## 4. Why the Bug Reappears

This pathology reappears because:

1. **Pipeline refactors change transform location** — moving scroll application from producers into a central layer (or vice versa) without migrating all call sites
2. **Legacy call sites assume previous contracts** — existing producers continue pre-transforming because "it worked before"
3. **scroll_y=0 masks the defect** — at zero scroll, double-application produces zero displacement (0 + 0 = 0), so all tests at initial viewport pass
4. **Correct local math hides global inconsistency** — each function is arithmetically correct in isolation; the error only manifests when the full pipeline is traced end-to-end
5. **New producers written to new contract work** — the guide mesh batching was correct, creating false confidence that the pipeline change was safe
6. **No compile-time or type-system enforcement** — mesh vertices are plain `[f32; 2]`; nothing distinguishes "unscrolled" from "pre-scrolled"

---

## 5. Critical Clarification

> PM-002 and PM-004 are distinct failure classes.

- **PM-002** = coordinate transform applied by **multiple hierarchical layers** (ScrollArea + child both subtract scroll). The issue is layer responsibility.
- **PM-004** = inconsistent **vertex authoring space** within the same pipeline stage. The issue is producer-consumer contract mismatch after a pipeline migration.

PM-002 is a hierarchy problem. PM-004 is a geometry contract problem.

Both produce "2× scroll" symptoms. The diagnostic differentiator: if the double-application is between **parent and child**, check PM-002. If it is between **mesh producer and batching layer**, check PM-004.

---

## 6. Hard Prevention Rules (Binding)

### PM-004-R1: Every Geometry Pipeline Must Declare Its Authoring Space

Each pipeline (rect, path, mesh, text, future) must document:
- What coordinate space producers must author vertices in
- Where spatial transforms (scroll, viewport, clip) are applied
- Whether the transform is in the producer, the batching layer, or the shader

If undocumented, assume the contract is violated.

---

### PM-004-R2: Transform Applied Exactly Once in a Defined Layer

For any spatial transform (scroll, clip offset, viewport shift):
- Exactly one layer applies it
- That layer is documented in the pipeline contract
- No other layer may apply the same transform

If `add_path` applies scroll via transform translation, producers MUST NOT pre-scroll mesh vertices. If producers pre-scroll, `add_path` MUST NOT apply scroll.

---

### PM-004-R3: No Pre-Transform If Downstream Transform Exists

If a batching or rendering layer applies a spatial transform to geometry, producers MUST author vertices in the untransformed space.

The following pattern is forbidden:

```
let scrolled = apply_scroll(rect);         // producer pre-scrolls
let mesh = build_mesh(scrolled);           // mesh in scrolled space
add_path(mesh);                            // add_path ALSO scrolls via transform
```

Correct:

```
let mesh = build_mesh(rect);               // mesh in content-local space
add_path(mesh);                            // add_path applies scroll once
```

---

### PM-004-R4: Pipeline Migration Requires Explicit Audit of All Call Sites

When transform responsibility moves between layers:
- Every producer calling the affected entry point must be listed
- Each must be verified against the new contract
- Pre-existing scroll/transform application must be removed or preserved, matching the new contract
- "Grep for all callers" is mandatory, not optional

---

### PM-004-R5: Mixed Authoring Space in Same Batcher Is Forbidden

At any given time, all geometry entering a pipeline through the same entry point (`add_path`, `add_rect`, etc.) must be in the **same coordinate space**.

If two producers disagree about whether vertices are pre-scrolled, the batcher cannot apply a uniform transform. One will be correct, the other will be double-transformed.

---

## 7. Affected Systems

This pathology applies to:

- Path pipeline (`add_path`, `add_rounded_rect`, `add_rounded_border`)
- Rect pipeline (`add_rect`, rect fallback paths)
- Mesh pipeline (any `PathMesh` producer)
- Text pipeline (if text geometry is ever batched through a shared transform layer)
- Any future GPU-driven geometry pipeline
- Any batching system that applies spatial transforms to submitted geometry

If a system accepts geometry from multiple producers and applies transforms, **this SPEC applies**.

---

## 8. Forbidden Patterns (Explicit)

**In geometry producers that call a transform-applying entry point:**
- `apply_scroll_to_rect(rect)` before building mesh for `add_path` — FORBIDDEN
- `current_scroll()` subtracted from vertex coords before `add_path` — FORBIDDEN
- "Pre-scroll for correct clipping" when `add_path` already scrolls — FORBIDDEN

**In pipeline entry points:**
- Applying transform without documenting the expected authoring space — FORBIDDEN
- Adding transform application without auditing all existing callers — FORBIDDEN
- Assuming new callers match old contract — FORBIDDEN

**In general:**
- "Some meshes are pre-scrolled, some are not" — FORBIDDEN
- "It works because both transforms cancel out" — FORBIDDEN
- "Add compensating inverse transform" — FORBIDDEN

---

## 9. Diagnostic Invariants

To verify correct implementation:

**INV-PM004-SINGLE-SCROLL**: At scroll_y=100, a path-rendered rounded rect at content Y=200 must paint at viewport Y=100. If it paints at Y=0, scroll was applied twice. If it paints at Y=200, scroll was not applied.

**INV-PM004-PIPELINE-PARITY**: A rounded rect (path pipeline) and a flat rect (rect pipeline) at the same content position must paint at the same viewport position after scroll. If they diverge, authoring space contracts are mismatched between pipelines.

**INV-PM004-ZERO-SCROLL-MASK**: If elements align at scroll_y=0 but diverge at scroll_y>0, a double-transform is active. The divergence magnitude equals the extra scroll application.

---

## 10. Contract Reference

The current authoring-space contract for the path pipeline is codified as **Contract P-001** in the batcher implementation (`src/renderer/batcher.rs`).

The invariant:

> All PathMesh vertices and bounds are authored in unscrolled content-local space. Scroll is applied exactly once by `add_path` via affine transform translation.

For the authoritative per-pipeline contract table (which pipelines exist, what space each expects, where transforms are applied), refer to the batcher documentation and the relevant HAL SPECs. This PM SPEC defines the **failure class and prevention law**, not the current implementation details — those belong in design SPECs and code documentation.

---

## 11. Enforcement Rule

If a future implementation:
- Shows path-rendered geometry drifting relative to rect or text geometry after scroll
- Shows 2× or 0× scroll rate on a specific primitive type
- Requires "scroll compensation" in a mesh producer that calls a transform-applying entry point

The response is **not debugging**.

The response is:

> "We violated SPEC-PM-004. Which producer is authoring in the wrong space?"

Fix the contract. Not the symptom.

Compensating inverse transforms are forbidden. Any fix that preserves mixed authoring space is invalid. The correct repair is always: align all producers to the declared authoring space of the entry point they call.

---

## 12. Closure

This failure was introduced by a single commit that moved scroll transform responsibility into the path batching layer without migrating existing producers. The local logic at every call site was correct. The global contract was violated.

The fix was removing pre-scroll from two functions (11 insertions, 30 deletions). The investigation took days.

From this point on, any pipeline migration that changes transform responsibility without auditing all producers is not an accident.

It is a process violation.
