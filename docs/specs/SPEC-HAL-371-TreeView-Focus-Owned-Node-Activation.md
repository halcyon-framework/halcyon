# SPEC-HAL-371: TreeView Focus-Owned Node Activation

**Author:** JC. Technical contract selected through delegated independent architectural review.
**Status:** ACCEPTED / ACTIVE / BINDING — narrow TreeView node activation contract.
**Date:** 2026-11-05.
**Layer:** TreeView widget interaction and consumer output.
**Source revision:** `b028a38df94f4564006fda7b0147bb08f862e5cd` (`halcyon 0.230.3-beta`).
**Implementation:** Implemented and source/input-route verified in the owner-qualified unit recorded in section 10.4. Absent at the original pinned before-image. Physical GUI acceptance remains unobserved.
**Scope:** One TreeView-owned callback carrying a stable leaf `node_id` after a qualified native row click or focused, routed Enter.

## Historical drafting scope

The following records the original inactive drafting unit. Current adoption and implementation authority are recorded in section 10; historical Draft statements below do not override that decision.

- Objective: specify the narrow TreeView activation output needed by a genuine one-column, headerless source-model tree; no application input workaround.
- Standing objective: preserve existing TreeView composition and navigation, ListView, Workbench, retained publication and PBR while enabling a separately authorized consumer integration.
- Roles: JC retains final product and constitutional authority; The delegated architectural reviewer selects and independently reviews this technical contract; the original drafting scope was this inactive document only. The application retains its source mapping and UI ownership.
- Authority and evidence: the affected contract/source spans were checked at the revision above. HAL-309 governs the distinction between proposed authority and implemented evidence.
- Governing status: HAL-002 is supreme and HAL-002A permanent; HAL-328 is Binding, HAL-176 Design / Binding, HAL-347 and HAL-348 Active / Binding, HAL-193 LOCKED / Binding. HAL-262 and HAL-337 remain Draft (Binding upon adoption). HAL-369 remains Draft / Proposed, INACTIVE, NOT ACCEPTED, NOT BINDING.
- Authority change: none. This is a new candidate, not adoption of this document, HAL-262, HAL-337, HAL-369 or their unrelated provisions.
- Verified current state: TreeView owns one shared core and one ordered-selection state, has no activation callback and no Enter arm; its existing Target-phase key route and body-owned pointer route are the proposed insertion owners.
- Owned file: only this SPEC. No code, test, example, other SPEC, index, manifest, renderer, consumer or asset change.
- Invariants and forbidden behavior: defined in sections 3–8; framework focus, routing, phase and identity sovereignty remain unchanged.
- Reuse: the existing TreeViewCore, HTS identity/projection, OrderedSelectionState, TreeViewBody hit-testing, EventRouter and Screen deferred-action boundary. No second router, focus owner, substrate callback store or listener system.
- External boundary: a consumer maps `node_id` to its own current domain entry and decides whether to act; this output proves no model/source eligibility.
- Verification: documentary source concordance, complete readback, status/link/heading and exact owned-diff checks only. Section 9 specifies future implementation proof; no runtime tests are added or run here.
- Completeness: one complete inactive contract for independent review. Adoption and implementation require separate authorization.
- Out of scope: application implementation, raw Screen key interception, focus-manager exposure, new input modes, activation-timing policy, ListView API changes, geometry, PBR, asset processing and native visual acceptance.

## 1. Purpose and authority boundary

A TreeView consumer needs an explicit activation output distinct from selection, row-axis focus, widget focus and disclosure. The framework owns the input qualification; the consumer receives only a stable identity and applies its domain action later.

The immediate consumer requirement is a real one-column TreeView with an omitted header, current source-model leaves and future grouping parents. The number of leaves, provider names and source formats are not Halcyon policy. Header omission is already admitted by [HAL-348](SPEC-HAL-348-Optional-Tabular-Header-Band.md); this candidate does not change it.

The MUST/MUST NOT clauses below are binding within the exact adoption in section 10. The original draft and source-evidence provenance remain preserved. Adoption is not implementation proof, consumer integration or native acceptance.

## 2. Pre-implementation source evidence

These are implemented facts at the pinned revision, not authority inferred from comments or Draft documents:

| Existing owner | Verified pinned before-image and insertion boundary |
| --- | --- |
| [TreeView](../../src/widgets/tree_view/tree_view.rs), lines 213–283 | One `Rc<RefCell<TreeViewCore>>`; construction initializes exactly one `OrderedSelectionState<u64>` and `focused: false`. There is no activation callback field or setter. |
| [TreeViewCore](../../src/widgets/tree_view/body.rs), lines 125–160 | Shared selection, range-drag and actual composite-focus projection live in this one core. The body shares it; row-axis focus is not another WidgetTree focus owner. |
| TreeView, lines 319–325 and 369–410 | Existing header visibility, source topology and expansion APIs. This contract adds no column/row geometry API. |
| TreeView, lines 772–779, 807–1045 and 1291–1297 | Selection queries expose row state only. `handle_keyboard` has no Enter arm; `handle_event` invokes it for routed Key events only in Target phase. |
| TreeView, lines 1542–1563 | TreeView is focusable; `on_focus` / `on_blur` set the existing core focus projection. No new focus manager is needed. |
| TreeViewBody, lines 1019–1098 | Native body Target dispatch excludes disclosure children and separators. Ordinary selection commits on primary Down; Shift uses the existing range reducer and drag capture. |
| TreeViewBody, lines 1150–1240 | Up currently finalizes a range gesture; parent DoubleClick enqueues disclosure; editable non-tree-cell behavior remains a separate existing owner. |
| [NodeMeta](../../src/engines/hierarchical/topology.rs), lines 30–47 | `node_id: u64` is caller-assigned stable identity; `has_children` declares the structural parent/leaf distinction. Physical and visible indices are not that identity. |
| [KeyEvent](../../src/core/event/mod.rs), lines 592–600 | The actual routed event retains `key_code`, `is_down`, `is_repeat`, modifiers and physical identity. No reconstructed external event is required. |
| [App input](../../src/core/app.rs), lines 6538–6645, and [EventRouter](../../src/core/event_router.rs), lines 4488–4568 and 6586–6658 | Existing engine-action precedence is retained. TreeView currently inherits false activation/capture declarations; an unconsumed key reaches the focused widget's Target route. PointerDriven outside Capture returns before that dispatch. Existing framework key translation remains unchanged. |
| App, lines 8431–8455 | `Screen::handle_input` precedes widget routing; `Screen::flush_pending_actions` drains pending actions after routing, before scene construction. Activation belongs to the widget route and the latter deferred boundary, not a new Screen Enter handler. |

HAL-262's older implementation inventory is not silently treated as a current source description. Live source already uses CMS/SharedHts and a topology-intent funnel; that fact does not adopt HAL-337. The new output must remain at the widget boundary in either case.

## 3. Public contract and ownership

### 3.1 Exact accepted signature

The sole new public method is this method signature, not a standalone program:

```rust
pub fn set_activation_callback<F>(&mut self, callback: F)
where
    F: FnMut(u64) + 'static;
```

The argument passed to the callback is the current canonical `NodeMeta.node_id`. It is never a visible row index, realization slot, widget ID, label or source-provider handle.

There is at most one installed callback per TreeView instance. Calling the setter replaces the previous callback; registration or replacement does not activate anything. No callback is installed by default. This method admits no listener registry, event bus, subscription token, additional public focus API or callback in HTS/CMS.

No callback registered preserves all preexisting TreeView selection, keyboard, pointer, disclosure, layout and painting behavior, including the existing unhandled Enter result. Installing a callback changes only the qualified activation output specified here.

### 3.2 One interaction owner

TreeViewCore remains the single storage owner for TreeView interaction state. Any bounded qualification of an unfinished pointer press belongs to that same owner; it is gesture lineage, not a second selection or focused-row state. It may not retain an unbounded input history or callback queue.

`OrderedSelectionState::focus_key` remains the sole row-axis navigation cursor. `core.focused` remains the projection established by actual framework focus lifecycle. Neither value grants keyboard delivery independently of EventRouter, the current window/domain, Capture or application interaction policy.

The body remains the row hit-geometry owner. Consumers MUST NOT reproduce row-coordinate, scroll-offset, clipping, separator, disclosure or virtualization calculations.

## 4. Pointer activation

A pointer activation may deliver one leaf `node_id` only after a qualified original primary Down and matching Up. All of these conditions are required:

1. The original Down was an unmodified single click delivered through the native TreeViewBody row route to a real leaf (`has_children == false`). Existing press-time selection reduction is unchanged.
2. Up belongs to that original admitted press and the same owning TreeView/body, and currently resolves to the **same stable node_id**. A different row at the same visible index does not qualify.
3. The current original node association is still live in the same source/topology ownership. Source replacement, topology replacement, owner removal/replacement or reuse of an old numeric ID cannot complete an old press.
4. The gesture was not canceled, converted into range/drag interaction or directed to a disclosure child, separator, scrollbar or another control. An outside or otherwise invalid terminal Up cannot activate.
5. No Shift-range drag or Ctrl/Cmd multi-toggle is involved; the click is unmodified. Existing modifier selection paths remain independent and emit no activation.
6. A callback is installed and every shared core/substrate borrow needed to qualify the output has ended before callback invocation.

An eligible release emits the identity once. An unmatched or duplicate Up emits nothing. A second distinct valid click is a distinct action and may emit once; DoubleClick metadata is not an additional activation and must not duplicate either click's output.

The existing canonical router encodes the second and third physical Down as `DoubleClick` / `TripleClick` rather than also sending an ordinary Down. On this route that event is the original press delivery: an eligible leaf may arm one press and still requires its corresponding Up. It emits nothing merely because of the multi-click kind. An already-held press receiving another Down-kind is canceled, not rearmed. This source-concordant qualification changes no parent disclosure or existing editable-cell behavior and reconstructs no raw input.

**Binding pointer-capture qualification (2026-11-05):** For a callback-installed, unmodified primary leaf Down, TreeViewBody MUST call the existing `EventContext::capture_pointer(PointerButton::Primary)`. The existing EventRouter drops ordinary uncaptured Up; capture is required to receive the actual matching terminal event, including outside release. At most one `ActivationPress` lives in the existing TreeViewCore and records the native PointerId, stable node_id, source/topology revision witness and originating owning body identity. No synthetic pointer session ID, extra selection/focus state or public API is introduced. Without a callback, ordinary clicks retain their prior noncapture behavior; Shift-drag capture is unchanged.

Completion requires the same active pointer/button/owner, unchanged topology and live original visible leaf, current in-row geometry and no cancellation. Clear the press terminally on Move outside the original row/bounds, DragStart, second Down/other button, focus/capture loss, owner removal, topology replacement or another observed invalidating gesture/lifecycle. Release capture through the existing owner where still owned. Valid Up clears the press and releases capture before callback delivery; framework teardown remains authoritative. No late Up can rearm or complete a canceled press. Parents, disclosure controls, separators, scrollbars and modified presses never take activation capture.

**Checked descendant retirement (delegated technical review, 2026-11-05):** When TreeView loses focus with a HAL371 press armed, it first removes that press and requests one crate-private EventContext effect containing the original `(PointerId, body_widget_id)`. After the lifecycle callback's widget/core borrows end, EventRouter must resolve both real owners in the current WidgetTree, require the requesting TreeView to be a strict ancestor of the requested body and require that exact body to be the current capture holder for that pointer in the same interaction domain. Only then it calls the existing `cancel_pointer_session_with_tree` once with `SystemCancel`. That existing engine operation revokes capture, settles the gesture/cursor and notifies the actual body through `on_pointer_capture_cancel`; the body clears its press idempotently. An absent/mismatched owner, non-descendant, different current capture holder or absent HAL371 press cannot cancel another capture. Settlement occurs synchronously before the new focused widget's on_focus, never on a timer or later Up. No parent impersonation, fabricated EventContext or public focus/capture API is admitted.

Hover, focus gain, selection API calls, scroll and native parent expansion/collapse never generate activation. A parent row or its disclosure caret never activates a model. DoubleClick on a grouping parent continues its native disclosure behavior without activation. Existing editable-cell behavior is not reinterpreted as this output.

## 5. Focus-owned Enter activation

The new Enter handling belongs in `TreeView::handle_keyboard`, reached through the existing `TreeView::handle_event` Target-phase key route. It does not belong in `Screen::handle_input` or a consumer raw-key handler.

An Enter activation is admissible only when **all** of the following hold:

1. The actual routed event is `KeyCode::Enter`, with `is_down && !is_repeat`.
2. The event is delivered in Target phase to the currently framework-focused TreeView in its real active window/domain, under the existing input priority and Capture rules.
3. The existing `core.focused` is true; row selection alone is insufficient.
4. The current existing `OrderedSelectionState::focus_key` resolves to a currently visible leaf in the same current HTS revision and owning association. A collapsed-away, removed or replaced node is not eligible.
5. A callback is installed and the original node association is verified before emission, with all qualification borrows released before the call.

One admissible fresh KeyDown emits one stable node_id. KeyUp and repeat emit nothing. Selection membership, a selected but unfocused row, range selection, a foreign/background window, a focused TextField or a hidden collapsed row cannot be used as activation authority. Range selection itself is not an activation source; no callback iterates or activates the selected range. The output resolves only the existing focused leaf, never `selected_keys` as a substitute.

[HAL-328](SPEC-HAL-328-Keyboard-Authority-Focus-and-Capture.md)'s application-policy qualification and [HAL-176](SPEC-HAL-176-Explicit-Keyboard-Authority-Modes.md) remain unchanged. In particular, PointerDriven outside actual Capture supplies **no automatic focused-widget Enter delivery**, and this API must not manufacture it, enter Capture automatically, expose focus state to the app or add a new input mode. A retained `focus_key` or `core.focused` does not bypass that boundary.

Existing engine action priority, framework key translation and Capture/scope lifecycle remain their owners. This document does not grant a second activation through `on_keyboard_activate`, broaden Space or raw character-key semantics, or replace the engine's Enter/Capture decision. If the existing route consumes an event before TreeView, the callback receives nothing from that event. Future proof must use the real routed path, not invoke `handle_keyboard` directly as evidence of public delivery.

## 6. Callback delivery and consumer boundary

Before invoking the callback, the widget MUST release every TreeViewCore, HTS and related shared `RefCell` borrow held for the event. The callback must be able to read the current TreeView/core state without a reentrancy borrow panic. No shared guard crosses the event/deferred boundary.

The receiver may only enqueue the pure `u64` identity into the consumer's explicit pending action. The consumer drains that action at its existing lawful deferred boundary, such as `Screen::flush_pending_actions`, after event routing. No synchronous AppState, WidgetTree, source, layout, navigation, focus or GPU mutation is permitted from the callback. No nested routing or fabricated key/pointer replay is admitted.

Qualification establishes a live leaf interaction, not semantic source eligibility. The consumer owns the mapping from node_id to its current domain entry, revalidates that association before its deferred action, and may ignore arbitrary identities. Provider/manufacturer groups remain consumer topology; Halcyon does not interpret application models, asset containers, catalogs or source-provider semantics. There is no automatic playback or activation when WidgetTree, topology or selection changes.

## 7. Invariants and forbidden behaviors

| Invariant | Required behavior | Paired prohibition |
| --- | --- | --- |
| **INV-371-IDENTITY** | Output is the current canonical stable leaf node_id. | **FORBID-371-IDENTITY:** No index/label/widget/source-handle substitution or stale numeric-ID reuse. |
| **INV-371-FOCUS** | Framework focus, actual Target delivery and current focus_key jointly qualify Enter; Blur with an armed press retires only the authenticated current descendant capture through the central engine cancellation owner before new Focus. | **FORBID-371-FOCUS:** No app focus-manager exposure, parallel focus enum, selection-as-focus, foreign-window delivery, unconditional focus-transition cancellation or sibling/foreign capture revocation. |
| **INV-371-KEY** | Only an admissible fresh routed Enter Down emits once. | **FORBID-371-KEY:** No global Screen Enter consumption, repeat/Up activation, reconstructed input or PointerDriven/Capture bypass. |
| **INV-371-PRESS** | One bounded original pointer/node/revision/body press uses existing pointer capture and qualifies one same-leaf Down→Up output; cancellation clears it terminally and releases capture where owned. | **FORBID-371-PRESS:** No uncaptured-Up inference, synthetic session ID, Down-only, canceled/outside/modifier/range/drag or unmatched-Up activation. |
| **INV-371-CURRENT** | Original node, topology and owner association is live at emission. | **FORBID-371-CURRENT:** No completion across source/topology/owner replacement, removal or collapsed hidden-row substitution. |
| **INV-371-CORE** | Existing TreeViewCore and ordered-selection reducer remain the single interaction/selection owners. | **FORBID-371-CORE:** No duplicate row focus, shadow selection, lifecycle global or per-app hit-test state. |
| **INV-371-CALLBACK** | One callback owner per instance; no callback means unchanged behavior. | **FORBID-371-CALLBACK:** No registry/event bus, HTS callback store, unsolicited activation or no-use behavior drift. |
| **INV-371-BORROW** | Callback runs after all event qualification borrows end. | **FORBID-371-BORROW:** No callback under shared RefCell guards, guard across phases or nested event replay. |
| **INV-371-DEFER** | Consumer receives only a pure identity and acts at its existing deferred boundary. | **FORBID-371-DEFER:** No routing-time App/tree/source/GPU mutation or inferred source validity. |
| **INV-371-DISCLOSURE** | Parent/caret expansion and existing range/selection mechanics remain independent. | **FORBID-371-DISCLOSURE:** No grouping-parent activation, DoubleClick duplication, changed press-selection reducer or synthetic selection-triggered action. |
| **INV-371-COMPOSITE** | The existing native TreeView/body/scroll/column owners remain intact. | **FORBID-371-COMPOSITE:** No fake TreeView wrapper, hidden ListView substitute, public reparent requirement, custom row geometry or alternate router. |
| **INV-371-STATUS** | Historical Draft, owner adoption, implementation and native proof remain distinct. | **FORBID-371-STATUS:** No executor self-adoption, runtime ENV/feature gate, automatic playback, Draft dependency promotion or capability claim from adoption alone. |

## 8. Compatibility and authority consistency

### 8.1 Behavioral matrix

| Current situation | Callback output | Existing owner behavior retained |
| --- | --- | --- |
| No callback registered | None | All existing pointer/keyboard/selection/disclosure behavior, including Enter, unchanged. |
| Unmodified primary Down on a leaf | None yet | Existing selection/focus reducer on Down. |
| Matching qualified Up on that same live leaf | One node_id | No second selection owner or source action in routing. |
| Invalid/outside/canceled Up, drag or modifier selection | None | Existing selection/range/toggle/cancellation behavior. |
| Parent row, disclosure child or grouping-parent DoubleClick | None | Native expansion/collapse and existing focus repair. |
| Focused TreeView, visible focused leaf, fresh routed Enter | One node_id | Original routing, domain/Capture priority and selection retained. |
| Row merely selected; widget unfocused, other editor focused or node hidden | None | Framework focus and editor ownership retained. |
| Enter repeat/Up or range-selection operation | None | No activation fan-out or repeat behavior added. |
| PointerDriven without actual Capture | No automatic keyboard activation | Pointer operation and explicit existing app commands unchanged. |
| Topology/source/owner replaced between Down and Up | None from the old gesture | New live nodes require their own qualified interaction. |
| One column with `set_header_visible(false)` | Same qualification as with a header | HAL-348 owns body origin, scrolling and header absence. |
| API selection, scroll, layout, retained rebuild or data publication | None | Existing invalidation/publication and retained behavior. |

### 8.2 Normative consistency matrix

| Authority / pillar | Exact disposition |
| --- | --- |
| [HAL-002](SPEC-HAL-002-Constitution-and-Architectural-Invariants.md), Pillar A — Retained-Mode Ownership | Existing WidgetTree and native TreeView composite remain owners; no alternate widget/renderer. |
| HAL-002, Pillar B — Phase Separation | Input qualification stays in routing; consumer structural/domain effects are deferred; no callback-induced tree mutation. |
| HAL-002, Pillar C — ID-based Identity | Stable node_id is the entire output; no retained widget references or realization-index identity. |
| HAL-002, Pillar D — Pure Data | Consumer pending action contains a u64; HTS/CMS do not acquire application callbacks or business state. |
| HAL-002 sovereignty and [HAL-002A](SPEC-HAL-002A-Constitution-Amendment-Guidelines.md) | No focus, state, phase, input or source authority transfer; no constitutional amendment proposed. |
| HAL-328 / HAL-176 | Existing actual focus, Capture, application interaction policy, window priority and routed delivery qualify Enter. No focus-only keyboard authority or new auto Capture. |
| [HAL-262](SPEC-HAL-262-TreeView-Widget-Surface-Semantics-and-Parity-Law.md), Draft | Selection/HTS ownership is source concordance and historical candidate context. This adoption admits only the additional activation surface; it adopts none of HAL-262's unrelated deferred features. |
| [HAL-337](SPEC-HAL-337-Shared-Topology-Substrate.md), Draft | No callback in the substrate; current core/HTS and deferred topology mechanism are reused. No shared-topology migration or Draft activation. |
| [HAL-347](SPEC-HAL-347-ListView-Focused-Row-Observation.md), Active / Binding | ListView-only precedent for distinguishing row focus/widget focus/activation and releasing borrows before pure output. Its ListView APIs and intent policies are not imported into TreeView. |
| HAL-348, Active / Binding | Headerless composition is already lawful and unchanged; no reconstruction of body geometry. |
| [HAL-193](SPEC-HAL-193-ListView-Public-Extension-Point-Lock.md), LOCKED / Binding | ListView's closed public surface remains untouched. This TreeView-specific contract is not another ListView callback exception. |
| [HAL-369](SPEC-HAL-369-Application-Selected-Ordinary-Activation-Timing.md), Draft / Inactive | Not authority. No application-selected Press/Release policy, timing toggle or general control activation extension is adopted. |
| [HAL-309](SPEC-HAL-309-Documentation-System.md), Adopted | Source facts, adopted norms, implementation tests and native acceptance are explicitly separate. No canonical guide/index or other SPEC status is changed. |

## 9. Required implementation proof

The original drafting unit executed no code or runtime tests. The implementation authorized in section 10 must use the existing TreeView/core/body and actual input owners. The current nearest registered test owners are [treeview_smoke.rs](../../src/tests/treeview_smoke.rs), existing TreeView module tests, and [pointer_driven_ui.rs](../../src/tests/pointer_driven_ui.rs) / the App input owner for application-policy delivery. Extend those owners only where a new reachable claim needs proof; no new harness or parallel input pipeline.

Use one real mounted two-level topology with a grouping root and real leaves, the existing body/ScrollArea and final authoritative layout. Required observable proof is:

1. Native row-route primary Down changes selection as before but emits nothing; an eligible installed callback arms the single core-owned press and real body pointer capture. Matching same-leaf Up emits exactly one node_id and releases capture. Outside/canceled/duplicate Up, nonprimary/modifier/range/drag, separator, scrollbar, disclosure target and topology/owner replacement cannot complete it. Move-out/DragStart/second Down and actual focus/capture loss clear the press terminally; late Up cannot revive it. With no callback, ordinary Down does not gain capture. A separate second valid click is independent; DoubleClick metadata adds no output.
2. Focus is obtained and lost through actual `on_focus` / `on_blur` lifecycle. Routed fresh Enter with the current visible focused leaf emits once; repeat/Up, merely selected unfocused rows, another focused widget (including TextField), a hidden node and the grouping root emit nothing. No direct private-handler invocation establishes public keyboard delivery.
3. The canonical app/input route proves PointerDriven outside Capture has no TreeView keyboard delivery or activation. Existing Capture/window priorities remain effective; the test must not force Capture or manufacture a delivered event to evade the policy.
4. The callback can read current core/TreeView state after qualification without a RefCell panic. It enqueues only node_id; the existing deferred consumer boundary, not routing, applies the domain effect and rechecks current association.
5. With no callback, existing behavior is unchanged. Disclosure, structural arrows, selection/range, Tab/focus, scrolling, highlight and the optional hidden header remain correct; no duplicate focus owner or selection state is introduced.
6. Holding a real leaf Down and pressing Tab proves immediate body capture retirement on parent Blur, before the neighboring TextField receives Focus; late Move/Up cannot activate or recapture. The checked engine effect must preserve capture on non-descendant/mismatched-holder requests and deliver one terminal notification on an authenticated request. A separate Shift-range press without a HAL371 press retains its existing capture behavior.

Tests must assert semantic output and actual routed identity, not a fake source label, pixel golden, manually called callback or direct private `handle_keyboard` result. Passing these later tests would prove their exercised framework route, not application source admission, GPU rendering or physical native-window acceptance.

## 10. Review, adoption and implementation disposition

### 10.1 Attributable adoption — 2026-11-05

Independent architectural review under JC's explicit request and delegated technical authority accepted the original 198-line Draft SHA256 `568d2f229e89ab399cb1d87285d9f97f36493837c9dead52858636a488d77af7` from local commit `0b8ff3244b0fa94d36b7040efbdbb82f33c3c7db`. Task `HAL-371-NATIVE-TREEVIEW-NODE-ACTIVATION-ADOPTION-IMPLEMENT-01` adopts that exact narrow TreeView contract together with the necessary pointer-capture qualification now materialized in sections 4, 7 and 9. The decision is effective before dependent runtime edits. JC retains ultimate authority and veto.

This adoption authorizes implementation only at the existing TreeView/core/body and nearest routed test owners, with section 9 proof. No alternative callback family, focus API, source-provider interface, activation-timing policy, input mode or routing-owner change is delegated. HAL-002/002A and HAL-193 remain unchanged; HAL-262, HAL-337 and HAL-369 remain Draft. A genuine blocking conflict returns to the governing owner rather than authorizing an app workaround.

### 10.2 Implementation evidence

**Historical blocker before runtime edits (2026-11-05), resolved by section 10.3:** The accepted focus-loss cancellation requirement could not be completed solely through the original TreeView/body edit scope using owner-bound EventContext release. The actual TreeViewBody owns pointer capture, while the framework-focused TreeView receives Blur. A TreeView Blur context cannot revoke the distinct body's capture:

- [EventContext::release_pointer](../../src/core/widget/mod.rs), lines 2007–2010, records a release request, not a targetable immediate revocation.
- [EventRouter::settle_composite_lifecycle_effects](../../src/core/event_router.rs), lines 2293–2332, applies that request using the lifecycle owner, hence the TreeView identity.
- [PointerCaptureManager::release_capture](../../src/core/event/mod.rs), lines 1564–1575, removes capture only when its actual owner equals the supplied widget identity. The body's capture therefore remains.
- EventRouter, lines 3970–4038 and 3518–3564, admits normal Tab focus traversal and invokes Blur on the former focused widget without canceling a distinct descendant's pointer capture. Holding a leaf press while Tab moves to a neighboring focusable widget is a reachable source-level witness. Clearing the shared press in Blur would suppress activation but would leave the required capture retirement incomplete; waiting for a later Up or pointer event is not the adopted terminal cancellation.

The canonical click-to-focus path itself is present: EventRouter lines 5699–5716 selects the nearest focusable ancestor, and TreeView is focusable. No app focus workaround is needed or authorized. Body capture-loss notification already exists in `Widget::on_pointer_capture_cancel`; it follows revocation and cannot cause an otherwise absent revocation on parent Blur.

The missing owner selection was returned before runtime edits. The original task excluded EventRouter/runtime-owner changes unless this precise boundary was returned. No alternate release owner, fabricated context, duplicate lifecycle or weakened cancellation condition was selected during implementation.

No runtime code or tests were changed or executed at that blocked checkpoint. This preserves the original causal evidence, not a current unresolved authority hold.

### 10.3 Same-unit owner-scope resolution — 2026-11-05

Independent architectural review verified the blocker and explicitly extended the same incomplete `HAL-371-NATIVE-TREEVIEW-NODE-ACTIVATION-ADOPTION-IMPLEMENT-01` unit to `core/widget/mod.rs` and `core/event_router.rs`. The exact selected operation is the checked descendant retirement in section 4: one optional ID-only effect, strict real-tree ancestry and exact current capture-holder checks, followed by the existing central SystemCancel operation after lifecycle borrows end. It is not a global focus-loss rule or a change to PointerCaptureManager, focus ownership, routing order, HAL-128, public application API or unrelated widgets. Section 9 proof is required before implementation completion.

### 10.4 Implementation verification

**Verified 2026-11-05**, package `halcyon 0.230.3-beta`, on the same-unit candidate based on `0b8ff3244b0fa94d36b7040efbdbb82f33c3c7db`. The only new public method is section 3.1. Engine effect/cancellation implementation stays in the existing EventContext and lifecycle settlement owners; TreeView and body share one callback and one optional press in TreeViewCore. The press uses the existing topology version plus dirty/current body projection checks: the general HTS revision also increments on ordinary viewport Layout and is not a source-replacement discriminator. A retained frame between Down and Up is explicitly covered.

Executed through attached local Grip, sequentially with no concurrent Cargo job:

| Command | Actual terminal evidence |
| --- | --- |
| `verify package halcyon test hal371_treeview_focus_owned_node_activation check timeout 600 output raw` | Production check exit 0; five selected tests passed, none failed/ignored. Includes the separate core/HTS borrow-release proof. |
| `verify package halcyon test hal371_treeview_descendant_capture_blur_retirement timeout 600 output raw` | Two selected tests passed. Real mounted TreeView/body capture retires immediately on Tab to a TextField; late Move/Up cannot activate. The lifecycle-effect probe observes one cancellation before new Focus and preserves sibling, mismatched-holder and self capture. |
| `verify package halcyon test pointer_driven_treeview_activation timeout 600 output raw` | One selected test passed, using the original RawInput → Fusion → App-priority → EventRouter path for both application policies and another focused TextField. |
| `verify package halcyon test treeview_smoke timeout 600 output raw` | Final nearest-owner run: twenty tests passed, none failed/ignored. Includes the added hidden-leaf Enter case, multi-click cycles, original source replacement/removal/collapse cancellation, no-callback behavior, unchanged Shift-range capture and the fourteen preexisting TreeView tests. |
| `verify package halcyon build timeout 600 output raw` | Exit 0. Library built; no consumer/native execution inferred. |

Raw output was used to retain exact nonzero test counts and failure diagnostics; bounded terminal summaries omitted unrelated compiler output. Every filter executed actual tests. Initial attempts remain failures: a test probe used the obsolete paint signature (compiler E0050), and a Ctrl single-click assertion accidentally exercised the router's second-click kind. The fixture was corrected using the actual route, and original physical double/triple-click cycles were added to the owner proof. A combined verification invocation ended before a completed second-filter receipt; the filters were subsequently executed individually and reached the terminals above. No missing result is relabeled PASS.

This is source and synthetic qualified-input proof through the actual framework owners. It is not observation of OS-delivered macOS gestures, application integration, source-model admission, GPU rendering, physical display or JC visual acceptance. Those remain independently owned and unobserved. HAL-262, HAL-337 and HAL-369 remain Draft; no unrelated authority is adopted.
