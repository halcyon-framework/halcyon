# SPEC-HAL-352: Transient Scroll Anchoring Transactions

## Pre-Flight

- SPEC: SPEC-HAL-352, read before writing this task: YES
- Policy: Interactive
- Interaction added: YES — explicit layout-change anchoring requests; existing input ownership remains unchanged
- Forbidden behaviors checked: FORBID-SCROLL-1 through FORBID-SCROLL-6, FORBID-SCROLL-QUERY, FORBID-SCROLL-REALIZATION, FORBID-SCROLL-LATE-RESOLVE, FORBID-TIMING-HACKS, PM-002-R1 through PM-002-R5
- Existing infrastructure: ScrollState, ScrollArea, ScrollContainer, WidgetTree ScrollArea index, post-layout anchor resolution, viewport geometry, focus reveal, nested scroll isolation, CollapsibleSection, DisclosureSectionHeader
- Reinvention risk: Extends the existing ScrollArea anchor pipeline; does not create a second scroll owner, content extent, transform, or disclosure controller

**Status:** Binding
**Audit:** Clean after five audit/fix passes on 2026-07-12
**Layer:** Core scroll substrate / frame pipeline
**Scope:** Explicit vertical layout-change anchoring inside `ScrollArea`
**Extends:** SPEC-HAL-131A
**Depends on:** SPEC-HAL-024, SPEC-HAL-026, SPEC-HAL-086, SPEC-HAL-088, SPEC-HAL-110, SPEC-HAL-128, SPEC-HAL-131A, SPEC-HAL-151
**Integrates with:** SPEC-HAL-215, SPEC-HAL-351

---

## 1. Authority Decision

SPEC-HAL-131A is locked and requires a new SPEC for future behavior changes. This SPEC extends its anchor machinery without modifying its true content extent, normal hard-clamp contract, input arbitration, or virtualization boundary.

`ScrollArea` remains the sole owner of scroll state. Disclosure headers and collapsible controllers do not acquire scroll authority. A caller may explicitly open a layout-change anchoring transaction before applying staged state to the live tree in a way that changes layout participation; `WidgetTree` binds that request to the nearest owning `ScrollArea`, and the engine resolves it after layout from authoritative geometry.

SPEC-HAL-131A contains early prose requiring anchor resolution before layout and later locked implementation stages requiring engine-owned resolution after layout and before paint. This SPEC follows the implemented Stage 13/14 placement because layout-change anchoring requires the post-mutation geometry produced by the single layout pass. It does not move ordinary realization into the post-layout phase or permit resolution during widget layout.

---

## 2. Problem

When content near the end of a scroll domain collapses, the true maximum scroll offset shrinks. Normal end clamping then moves the viewport upward and makes the toggled header jump downward, even though the user did not request a scroll.

Preserving that header may require temporarily translating content beyond the new true maximum. The resulting empty tail is not content and must not be reported as content extent. It is transient anchoring slack owned by the `ScrollArea`.

Expansion has the inverse requirement: if newly opened content would remain below the scrollport, the transaction minimally reveals it while retaining the header as the visible reference point.

---

## 3. Definitions

### 3.1 True content extent

The logical content size reported by the content widget. It remains exact under SPEC-HAL-110 `INV-CONTENT-HEIGHT-EXACT` and SPEC-HAL-131A `INV-EXTENT-1`.

### 3.2 Base logical offset

`ScrollState::logical_offset`. It remains clamped to the true scroll range:

```text
true_max_y = max(0, content_extent.height - viewport_extent.height)
0 <= base_offset_y <= true_max_y
```

### 3.3 Transient tail slack

A non-negative vertical translation component owned by `ScrollArea` while a collapse-preservation result cannot be represented by the base logical offset:

```text
effective_visual_y = base_visual_y + tail_slack_y
tail_slack_y >= 0
```

Tail slack is not content extent, overscroll state, viewport extent, padding, child geometry, or scrollbar range.

### 3.4 Layout-change anchoring transaction

One explicit request containing:

- the anchor widget whose viewport Y is sampled before mutation;
- the affected region whose post-layout bottom is used for expansion reveal;
- one intent: `CollapsePreserveHeader` or `ExpandRevealRegion`;
- the nearest owning `ScrollArea` resolved by the engine;
- the sampled anchor viewport Y from the current effective visual offset;
- the owning scrollport size at snapshot time.

The anchor and region use tree-managed stable widget identity. Node indices are not persisted across the transaction boundary as semantic identity.

### 3.5 Scrollport size and reveal viewport height

`scrollport_size` is the actual container clip size and is used to detect resize staleness. `reveal_viewport_height` is the occlusion-aware `viewport_extent.height` used for vertical range and reveal math when a horizontal overlay scrollbar obscures the opposing-axis edge. They are distinct testimony.

---

## 4. Ownership Invariants

- **INV-352-OWNER-1:** `ScrollArea` is the sole owner of pending transactions, active tail slack, base offset commits, and effective scroll translation.
- **INV-352-OWNER-2:** `WidgetTree` may locate the nearest scroll owner and snapshot geometry, but must not own a second scroll state or reinterpret content extent.
- **INV-352-OWNER-3:** The requesting widget, disclosure header, collapsible controller, and affected content must not query ancestor scroll state, compute compensation, or apply transforms.
- **INV-352-OWNER-4:** `CollapsibleSection` remains scroll-neutral under SPEC-HAL-215 `INV-E2`. The explicit caller coordinates the transaction and the state mutation as separate operations.
- **INV-352-OWNER-5:** `DisclosureSectionHeader` does not automatically request anchoring. Consumers opt in through the explicit tree transaction API before changing layout participation.
- **INV-352-OWNER-6:** Ordinary SPEC-HAL-131A content anchors and layout-change transactions are mutually exclusive offset authorities. A valid layout-change request explicitly retires the owner's ordinary content anchor and dirty flag after the request snapshot succeeds.

---

## 5. Request Contract

The engine exposes one explicit request operation on `WidgetTree`:

```text
begin_layout_scroll_anchor(anchor_node, affected_region_node, intent) -> bool
```

The caller must invoke it before applying the staged expanded/collapsed value, or any other layout-affecting value, to the live widget tree. External/controller state may already contain the next value so long as live tree geometry and layout participation still represent the pre-change state when the snapshot is taken.

The request succeeds only when:

1. anchor and affected region exist;
2. both belong to the same nearest `ScrollArea` content subtree;
3. neither lookup crosses a nested `ScrollArea` boundary;
4. the anchor is the affected region or a descendant of it;
5. the anchor intersects the owning scrollport at request time under the effective visual offset;
6. the owner reports no active direct user scroll manipulation.

Failure returns `false` and performs no scroll mutation. It does not block the caller's layout state change.

- **INV-352-REQUEST-1:** Requests are explicit and precede mutation.
- **INV-352-REQUEST-2:** At most one pending transaction exists per `ScrollArea`; a later valid request replaces the earlier pending request deterministically.
- **INV-352-REQUEST-3:** A new valid request samples the current effective visual position, then retires any previous active slack before the next layout.
- **INV-352-REQUEST-4:** Requests do not trigger layout, mutate application/widget state, change an offset, or resolve post-mutation geometry immediately. The only scroll-state lifecycle mutation at request time is the explicit retirement required by `INV-352-OWNER-6` and replacement of pending/active layout-anchor state.
- **INV-352-REQUEST-5:** Programmatic anchoring never preempts an active user gesture.
- **INV-352-REQUEST-6:** Snapshot scrollport size is part of transaction testimony. If width or height changes before resolution, the pending transaction is stale and must fail normally.
- **INV-352-REQUEST-7:** The activity check is owner-local and read-only. `ScrollArea` derives it from its existing direct-manipulation state, including active scrollbar drag, proximity scroll, any active scroll input mode, and unresolved base visual-to-target convergence. `WidgetTree` does not query or duplicate `InteractionDomainState`.

---

## 6. Pipeline Placement

Transaction resolution extends the existing engine-owned post-layout scroll-anchor phase:

```text
0. pre-layout scroll synchronization
1. layout_tree
2. input projection population
3. post-layout scroll-anchor resolution
   3a. resolve ordinary content anchors
   3b. resolve layout-change anchoring transactions
4. focus reveal
5. post-layout virtualization synchronization
6. paint
```

- **INV-352-PIPELINE-1:** Request snapshot occurs before the layout-affecting mutation; resolution occurs after the resulting layout and before focus reveal and paint.
- **INV-352-PIPELINE-2:** `LayoutContext`, widget layout signatures, and widget layout methods carry no transaction queue or post-layout responsibility.
- **INV-352-PIPELINE-3:** Resolution reads completed layout geometry and mutates only the owning `ScrollArea` scroll state.
- **INV-352-PIPELINE-4:** Paint and hit-testing consume the same already-resolved effective offset and never resolve transactions.
- **INV-352-PIPELINE-5:** A pending request expires after one post-layout resolution attempt, including missing-target failure.
- **INV-352-PIPELINE-6:** Resolution uses the final tight-pass `content_extent`, `viewport_extent`, true maximum, container size, and post-layout geometry. Measurement-pass testimony is invalid.

---

## 7. Geometry Model

All transaction geometry is logical. The engine uses unscrolled authoritative layout positions and the owning scrollport. Widgets do not compute absolute coordinates.

At request time:

```text
pre_anchor_viewport_y =
    anchor_content_y_before - effective_visual_y_before
```

After layout:

```text
preserve_offset_y =
    anchor_content_y_after - pre_anchor_viewport_y
```

`anchor_content_y` and the affected region bottom are measured in the owning `ScrollArea` content coordinate space. Ancestor transforms outside the owner cancel out. Nested scroll transforms are not crossed.

- **INV-352-GEOM-1:** Vertical translation has exactly one existing owner. Standard ScrollArea content receives the effective offset through `Widget::scroll_offset()`. Content-owned vertical translation mode continues to return zero vertical scene translation and receives the same effective offset through its existing scroll synchronization projection. Applying both paths is forbidden.
- **INV-352-GEOM-2:** No widget-local compensation, child rect inflation, fake spacer, clip change, or paint-only transform is permitted.
- **INV-352-GEOM-3:** Paint and hit-test traversal receive the same effective offset.
- **INV-352-GEOM-4:** Tail slack is vertical only. Horizontal layout-change anchoring requires separate authority.

---

## 8. Collapse Resolution

For `CollapsePreserveHeader`:

```text
desired_effective_y = max(0, preserve_offset_y)
base_y = min(desired_effective_y, true_max_y)
tail_slack_y = desired_effective_y - base_y
```

The engine commits `base_y` to `ScrollState`, jumps the base target and base visual offset to the same value, and exposes `base_y + tail_slack_y` through the single active vertical translation owner defined by `INV-352-GEOM-1` in the same frame.

- **INV-352-COLLAPSE-1:** If the desired offset is inside the true range, no slack is created.
- **INV-352-COLLAPSE-2:** If the desired offset exceeds the true maximum, the excess becomes tail slack; true content extent and true maximum remain unchanged.
- **INV-352-COLLAPSE-3:** Compensation is immediate and deterministic. It is not smoothed, animated, delayed, or frame-counted.
- **INV-352-COLLAPSE-4:** No leading/top slack is created.

---

## 9. Expansion Resolution

For `ExpandRevealRegion`, the transaction first computes the header-preserving offset. It then computes the minimum offset that reveals the affected region's post-layout bottom:

```text
reveal_bottom_y = affected_region_bottom_y - reveal_viewport_height
desired_y = max(preserve_offset_y, reveal_bottom_y)
desired_y = min(desired_y, anchor_content_y_after)
base_y = clamp(desired_y, 0, true_max_y)
tail_slack_y = 0
```

The second bound keeps the header at or below the scrollport top. If the expanded region is taller than the scrollport, the header is aligned at the top and the maximum possible newly opened content is shown; the remainder stays normally scrollable.

- **INV-352-EXPAND-1:** Expansion never creates tail slack.
- **INV-352-EXPAND-2:** If the affected region is already fully visible, the header-preserving offset is retained.
- **INV-352-EXPAND-3:** Reveal is minimal while the entire region can fit; oversized regions keep the header visible and reveal the maximum possible content.
- **INV-352-EXPAND-4:** Expansion uses the affected region's actual post-layout geometry, not estimated height or pre-toggle content size.

---

## 10. Slack Lifecycle

Active slack survives idle frames so the preserved header does not jump on the next frame. It is retired only at an explicit boundary:

1. any committed user scroll source, including wheel, trackpad, thumb drag, proximity scroll, or keyboard scroll;
2. a committed programmatic offset change, linked-scroll update, or focus reveal that actually changes the offset;
3. viewport size change;
4. a new valid layout-change anchoring transaction;
5. owning `ScrollArea` removal or content rewiring;
6. true maximum growth reaching the preserved effective offset.

When true maximum grows but remains below the preserved effective offset, the base offset advances to the new true maximum and slack decreases by the same amount. The effective offset remains unchanged. When the true maximum reaches the preserved offset, base target and visual are committed to that offset and slack clears without a visual jump.

Content-origin mutation APIs retain their existing SPEC-HAL-131A compensation semantics while slack is active:

- bottom extension or bottom trim changes the true maximum and reconciles base/slack against the unchanged preserved effective offset;
- top prepend by `delta` increases both the preserved effective offset and base offset by `delta`, leaving slack unchanged before normal reconciliation;
- top trim by `delta` decreases the preserved effective offset and base offset by the applied trim amount, clamped at zero, then reconciles slack against the new true maximum.

These rules preserve the same logical content under the viewport. They do not resolve the layout-change anchor target again or reinterpret item identity.

- **INV-352-LIFE-1:** Idle time and repaint alone never retire slack.
- **INV-352-LIFE-2:** Clearing slack clamps to the current true range before applying the new scroll intent.
- **INV-352-LIFE-3:** A no-op focus reveal or no-op programmatic request does not retire slack.
- **INV-352-LIFE-4:** Further content shrink while slack is active may increase slack to preserve the same effective offset; it still must not change content extent testimony.
- **INV-352-LIFE-5:** Content-origin compensation updates the preserved effective offset exactly once. The effective paint/hit-test transform must not apply an additional prepend or trim adjustment.

---

## 11. Scrollbar, Virtualization, and Linked Scroll

- **INV-352-SCROLLBAR-1:** Scrollbar visibility, thumb size, and drag range use true content extent and viewport extent. Thumb position uses the ordinary smoothed base visual offset. No scrollbar calculation uses the effective offset with slack.
- **INV-352-VIRT-1:** Virtualization receives the effective visual offset used by paint so realization matches the actual viewport.
- **INV-352-VIRT-2:** Virtualization still receives true content extent and must clamp realized/visible item ranges to real content. Slack must never appear as additional items, rows, slots, or logical extent.
- **INV-352-VIRT-3:** Transaction resolution never triggers realization. The normal post-layout synchronization may publish the resolved effective offset for the next realization cycle.
- **INV-352-VIRT-4:** A content-owned vertical translation consumer receives effective offset exactly once through its existing projection; the ScrollArea scene transform remains zero on that axis. Paint, projection hit-testing, and visible-range synchronization must consume the same effective value.
- **INV-352-LINK-1:** Tail slack is local and is never propagated through a scroll group.
- **INV-352-LINK-2:** Receiving a linked base offset retires local slack before applying the linked update.

---

## 12. Focus, Input, and Interaction Domains

- **INV-352-FOCUS-1:** First focus remains scroll-neutral. Layout-change anchoring is created only by the explicit transaction API.
- **INV-352-FOCUS-2:** Focus reveal runs after transaction resolution. If the target is already visible under the effective offset, it is a no-op and slack remains. If reveal commits a different offset, it retires slack and uses normal true bounds.
- **INV-352-INPUT-1:** User input dominance from SPEC-HAL-131A and pointer capture from SPEC-HAL-128 remain unchanged.
- **INV-352-INPUT-2:** A transaction is scoped to the nearest owning scroll context. Parent and nested child ScrollAreas are not mutated.
- **INV-352-DOMAIN-1:** Pending and active anchoring state lives in the ScrollArea inside its window's widget tree. No global or cross-window anchoring state is allowed.

---

## 13. Dirty Layout and Failure Semantics

- **INV-352-DIRTY-1:** The transaction request does not replace normal SPEC-HAL-026 invalidation. The caller must still mark every layout-affecting widget/ancestor dirty through existing mechanisms.
- **INV-352-DIRTY-2:** Resolution does not request a second layout pass. It changes scroll translation only.
- **INV-352-FAIL-1:** Missing anchor, missing affected region, containment mismatch, owner mismatch, owner removal, snapshot scrollport size mismatch, unavailable final-pass metrics, or non-finite geometry cancels the pending transaction and leaves normal clamped scroll state.
- **INV-352-FAIL-2:** Failure never fabricates fallback geometry, selects a nearest widget, or preserves stale slack from the failed request.

---

## 14. Forbidden Behaviors

- Reporting tail slack as `content_extent`, child `content_size`, viewport extent, padding, or scrollbar range.
- Allowing ordinary user scrolling beyond true content end.
- Adding a spacer widget or inflating the collapsible region to simulate anchoring.
- Applying compensation in `DisclosureSectionHeader`, `CollapsibleSection`, the affected content widget, paint code, or hit-test code.
- Resolving a transaction during layout, event dispatch, animation update, or paint.
- Using timers, frame delays, first-frame masks, or animation to hide the clamp.
- Letting slack cross nested ScrollAreas, linked-scroll members, or window interaction domains.
- Triggering anchoring automatically for every size change or every disclosure widget.
- Exposing slack as virtual rows, realized slots, or supported scroll extent.

---

## 15. Acceptance Tests

Mechanical tests must prove:

1. collapse near the true end preserves the header viewport Y and creates only the exact required tail slack;
2. collapse away from the end preserves the header with zero slack;
3. expand reveals a fitting affected region with the minimum scroll delta;
4. expand of an oversized region keeps the header visible at the scrollport top and creates no slack;
5. content extent, max scroll, scrollbar thumb range, and linked-scroll payload remain unchanged by slack;
6. paint and hit-testing agree while slack is active;
7. virtualized visible range observes the effective offset but never realizes beyond true item count;
8. wheel, thumb drag, proximity scroll, keyboard scroll, committed focus reveal, and committed programmatic scroll retire slack before applying their intent;
9. a no-op focus reveal preserves slack;
10. viewport resize retires slack and clamps normally;
11. content growth transfers slack into base offset without visual movement and retires slack at zero;
12. top prepend and top trim compensate the preserved effective offset exactly once while active slack remains separate from true extent;
13. nested ScrollAreas mutate only the nearest owner;
14. active user gesture rejects a transaction without scroll mutation;
15. missing target or region cancels cleanly after one resolution attempt;
16. a valid layout-change request retires an ordinary content anchor so only one offset authority remains;
17. scrollport resize between request and resolution cancels the pending transaction;
18. content-owned vertical translation consumes the effective offset once while the ScrollArea scene Y transform remains zero;
19. a visible horizontal overlay scrollbar reduces expansion reveal height without changing the full-size resize snapshot;
20. the section-surface demo keeps its ScrollArea mounted, explicitly requests the transaction before applying the staged toggle to live widgets, preserves Recovery header Y on collapse, and reveals Recovery content on expand.

---

## 16. Non-Goals

- General overscroll, bounce, elastic physics, or scrolling past content end.
- Horizontal layout-change anchoring.
- Automatic anchoring for arbitrary layout changes.
- New disclosure state ownership or changes to `CollapsibleSection`.
- Animation policy.
- A second anchor resolver or virtualization protocol.
- Consumer-visible fake content or demo-only empty space.

---

## 17. Completion Contract

This SPEC is complete when its audit against all listed dependencies has no unresolved findings and the acceptance tests demonstrate that transient slack preserves explicit layout-change anchors without changing true scroll testimony, input ownership, coordinate transforms, nested-scroll isolation, focus behavior, or virtualization extent.
