# SPEC-HAL-231: Application Navigation Guard and Resolution Override

**Subtitle:** Pre-Execution Gate for Navigation Intents (Scenario-Owned Rules)
**Status:** Draft (Binding upon adoption)
**Layer:** Application Semantics / Navigation Policy Seam
**Audience:** NavigationHost, App layer, any navigation projection or command source
**Depends On:** SPEC-HAL-002, SPEC-HAL-228, SPEC-HAL-229, SPEC-HAL-220
**Explicit Non-Dependencies:** Hierarchy (SPEC-HAL-230), Selection models, Focus, Widgets, Rendering, Layout

---

## 0. Purpose

Define a single, explicit seam where the application may allow, deny, or override navigation execution, including structural and history-navigation restrictions, without contaminating NavigationHost with scenario rules.

This SPEC answers only:

"May this resolved navigation transition execute right now, under application rules?"

---

## 1. Binding Definitions (Frozen)

### NavigationIntent

An explicit request to navigate within one identified navigation context, including `NavigateTo`, validated complete-path `NavigatePath`, `NavigateUp`, `HistoryBack`, and `HistoryForward`. Produced by projections/commands; never inferred.

### Resolution

The deterministic candidate transition computed by NavigationHost from the current committed navigation state/history cursor + intent.

### Guard

An application-owned decision function consulted before executing a resolved transition.

---

## 2. Authority Boundaries

### INV-NG-231-01: NavigationHost Owns Mechanics, Not Permission

NavigationHost MUST:
- own navigation state and deterministic resolution,
- compute candidate transitions from intents,
- execute transitions only when allowed.

NavigationHost MUST NOT:
- decide allow/deny based on application semantics,
- embed scenario rules (wizard locks, irreversible steps, security boundaries, expiry),
- infer "back should be blocked" heuristically.

---

### INV-NG-231-02: Application Owns Scenario Rules

All restrictions like "Back is not allowed here" are application rules and MUST be decided by the application guard, not by NavigationHost.

---

### INV-NG-231-03: Guard Is Consultative, Not Executing

The guard MUST NOT execute navigation or mutate NavigationHost directly. It
returns a decision; NavigationHost performs only its authorized mechanics
commit, while any surface/focus/binding effects execute through their owning
authorities in the prepared transaction defined by SPEC-HAL-237.

---

## 3. Guard Contract

The guard is invoked only with fully resolved candidate transitions.

### 3.1 Inputs

The guard MUST receive:
- **requested_intent:** the initiating NavigationIntent, invariant across the complete replacement chain
- **evaluated_intent:** the current NavigationIntent that produced `to`; initially equal to `requested_intent` and replaced after `ReplaceWith`
- **from:** current resolved navigation state (opaque to guard except as data)
- **to:** the candidate resolved target state produced by NavigationHost

The guard MAY also receive an opaque context payload supplied by the application (domain state snapshot, flow state machine token, etc.). This payload is application-defined and outside engine semantics.

### 3.2 Outputs

The guard MUST return exactly one of:
- **Allow**
- **Deny**
- **ReplaceWith(new_intent)** (explicit override)

No other outcomes exist.

---

### INV-NG-231-04: Deny Stops Execution

If the guard returns Deny, NavigationHost MUST NOT execute the transition and MUST NOT mutate committed navigation state, history entries, or the history cursor.

---

### INV-NG-231-05: Allow Does Not Bypass Determinism

Allow means "execute the already-resolved candidate." NavigationHost must execute exactly what it resolved.

---

### INV-NG-231-06: ReplaceWith Is Explicit and Re-enters Resolution

ReplaceWith MUST:
- substitute the intent explicitly,
- re-run NavigationHost resolution deterministically,
- return `Unavailable` or `NoOp` immediately if replacement resolution yields either non-executable outcome,
- otherwise re-invoke the guard with the invariant `requested_intent` and the replacement as `evaluated_intent`,
- terminate with Committed (marked redirected), Denied, Unavailable, or NoOp.

One candidate-authorization attempt permits at most eight total guard evaluations, matching the existing runtime safety cap. If the eighth evaluation returns `ReplaceWith`, the attempt terminates as `Denied` with no authorization, committed-state, history-entry, or cursor mutation. This bound is recursion protection against a non-terminating guard, not application navigation policy.

History mutation follows only the final accepted replacement intent. Replaced intermediate candidates never enter history or move the cursor.

### INV-NG-231-14: Guard Evaluation Produces One-Shot Authorization

Guard evaluation runs against the immutable, versioned
`ResolvedNavigationCandidate` defined by SPEC-HAL-228. `Allow` produces a sealed,
single-use `NavigationGuardAuthorization` bound to that exact host, mechanics
generation, candidate identity, requested intent, and final evaluated intent.
`ReplaceWith` resolves a new candidate from the same immutable host snapshot and
re-enters the guard; it does not authorize the replaced candidate. `Deny`, an
unhandled or fail-closed guard execution failure, and exhaustion of the
replacement bound produce no authorization. If the application integration's
explicit failure policy is fail-open, that outside-token policy converts the
failure to `Allow` for this exact candidate and produces the same sealed one-shot
authorization; the engine never performs that conversion by default.

The authorization is permission for that exact candidate, not durable policy
state and not authority to mutate the host. If guard-relevant application state
may change independently before commit, the application integration must also
bind a checked `NavigationGuardContextGeneration` to the authorization and expose
a non-blocking generation read for guarded revalidation. An integration may omit
that generation only when issuance itself is the application's irrevocable
one-shot permission for the candidate. Revalidation never invokes the guard or
other application/user code.

---

## 4. Invocation Rules

### INV-NG-231-07: Pre-Execution Mandatory

The guard MUST be consulted for every navigation execution, including:
- direct navigation intents,
- NavigateUp and HistoryBack/HistoryForward,
- programmatic navigation requests.

No execution path may bypass the guard.

The guard and its application failure policy execute before any SPEC-HAL-236
`DomainTransitionGuardToken` is acquired. After authorization, application
orchestration may acquire that token and revalidate only immutable identities and
checked host/routing/guard-context generations. A mismatch drops the
authorization, releases the token without mutation, and restarts candidate
resolution and guard evaluation on a later scheduling turn. The guard must never
be called, awaited, retried, or failed while the domain token is held.

---

### INV-NG-231-08: No Implicit Defaults

If no guard is installed, the system MUST behave as if the guard always returns Allow. (The absence of the seam must not silently block navigation.)

The no-guard path creates the same candidate-bound one-shot authorization during
the outside-token candidate phase; it does not bypass generation revalidation or
prepared commit.

---

## 5. Structural and History Navigation Restriction Semantics

`NavigateUp`, `HistoryBack`, and `HistoryForward` restrictions are implemented only by returning Deny (or ReplaceWith) for the corresponding intent/candidate pair.

### INV-NG-231-09: No "Back Disabled" State in NavigationHost

NavigationHost MUST NOT store application-policy "back disabled" flags or counters. It owns ordinary root-local history entries/cursor and deterministically derives `can_history_back`, `can_history_forward`, and `can_navigate_up`; the guard may still deny an otherwise available candidate per scenario.

### INV-NG-231-11: Unavailable and NoOp Resolution Are Not Denials

When no adjacent history entry or structural ancestor exists, NavigationHost returns `Unavailable` without invoking the guard and without mutation. When deterministic resolution produces the already-current committed state, it returns `NoOp` under the same rules. Neither outcome has an executable candidate transition to authorize. Controls project derived availability but do not own it.

### INV-NG-231-12: Forward-Branch Mutation Occurs Only on Commit

An ordinary navigation candidate must not truncate the retained forward branch until the guard allows the final candidate. Denial, unavailable traversal, no-op resolution, or replacement-chain intermediates leave history entries and cursor unchanged.

### INV-NG-231-13: Guards Are Context-Local

A NavigationHost guard evaluates transitions only within its owning navigation context. `ReplaceWith` may replace the local navigation intent but must not activate, delegate to, dismiss, or mutate another navigation root. Cross-root priming/delegation belongs to explicit interaction-domain navigation routing under SPEC-HAL-237; NavigationRootRegistry receives only the final explicit root target.

---

## Runtime Status (0.176.0-beta)

The live `NavigationHost` classifies `Unavailable` and `NoOp` before guard
invocation, preserves the initiating `requested_intent` through replacement
evaluation, caps replacement chains at eight evaluations, and produces sealed
one-shot authorization for an exact versioned candidate. Optional application
guard-context generation is revalidated with host generation during preparation,
and the resulting prepared host commit is non-fallible and exclusively leased.

The surface orchestrator is migrated to candidate authorization plus prepared
host and surface commit. Registered-root execution uses the application-owned
SPEC-HAL-237 transaction, and frame-bound standalone execution uses the
equivalent coordinator transaction without router generations. Permanent
application participation and frame-binding participation make direct host
execution fail closed for every compound source. The direct `decide`/`apply`
façade remains only as a standalone-host compatibility boundary for hosts that
were never application-admitted or frame-bound; it is not a compound commit
path.

---

## 6. Failure Semantics

### INV-NG-231-10: Fail Closed vs Fail Open Is Not Engine-Owned

Engine behavior on guard execution failure (panic, returned integration error, or guard-service unavailability) MUST be explicitly defined by the application integration. This is distinct from the navigation outcome `Unavailable`. The engine MUST NOT invent fallback semantics beyond "no guard installed = Allow".

(If you want "deny on guard failure," that is an application wiring choice, not an engine default.)

Regardless of the application's reported failure outcome, the guard failure and
failure-policy evaluation themselves occur before guarded transaction
preparation and cannot leave a domain token held, a prepared surface live, or
any host/history/routing/focus authority mutated. If explicit fail-open policy
produces authorization, only the later fully prepared commit may mutate those
authorities.

---

## 7. Explicit Non-Goals

This SPEC does NOT define:
- permissions/roles/groups
- identity/auth
- UI affordances (disabled buttons)
- breadcrumbs (history projection already defined)
- hierarchy/taxonomy
- wizard frameworks
- auditing/logging

---

## 8. Summary (Binding)

- NavigationHost remains deterministic and policy-free.
- The application owns scenario meaning and may deny NavigateUp, HistoryBack/HistoryForward, or any other available transition at execution time.
- History availability and forward-branch mutation remain deterministic NavigationHost mechanics, not application policy.
- Guards are context-local and cannot perform cross-root activation or delegation.
- The guard seam is mandatory for all navigation executions and cannot be bypassed.
- ReplaceWith enables explicit redirection without embedding rules into NavigationHost.
- Guard execution occurs outside the interaction-domain transition token and
  yields only one-shot, candidate-bound authorization for later guarded
  revalidation and preparation.
