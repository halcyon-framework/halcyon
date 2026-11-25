# SPEC-HAL-194: ListView Spatial Delegation Invariant

## Status

**LOCKED** — Binding.

## Purpose

This SPEC codifies the spatial authority boundary between ListView and TabularSubstrate.

ListView has historically contained manual coordinate arithmetic (`+ header_height`, `- scroll_y`, viewport origin computation). This SPEC declares such computation **forbidden** and establishes TabularSubstrate as the sole spatial authority.

This is a law, not a design proposal. No implementation guidance. No refactors. No new abstractions.

---

## 1. Invariant Statement

**INV-LISTVIEW-SPATIAL-DELEGATION**

ListView MUST NOT perform manual spatial reasoning for header, body, or footer.

All spatial ownership — rects, offsets, clipping boundaries, scroll application, hit-test coordinate space — is delegated to TabularSubstrate, which provides authoritative domain rectangles.

ListView consumes domain rects. ListView never infers them.

---

## 2. Forbidden Behaviors

The following are **explicitly forbidden** within ListView's `paint()`, `hit_test()`, `handle_event()`, and `layout()` methods:

| Forbidden Pattern | Example | Rationale |
|-------------------|---------|-----------|
| Manual header offset addition | `rect.y + header_height` | Header geometry is substrate authority |
| Manual scroll offset subtraction | `y - scroll_y` | Scroll application is substrate authority |
| Manual scroll offset addition | `y + scroll_y` | Scroll compensation is substrate authority |
| Manual viewport origin computation | `RectF::new(0.0, header_height, ...)` | Viewport origin is substrate authority |
| Manual body offset computation | `body_y = header.height()` | Body origin is substrate authority |
| Manual footer offset computation | `footer_y = body_y + body_height` | Footer origin is substrate authority |
| Parallel geometry state | `cached_header_height: f32` | Single source of truth violation |
| Hit-test coordinate transformation | `local_y = y - viewport.y + scroll_y` | Hit-test space is substrate authority |

**INV-194-FORBIDDEN-MATH**: Any arithmetic involving `header_height`, `scroll_y`, `viewport.y`, `body_height`, or `footer_height` inside ListView is a violation.

---

## 3. Authority Declaration

**TabularSubstrate is the sole spatial authority for header/body/footer geometry.**

| Authority | Owner | Consumer |
|-----------|-------|----------|
| Header rect | TabularSubstrate | ListView |
| Body rect | TabularSubstrate | ListView |
| Body viewport (with scroll applied) | TabularSubstrate | ListView |
| Footer rect | TabularSubstrate | ListView |
| Row projections (y_offset, height) | TabularSubstrate | ListView |
| Cell geometry slots | TabularSubstrate | ListView |
| Hit-test coordinate space | TabularSubstrate | ListView |
| Scroll offset application | TabularSubstrate | ListView |

ListView may:
- Forward layout constraints to substrate
- Forward input events in widget-local coordinates
- Forward intents to substrate
- Paint using substrate-provided projections only

ListView must NOT:
- Add or subtract header/body/footer offsets
- Compensate for scroll manually
- Compute viewport origins
- Perform hit-test math beyond forwarding raw local coordinates
- Maintain parallel geometry state

**INV-194-SOLE-AUTHORITY**: TabularSubstrate owns all spatial computation. ListView owns none.

---

## 4. Violation Signatures

Mechanical detection of invariant breaches:

### 4.1 Source Scan Patterns

The following patterns in `list_view.rs` indicate violations:

```
# Forbidden arithmetic
+ header_height
- header_height
+ scroll_y
- scroll_y
+ body_height
- body_height
+ viewport.y
- viewport.y

# Forbidden field access for computation
self.substrate.header().height()  # in arithmetic context
self.substrate.scroll_y()         # in arithmetic context
```

### 4.2 Structural Signatures

| Signature | Violation |
|-----------|-----------|
| `let row_y = rect.y + <anything>` | Manual row Y computation |
| `let header_y = <anything>` | Manual header Y computation |
| `let body_y = <anything>` | Manual body Y computation |
| `let local_y = y - <anything> + <anything>` | Manual hit-test transformation |
| `RectF::new(<anything>, header_height, ...)` | Manual viewport construction |

### 4.3 Test Enforcement

A source scan test MAY verify absence of forbidden patterns:
- Grep `list_view.rs` for patterns in 4.1
- Fail if matches found outside comments

---

## 5. Non-Goals

This SPEC explicitly does NOT:

| Non-Goal | Clarification |
|----------|---------------|
| Mandate widget splitting | ListView may remain a single widget |
| Require child widgets | No HeaderWidget, BodyWidget, FooterWidget mandated |
| Prescribe rendering implementation | How domains are painted is not specified |
| Mandate substrate API changes | Existing substrate API is sufficient |
| Require refactoring | This is diagnostic/preventive, not a refactor mandate |

**INV-194-NO-REDESIGN**: This SPEC exists to prevent reintroduction of manual coordinate math. It does not mandate any architectural change.

---

## 6. Relationship to Other SPECs

| SPEC | Relationship |
|-----|--------------|
| SPEC-HAL-186 | Defines header/body/footer partition; this SPEC locks delegation of that partition |
| SPEC-HAL-190 | Defines authority projection; this SPEC declares spatial authority belongs to substrate |
| SPEC-HAL-192 | Locks ListView as closed composite; this SPEC specifies what "closed" means for spatial reasoning |
| SPEC-HAL-193 | Locks external extension points; this SPEC locks internal spatial delegation |

---

## 7. Invariant Summary

| ID | Statement |
|----|-----------|
| INV-LISTVIEW-SPATIAL-DELEGATION | ListView delegates all spatial computation to TabularSubstrate |
| INV-194-FORBIDDEN-MATH | Arithmetic with header_height, scroll_y, viewport.y in ListView is forbidden |
| INV-194-SOLE-AUTHORITY | TabularSubstrate is sole spatial authority; ListView consumes, never infers |
| INV-194-NO-REDESIGN | This SPEC is diagnostic/preventive, not a refactor mandate |
