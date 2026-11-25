# SPEC-PM-010: Incremental Cache Fragility in Ring-Buffered Virtualization

**Status:** Active
**Series:** SPEC-PM (Pathology & Mistake Prevention)
**Layer:** Meta-Architecture / Failure Law
**Owner:** Core Architecture
**Audience:** Engine, Widget, Interaction
**Applies To:** Console, EventLog, any widget backed by a ring buffer (SlidingWindow) with cached per-entry state

---

## 0. Pathology Name

**Steady-State Invalidation Blindness**

Also known as:
- "Cache works during growth, breaks at capacity"
- "Visual glitches only appear after running for a while"
- "Return to live has wrong positions"
- "New entries look fine, old entries drift"
- "Eviction counter doesn't match row count delta"
- "Review mode corrupts live state"

---

## 1. Summary of the Failure

This pathology occurs when **a cached per-entry state** (heights, layout data, shaping results) is invalidated incrementally based on **row count changes**, but the backing store is a **ring buffer that evicts at capacity**.

Before capacity: `line_count` grows → cache grows → correct.

At capacity: each push evicts one, adds one → `line_count` stays constant → delta detection says "nothing changed" → cache becomes stale because the FIRST entry in the cache corresponds to an entry that has been evicted thousands of pushes ago.

The cache is technically "in sync with line_count" while being completely wrong about which entries it describes.

Each "fix" for this pathology introduces a new variant:
- Track `push_count` instead — works until Review mode, where snapshot is frozen but live `push_count` keeps advancing
- Skip updates in Review mode — works until return to Live, where stale cache is now mixed with fresh live data
- Clear cache on mode transition — works until rapid scroll during transition exposes partial rebuild

Eventually the correct solution is reached: **abandon incremental invalidation**, rebuild fully on any change, or key cache entries by stable identity rather than position.

It must never be rediscovered.

---

## 2. Observable Symptoms

Any of the following are red flags:

1. Widget works correctly when data is below capacity, breaks after running long enough to fill the ring buffer
2. Visual artifacts appear gradually over minutes of continuous data arrival
3. "Works when I test it briefly, breaks overnight"
4. Return-to-Live (or similar mode transition) produces wrong positions for ~N frames
5. Cache invalidation relies on comparing a counter (`push_count`, `version`, `generation`) against a stored previous value
6. Eviction arithmetic uses formulas like `evicted = (cache_len + new_entries) - row_count`
7. View mode transitions require explicit cache resets in multiple handlers
8. "Fix" involves adding another invalidation trigger to an existing incremental path

If any of these occur, **this pathology is active**.

---

## 3. Root Cause (Architectural)

The failure is caused by **conflating position-delta with content-change** in cache invalidation.

Specifically:

- Cache indexes entries by position (`cache[0]`, `cache[1]`, ...)
- Ring buffer shifts positions on eviction (position 0 now refers to a different logical entry)
- Incremental invalidation detects "something changed" via a counter delta
- The counter delta alone does not convey which cache positions are now invalid
- Incremental logic attempts to reconstruct which positions to invalidate from counter arithmetic
- The reconstruction is fragile — any mode transition, rate change, or bounded-buffer edge case breaks it

This is **not a counter-bug**.

Counters are monotonic and correct.
Position deltas are arithmetically correct.

The failure occurs because **incremental invalidation assumes a relationship between positions and content that a ring buffer does not guarantee**.

---

## 4. Critical Clarification

> Tracking `push_count` does not fix this. Neither does `version`, `generation`, or `topology_stamp`.

Any counter alone is insufficient because:
- Counter increments do not tell you **which** positions changed
- Counter increments during one mode (e.g., Live) do not map to meaningful changes in another mode (e.g., Review on a frozen snapshot)
- Mode transitions create discontinuities the counter cannot express
- Return transitions face stale cache of unknown extent

The fundamental issue is that **positional cache + ring buffer + multi-mode** has no incrementally-correct invalidation strategy. Each special case requires another.

The correct solutions are:
1. Full rebuild on change (accept the cost)
2. Rebuild only for the visible slice (bound the cost)
3. Identity-keyed cache (cache by logical entry ID, not position — ring buffer doesn't affect it)

Incremental positional invalidation is not on the list.

---

## 5. Why the Bug Reappears

This pathology reappears because:

1. "Just invalidate what changed" feels obviously correct
2. Growth phase testing passes (incremental works while `row_count < capacity`)
3. Each fix for a new failure mode adds plausible logic to existing incremental path
4. The accumulated complexity appears to work until the next edge case
5. No written invariant forbids positional incremental invalidation with ring buffers
6. Engineers believe they can "just be careful with the arithmetic"
7. Review/snapshot/mode-transition patterns are discovered incrementally, each requiring a new special case
8. The correct solution (abandon incremental) feels wasteful and is deferred

Without explicit law, the same pathology is re-entered via a different counter scheme each time.

---

## 6. Hard Prevention Rules (Binding)

### PM-010-R1: No Positional Incremental Invalidation With Ring Buffers

If cached per-entry state is **indexed by position** and the backing store is a **ring buffer with eviction**, the cache MUST NOT be invalidated incrementally via count/counter deltas.

Permitted strategies:
- **Full rebuild on change**: clear and rebuild when any input changes
- **Visible-slice rebuild**: rebuild only for currently visible entries, treat others as estimated
- **Identity-keyed cache**: key by logical entry ID (monotonic, never reused), not position

Forbidden: any scheme that tracks `push_count_since_last_build`, `new_entries = push_count - last_push_count`, `evicted = cache_len + new - row_count`, or similar positional arithmetic.

---

### PM-010-R2: Visible-Slice Rebuild Is Default

For ring-buffered widgets, rebuild cost is bounded to the **visible slice** (typically 20-50 entries out of thousands). Full rebuild of the visible slice on every layout is cheap, bounded, and correct.

This is the default strategy. Incremental schemes require explicit justification tied to measured cost.

---

### PM-010-R3: Mode Transitions Invalidate Everything

For widgets with multiple view modes (Live, Review, snapshot, filter), any mode transition MUST be treated as a full cache invalidation.

Forbidden: "skip updates in mode X but preserve cache for return to mode Y". This is how stale cache leaks across mode boundaries.

---

### PM-010-R4: Cache Correctness Must Be Mode-Agnostic

Cache logic MUST NOT contain conditionals like `if view_mode == Live { update_cache() }`.

Either the cache is valid for all modes or it is rebuilt on mode entry. A cache that only updates in some modes is guaranteed to be stale in others.

---

### PM-010-R5: Identity-Keyed Cache When Incremental Is Required

If incremental caching is genuinely required (measured cost justifies complexity), entries MUST be keyed by logical identity (monotonic entry ID), not by position.

Under a ring buffer, position-0 refers to different entries over time. Identity does not.

---

## 7. Affected Systems

This pathology applies to:

- Console (ring-buffered entries + wrap heights)
- EventLog (ring-buffered entries + expansion heights)
- Any widget backed by `SlidingWindow` or equivalent ring buffer
- Any widget caching per-entry layout, shaping, or geometry
- Any widget with multiple view modes over the same underlying data

If a widget caches anything per-entry and evicts entries at capacity, **this SPEC applies**.

---

## 8. Forbidden Patterns (Explicit)

The following patterns are forbidden:

**Counter-delta invalidation:**
```
let new_entries = push_count - wrap_last_push_count;
let evicted = (cache.len() + new_entries) - line_count;
for _ in 0..evicted { cache.remove(0); }
```

**Mode-gated cache updates:**
```
if state.view_mode == Live {
    // update cache
}
```

**"Return transition" manual resets:**
```
// On return to Live:
state.wrap_last_push_count = 0;  // Reset so next layout rebuilds
```
(The existence of these resets indicates the incremental scheme is fragile.)

**Arithmetic eviction counting:**
Any formula that derives which cache positions to evict from counter deltas is an SPEC-PM-010 violation.

---

## 9. Diagnostic Invariants

To verify correct implementation:

**INV-CACHE-STEADY**: After the ring buffer reaches capacity, the widget behaves identically to when it was filling. No visual or interaction degradation appears after the capacity threshold.

**INV-CACHE-MODE-SWITCH**: Transitioning between view modes produces no visual artifact frames. The first frame in the new mode is as correct as any subsequent frame.

**INV-CACHE-LONG-RUNNING**: A widget running for hours with continuous data arrival shows no gradual degradation of visuals, hit-test accuracy, or scroll precision.

If these invariants fail over time, steady-state invalidation blindness is active.

---

## 10. Relationship to Existing SPECs

This SPEC constrains and overrides assumptions in:

- SPEC-HAL-118 (Virtualized ListView Contract) — any cache-backed height strategy
- SPEC-HAL-164 (TabularSubstrate) — per-row cached state
- SPEC-HAL-305 (Console Widget) — wrap height caching at capacity
- SPEC-HAL-312 (EventLog Widget) — expansion state caching

Cross-reference:
- **SPEC-PM-009** (Dual-Source Geometry Drift) — dual-source failures often emerge from attempts to patch PM-010 violations

---

## 11. Enforcement Rule

If a future implementation:
- Works in initial testing but glitches after minutes of use
- Requires manual cache resets in view-mode-transition handlers
- Contains eviction arithmetic tied to push/pop counters
- Has visual artifacts that grow with running time

The response is **not adding another invalidation trigger**.

The response is:

> "We violated SPEC-PM-010."

Replace incremental invalidation with visible-slice rebuild, full rebuild, or identity-keyed caching.

---

## 12. Closure

This failure already happened.

Multiple iterations of incremental invalidation for Console wrap heights each appeared correct and each failed at a different edge case — steady-state capacity, Review mode, return-to-Live, rapid scroll during transition.

The correct solution was not a better invalidation scheme. It was abandoning positional incremental invalidation entirely for ring-buffered widgets.

From this point on, reintroducing positional incremental invalidation over ring buffers is not an accident.

It is a process violation.
