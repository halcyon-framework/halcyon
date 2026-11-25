# SPEC-HAL-166: TabularSubstrate — Header Domain

**Status:** Implemented / Closed
**Layer:** Structural Domains / Interaction Routing
**Audience:** Engine, Layout, Interaction
**Depends on:**
SPEC-HAL-008, SPEC-HAL-024, SPEC-HAL-037, SPEC-HAL-079, SPEC-HAL-141, SPEC-HAL-158
**Parent:** SPEC-HAL-164
**Consumes:** SPEC-HAL-165

---

## 5. Header Cell Identity (continued)

**INVARIANT-4:**
Header cell identity is stable across:

- scrolling
- virtualization
- column visibility toggling

Header identity must never be frame-derived.

---

## 6. Interaction Routing

Header input routing is isolated.

**INVARIANT-5:**  
Pointer events entering header bounds are routed exclusively to the Header Domain.

**INVARIANT-6:**  
Header interactions may emit **intent directives** but must not mutate state directly.

Examples of allowed intents:

- request sort toggle
- request column resize start
- request column context menu

Actual mutations are resolved upstream.

---

## 7. Hit-Testing Rules

Hit-testing is performed against header-local geometry only.

Forbidden:

- forwarding header hits into body rows
- body widgets intercepting header input
- geometry inference during hit-testing

Hit zones must be explicitly defined.

---

## 8. Rendering Constraints

Headers are rendered in their own structural pass.

**INVARIANT-7:**  
Headers may not read body geometry or row state.

Visual styling is permitted but non-authoritative.

---

## 9. Forbidden Behaviors

Explicitly illegal:

- computing column widths in header code
- resizing columns directly from header handlers
- sorting data inside header widgets
- collapsing header into body rows
- conditional header existence per column

---

## 10. Scope Boundaries

This SPEC does **not** define:

- resize drag semantics
- sort behavior
- pinning
- multi-row headers
- footer domains

Those are layered later.

---

## 11. Relationship to Deprecated SPECs

This SPEC replaces header-related intent from:

- SPEC-HAL-117
- SPEC-HAL-118M
- SPEC-HAL-118L

No semantics are inherited.

---

## 12. Next SPEC

SPEC-HAL-167 will define:

**Body Domain, Row Projection, and Virtualization Wiring**

---

## 13. Closure

**This SPEC is fully implemented and closed.** Implementation: `src/widgets/tabular_view/header.rs` (HeaderDomain, header cell projection, hit-testing). All invariants (INVARIANT-4 through INVARIANT-7) enforced by domain isolation and intent-only interaction routing. No further work permitted under this SPEC.
