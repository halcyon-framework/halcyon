SPEC-HAL-131
Virtualization, Projection, and Scroll Contract (Index)

Status: Superseded (split into SPEC-HAL-131A and SPEC-HAL-131B)

────────────────────────────────────────────────────────────────────────────────

This SPEC has been restructured into two companion documents:

SPEC-HAL-131A: Scroll Substrate & Input Arbitration
    - ScrollState formal object
    - InputGesture formal object
    - Scroll core invariants
    - Content extent contract
    - Scrollbar semantics
    - Resize behavior (table)
    - Input arbitration and priority (table)
    - Keyboard navigation mapping (table)
    - Nested scroll chaining
    - IME/Text safety
    - Forbidden scroll patterns

SPEC-HAL-131B: Realization & Virtualization
    - DatasetView formal object
    - RowId formal object
    - ViewEpoch formal object (table)
    - RealizationWindow formal object
    - Non-virtualized subsumption (formal equivalence)
    - Slot pool contract
    - Overscan policy
    - Variable item height
    - Dataset mutation handling (table)
    - Selection & focus invariants
    - Pagination invariants
    - Filtering & sorting semantics (table)
    - Drag & drop stability
    - Forbidden virtualization patterns

────────────────────────────────────────────────────────────────────────────────
Rationale for Split
────────────────────────────────────────────────────────────────────────────────

Scroll substrate is reusable for everything:
    - Text areas
    - Code editors
    - Image viewers
    - Any scrollable content

Virtualization is a consumer, not a foundation:
    - Virtualization depends on scroll state
    - Virtualization never produces scroll state
    - Virtualization is one of many possible content types

The split enforces this architectural boundary at the SPEC level.

────────────────────────────────────────────────────────────────────────────────
Layer Architecture
────────────────────────────────────────────────────────────────────────────────

    Layer A: Logical Model (Data & Time)     → SPEC-HAL-131B
    Layer B: Scroll Substrate (Pure Math)    → SPEC-HAL-131A
    Layer C: Realization & Virtualization    → SPEC-HAL-131B
    Layer D: Input Arbitration               → SPEC-HAL-131A

Data flows: A → B → C (one direction only)
Input flows: D → B (scroll intent only)

────────────────────────────────────────────────────────────────────────────────
