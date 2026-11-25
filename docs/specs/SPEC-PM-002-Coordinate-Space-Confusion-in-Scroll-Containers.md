# SPEC-PM-002: Coordinate Space Confusion in Scroll Containers

**Status:** Active
**Series:** SPEC-PM (Pathology & Mistake Prevention)
**Layer:** Meta-Architecture / Failure Law
**Owner:** Core Architecture
**Audience:** Engine, Widget, Interaction
**Applies To:** ScrollArea, List, ListView, List, TreeView, any scrollable container

---

## 0. Pathology Name

**Scroll Coordinate Double-Application**

Also known as:
- "Works until scroll"
- "Rows paint at wrong position after scroll"
- "Hit-test wrong after scroll"
- "Fixed it in paint, broke hit-test"
- "Scroll applied twice"

---

## 1. Summary of the Failure

This pathology occurs when **multiple layers each apply scroll transforms**, believing they are responsible for the conversion.

The failure manifests in two forms:

1. **Paint**: Child widget subtracts scroll from row positions, but ScrollArea already applies scroll transform via `scroll_offset()` → rows paint at wrong Y
2. **Hit-test**: Tree passes coordinates in content space (scroll applied), but hit-test function adds scroll again → clicks select wrong row

Both are the same root cause: **no clear contract about which coordinate space each layer operates in**.

This bug has appeared in:
- Original List implementation
- List rewrite
- Multiple ListView implementation attempts
- Each time "fixed" locally, then reintroduced

It must never be rediscovered.

---

## 2. Observable Symptoms

Any of the following are red flags:

1. Rows paint correctly at scroll_y=0, wrong after scrolling
2. Clicking selects correct row at scroll_y=0, wrong row after scrolling
3. Virtualization "appears broken" (realized rows exist but paint off-screen)
4. Hit-test works in initial viewport, fails for scrolled content
5. "Fix" involves adding or removing scroll subtraction somewhere
6. Different code paths (paint vs hit-test) handle scroll differently
7. Diagnostic overlay shows correct data but visual is wrong

If any of these occur, **this pathology is active**.

---

## 3. Root Cause (Architectural)

The failure is caused by **ambiguous coordinate space contracts**.

Specifically:

- ScrollArea applies scroll transform for paint (via `scroll_offset()`)
- Child widget ALSO subtracts scroll, not knowing parent already did
- Tree transforms `local_pos` to content space for scroll children
- Hit-test function ALSO adds scroll, not knowing coords are already transformed
- No documentation states which space each function expects/produces

This is **not a math bug**.

The arithmetic is locally correct.
The transforms are individually valid.

The failure occurs because **responsibility is unclear**, so multiple layers do the same work.

---

## 4. Critical Clarification

> Correct local logic does not prevent this bug.

Even if:
- ScrollArea correctly implements `scroll_offset()`
- ListView correctly implements row Y calculation
- Hit-test correctly adds scroll for viewport→content conversion

The bug still occurs if:
- ListView doesn't know ScrollArea already applied transform
- Hit-test doesn't know tree already transformed coordinates
- Each layer assumes it must handle scroll because "that's how scrolling works"

The failure happens because **the contract is missing**, not because any single component is wrong.

---

## 5. Why the Bug Reappears

This pathology reappears because:

1. Scroll transform is "obviously needed" so each layer adds it
2. Testing at scroll_y=0 passes (0 + 0 = 0, 0 - 0 = 0)
3. Each component works correctly in isolation
4. Integration reveals the double-application
5. "Fix" in one place breaks the other (paint vs hit-test)
6. No written invariant assigns responsibility to ONE layer
7. New implementations copy patterns without understanding coordinate contracts

Without explicit law, the mistake is repeated with every new scrollable widget.

---

## 6. Hard Prevention Rules (Binding)

### PM-002-R1: ScrollArea Owns Paint Transform

ScrollArea is the **ONLY** entity that applies scroll transform during paint.

Child widgets MUST paint at their layout-assigned positions.
Child widgets MUST NOT subtract scroll_y from row positions.

The transform is applied by the tree traversal using `scroll_offset()`.

---

### PM-002-R2: No Scroll Arithmetic in Child Paint

The following patterns are FORBIDDEN in paint methods of ScrollArea children:

```
row_y = y_offset - scroll_y       // FORBIDDEN
rect.y = base_y - scroll_offset   // FORBIDDEN
position.y -= self.scroll_y       // FORBIDDEN
```

Correct pattern:
```
row_y = rect.y + y_offset         // Scroll applied by parent
```

---

### PM-002-R3: Event Coordinates Have Defined Space

When inside a ScrollArea:
- `local_pos` is in **content space** (scroll already applied by tree)
- `global_pos` is in **window space** (no scroll applied)

To get viewport-local coordinates for hit-testing:
```
let viewport_y = global_pos.y - widget_rect.y;  // CORRECT
let viewport_y = local_pos.y;                    // WRONG (content space)
```

---

### PM-002-R4: Hit-Test Must Document Coordinate Space

Every hit-test function MUST document:
- What coordinate space it expects (viewport vs content)
- Whether it applies scroll internally
- What coordinate space it returns

If a function expects viewport coords and applies scroll internally, callers MUST NOT pre-apply scroll.

---

### PM-002-R5: Scroll Applied Exactly Once

For any coordinate transformation:
- Paint: ScrollArea applies scroll via `scroll_offset()` — children do not
- Hit-test: Either tree transforms OR hit-test function transforms — not both

If you find yourself adding scroll compensation "because it's off by scroll_y", **stop and check which layer already applied it**.

---

### PM-002-R6: Reference Implementation Parity

When implementing a new scrollable widget:
- Find a working reference (List, ScrollArea children)
- Match coordinate handling EXACTLY
- Do not "improve" or "simplify" the pattern
- If List uses `global_pos - rect.y`, so must you

---

## 7. Affected Systems

This pathology applies to:

- ScrollArea (as transform owner)
- List / List
- ListView
- TreeView
- Any widget that is a child of ScrollArea
- Any widget with internal virtualization
- Any hit-test function in scrolled contexts

If a system can scroll content, **this SPEC applies**.

---

## 8. Forbidden Patterns (Explicit)

The following patterns are forbidden:

**In paint (for ScrollArea children):**
- `y - scroll_y` or `y - self.scroll_offset`
- "Adjust for scroll in paint"
- "Compensate for scroll transform"

**In hit-test (for ScrollArea children):**
- Using `local_pos` directly when inside ScrollArea
- Adding scroll when tree already transformed coords
- "Scroll adjustment for hit accuracy"

**In general:**
- "Scroll handled here AND there"
- "Apply scroll because it looks wrong"
- "Remove scroll because it's doubled"

Any appearance of these patterns without understanding the full coordinate contract is an SPEC-PM-002 violation.

---

## 9. Diagnostic Invariants

To verify correct implementation:

**INV-SCROLL-PAINT-ONCE**: At scroll_y=100, row 0 should paint at `layout_y + 0`, not `layout_y - 100`. ScrollArea's transform shifts the entire coordinate system.

**INV-SCROLL-HIT-ONCE**: At scroll_y=100, clicking at window Y=50 (within viewport) should hit row ~4 (100+50=150, 150/24≈4), with scroll applied exactly once.

**INV-SCROLL-MATCH**: If paint is correct and hit-test is wrong (or vice versa), coordinate spaces are mismatched between the two paths.

---

## 10. The Contract (Summary)

| Layer | Paint Responsibility | Hit-Test Responsibility |
|-------|---------------------|------------------------|
| ScrollArea | Provides `scroll_offset()` for tree | Tree transforms `local_pos` to content space |
| Tree traversal | Applies scroll transform | Passes content-space `local_pos` to children |
| Child widget | Paint at layout position (NO scroll math) | Use `global_pos - rect` for viewport coords |
| Hit-test function | N/A | Document expected space; apply scroll if expecting viewport |

---

## 11. Historical Record

This bug has appeared in:

1. **Original List**: Paint subtracted scroll, broke when ScrollArea integrated
2. **List**: Hit-test used wrong coordinate space, "fixed" multiple times
3. **ListView attempt 1**: Paint correct, hit-test double-applied scroll
4. **ListView attempt 2**: Hit-test fixed, paint broke
5. **ListView final**: Both fixed by matching List patterns exactly

Each instance consumed debugging time that could have been avoided with this document.

---

## 12. Relationship to SPEC-PM-001

SPEC-PM-001 (Invisible Selection) and SPEC-PM-002 (Coordinate Confusion) both manifest as "works until scroll" but have different root causes:

- **PM-001**: Selection logic operates on visibility instead of identity
- **PM-002**: Coordinate transforms applied multiple times

If clicking selects the wrong row after scroll:
- Check PM-002 first (coordinate mismatch)
- Check PM-001 second (selection domain error)

Both may be present simultaneously.

---

## 13. Enforcement Rule

If a future implementation:
- Paints rows at wrong position after scroll
- Hits wrong row after scroll
- Requires "scroll compensation" in multiple places
- Has paint and hit-test disagree about positions

The response is **not debugging**.

The response is:

> "We violated SPEC-PM-002. Which layer is applying scroll twice?"

Fix the contract. Do not add compensating transforms.

---

## 14. Closure

This failure has occurred at least five times across three widget implementations.

Each time it was "fixed" locally.

Each time it reappeared in the next implementation.

From this point on, reintroducing it is not an accident.

It is a process violation.
