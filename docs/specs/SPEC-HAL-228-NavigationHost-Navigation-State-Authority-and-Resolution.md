# SPEC-HAL-228: NavigationHost — Navigation State Authority & Resolution

**Status:** Draft (Binding upon adoption)
**Layer:** Application Navigation / State Authority
**Scope:** Navigation state ownership and resolution
**Applies To:** Breadcrumbs, TabBar, future NavigationRail, navigation-aware content
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-207, SPEC-HAL-219, SPEC-HAL-220

---

## 0. Purpose

Define one explicit authority per navigation context responsible for navigation state, separating:
- navigation intent emission (controls)
- from navigation state ownership and resolution (application policy)

This SPEC exists to prevent:
- navigation controls drifting out of sync
- widgets guessing or caching navigation state
- implicit navigation "fixups" inside UI components
- double-activation and focus/selection desync bugs

The Breadcrumbs double-click bug is a direct consequence of this authority not existing.

---

## 1. Problem Statement (Observed Failure)

With SPEC-219 (TabBar) and SPEC-220 (Breadcrumbs):
- Controls emit navigation intents correctly
- Controls do not own navigation state (by design)

Observed failures:
- Breadcrumb visual focus can diverge from actual navigation state
- Re-navigation does not mutate breadcrumb path
- Multiple controls cannot converge on a single truth
- Widgets remain correct individually, but system behavior is wrong

This is not a widget bug.
This is a missing layer.

---

## 2. Conceptual Definition

NavigationHost is:
- the single source of truth for navigation state within its context
- non-visual
- non-widget
- intent-driven
- deterministic

It:
- receives navigation intents
- resolves them into a canonical navigation state
- exposes that state to navigation controls
- never renders UI
- never handles input directly

---

## 3. Architectural Positioning

| Layer | Responsibility |
|-------|----------------|
| Navigation Controls (219, 220) | Emit navigation intents |
| NavigationHost (228) | Own and resolve navigation state |
| Application | Provide routes, content, persistence |

Controls never talk to each other or hold/call a NavigationHost directly. They receive caller-projected state and emit through caller-provided intent sinks; application routing delivers the intent to the owning host.

---

## Invariant Class A — Authority

### INV-228-A1: Single Navigation Authority

Exactly one NavigationHost owns navigation state **per navigation context**.

This is not a global singleton rule. Multiple independent navigation contexts may coexist in one application, window, or screen. Each context owns exactly one NavigationHost and never shares mutable navigation state or history with another context.

---

### INV-228-A2: Controls Are Stateless

Breadcrumbs, TabBar, etc. must not cache or infer navigation state.

---

## Invariant Class B — Intent Resolution

### INV-228-B1: Explicit Intent Input

NavigationHost reacts only to explicit navigation intents.

---

### INV-228-B2: Deterministic Resolution

Candidate resolution is a pure function of the complete NavigationHost mechanics state plus the intent. The same history entries, history cursor, and intent produce the same candidate or the same unavailable/no-op result.

Guard execution is subsequent application policy. The same candidate produces the same final outcome only when the guard and its application context produce the same decision.

---

## Invariant Class C — State Exposure

### INV-228-C1: Push, Not Pull

Navigation state is pushed to controls explicitly. The caller reads one immutable `NavigationControlProjection` from the owning NavigationHost after initial binding and after every `Committed` result, then replaces the control's projected state. NavigationHost does not own a subscriber registry or broadcast channel.

The projection contains:

- the committed `path: Vec<NavigationEntry>`,
- `current: NavigationEntry`, derived from the last path entry,
- `can_navigate_up`,
- `can_history_back`,
- `can_history_forward`.

`Denied`, `Unavailable`, and `NoOp` leave committed state unchanged and therefore do not require a projection update.

---

### INV-228-C2: No Widget Querying

NavigationHost never inspects widgets or focus trees.

---

## Invariant Class D — Synchronization

### INV-228-D1: All Navigation Controls Are Projections

Breadcrumbs, TabBar, etc. are projections of NavigationHost state.

---

### INV-228-D2: No Partial Updates

Navigation state updates atomically. A `NavigationControlProjection` is one coherent snapshot of a single committed host state; callers must not assemble or push its fields independently.

---

## 4. Minimal Architecture Shape

NavigationIntent:
- NavigateTo(entry)
- NavigatePath(path)
- NavigateUp
- HistoryBack
- HistoryForward

NavigationState:
- committed resolved path (root to current, inclusive)
- current entry derived from the path

NavigationHost:
- root-local navigation history and cursor
- checked, non-reused mechanics generation covering history entries/cursor
- snapshot() -> owned NavigationMechanicsSnapshot | Retry
- resolve(snapshot, intent) -> ResolvedNavigationCandidate | Unavailable | NoOp
- prepare_commit(authorized_candidate, expected_generation) -> Result<PreparedNavigationCommit>
- commit_prepared(PreparedNavigationCommit) -> Committed { redirected } // non-fallible
- control_projection() -> immutable NavigationControlProjection

No widgets.
No layout.
No rendering.

---

## 5. Navigation Acts, Committed State, and History

Navigation and history are related but distinct:

- A **navigation intent** is an explicit request to act.
- A **candidate state** is the ephemeral deterministic result of resolving that intent.
- A **committed navigation state** is the stable result after guard acceptance.
- **Navigation history** is the root-local trail of committed navigation states through which a cursor may move.

History is operational state, not an append-only audit journal. New navigation after moving backward truncates the abandoned forward branch. Auditing, analytics, and denied-intent journals are separate application concerns.

Navigation outcomes are distinct:

- **Committed { redirected }:** a final guard-accepted state change was recorded; `redirected` states whether `ReplaceWith` changed the executed intent.
- **Denied:** a candidate existed and the application guard rejected it.
- **Unavailable:** the requested traversal has no structural or historical target.
- **NoOp:** deterministic resolution produced the already-current committed state.

`Unavailable` and `NoOp` are resolved before guard invocation because neither represents an executable transition.

### INV-228-H0: Candidate Resolution Is Versioned and Commit Is Prepared

`NavigationHost::snapshot()` returns an owned, immutable mechanics snapshot
containing the complete cursor-selected state/history required for resolution and
the host's checked, non-reused `NavigationMechanicsGeneration`. Snapshot creation
is a linearizable bounded read of the host record; a commit-in-progress marker or
changed generation causes an internal retry, and exhaustion of the bounded read
attempts returns `Retry` without a snapshot. It cannot combine entries, cursor,
and generation from different commits, and it holds no lock or live reference
after the owned copy is complete. Resolving an intent or a SPEC-HAL-231
replacement against that snapshot performs no mutation.
Every executable result is an opaque `ResolvedNavigationCandidate` bound to the
host identity, snapshot generation, requested/evaluated intents, exact `from` and
`to` states, and the complete prospective history/cursor mutation. Candidate
values are neither committed state nor authority to commit.

After SPEC-HAL-231 returns a one-shot authorization for the final candidate,
`prepare_commit` validates that candidate and authorization identities match and
that the current host generation still equals the candidate generation. It also
checks the next generation before mutation and returns a single-use
`PreparedNavigationCommit` containing the complete final history/cursor write.
The prepared value holds a non-cloneable exclusive lease on that host's mechanics
mutation lane and generation until commit or drop. A second preparation or any
direct mechanics mutation while the lease is live returns `Busy/Retry` without
mutation; ordinary immutable snapshots may still copy the pre-commit state, but
their old generation cannot later pass preparation after this lease commits.
A stale generation, busy lane, exhausted generation lane, wrong host, wrong
candidate, or consumed authorization rejects preparation without mutation.
Commit consumes the lease and prepared value, applies exactly that write, and
advances the generation once as a non-fallible step. Dropping a candidate or
authorization changes nothing; dropping a prepared commit releases only its
lease and changes no history/cursor state.

Guard callbacks, application failure policy, routing, surface preparation, and
focus work are outside NavigationHost mechanics. A compound caller may hold the
prepared host commit while it prepares those other authorities, but it must not
commit the host until every fallible participant has prepared successfully.
`Denied`, `Unavailable`, `NoOp`, failed guard execution, stale revalidation, and
dropped preparation leave history, cursor, and mechanics generation unchanged.

### INV-228-H1: History Contains Committed States Only

History entries contain only fully resolved, guard-accepted navigation state. Candidate, denied, loading, animated, focused, rendered, partially mounted, or otherwise transitional state must never enter navigation history.

### INV-228-H2: Current State Has One Authority

The current navigation state is the history entry addressed by the history cursor. NavigationHost must not maintain a second independently mutable `current` state that can diverge from history.

Conceptual shape:

```text
NavigationHistory {
    entries: Vec<HistoryEntry>,
    cursor: usize,
    mechanics_generation: NavigationMechanicsGeneration,
}

HistoryEntry {
    state: NavigationState,
}

NavigationState {
    path: Vec<NavigationEntry>,
}
```

On creation, a NavigationHost initializes its history with exactly one committed root state and points the cursor at that entry. A live host therefore never has an empty history or a cursor without a corresponding entry.

`HistoryEntry` is a committed semantic state, not an intermediate-state snapshot. Until a deterministic route graph can reconstruct a complete path, the full committed resolved path is retained for exact restoration.

### INV-228-H3: History Is Context-Local

Every NavigationHost owns an independent history. Histories are never merged, broadcast, inherited, or traversed across navigation-context boundaries.

### INV-228-H4: Structural Candidate Resolution Is Explicit

From the cursor-selected committed path:

- `NavigateTo(current_entry)` resolves to `NoOp`.
- `NavigateTo(entry)` matching an existing ancestor truncates the candidate path through the nearest matching ancestor.
- `NavigateTo(entry)` not already present as current/ancestor appends the entry to the candidate path.
- `NavigatePath(path)` accepts one explicit complete root-to-current path as its candidate. The supplied path must be non-empty, begin with this host's immutable root entry, and contain no duplicate entry; its last element becomes the candidate current entry. An identical path resolves to `NoOp`; an invalid or foreign-root path resolves to `Unavailable`. NavigationHost never splices the supplied path into the existing path.
- `NavigateUp` removes exactly one trailing path entry; at the root it resolves to `Unavailable`.

These rules resolve only the candidate path. They do not mutate committed history until guard acceptance.

### INV-228-H5: Ordinary Navigation Creates a History Entry

An accepted `NavigateTo`, `NavigatePath`, or `NavigateUp` that changes the resolved state:

1. resolves a versioned candidate from one immutable current mechanics snapshot,
2. passes that exact candidate through the application guard/replacement chain,
3. revalidates its generation and prepares the final history/cursor write,
4. on non-fallible prepared commit, truncates entries after the current history cursor,
5. appends exactly one final committed state and advances the cursor to it.

Denied, unavailable, and no-op intents do not alter the entries or cursor. If a guard replaces the intent, history mutation follows only the final accepted replacement intent; replaced intermediate candidates never enter history.

### INV-228-H6: History Traversal Moves the Cursor

`HistoryBack` and `HistoryForward` select the adjacent retained committed state and move the cursor only after guard acceptance. They do not append duplicate history entries.

At the oldest or newest entry, the corresponding traversal is explicitly unavailable and causes no mutation. Projection exposes `can_history_back` and `can_history_forward`; controls must not infer availability from path length.

### INV-228-H7: Structural Retreat Is Not History Back

`NavigateUp` moves to a structural ancestor and, when accepted, creates a new history entry. `HistoryBack` moves to the previously committed state. These are separate operations.

The legacy name `NavigateBack`, whose current implementation pops one path element, is semantically `NavigateUp`. It must not be used to represent chronological history traversal and must be deprecated or migrated without silently changing meaning.

### INV-228-H8: Navigation Entries Are Semantic

A `NavigationEntry` is an immutable, cloneable, comparable value identifying one restorable semantic destination, including stable route identity and every application-defined parameter or restoration key required to resolve it. Equality used by ancestor matching covers that complete restorable identity, not route identity alone. It must not contain widget instances, focus state, compositor surface IDs, animation state, or renderer data.

---

## Runtime Status (0.176.0-beta)

The live `NavigationHost` implements this history contract: it stores complete
semantic `NavigationEntry` paths in cursor-addressed history, derives current
from the selected path, resolves validated complete paths plus independent
`NavigateUp`/`HistoryBack`/`HistoryForward` intents, classifies unavailable and
no-op outcomes before guard invocation, and exposes the canonical control
projection. Candidate authorization is bound to host identity, mechanics
generation, candidate identity, requested/final intent, and optional guard-context
generation. `prepare_commit` acquires the exclusive mechanics lease and produces
the non-fallible history/cursor write; stale, foreign, consumed, busy, and dropped
preparations mutate nothing.

`NavigationSurfaceOrchestrator` no longer depends on the direct `decide`/`apply`
façade. Standalone orchestrator dispatch lowers through snapshot/candidate
authorization plus prepared host and surface commits, while registered-root
transactions use the application-owned SPEC-HAL-237 coordinator. The deprecated
`NavigateBack` name and direct façade remain source compatibility only for
standalone hosts that were never application-admitted or frame-bound; they
retain structural-up meaning and do not represent chronological history.
Permanent application participation and frame-binding participation make direct
execution fail closed for every compound source. No compound integration may
treat the compatibility façade as the SPEC-HAL-237 transaction.

---

## 6. Non-Goals

SPEC-HAL-228 does NOT define:
- routing tables
- URL handling
- cross-session history persistence
- animations
- content mounting
- deep linking

Those are future layers.

---

## 7. Acceptance Criteria

- Breadcrumbs update when navigation changes
- TabBar selection stays in sync
- Double-activation does not desync visuals
- Removing NavigationHost reintroduces the observed bug
- Widgets remain unchanged
- Multiple NavigationHosts may coexist without shared state or history
- Accepted ordinary navigation appends one committed history entry and truncates any abandoned forward branch
- Available HistoryBack/HistoryForward transitions move only the history cursor and remain guard-gated
- NavigateUp remains structurally distinct from HistoryBack
- NavigatePath validates and commits one complete same-root path without splicing it into the prior path
- Denied, unavailable, no-op, and transitional states never enter history
- Initial binding and every committed change expose one coherent availability-bearing `NavigationControlProjection`
- Stale candidate generations and failed/dropped preparations mutate neither
  history nor cursor; an authorized prepared commit advances the mechanics
  generation exactly once and cannot fail
- A live prepared commit exclusively leases the host mechanics mutation lane;
  competing preparation/mutation returns busy and cannot invalidate its commit
