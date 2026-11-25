# SPEC-PM-001: Virtualization, Identity, and the “Invisible Selection” Failure

**Status:** Active  
**Series:** SPEC-PM (Pathology & Mistake Prevention)  
**Layer:** Meta-Architecture / Failure Law  
**Owner:** Core Architecture  
**Audience:** Engine, Widget, Interaction  
**Applies To:** List, ListView, TreeView, Grid, PropertyEditor, any virtualized collection

---

## 0. Pathology Name

**Invisible Selection Collapse**

Also known as:
- “Selection breaks after scroll”
- “Clicking wrong row after scroll”
- “Selection disappears when collapsed”
- “Works until virtualization”

---

## 1. Summary of the Failure

This pathology occurs when **selection logic operates on realized (visible) items instead of logical identity**.

The failure manifests when rows become invisible due to:
- Scrolling (virtualization)
- Tree collapse / expand
- Filtering
- Sorting
- Deferred realization

Selection appears to work locally, then silently corrupts state when visibility changes.

This bug has already occurred, persisted, and consumed significant time.

It must never be rediscovered.

---

## 2. Observable Symptoms

Any of the following are red flags:

1. Selection is lost after scrolling away and back
2. Clicking a row after scroll selects the wrong logical item
3. Collapsing a tree node deselects its children
4. Expanding a node does not restore previous selection
5. Selection count changes when visibility changes
6. Selection version increments due to scroll, not intent
7. “Fixes” involve re-syncing selection from rendered rows

If any of these occur, **this pathology is active**.

---

## 3. Root Cause (Architectural)

The failure is caused by **conflating visibility with existence**.

Specifically:

- Realized rows are treated as the selection universe
- Selection is recomputed, filtered, or mutated based on visible rows
- Invisible items are implicitly treated as non-existent
- User intent is lost without any explicit action

This is **not a state-storage bug**.

Items may be stateless.  
Rows may store nothing.

The failure occurs entirely in **selection logic and iteration domain**.

---

## 4. Critical Clarification

> Stateless items do **not** make this safe.

Even if:
- Rows store no selection flag
- Data is immutable
- Rendering is pure

The bug still occurs if selection logic:
- Iterates over realized rows
- Clears or rebuilds selection from visible subsets
- Applies selection deltas only to visible items
- Derives selection from projection state

The loss happens because **the algorithm’s domain is wrong**, not because state was stored incorrectly.

---

## 5. Why the Bug Reappears

This pathology reappears because:

1. Virtualization hides the error during initial testing
2. Selection appears correct until scroll / collapse
3. Local fixes “work” in static scenarios
4. Engineers forget invisibility is not deletion
5. No written invariant forbids visibility-based logic
6. Human memory is unreliable under time pressure

Without explicit law, the mistake is repeated.

---

## 6. Hard Prevention Rules (Binding)

### PM-001-R1: Identity-Based Selection Only

Selection MUST be keyed by **logical identity** (e.g. RowId), never by:
- Render index
- Realized row slot
- Viewport position

---

### PM-001-R2: Visibility Is a Read-Only Concern

Selection logic MUST NOT:
- Iterate over realized rows as source of truth
- Clear selection based on invisibility
- Rebuild selection from rendered state
- Assume visibility implies existence

Visibility affects rendering only.

---

### PM-001-R3: Realization Must Never Mutate Selection

The following MUST NOT change selection:
- Scroll
- Re-realization
- Collapse / expand
- Filter application
- Sort application

Only **explicit user intent** may mutate selection.

---

### PM-001-R4: Selection Must Survive Invisibility

If an item is:
- Scrolled out
- Hidden
- Collapsed
- Filtered

Its selection state MUST remain intact unless explicitly changed by intent.

---

### PM-001-R5: No Derivation From Projection

Selection MUST NOT be:
- Derived from view projections
- Inferred from render nodes
- Reconstructed from geometry
- Synchronized from realized subsets

Projection consumes selection.  
Projection does not define selection.

---

## 7. Affected Systems

This pathology applies to:

- List
- ListView
- TreeView
- Grid / Table
- PropertyEditor
- Any virtualized or hierarchical collection

If a system can hide items without deleting them, **this SPEC applies**.

---

## 8. Forbidden Patterns (Explicit)

The following patterns are forbidden:

- “For each visible row, update selection…”
- “Clear selection when row is not realized…”
- “Recompute selected rows after scroll…”
- “Sync selection from rendered state…”
- “Selection index == render index”
- “Selection lives in row widget”

Any appearance of these patterns is an SPEC-PM-001 violation.

---

## 9. Relationship to Existing SPECs

This SPEC constrains and overrides assumptions in:

- SPEC-HAL-167 (Body and Row Projection)
- SPEC-HAL-172 (Tree Projection)
- SPEC-HAL-178 (Selection Semantics)

If any interpretation of those SPECs allows visibility-driven selection logic, **that interpretation is invalid**.

---

## 10. Enforcement Rule

If a future implementation:
- Loses selection after invisibility
- Breaks clicking after scroll
- Requires “re-sync after scroll”
- Treats realized rows as selection truth

The response is **not debugging**.

The response is:

> “We violated SPEC-PM-001.”

Fix the architecture. Do not patch symptoms.

---

## 11. Closure

This failure already happened.

It already cost time.

It already required re-explaining.

From this point on, reintroducing it is not an accident.

It is a process violation.
