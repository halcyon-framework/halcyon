# SPEC-PM-007: **Clip-Hash Ordering Hazard — Z-Index Discipline**

### *Preventing Draw Order Instability Across Clip Boundaries*

**Author:** JC
**Status:** Post-Mortem + Permanent Rule
**Severity:** P0 — Caused 2+ months of visible flicker in ListView
**Applies to:** All widgets that paint at the same z_index across different clip states
**Amended:** 2026-07-05 — reconciled with the `90bf3812` ListView body ruling that normal column separators remain above row-state overlays; 2026-09-13 — SPEC-HAL-359 made strictly different Z values executable across pipeline families without giving equal-Z ties semantic order and required every visible `TextEngine` draw to admit an explicit producer-owned Z value.

---

# 1. Problem

Halcyon's batcher sorts primitives by `DrawBatchKey`:

```
(z_layer, clip_hash, pipeline, texture_id, shape_type_id)
```

When two primitives share the same `z_layer` but exist in
**different clip states** (e.g., one inside a clip region,
one outside it), they land in different batches. The relative
order of these batches is determined by `clip_hash` — a u64
hash of the clip rect coordinates.

**The hazard:** clip rects change during window resize. The
hash changes every frame. The comparison between two
different hashes can flip on every tick, causing the two
batches to alternate draw order. If one batch contains a
background and the other contains a separator or border
painted on top of it, the background alternates between
rendering before (separator visible) and after (separator
covered). The visual result is a separator that flickers
in and out of existence every frame during resize.

This is not a theoretical concern. It caused the ListView
column separator flicker bug that persisted for 2+ months.

---

# 2. Root Cause Analysis

**Widget:** ListView header + body

**Structure:**
```
Header clip: [header_rect]
  ├── Header background (z=0)
  ├── Scrollable clip: [header_rect, scrollable_rect]
  │     ├── Cell text (clipped)
  │     └── Column separators (z=0)  ← DIFFERENT clip state
  └── Pinned cells (z=0)
Header bottom border (z=0)  ← OUTSIDE header clip
```

**Failure:** Background batch (clip_hash_A) and separator
batch (clip_hash_B) both at z_layer=0. During resize,
clip_hash_A < clip_hash_B on frame N (separators paint
after background → visible), clip_hash_A > clip_hash_B on
frame N+1 (background paints after separators → covered).

**Why width doesn't matter:** Even at 3px width, if the
background renders AFTER the separator, it covers it
completely. This is a draw order bug, not a subpixel bug.

---

# 3. The Rule (Permanent)

**INV-PAINT-Z-ACROSS-CLIPS:** Any primitive that MUST
render on top of another primitive MUST use a strictly
higher effective order, regardless of whether they share the same clip state.
Ordinary callers and primitives within one member require a strictly higher
`z_layer`. The independently accepted SPEC-HAL-359 §2.5 closed ordinary-control
scope additionally admits explicit member precedence over unchanged local Z.
This 2026-10-08 delegated technical qualification does not adopt 363/364, grant
equal-effective-key semantics, or change the failure history below.

Relying on clip_hash ordering for visual layering is
**forbidden**. Clip hash determines batch grouping for
GPU efficiency — it has NO semantic meaning for draw
order and MUST NOT be relied upon for visual correctness.

### Z-Index Allocation Convention

| z_index | Purpose |
|---------|---------|
| 0 | Backgrounds, fills, tints |
| 1+ | Borders, separators, dividers |
| 2+ | LAYER_CONTENT and above — widget content |
| LAYER_FOCUS | Focus rings |
| LAYER_SCROLLBAR | Scrollbar affordances |
| LAYER_OVERLAY | System overlays |

These are allocation minima, not fixed global layer numbers.
Widgets that have multiple overlapping foreground classes MUST define a
strict local ordering for those classes. For example, ListView body uses
`base row background < row-state repaint < normal separators <
interaction separators`, with normal separators above the row-state
repaint so column boundaries remain visible on hovered/selected rows.

Widgets that paint backgrounds AND borders/separators in
the same paint method MUST use different z_index values
even if the primitives are in the same clip context. This
prevents the bug from reappearing if clip structure changes
during refactoring.

Visible text is not exempt. Every layout-owned `TextEngine::draw*` call MUST
name its Z value, and text that must cover an overlapping background, fill,
image, path, or row-state repaint MUST use a strictly larger value. A text
backend default, call order, or historical text-last pipeline sequence is not
layering authority.

### Specific Affected Patterns

1. **Column separators painted inside nested clips with
   backgrounds outside:** separators MUST be z=1+, backgrounds
   z=0.

2. **Horizontal borders (header bottom, footer top) painted
   outside clip regions with backgrounds inside:** borders
   MUST be z=1+, backgrounds z=0.

3. **Any widget that opens/closes clips between painting
   background and foreground elements:** foreground z MUST
   be strictly greater than background z.

4. **Row-state overlays (selection, hover, drag-band, focus
   fill) painted as z=0 row backgrounds:** in a clip-heavy,
   multi-pass tabular widget (ListView, TreeView, Grid) the
   cell and separator passes run after the base row background
   and may use different clip states. A row highlight emitted
   only at z=0 (Pass 1) can therefore be hidden by later clipped
   content or same-z cross-clip ordering hazards. Every such
   overlay MUST be re-painted at `LAYER_CONTENT` or higher in a
   dedicated later pass so it survives the base-background pass.

   This does NOT mean the overlay owns the top foreground layer.
   If column boundaries must remain visible on hovered/selected
   rows, normal body separators MUST use a strictly higher layer
   than the row-state repaint, and active/hovered interaction
   separators MUST use a strictly higher layer than normal
   separators. The binding rule is relative ordering between
   overlapping primitives, not a fixed absolute z number.

---

# 4. Invariants

**INV-PM-007-1:** Primitives that must visually layer on
top of other primitives MUST use a strictly higher effective order under
SPEC-HAL-359, independently of clip hash, pipeline and batching identity.
Outside its explicit §2.5 member precedence, strictly higher z_index is required.
Equal effective keys across different clip states remain a draw order hazard.

**INV-PM-007-2:** clip_hash in DrawBatchKey is for GPU
batch grouping only. It MUST NOT be relied upon for visual
ordering semantics.

**INV-PM-007-3:** Backgrounds/fills use z_index 0. Borders/
separators/dividers use z_index >= 1. Content uses
LAYER_CONTENT (2) or higher. This allocation is mandatory
for all new widgets.

**INV-PM-007-4:** Row-state overlays (selection, hover,
drag-band, focus fill) in clip-heavy tabular widgets are NOT
ordinary z=0 backgrounds: each MUST be re-painted at
`LAYER_CONTENT` or higher in a dedicated later pass when it
must survive later clipped cell/separator passes. The row-state
repaint MUST still remain below any primitive that must visually
sit above it, such as normal column separators or interaction
resize separators in ListView body.

**INV-PM-007-5:** Every visible `TextEngine` draw admits an explicit Z value.
The producer's local paint contract, not the text backend or pipeline family,
places that run relative to every overlapping primitive.

---

# 5. Forbidden Behaviors

**FORBID-PM-007-1:** Painting a background and a separator
at the same z_index when they are in different clip states.

**FORBID-PM-007-2:** Assuming that insertion order guarantees
draw order across clip boundaries. Insertion order only
guarantees order within the same batch (same DrawBatchKey).

**FORBID-PM-007-3:** Using z_index 0 for separators, borders,
or any element that must render on top of a background.

**FORBID-PM-007-4:** Introducing a new row-state highlight
(hover, selection, drag-band, focus fill) at z_index 0 only,
without the higher-z (z >= 2) re-paint pass, in any widget
that opens clips between painting the row background and the
row content. The z=0 paint alone can be hidden by later clipped
content or same-z cross-clip ordering hazards.

---

# 6. Affected Widgets (Audit Required)

| Widget | File | Status |
|--------|------|--------|
| ListView body row state and separators | src/widgets/list_view/body.rs | FIX: row-state repaint z=2, normal separators z=4, interaction separators `LAYER_FOCUS` |
| ListView header separators and bottom border | src/widgets/list_view/list_view.rs | FIX: header background z=0, normal separators/border z=1, hovered/active separators z=2 |
| ListView footer separators and top border | src/widgets/list_view/footer.rs | FIX: background/tint z=0, normal separators/top border z=1, hovered/active separators z=2 |
| TreeView header separators and bottom border | src/widgets/tree_view/tree_view.rs | FIX: header background/content at `LAYER_CONTENT`, separators/border at `LAYER_CONTENT + 1` |
| TreeView body row state and separators | src/widgets/tree_view/body.rs | AUDIT: row fills at z=0, guides/content/separators strictly above; no confirmed same-z cross-clip row-state symptom |
| TreeView footer separators and top border | src/widgets/tree_view/footer.rs | VIOLATION: background, separators, border, and active separators all at `LAYER_CONTENT` across clip boundary |
| DisclosureToggle | src/widgets/disclosure_toggle.rs | INVESTIGATE: child widget, may not be affected |
| OrderBook row backgrounds, bars, hover tint, and text | src/widgets/order_book_body.rs | FIX: explicit strict local ordering; text remains above every overlapping fill |
| Console owner background, span backgrounds, and text | src/widgets/console.rs | FIX: owner background below content; text above span fills |
| PropertyEditor band/data backgrounds and labels | src/widgets/property_editor/body.rs | FIX: explicit text content layer above row and band fills |
| Any widget with nested clips | all | AUDIT per rule |

---

# 7. Detection

New widgets should be audited during SPEC review for this
pattern. The check is: "Does this widget paint primitives
at the same z_index in different clip states?" If yes, the
lower-layer primitives must use a lower z_index.

Second check (pattern 4): "Does this widget add a row-state
highlight (selection / hover / drag-band / focus fill)?" If
yes, confirm it has a `LAYER_CONTENT`-or-higher re-paint pass
when it must survive later clipped cell/separator passes — not
just a z=0 background. Then confirm every overlapping
foreground class has a strict local ordering, such as row-state
repaint below normal separators and interaction separators.

A compile-time or runtime check is not feasible (clip state
is dynamic). This is a design-time discipline enforced by
review and by this SPEC.

---

# 8. Consistency Matrix (SPEC-HAL-002A)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode | N/A (renderer concern) |
| B — Phase Separation | Preserved. Paint-phase only. |
| C — ID-based Identity | N/A |
| D — Pure Data | Preserved. z_index is declarative. |
| E — GPU-First | **Strengthened.** Eliminates a GPU ordering hazard. |
| F — Sovereignty | N/A |
