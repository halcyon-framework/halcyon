# SPEC-HAL-101: TextField Widget (Single-Line Text Input)

Single-Line Editable Text Widget: State, Interaction, Hit-Testing, Scrolling, Painting

**Status:** Stage-4 Implemented (Proper Frame Timing + Vertical Alignment)
**Author:** JC

**Depends on:**

* SPEC-HAL-007 – State Model
* SPEC-HAL-009 – Event Routing & Hit Testing
* SPEC-HAL-018 – App Framework & Screens
* SPEC-HAL-020 – Action System (for “commit” hooks / bindings)
* SPEC-HAL-033/036 – Input Fusion & Coalescing
* SPEC-HAL-048 – Paragraph & Text Layout Engine
* SPEC-HAL-049 – Font Collections & Fallback Rules
* SPEC-HAL-050 – Unified Text Shaping Architecture
* SPEC-HAL-052 – Interactive Text Editing Model
* SPEC-HAL-090–099 – Layout Grid, Typography, Pointer, Input Precision
* SPEC-HAL-100 – Widget Interaction State Machine

**Planned follow-ups:**

* SPEC-HAL-055 – Clipboard, Undo/Redo & History Model
* SPEC-HAL-056 – Widget-Level IME Composition Model
* SPEC-HAL-057 – PasswordField / SecureEntry variants
* SPEC-HAL-058 – TextField Validation & Formatting Rules

---

## 0. Purpose

This SPEC defines the **single-line TextField widget** in Halcyon:

* The **state** it owns (document, selection, caret, scroll offset, focus flags, config).
* How it **maps events** (pointer + keyboard) into the SPEC-HAL-052 editing substrate.
* How **hit-testing** works in widget-local coordinates.
* How **caret + selection** are painted for a *single visual line* with horizontal scrolling.
* How it integrates with **input fusion**, **focus**, **action system**, and **grid/typography** rules.

Scope is strictly **single-line** editing with horizontal scroll, no wrapping.
Multi-line text areas will have their own SPEC; this one is the basic input box.

The main goal: give the coding agent a **lossless contract** so it **never re-invents selection rules or cluster math**.
All editing semantics live in SPEC-HAL-052; TextField is just the shell.

---

## 1. Role & Non-Goals

### 1.1 Role

TextField is the canonical:

* “Enter a value” widget: search bars, small forms, prefixes/symbols, numbers when we still use plain text.
* Single logical paragraph, single visual line at a time.
* May contain arbitrary Unicode; layout is still delegated to ParagraphEngine and text engine.

It must:

* Use **cluster-safe editing** from SPEC-HAL-052.
* Honor **word selection, double-click, triple-click** like a normal text input.
* Handle **horizontal overflow** by scrolling the content, never wrapping.
* Integrate cleanly with **focus, tab navigation, actions**.

### 1.2 Non-Goals

This SPEC does **not** cover:

* Multi-line editing or soft wrapping (that will be TextEdit / TextArea SPEC).
* Rich text (spans, inline widgets, syntax highlighting).
* IME composition details (preedit, candidate windows).
* Clipboard and undo/redo semantics.
* Password masking specifics beyond “masking exists”.

Those are separate SPECs stacked on top of this one.

---

## 2. Data Model (TextFieldState)

Everything here is widget-level state; editing logic and cluster invariants come from SPEC-HAL-052.

### 2.1 Core Types

The TextField widget owns a dedicated state struct:

```rust
/// Stateful backing for a single-line TextField widget.
pub struct TextFieldState {
    /// Editable document, single paragraph, UTF-8.
    pub document: TextDocument,        // or EditingState<SimpleTextBuffer> if we go pure-HAL-052

    /// Caret and selection, per SPEC-HAL-052.
    pub cursor: TextCursor,
    pub selection: Selection,

    /// Text layout cache for a single paragraph.
    pub layout: TextLayoutCache,

    /// Horizontal scroll offset, logical units (same space as x_logical).
    pub scroll_x: f32,

    /// Preferred X for vertical navigation (mostly unused for single-line,
    /// but kept for consistency with SPEC-HAL-052; can be reused by future variants).
    pub preferred_x: Option<f32>,

    /// Focus and interaction.
    pub is_focused: bool,
    pub is_enabled: bool,
    pub is_read_only: bool,

    /// Visual caret blinking.
    pub caret_visible: bool,
    pub last_blink: Instant,
    pub blink_interval: Duration,

    /// Pointer drag mode for selection (word/line semantics).
    pub drag_mode: TextFieldDragMode,

    /// Cached widget size for hit-testing and scroll behavior.
    pub bounds: Size,

    /// Cached last known text width (for scroll clamping).
    pub content_width: f32,

    /// TextField configuration (behavior & appearance).
    pub config: TextFieldConfig,
}
```

### 2.2 Drag Mode

Widget-level notion of what kind of drag is in effect.
Anchor is handled **here**, not in the substrate.

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextFieldDragMode {
    None,

    /// Single-click drag: character-based expansion.
    /// Anchor is the byte position set on mouse down.
    Character { anchor: usize },

    /// Double-click drag: word-based expansion.
    /// `initial` is the word boundary selected at double-click.
    Word { initial: WordBoundary },

    /// Triple-click drag: line-based expansion (whole line).
    /// `anchor_line` is logical line index; for TextField it's always 0,
    /// but we keep the shape identical to multi-line widgets.
    Line { anchor_line: usize },
}
```

### 2.3 Config

Behavioral and visual knobs. The agent must not improvise them; they’re declared here.

```rust
pub struct TextFieldConfig {
    /// Placeholder text when empty (not editable).
    pub placeholder: Option<String>,

    /// If true, mask characters for password entry.
    /// Rendering logic changes, underlying text and editing logic do not.
    pub is_password: bool,

    /// If true, select all text on first focus gained via pointer or Tab.
    pub select_all_on_focus: bool,

    /// If true, pressing Enter triggers a "commit" action (Action System)
    /// and does not insert a newline (TextField is always single-line).
    pub commit_on_enter: bool,

    /// If Some, forcibly restrict text length to this many bytes (UTF-8).
    /// Editing operations must clamp inserts.
    pub max_len_bytes: Option<usize>,

    /// Horizontal text alignment inside the field (before scroll).
    pub align: TextAlign, // Left, Center, Right

    /// Internal padding between widget rect and text region.
    pub padding: f32,

    /// Colors, radii and thicknesses follow typography/color SPECs.
    pub colors: TextFieldColors,
}
```

Where:

```rust
pub enum TextAlign {
    Left,
    Center,
    Right,
}

pub struct TextFieldColors {
    pub bg_normal: Color,
    pub bg_focused: Color,
    pub bg_disabled: Color,

    pub border_normal: Color,
    pub border_focused: Color,
    pub border_error: Color,

    pub text_normal: Color,
    pub text_disabled: Color,

    pub placeholder: Color,
    pub selection: Color,
    pub caret: Color,
}
```

These are wired into the global semantic palette, but the widget treats them as given.

### Standard qualified visual motion and explicit plain mode

Ordinary TextField constructors and Default enable only the qualified motion admitted by §§8.4.1–8.4.5. Explicit per-instance false preserves ordinary static presentation. Password, embedded and otherwise ineligible states retain their existing canonical presentation. This TextField-specific default creates no general motion configuration, framework-wide timing default or implicit mount animation.

### 2.4 Tree-owned composite text entry

**INV-101-CT-OWNERSHIP.** ComboBox, SpinBox, Slider and TagInput shall not own, retain or borrow a TextField Widget. For each existing text-entry role the engine's WidgetTree construction owner installs at most one real TextField child. The host stores only its sealed copied structural relation, its own semantic state, immutable owned observations and owned configuration/command values. The child alone owns its text document, cursor, selection, editing history, IME/cache state and TextField presentation. No wrapper Widget may own the child instead. Temporary construction creates the child directly for immediate transfer to WidgetTree, never inside a host field or callback capture.

**INV-101-CT-INSTALL.** Install the host and its fixed internal text-entry child during the existing construction/rebuild boundary, complete the role relation before first Layout, and seal it under HAL-154. Apply this to root and child installation and true host replacement; no event, Widget Update, Layout or Paint creates/removes an editor. Editing entry/exit changes session eligibility, not tree topology. The stable child survives ordinary popup close/edit-mode exit when existing buffer policy requires preservation. Its inactive role contributes no independent focus/tab stop, hit target, caret or motion demand. Presentation uses the existing host shell and borderless child text region; no visible alignment, padding or baseline change is admitted.

**INV-101-CT-IDENTITY.** The engine qualifies the relation by tree lifetime and BOTH actual concrete mounts, plus one nonrecycled editing-session generation. Session generation distinguishes successive uses of the same child and is not a routing ID. Rebuild/replacement retires old association and handles before any successor Update. Replacing/removing the registered child is a composition-owner transaction, never an unobserved substitute or automatic successor rebind. An incomplete, mismatched or retired role cannot enter live operation. Same-instance Layout take/restore preserves mount lineage and does not restart a session. Required child wiring cannot be changed after seal.

**INV-101-CT-CAPTURE.** The host remains the SOLE actual framework Focus/Capture owner under its existing category and mode policy. The child is not independently focusable/capturable and gains no separate tab stop. The engine may delegate only the host's currently authorized text-entry operation to that exact live child/session. The delegation contains current host owner/domain/window/activity/availability and child/session identity; it is neither a second Capture nor a widget-local mode mirror. Host popup browsing, numeric stepping, track drag, chip activity or focus alone does not grant text delegation. Suspension, focus/Capture loss, mode exit or wrong-session evidence revokes it before further delegated input or presentation is admitted. Restoration alone never resurrects an old session or leg.

**INV-101-CT-FACTS.** Actual FrameworkAppearanceFacts remain truthful: the host is focused/captured and the child is not. A DISTINCT engine-qualified editing-delegation fact may authorize this child's editing/caret/blink/motion; it must not set child focused/captured appearance bits to impersonate ownership. The fact is scoped to the real child context and current session and consumed together with the child's own geometry and availability. The host remains responsible for its one complete outer focus/Capture cue; the child emits no duplicate outer cue. Missing or invalid delegation is canonical inactive/static presentation, not inferred permission.

**INV-101-CT-ROUTING.** Preserve existing input-owner decisions and original fused/key/text/pointer provenance. The host's existing mode/loading/region policy selects whether an input is its own command or an authorized text operation. For a text operation the existing engine router alone resolves the registered child and supplies a fresh child-identity context with actual child-local geometry and the explicit delegated authority. Focus, keyboard and pointer requests refer to the true host input owner; child identity remains the source for child geometry, editing and invalidation. No host gets a child reference or supplies fabricated context facts. Pointer forwarding remains gated to the real text region and existing gesture policy; no duplicate delivery, ancestor leak, invented repeat origin or input replay is admitted.

**INV-101-CT-RESULT.** A delegated edit completes as one bounded engine-owned operation under the existing routing authority. After ending the child's borrow, return an OWNED, source/session/revision-qualified result before the host consumes input text, refilters, recomputes ghost completion, resolves a commit, emits a callback or processes a later dependent event. Preserve immediate logical editing and existing same-batch order. The host's retained draft observation is a read-only value projection of that exact result, not a second editable document or authority. Rejected/no-op results remain explicit; stale, denied, retired or unavailable-child outcomes cannot be reported as accepted edits. No best-effort drop, shared mutable child model, later-frame queue or old snapshot substitutes for a known current result. The two owner borrows must remain separate engine scopes; no host callback may retain or receive peer state.

**INV-101-CT-COMMANDS.** Enter, popup/list navigation, numeric commit/cancel/step, tag creation/toggle, Tab and engine-owned Escape keep their current owning policies. Ordinary standalone TextField Enter behavior shall not override a host command. Engine-mediated clear/reset/seed operations complete before the next dependent host command and return a new qualified observation. Programmatic replacement is a static presentation boundary and preserves the existing operation's cursor/selection/history-reset disposition; it is not a new smooth edit class. ComboBox popup close without commit preserves text. TagInput's explicit commit-and-clear remains immediate. Numeric state, domains, parsing, clamp, emission and edit-start value remain host-owned; child text never becomes numeric authority. No command is delayed until an animation ends.

**INV-101-CT-API.** Construction-time methods may configure owned initial text/filter/presentation props without constructing or retaining a child Widget. Once mounted, any operation requiring child state must execute through the qualified owning-tree/session operation and synchronously return its actual applied/rejected/stale/retired result before a dependent read or command. An unqualified self-only setter cannot silently queue, replace a peer, publish requested text as applied text, or synthesize a success. Existing no-context draft replacement/force-commit compatibility must be explicitly reconciled and affected callers migrated; this is an intentional proposed public compatibility tightening, not a claim of unchanged API shape. Pure host-owned value/options/policy changes remain under their existing semantics, but any dependent child synchronization uses this same owner boundary. Read access may retain an owned exact observation; it cannot return or retain a borrow into the child.

**INV-101-CT-LAYOUT.** The host arranges the sealed real child by ID through ordinary LayoutContext child APIs using the same intended input region, origin, font/scale, clip, alignment and scroll geometry. Layout does not install children, read/mutate peer editing state, execute pending semantic commands or publish motion. Ordinary engine Paint traverses the child with its own real geometry and inherited clip/scroll scopes; the host shall not call TextField paint or paint a second input/caret. Changed or ambiguous geometry/source suppresses obsolete motion at first consumption and retires it at the next lawful Update. Chip reflow, ghost source changes and numeric centering do not imply new motion eligibility.

**INV-101-CT-UPDATE.** The real child receives exactly one canonical engine-timed Widget Update, its existing concrete-mount animation owner, own geometry and qualified session facts. The host does not forward a second Update or aggregate a private child Timeline. All text/session semantic transactions settle at their existing routed or lawful deferred owner boundary before the affected Update; Update changes presentation only. Child Layout/Paint consume immutable projection. Existing Timeline, cancellation/failure, narrow invalidation, source bounds and no-backlog laws remain. Host retirement and session exit revoke child eligibility and retire its presentation through existing engine/Update lifecycle; no successor inherits an old source or handle.

**INV-101-CT-MOTION.** Only after separate joint activation of the reviewed motion scope may the registered child use it. This is the exact scoped exception to section 2.3 and sections 8.4.1-8.4.5's SAME-TextField actual Focus/Capture, non-embedded/standalone and other-widget exclusions: current engine delegation from the true host may qualify the real child/session without falsifying child appearance ownership. All other operation, source, geometry, IME/password, selection, plain-mode and repeat conditions remain. Navigation remains 65 ms; P6 end edits use 55 ms under §11.5; P7/P8 range bounds/timing remain unchanged. Centered SpinBox/Slider content stays ineligible for left-aligned P6/P7; this contract admits neither alignment changes nor centered reveal/conceal. Default/explicit plain settings persist across sessions.

**INV-101-CT-NO-USE.** An inactive, plain, password, composing, unavailable, nondelegated or otherwise excluded child with no channel to retire short-circuits motion work before motion-specific scans/shaping/source allocation/Timeline admission. The stable child still has ordinary baseline tree lifecycle; no claim of zero total child work is made. Only actual unsettled admitted effects contribute motion demand. Capture retention after delegation ends is not invisible child animation demand.

**FORBID-101-CT.** No host-owned Widget or peer-state reference; no copied parent context masquerading as the child's; no fake child focus/Capture; no implicit rebinding by NodeId/TreeWidgetId; no Layout installation or semantic settlement; no hidden/global registry or general bus; no new router, clock, frame phase or second Update; no delayed/guessed edit, callback or numeric/resolver/tag semantics; no per-host shaper/caret/motion engine; no unapproved public compatibility change or native-conformance claim.

### 2.5 Engine-owned composite editing transaction

**Status:** Adopted / Binding, 2026-10-29. The complete mechanical contract is integrated below. Its scope is unchanged: four built-in composite hosts, engine-owned transactions, and the affected mounted cell-choice factory path. The 2026-11-23 editorial integration changes no runtime requirement or implementation status.

Sections 2.5.1–2.5.4 define the composite editing interface, closed operation/reply/continuation set, separate borrow sequence, per-host callback positions, lifecycle/property origin, public owner context, construction, factory synchronization and readiness boundaries. A request stages only the current original dispatch; it is not an asynchronous queue or general action bus. The engine ends the host scope before child operation and ends child scope before host continuation. At most two child crossings occur per qualified host/child transaction; the final continuation terminates. No copied host context, host handle_event replay, child appearance impersonation, delayed accepted edit/result, unqualified runtime setter, Layout installation or stale-manifest rebinding is admitted. Readiness gates only the unready role and its dependent input/publication; unrelated scene content, widgets, rendering, input and independent roles remain governed by their ordinary contracts and cannot be blocked by that gate. All existing operation/source/alignment/timing/plain/safety constraints remain.

Post-mount ComboBox factory sync takes the scoped owner context and exact target, not mutable peer Widget access. Collect current mounted-key specs at the admitted deferred owner cut from actual Ready source data and policy, then synchronously settle the complete operation before affected Update/Layout. Layout only outputs pure owned desired-role/geometry data; it does not mount/remove/sync an editor. The next lawful Phase 2 qualifies and applies the exact current desired-role delta; stale manifests cannot rebind. A newly discovered unready role gates only that role's dependent input/publication under §2.5.4.3; it cannot block unrelated scene, widgets, rendering, input or independent roles. This is limited to the affected composite choice path and supplies no blanket Layout exception or unrelated PropertyEditor session rewrite.

#### 2.5.1 Interface surface and ownership

These interfaces are admitted only for the four built-in hosts: ComboBox, SpinBox, Slider and TagInput. A private implementation module in core owns the transaction and is reached by existing EventRouter/App/WidgetTree owners. No additional Widget trait event protocol, general user-defined continuation, executable closure queue or generic peer-access capability is admitted.

| Interface/data | Visibility and exact meaning |
|---|---|
| CompositeEditorId | Public opaque owned relation handle; private constructor; binds tree lifetime, host and child TreeWidgetIds and BOTH concrete mounts. Resolving a new handle is an explicit current-owner operation, never automatic rebinding of an old handle. Session identity is separately checked at execution. It grants no Widget reference. |
| CompositeEditRequest | Crate-private one-slot EventContext request: current operation, fixed host-kind continuation tag and owned parameters. Target is implicit in the current registered host; no arbitrary peer target. It cannot retain an Event/context/tree/Widget borrow. A second request in the same host stage is an explicit error. |
| CompositeEditOperation | Closed private set: ForwardCurrentInput, ObserveAndActivity, SeedForEntry, SeedForEntryThenCurrentInput, ClearAfterTagCommand, ResetAfterResolverCommit, RetireDelegation. ForwardCurrentInput uses the engine's original current input, not a copied/replayed/synthetic event. ObserveAndActivity may combine the original key's required blink activity with a fresh snapshot in one child borrow. The combined entry operation may initialize the child and perform the already-authorized original pointer operation in that same child scope, only after actual host activation. |
| CompositeEditSnapshot | Fully OWNED applied text, document revision, canonical cursor/selection values and source relation/session qualification. No layout/glyph/context references. At most the currently needed snapshot and existing bounded visual source are retained; no transaction history. |
| CompositeEditReply | Definitive operation outcome plus owned snapshot when available, final handled/policy disposition and narrow child invalidation/directives. Outcomes distinguish Applied, Unchanged, DeniedConsume, DeniedBubble, Stale, Retired, Unavailable and WrongPhase/Reentrant. A terminal failure after a host action records the reached stage and whether its existing callback/action already occurred; it cannot fabricate reset or replay the action. |
| resume_composite_edit | Private closed adapter for ComboBox/SpinBox/Slider/TagInput. Receives only owned reply and fixed continuation tag under a newly scoped host borrow. Returns Done or one explicitly typed final child reset plus final host continuation. It is NOT handle_event and cannot reenter EventRouter. |
| CompositeEditorOwnerContext | Public borrow-limited engine capability, supplied only inside the existing post-route Screen::flush_pending_actions owner boundary, or privately to the same App deferred executor. It contains explicit exclusive resource borrows, not a hidden global or stored raw escape. It is unavailable during Screen handle_input, Widget callbacks, Update, Layout, Paint, custom preparation or recursive owner calls. |
| ScreenContext::with_composite_editors | Public scoped entry during that post-route flush only. The closure receives CompositeEditorOwnerContext, never mutable child/host Widgets. While active, the context's ordinary tree access is unavailable so it cannot alias the same mutable tree. Other ScreenContext uses return WrongPhase/Unavailable rather than silently queueing. App's internal flush plumbing must explicitly supply the live tree, router, domain and text engine. |
| Owner context operations | resolve(host TreeWidgetId), snapshot(id), replace_text(id, owned String), force_commit(id), sync_combo_box(id, owned ComboBoxCellEditorSpec), set_tag_items(id, owned Vec<TagItem>), set_tag_policy(id, TagInputPolicy). Each is synchronous, revalidates exact current relation/session and returns owned applied or typed failure. Numeric return values/callback counts remain those of the existing host policy. No operation returns &mut Widget or reports queued intent as applied state. |
| CellEditorFactory::sync_combo_box_editor | Factory synchronization contract: receives the owning CompositeEditorOwnerContext, opaque target and complete owned ComboBoxCellEditorSpec; returns the owned sync result. It no longer receives &mut dyn Widget for a downcast. Default factory delegates to sync_combo_box; other trait implementations must migrate explicitly. |

These interface names, inputs, visibility and semantics define the admitted boundary. The adoption deliberately tightens source compatibility where the former self-only API could mutate a host-held peer. Exact Rust module exports and lifetimes must implement these ownership bounds, not create an alternate public contract during coding.

#### 2.5.2 One bounded relation transaction within the original dispatch

1. **Host stage.** The existing router delivers the original input to the actual host under its normal complete EventContext. The host performs its existing policy/region/loading classification. A branch needing child work records exactly one CompositeEditRequest and returns its provisional handled state; it does NOT run any dependent action that requires the child's resulting value.
2. **End host scope.** Router extracts owned request and ordinary directives/invalidation, then drops the entire host EventContext and node borrow. No child lookup or borrow occurs while that Widget borrow remains. The original event stays in the outer engine call and is not dispatched again to the host.
3. **Resolve activation, qualify and execute child.** For an initial pointer entry, process only the host's ordinary prerequisite focus/Capture directives through existing engine rules before admitting delegated editing. Honor explicit release/toggle precedence and the existing already-Capture guard. Any required host lifecycle callback uses its own complete scope and ends before child access; its seed may combine with the already-selected original pointer operation in one child scope. Failed activation admits no keyboard edit. Never forward the engine-consumed entry i/Enter as phantom text. For already active keyboard editing, require current actual host Capture. Validate tree/relation mounts, current session, original domain/window and effective availability. Resolve only the sealed registered child and borrow that child alone. Construct a fresh child context: real child widget_id/geometry/animation owner/invalidation, original event/time/provenance, and DISTINCT delegated host input authority. Actual child appearance facts remain unfocused/uncaptured. ForwardCurrentInput calls the existing TextField policy/handler once. Preserve HAL-108 Allow/DeniedConsume/DeniedBubble, not a geometry-based policy bypass.
4. **End child scope.** Copy the applied canonical result, request effects and invalidation into CompositeEditReply; end context and borrow. A known synchronous result is definitive. No delayed result mailbox or next-frame semantic settlement exists.
5. **Host continuation.** Requalify and borrow only the host. Supply the owned result to the fixed private continuation. It executes the original branch's dependent actions at their actual source-defined points, including callbacks. It may return Done, or one named clear/reset operation and one final continuation. It does not invoke handle_event, borrow the child or emit another routed input.
6. **Optional second child crossing.** End host scope; requalify; execute the named clear/reset in a fresh child context; end that scope; deliver the final owned result under a new host scope. This final continuation MUST terminate. No third crossing, open-ended trampoline, queued command chain, callback-defined continuation or per-character loop is admitted.
7. **Finish original dispatch.** Aggregate ordinary host/child directives with their TRUE owners, finalize handled disposition, and finish the existing router's post-dispatch request/gesture/cursor/demand handling; do not apply an entry directive or lifecycle notification twice. Only then may routing proceed to a later original event or fallback. Pointer capture remains host-owned and Up/cancel delivery is not duplicated. Child paint/layout invalidation targets the child; host semantic visual changes target the host and required ancestors. Existing handled repaint behavior is not claimed to be a no-op optimization.

This whole bounded sequence is the CURRENT dispatch for HAL-108 section 13.5 and HAL-354 availability. A queued availability request is not newly effective halfway through the transaction. Complete the current dispatch, stop at the normal between-original-events boundary, then settle availability at its lawful cutoff. An actual immediate session retirement or invalid mount denies remaining stages. Recheck after a callback; never roll back or replay a host semantic action already emitted before a later reset failure.

##### Per-host continuation points

| Path | Required sequence, not a common reordered callback endpoint |
|---|---|
| ComboBox ordinary text/key | Child edit and owned current result → current search/highlight/ghost work. |
| ComboBox resolver commit | Fresh draft → host resolver → accepted child reset/seed → latch/popup cleanup and release request → on_committed. Rejection resets/emits nothing. |
| TagInput ordinary edit | Child edit → refilter/ghost, using its owned applied text. |
| TagInput Enter/create/split input | Fresh snapshot/activity → host validates/toggles/creates and fires its existing on_change → child clear → final refilter. Callback-before-clear is preserved for tag creation, Enter and split input. |
| TagInput intercepted KeyDown | Child ObserveAndActivity occurs before the host's intercepted key action, preserving reset_caret_blink_on_activity without fabricating another event. |
| SpinBox Enter | Fresh draft → leave direct-edit mode → parse/clamp → changed-value callback then binding write → on_commit on successful parse even when value unchanged → original Capture-release request. No numeric meaning is delegated to child. |
| Slider Enter | Fresh draft → leave value-edit mode → parse/clamp → changed-value callback then binding write. Existing direct-edit Enter does not request Capture release; do not add it. |

Two value-dependent child crossings is an explicit source-path bound for ONE qualified host/child transaction, not permission for arbitrary other interactions. Ordinary old-owner retirement and new-owner activation remain distinct existing lifecycle origins, not recursive continuations or a promise that the complete external event has no other lifecycle work. Within each qualified relation, entry seed/activity may combine with the original operation; it cannot create a third crossing. Failure before a child edit or host action cannot claim either occurred. Failure after a callback reports that stage without inventing a successful clear, repeating the callback or rewriting already admitted host state.

#### 2.5.3 Lifecycle and runtime property transactions

Capture entry/leave, focus loss, availability, mount retirement and explicit runtime property operations may have no original input. They use a distinct engine-only Lifecycle or OwnerProperty origin with current frame/domain/mount/session cause; never manufacture an Event or repeat provenance. Authority is ORIGIN-SPECIFIC: keyboard editing needs actual current host Capture; a lifecycle terminal uses its genuine engine cause even after Capture has ended; an owner property operation uses the explicit owning capability and its existing programmatic policy, without requiring or acquiring user Capture. A disabled/read-only or non-Capture target must not lose a previously legal programmatic property operation merely because it cannot accept user editing. The same separate-borrow/result kernel executes only the applicable closed seed/snapshot/reset/retire operations. It does not recursively route input or invoke child on_focus/on_capture_enter to pretend the child became a semantic focus owner. Child role initialization/retirement is explicit and actual host lifecycle notification occurs once. Required text-engine access is preserved under HAL-328 section 13. Presentation handles remain Update/lifetime-owned; these operations record canonical state/snap intent, not a second animation tick.

Public compatibility is explicit:

- ComboBox input_text may continue borrowing only the host's exact owned applied observation. It cannot borrow child storage.
- ComboBox set_input_text and TagInput set_items/set_policy become checked construction-only self methods; mounted child-dependent calls return OwnerContextRequired before changes. Runtime callers use the named owning context operations. This intentionally requires source/API migration; it is not a silent queue, panic fallback or claim of unchanged signature. Builders configure initial owned props without making a TextField Widget.
- Remove the self-only SpinBox force_commit route in favor of the scoped force_commit operation. Its current numeric/callback result is preserved by the synchronous host policy continuation; no private lookup or stale observation replaces the missing owner capability.
- Pure host value setters retain their existing behavior; in particular SpinBox/Slider set_value do not newly replace an active draft. Current TagInput item/policy changes that refilter must acquire a fresh owned snapshot in the owner transaction before refiltering.
- Direct external mutation of the registered internal TextField bypassing this relation is not an admitted runtime API. Child mutation methods must reject unqualified mounted-role use; public node_mut/downcast must not become a backdoor. The implementation must mechanically enforce this boundary.

#### 2.5.4 Construction, rebuild and factory synchronization

##### 2.5.4.1 Fixed composite assembly

Extend the existing WidgetTree construction owner used by create_root/add_child/true replace with one closed four-host assembly adapter. It is not Widget::on_mount tree access and not a generic child factory:

1. Install the host Box in the real tree with its actual new concrete mount and keep the composition not-ready.
2. Borrow only host to extract COMPLETE owned initial editor props/filter/presentation description; end that borrow. The host never constructs/stores the TextField Widget.
3. Engine constructs the ordinary TextField from that description and immediately installs it as the registered real child with actual tree ID/mount.
4. Under a fresh host scope bind only copied relation identity through existing Deferred wiring; record exact host/child mount association in the explicit tree-owned role state. Seal before first Layout. Only complete construction becomes ready.
5. True replacement first retires predecessor delegation/child/handles; existing preserved child lists do not confer successor authority. Preserve unrelated children and registered Update-to-Viewport transfers. Same-instance Layout lease/restore does not run assembly again.

The public tree installation methods must not return a silently incomplete live composition. Construction invariant failure is a hard construction failure under HAL-154; ordinary typed runtime/session refusal is distinct. Structural changes occur only at existing lawful construction/rebuild or Phase-2 deferred mutation, never inside an active host/child callback, Update, Layout or Paint.

##### 2.5.4.2 Already-mounted cell choice synchronization

Layout must not synchronize mutable cell-choice editor state. At the existing App deferred/pre-Update owner cut, a PRIVATE bounded collector borrows the ListView data owner only, snapshots its CURRENT registered mounted choice keys, and derives complete owned specs from current DataAdapter::read_cell, capabilities and column options. It then ends that owner borrow. Only actual Ready(String) and current visible/readable/editable/committable policy establish a string spec; Pending/Denied/Error/Missing remain explicit non-ready outcomes. Do not use the old realized-cell cache or display strings as current values.

The engine resolves each exact mounted target and invokes the owning-context factory synchronization synchronously, with separate host/child scopes. Apply options, current text/reset only when required, and text-input-disabled state as one completed owner operation before the affected Update/Layout and before publishing a successful result. Preserve no-op behavior and current display/commit meaning. No &mut dyn Widget factory parameter, stale draft, best-effort drop or next-frame reply is admitted. Retired/reused row, column, target or provider revision is a typed stale/non-ready disposition, not successor rebinding. The batch is bounded by current registered roles and contains at most one current spec per role.

##### 2.5.4.3 New role discovery and role-local readiness

At the adoption baseline, the ListView choice pool was discovered after Layout geometry and added or removed nodes through cell mechanics. Earlier discovery had not been established. The adopted boundary is: Layout may return a PURE owned desired-choice-role manifest as output; after all Layout borrows end it is only a requested structural description. Apply its qualified current delta at the NEXT ordinary lawful Phase 2, construct complete host+child roles there, then use the ordinary single Layout of that later frame. Revalidate current provider/source and role identity; no stale manifest may install or resurrect a role. Retain one coalesced current manifest per owner, not history.

Readiness is STRICTLY ROLE-LOCAL. Until the exact structural delta for a newly discovered or reused editor role is complete, gate ONLY that role's input and publication that depend on its readiness. Do not publish that role as ready, target it through stale identity, or replay input withheld before readiness. Other ready roles, including independent roles in the same owner or manifest, continue under their ordinary contracts. An unready role MUST NEVER defer the whole scene, block unrelated widgets or input, suspend rendering, or withhold independent publication. Rejected, stale or superseded manifests cannot install or rebind the role and confer no successful installation result; their failure does not revoke unrelated readiness. Existing mounted accepted edits and their owned results remain synchronous. This scoped readiness contract does not itself establish implementation or imply that later Layout discovery was knowable earlier. No extra Layout, phase or clock is introduced.

At adoption, LayoutResult had no manifest result and a complete role-local input/publication gate had not been established. Whole-scene SceneBuildContinuation::Deferred MUST NOT implement this role-local gate. The pure manifest output and localized readiness qualification are required parts of this adopted contract and need their own implementation evidence. Proof of the event kernel alone does not establish the factory/pool connection or permit a broader gate. Ordinary PropertyEditor/TreeView text/integer/float motion needs no update; their separate Layout-structural discrepancy is not silently repaired or used as permission.

---

## 3. Geometry & Hit-Testing

### 3.1 Text Region

Given the widget rect `R`:

```text
R.x, R.y, R.width, R.height
```

Text region:

```text
text_x0 = R.x + padding
text_y0 = R.y + padding
text_width  = R.width  - 2 * padding
text_height = R.height - 2 * padding
```

All hits and rendering are defined relative to this region.

### 3.2 Coordinate Spaces

1. **Widget-local**: `(local_x, local_y)` from pointer event.
2. **Text-region-local**: `(tx, ty) = (local_x - padding, local_y - padding)`.
3. **Scrolled text space**: logically, the text starts at `x = -scroll_x` from the left edge.
   So when hit-testing, we feed `tx + scroll_x` into the layout / cluster hit-test.

### 3.3 Hit-Testing to Caret Index

Given widget-local `(local_x, local_y)`:

1. Transform to text-region:

   ```text
   tx = local_x - padding
   ty = local_y - padding
   ```

2. Clamp `ty` to the vertical extent of the single visual line:

    * TextField always has at least one line in layout (`line_index = 0`).
    * Clicking above or below still hits that line.

   ```text
   line_index = 0
   ```

3. Effective X for cluster map (accounting for scroll):

   ```text
   x_for_hit = tx + scroll_x
   ```

4. Call into layout/cluster helpers (SPEC-HAL-052):

   ```rust
   let boundaries = state.layout.boundaries();
   let lines = state.layout.lines(); // single visual line for TextField
   let pos = hit_test(boundaries, lines, x_for_hit, /* y ignored or 0.0 */ 0.0);
   ```

5. Clamp:

   ```rust
   let pos = pos.min(state.document.len());
   ```

This is **the only** place the widget converts pointer coordinates into a byte index.

---

## 4. Focus & Lifecycle

### 4.1 Focus Acquisition

TextField becomes focused when:

* Pointer **Down** inside its rect (primary button), and it’s enabled.
* Focus traversal (Tab / Shift+Tab) chooses it (via App Framework rules).

On focus:

* `state.is_focused = true`

* If `config.select_all_on_focus` and the field was previously unfocused:

  ```rust
  state.selection.anchor = 0;
  state.selection.head   = state.document.len();
  state.cursor.pos       = state.document.len();
  ```

* `state.caret_visible = true`

* `state.last_blink = now`

### 4.2 Blur

On blur (focus moved elsewhere or widget disabled):

* `state.is_focused = false`
* `state.drag_mode = TextFieldDragMode::None`
* Selection is retained (copy-on-blur is allowed); caret is simply not painted.

### 4.3 Enabled vs Read-Only

* `is_enabled == false`:

    * No pointer focus acquisition.
    * No edits allowed.
    * Selection and caret movement may be allowed **only** via pointer (like a read-only label) if we decide so, but by default for TextField we can say: **no interaction**, it’s visually disabled.

* `is_read_only == true` while enabled:

    * Focus allowed.
    * Selection allowed (pointer & keyboard).
    * Editing calls (insert/delete) are ignored or short-circuited.

This detail is important for search bars and logs.

---

## 5. Pointer Interaction Semantics

### 5.1 Single Click (Primary Button, No Modifiers)

On `PointerDown` with primary button inside widget rect:

1. Request focus (if enabled).

2. Compute `pos = hit_test_local_to_index(local_x, local_y)` as per §3.

3. If `Shift` is not pressed:

    * Collapse selection:

      ```rust
      state.selection.anchor = pos;
      state.selection.head   = pos;
      state.cursor.pos       = pos;
      state.cursor.preferred_x = None;
      ```

4. If `Shift` **is** pressed:

    * If selection is empty:

        * Anchor = old `cursor.pos`, head = `pos`.

    * Else: anchor stays, only head moves to `pos`.

5. Set drag mode:

   ```rust
   state.drag_mode = TextFieldDragMode::Character { anchor: state.selection.anchor };
   ```

6. Capture pointer (primary button) via EventContext.

### 5.2 Drag (Character-wise)

On `PointerMove` while primary button down:

* If `drag_mode == Character { anchor }`:

    1. Compute `pos` via hit-test.
    2. Clamp as usual.
    3. Update:

       ```rust
       state.selection.anchor = anchor;
       state.selection.head   = pos;
       state.cursor.pos       = pos;
       state.cursor.preferred_x = None;
       ```

* Selection is purely expanded/shrunk; anchor does not move.

### 5.3 Double-Click (Word Selection)

Double-click uses **existing click-count** from fused events + optional `PointerEventKind::DoubleClick`, but semantics are:

On double-click at `(local_x, local_y)`:

1. Compute `pos` via hit-test.

2. Compute word boundaries per SPEC-HAL-052:

   ```rust
   let wb = find_word_at(boundaries, text, pos);
   ```

3. Set selection and cursor:

   ```rust
   state.selection.anchor = wb.start;
   state.selection.head   = wb.end;
   state.cursor.pos       = wb.end;
   state.cursor.preferred_x = None;
   ```

4. Drag mode:

   ```rust
   state.drag_mode = TextFieldDragMode::Word { initial: wb };
   ```

### 5.4 Double-Click + Drag (Word-wise)

On `PointerMove` while primary button down and `drag_mode == Word { initial }`:

1. Compute `pos` via hit-test.

2. Compute current word boundary:

   ```rust
   let current = find_word_at(boundaries, text, pos);
   ```

3. Determine range:

    * If `current.start <= initial.start`:

      ```rust
      start = current.start;
      end   = initial.end;
      ```

    * Else:

      ```rust
      start = initial.start;
      end   = current.end;
      ```

4. Update:

   ```rust
   state.selection.anchor = start;
   state.selection.head   = end;
   state.cursor.pos       = end;
   state.cursor.preferred_x = None;
   ```

### 5.5 Triple-Click (Line Selection)

For TextField, there is exactly **one logical line** (line 0).

On triple-click:

1. `line_index = 0`.

2. Compute logical line boundaries per SPEC-HAL-052:

   ```rust
   let lb = find_line_boundary(boundaries, line_index);
   ```

   For TextField, this is basically `[0, text.len())` unless we allow internal `\n`, but TextField semantics say: no newline insertion.

3. Set selection:

   ```rust
   state.selection.anchor = lb.start; // usually 0
   state.selection.head   = lb.end;   // usually text.len()
   state.cursor.pos       = lb.end;
   state.cursor.preferred_x = None;
   ```

4. Drag mode:

   ```rust
   state.drag_mode = TextFieldDragMode::Line { anchor_line: line_index };
   ```

### 5.6 Triple-Click + Drag (Line-wise)

For single-line TextField, triple-click drag degenerates to “select-all and stay that way”; dragging has no new line to expand to.

Implementation:

* `drag_mode == Line { anchor_line: 0 }` and any drag keep selection at `[0, text.len())`.

This keeps behavior consistent with multi-line text areas but trivial here.

### 5.7 Pointer Up

On `PointerUp` (primary button):

* `state.drag_mode = TextFieldDragMode::None`
* Release pointer capture.

---

## 6. Keyboard Semantics (Single-Line)

Keyboard behavior is strictly derived from SPEC-HAL-052 with **single-line constraints**.

### 6.1 Movement Without Shift

If `is_focused && is_enabled`:

* `Left` / `Right`:

    * Use `move_left_collapse`, `move_right_collapse` from editing.rs.
    * This handles breaking out from selection: Left → start, Right → end.

* `Home`:

    * Move to logical line start (`move_home`).

* `End`:

    * Move to logical line end (`move_end`).

* `Ctrl+Left` / `Ctrl+Right`:

    * `move_word_left`, `move_word_right`.

* `Up` / `Down`:

    * For TextField, **no vertical movement** is needed.
    * Options:

        * No-op (recommended default).
        * Or later: pass focus upward/downward via App Framework (but that’s outside this SPEC).

After any movement without Shift:

* Selection cleared.
* `cursor.pos` set by the editing function.
* `cursor.preferred_x` updated inside editing.rs as needed.

### 6.2 Movement With Shift

Same as SPEC-HAL-052:

* First Shift-movement:

    * Anchor = old `cursor.pos`.
    * Head = new position.

* Subsequent Shift-movement:

    * Anchor unchanged, head moves.

`Shift` variants:

* `Shift+Left` / `Shift+Right` → `move_left_select`, `move_right_select`.
* `Ctrl+Shift+Left` / `Ctrl+Shift+Right` → `move_word_left_select`, `move_word_right_select`.
* `Shift+Home` / `Shift+End` → `move_home_select`, `move_end_select`.

### 6.3 Deletion

TextField never inserts newlines. So:

* `Enter` never inserts `\n`.

Deletion operations:

* If selection non-empty:

    * Backspace/Delete both delete `[start, end)` and collapse at `start`.

* If selection empty:

    * `Backspace`:

        * Delete cluster before cursor (`TextEditOp::Backspace` / `apply_edit_op` or `apply_delete_word_back` when with Ctrl).

    * `Delete`:

        * Delete cluster at cursor (`TextEditOp::Delete` / `apply_delete_word_forward` when with Ctrl).

All mutation calls must:

1. Use the **existing** `apply_*` functions from editing.rs with **fresh cluster boundaries**.
2. On success:

    * `document.mark_modified()`
    * `layout.invalidate()`
    * `cx.invalidate_layout()` (or an equivalent “needs layout” flag).

### 6.4 Character Input

On `KeyCode::Character(ch)` or text input events:

* If `is_read_only`: ignore.

* If `max_len_bytes` is set:

    * Compute potential new length; if it’d exceed, ignore or partially accept (up to max) — define this clearly in implementation.

* Insert via:

  ```rust
  let boundaries_vec = state.layout.boundaries().to_vec();
  apply_insert_op(
      state.document.content_mut(),
      &mut state.selection,
      &mut state.cursor,
      &boundaries_vec,
      &s,
  );
  ```

Then mark document modified, invalidate layout, reset caret blink.

### 6.5 Enter Key

TextField is always single-line.

On `Enter`:

* If `config.commit_on_enter` is true:

    * Do **not** insert newline.
    * Invoke an action or callback:

        * Example: `cx.emit_action(TextFieldCommit(self.id))` or use Action System.

* If `commit_on_enter` is false:

    * Recommended: no-op in TextField.
    * Multi-line widgets will handle newline insertion.

---

## 7. Horizontal Scrolling Behavior

TextField must maintain `scroll_x` such that the caret and (optionally) selection remain visible.

### 7.1 Content Width

After layout rebuild (`TextLayoutCache::rebuild`):

* `state.content_width = layout.content_width()`

### 7.2 Visible Region

Text region width: `text_width`.

Visible horizontal range in text space:

```text
visible_start = scroll_x
visible_end   = scroll_x + text_width
```

### 7.3 Keeping Caret Visible

After any operation that changes `cursor.pos`:

1. Find caret cluster:

   ```rust
   let boundaries = state.layout.boundaries();
   let cluster = visual_cluster_for_pos(boundaries, state.cursor.pos);
   let caret_x = cluster.x_logical;
   ```

2. Define a small margin inside visible region (optional, but nice): `margin = text_width * 0.1` clamped to some min/max.

3. Adjust `scroll_x`:

    * If `caret_x < visible_start + margin`:

      ```rust
      scroll_x = (caret_x - margin).max(0.0);
      ```

    * If `caret_x > visible_end - margin`:

      ```rust
      scroll_x = (caret_x + margin - text_width).max(0.0);
      ```

4. Clamp:

   ```rust
   let max_scroll = (state.content_width - text_width).max(0.0);
   scroll_x = scroll_x.clamp(0.0, max_scroll);
   ```

**Resize boundary clarification — JC incident contract, 2026-10-24:** Section
7.1 uses the existing layout-cache scroll extent, which already includes trailing
caret space; the glyph-boundary width used for text placement is unchanged.
Visibility considers the complete caret interval and is settled after viewport
resize as well as cursor movement. Clamp to the current extent before early
returns; optional margins remain best-effort within that bound. Full containment
is required whenever the text viewport is at least as wide as the caret. Below
that width, including zero, scrolling remains finite, nonnegative and bounded,
without claiming impossible full visibility. Shrink and restoration do not change
text, cursor, selection, focus, Capture or callbacks.

### 7.4 Hit-Testing with Scroll

Reminder: hit-test uses `x_for_hit = tx + scroll_x`.
So clicking to the right of visible area will move caret near the rightmost visible cluster; text will scroll as the caret moves.

---

## 8. Painting Semantics

Painting has three layers:

1. **Background + border**
2. **Selection + text**
3. **Caret + debug overlay (optional)**

### 8.1 Background & Border

* Background color chosen from:

    * `bg_disabled` if not enabled.
    * `bg_focused` if focused and enabled.
    * Otherwise `bg_normal`.

* Border:

    * `border_focused` if focused & enabled.
    * `border_error` if external validation marks field as invalid.
    * `border_normal` otherwise.

Rounded rect and radii come from typography/layout SPECs; TextField just consumes.

### 8.2 Selection Painting

Text selection is drawn as a solid rect under the text.

Given:

* `selection.start()`, `selection.end()` from SPEC-HAL-052 helper.
* Single line `line_index = 0`.

Steps:

1. Get clusters:

   ```rust
   let boundaries = layout.boundaries();
   let lines = layout.lines(); // one line
   let line = &lines[0];
   ```

2. Find the first cluster that intersects selection:

    * A cluster intersects `[sel_start, sel_end)` if:

      ```text
      cluster.byte_end > sel_start && cluster.byte_start < sel_end
      ```

3. Compute `x0` and `x1` in text space:

    * `x0` = `x_logical` of the first intersecting cluster (or caret’s cluster if selection is degenerate).
    * `x1` =

        * Either `x_logical` of the first cluster whose `byte_start >= sel_end`,
        * Or `line.width` if selection goes to the end.

4. Convert to widget coordinates, accounting for scroll:

   ```rust
   let visible_x0 = x0 - scroll_x;
   let visible_x1 = x1 - scroll_x;
   ```

5. Clamp to visible region `[0, text_width]` and draw a rect:

   ```rust
   let sel_rect = RectF::new(
       R.x + padding + visible_x0.max(0.0),
       R.y + padding,
       (visible_x1 - visible_x0).max(2.0).min(text_width),
       line.height,
   );
   ```

6. Use `colors.selection` for fill and a Z-order above background but below text.

### 8.3 Text Painting

Text is drawn once, scrolled horizontally.

1. Determine the displayed string:

    * If `is_password`:

        * Do **not** change underlying text, but shape a string of mask characters of equal grapheme count (e.g. `"●●●●"`).
        * Or use a single glyph repeated per cluster.

    * Else:

        * Use `state.document.text()`.

2. Use layout lines and runs:

    * `TextLayoutCache` should already be computed for the full line.
    * Instead of reshaping substrings, the ideal is to reuse existing glyph runs and draw them offset by `-scroll_x`.

3. Effective draw offset:

   ```text
   draw_x = R.x + padding - scroll_x
   draw_y = R.y + padding
   ```

4. For each run in the single line:

    * Ask text engine to draw it at `(draw_x, draw_y)`, with `colors.text_normal` or `colors.text_disabled`.

Placeholder:

* If document is empty and placeholder is set and field is not focused:

    * Draw placeholder text instead of actual text, **without** scrolling, in `colors.placeholder`.

### 8.4 Caret Painting

Caret is shown if:

* `is_focused == true`
* `is_enabled == true`
* `selection.is_empty() == true`
* `caret_visible == true`

Steps:

1. Get caret cluster for current `cursor.pos`:

   ```rust
   let boundaries = layout.boundaries();
   let cluster = visual_cluster_for_pos(boundaries, state.cursor.pos);
   ```

2. Compute x in text space and then in widget space:

   ```text
   caret_x_text = cluster.x_logical
   caret_x_vis  = caret_x_text - scroll_x
   ```

3. Caret rect:

   ```rust
   let line = &layout.lines()[0];
   let caret_rect = RectF::new(
       R.x + padding + caret_x_vis,
       R.y + padding,
       1.5,               // caret thickness (from typography SPEC)
       line.height,
   );
   ```

4. Draw with `colors.caret`.

Blinking:

* `update_blink()` toggles `caret_visible` every `blink_interval` when focused.
* Any cursor movement / selection change calls `reset_blink()` to force caret visible.

### 8.4.1 Optional smooth visual caret — prototype candidate

**Operative reconciliation:** JC's adopted §13 supersedes the content/script/size admission exclusions in §§8.4.2–8.4.6. Historical prototype wording below is preserved; current timing is governed by §11.5 and §13.

**INV-101-SC-OPT-IN.** An ordinary standalone, non-password TextField with motion enabled under §8.4.5 may interpolate its painted caret under this subsection. The initial motion-enabled frame, mount/replacement, re-enable and requalification after any snap boundary establish the current canonical static caret; none starts an implicit transition. Disabled mode MUST preserve existing editing, painting, blink, demand and invalidation behavior and retain no smooth-caret handle or pending destination.

**INV-101-SC-LOGICAL.** The document, cursor.pos, selection, current cluster, hit testing, undo/redo, callbacks, IME and horizontal scroll/reveal remain immediately authoritative under their existing owners. A sampled visual x MUST NOT feed any of them. The canonical destination is the fresh current cluster's unscrolled text-space x computed by the existing TextField mapping. Only the painted caret x may differ temporarily. The usual padding/alignment/scroll conversion, clipping, thickness, height, pixel snapping and blink visibility still apply; this mode does not animate text, selection, opacity, width, height or scroll.

**INV-101-SC-SOURCE.** Eligibility MUST be established from the same live TextField instance, current framework-qualified focus and active keyboard Capture in the same interaction domain, enabled/non-password/non-embedded state, collapsed selection both before and after the operation, and fresh unambiguous current document/cache/cluster geometry. Focus alone or an unqualified keyboard-mode flag is insufficient.

An interpolated destination change is ONLY an accepted, non-repeat, unmodified Left, Right, Home or End navigation operation handled by this TextField under its existing editing policy, within the same unchanged document revision and compatible coordinate space. Shift selection and Ctrl/Alt/Meta navigation are static snap boundaries. Every accepted content-changing operation, including TextInputEvent insertion, Space, Backspace, Delete, clipboard cut/paste, undo/redo, programmatic replacement and IME editing, MUST retain immediate logical editing, cancel any earlier smooth projection at the next lawful Update and paint the current canonical static caret. No text content is delayed or animated. Verified repeat follows INV-101-SC-REPEAT and remains static. Pointer placement/drag, selection and all other existing snap/qualification rules remain unchanged. A rejected or no-op ordinary insertion/navigation that is not itself a separately qualified snap boundary MUST NOT create an interpolated destination, cancel an already qualified leg, or restart it. Explicit clipboard, undo and redo command boundaries remain static even if unchanged. Source eligibility derives from the actually accepted owner branch and fresh current cache/cluster evidence, not byte-index difference, cadence, content equality, timing or inferred TextInput origin. An eligible navigation operation must produce a changed fresh canonical x before starting or retargeting a motion leg.

**INV-101-SC-REPEAT.** In a motion-enabled TextField under the existing qualified keyboard Capture path, a down KeyEvent whose is_repeat is true and whose key is Space, Left, Right, Home, End, Backspace or Delete is a static-caret boundary. The event does not gain semantic permission: editing/navigation, policy rejection, consumption, document/cursor/selection, history, callbacks and scrolling retain their existing owners and immediate behavior. The boundary is recorded before local edit-policy early returns; an equal/no-op or rejected verified repeat does not preserve an earlier smooth leg.

The event handler records only the existing bounded snap intent. At the ordinary lawful Widget Update, the TextField MUST cancel and clear its preceding smooth handle and pending destination, then publish the current fresh canonical caret x. It MUST NOT start or retarget interpolation for that repeat. The existing read-only Paint qualification and canonical geometry/cache freshness rules remain in force. This boundary dominates other input in the same pending batch under INV-101-SC-RETARGET.

Later repeats, key release, idle frames, an equal destination, unrelated input or requalification alone MUST NOT resume the cancelled leg. Only a later separately qualified eligible discrete operation, after ordinary boundary reconciliation, may start a new leg from the then-current qualified projection; a KeyEvent with is_repeat true never qualifies. This exception does not infer a repeat classification for a later TextInputEvent or change that event's existing admission. The candidate for those discrete legs remains 65 ms Once/EaseOut. No clock, phase, queue, history replay, local interpolation or additional Widget Update is admitted.

**FORBID-101-SC-REPEAT.** No verified repeat may restart, extend or retain smooth-caret motion. TextInputEvent does not acquire repeat provenance from this exception. Cadence, timestamps, equal characters, text content, a held-key guess or an unproved key/text pairing MUST NOT classify it as repeat. Absent separately admitted owner-qualified insertion provenance, its current ordinary text-insertion path remains unchanged and held-insertion correction MUST NOT be claimed. This amendment admits no new input transport or public event representation.

**INV-101-SC-P4-EDIT-COHERENCE.** Smooth visual-caret interpolation MUST NOT cross a document revision or an accepted content mutation. For every accepted content mutation the existing bounded snap intent MUST dominate any pending navigation destination in that same pre-Update batch; the current fresh canonical caret is published statically at the next ordinary Update, and any preexisting smooth handle is cancelled there. The first consuming Paint MUST suppress an obsolete visual projection if a later canonical Layout reveals revision or coordinate-space invalidity. A later eligible non-repeat navigation may start a new leg only after the snap boundary has been reconciled by an ordinary Update and from the then-current qualified static projection. Rejected/no-op content operations without an independently specified snap boundary preserve the existing qualified leg; they cannot create new motion. Disabled mode and existing IME, selection, Capture, scrolling, geometry, availability and mount-lifetime guarantees are unchanged. No TextInputEvent repeat provenance or text animation is required or admitted.

**FORBID-101-SC-P4.** No content change may create, restart, extend or retain a smooth caret leg or show a painted caret detached from the immediately displayed canonical text boundary. No same-batch navigation may override a content-edit snap. No guessed keyboard-text association, renderer/text animation, new clock/phase, other-widget changes or G1 changes.

The owner MUST qualify the destination with current document/cache identity and legal cursor boundary, and qualify the origin with its own prior valid presentation and unchanged coordinate space. A fresh cache for a proven eligible navigation may establish a new destination generation; this does not retrospectively relabel old document data as current. Canonical rect/text viewport, scale, font/metrics, padding/alignment, caret metrics, clipping and scroll dependencies must be known compatible. Missing, stale, non-finite or ambiguous evidence forbids interpolation. Only owned scalar/ID/revision/qualification values may persist; no layout, cluster, text-run, widget or context borrow may persist.

**INV-101-SC-RETARGET.** The TextField owns at most one pending latest destination and one active finite smooth-caret handle. Accepted eligible operations between two Updates coalesce to the final current destination. A disqualifying boundary dominates later input within that same pending batch; later typing cannot resurrect an invalid origin. No queue, per-character path, delayed logical operation or replay history is allowed.

During Update, an eligible changed destination replaces the previous effect from the current qualified visual value: first sample the existing handle at the current engine frame time (or use the qualified current cached presentation if settled), then cancel the superseded handle, start the new Once leg, and publish its exact from value for that start frame. “Current displayed value” means this owner-qualified displayable projection, not an old logical endpoint and not a GPU/display-readback claim. If no valid origin exists, snap to the current canonical destination instead. An equal destination does not restart an effect or add work; an already active leg continues under its existing descriptor.

**INV-101-SC-TIME.** This experiment's single candidate is a positive finite 65 ms Once transition using the existing Timeline Easing::EaseOut, whose current deterministic curve is E(t) = 1 - (1 - t)^2 for t in [0,1]. It is evaluated only by the existing engine Timeline. It is the default for qualified TextField navigation under §8.4.5, not a framework-wide animation default. There is no overshoot, spring, loop, alternate mode, additional delay or per-input frame request. With no later eligible retarget, the next admitted sample at or after 65 ms MUST expose the exact destination and end this motion channel. A retarget may extend movement only because new accepted input supplied a new destination; it never drains earlier destinations. Changing the candidate requires an attributable experiment decision and new evidence, not silent parameter tuning.

**INV-101-SC-SNAP.** Maintaining an already collapsed anchor/head together with the immediate logical cursor during an eligible operation is not a selection transition. Likewise, a proven eligible unchanged-document navigation changing the canonical destination cluster is not by itself a coordinate-space change; changed rect, viewport, alignment origin, font/metrics, caret dimensions, clip or scroll still is. Any IME preedit/commit/composition, selection creation/expansion/contraction/clearing/drag or any selection-bearing operation, scroll/reveal offset change, coordinate-space geometry/viewport/scale/font/metrics/alignment/clip change, password/embedded mode, opt-in disablement, availability change, focus/Capture/domain transition or suspension, mount/replacement/unmount, ineligible source, or stale/ambiguous/non-finite cache/sample invalidates the smooth projection. It MUST NOT interpolate through that boundary, retain a competing old caret, or resume the old leg after requalification. Current caret visibility and required focus/Capture indication remain governed by the existing contracts; loss of Capture does not make a focused widget show a caret.

For a boundary known before the ordinary Update, that Update MUST cancel/clear the smooth effect and establish the canonical static projection, with no stale leg painted. For invalidity first revealed by later canonical Layout or current Paint inputs, the FIRST consuming Paint MUST select the fresh canonical static caret, never the invalid animated value. Paint and Layout MUST NOT cancel handles or mutate smooth-caret state. The next ordinary Update MUST retire the invalid handle/state before any new eligible leg; when current validity cannot be established without such reconciliation, it MUST choose static state. This is the meaning of immediate snap/cancel at a phase-separated boundary: visible suppression at first consumption, cancellation at the first lawful Update. It adds no callback, second Update or phase. Unmount/replacement MUST also cancel/release every handle belonging to the removed concrete instance through the engine-owned animation lifetime boundary. Same-TreeWidgetId replacement is not identity continuity for this effect. This is a required proof, not a claim that the present replacement path already performs that cleanup. It must not leave anonymous demand.

If canonical text geometry is not yet fresh, this exception provides no substitute coordinate and does not weaken any existing freshness assertion. The unchanged owner must complete its legal layout/cache path before painting the canonical caret. A motion failure must never conceal a baseline text-layout failure.

**INV-101-SC-PHASE.** Existing event handlers and property operations may update their already-authorized logical state and bounded source/snap-intent metadata. Smooth-caret value publication, handle start/replacement/cancellation and motion-state reconciliation occur only in the existing Widget::update; engine lifetime cleanup retains its existing owner. Timeline advances exactly once at the captured engine timestamp under HAL-021/042. Update sampling is frame-stable. Subsequent Layout/Paint consume the immutable publication, with the read-only static qualification gate above. No caret timer, accumulated-dt interpolation, private clock, Timeline instance, renderer sampling, extra layout or extra frame phase is admitted.

Animation capability unavailable, start refusal, Cancelled, Missing, TypeMismatch, invalid qualification and Completed are distinct outcomes. Known start/sampling failure MUST cancel/clear the motion channel at the lawful owner boundary and choose the current canonical static caret; no local-animation fallback, fabricated success or stale-last-value freeze is allowed. Completed publishes the exact terminal value before clearing the owned handle.

**INV-101-SC-DEMAND.** Only a still-qualified active smooth leg contributes continuing motion demand, through the existing scheduler. Changed visual samples and changed terminal/static projection invalidate only the affected TextField paint and necessary ancestor composition; equal samples do not invalidate. Completion, cancellation or suppression contributes no continuing smooth-motion demand. The existing Capture-qualified blink channel remains separate and unchanged; its legitimate demand is not confused with a settled smooth leg. Focus retained after Capture release MUST NOT sustain this new channel. No smooth-caret handle is a reason to dirty layout, repaint unrelated siblings or admit one frame per typed character.

**FORBID-101-SC.** Every behavior forbidden by the above invariants is forbidden even when described as a prototype. In particular: no logical lag, backlog, stale source rebinding, IME/password/selection/scroll animation, ownership-cue tail, read-phase motion mutation, new timing owner, enablement outside §8.4.5, or extension to another widget. This subsection neither adopts HAL-097/097A nor amends HAL-002/002A.

---

### 8.4.2 Experimental shared caret-boundary reveal and conceal

**INV-101-CB-SCOPE.** Only a motion-enabled ordinary standalone, non-password, non-embedded TextField under §8.4.5 may use this subsection. For operations actually qualified below, this subsection is the sole exception to §8.4.1's caret-only/text-animation prohibition, static content-edit rule, unchanged-document restriction and verified-repeat static rule in INV-101-SC-LOGICAL, INV/FORBID-101-SC-REPEAT, INV-101-SC-P4-EDIT-COHERENCE and FORBID-101-SC-P4. Their immediate logical editing, input provenance, Capture, safety, phase, lifetime and disabled-mode rules remain unchanged. All unqualified operations keep their existing canonical behavior. This subsection does not adopt P3, the proposed P5 opacity effect or HAL097/097A.

**INV-101-CB-CANONICAL.** Document mutation, cursor, selection, history, callbacks, hit testing, IME and horizontal focus visibility remain immediate and canonical. A moving boundary never feeds them. The sole transient presentation is one text-native caret at boundary B and whole-string glyph ink visible on the left side of that same B. No character position, glyph advance, baseline, alpha or logical operation is animated.

**INV-101-CB-ELIGIBILITY.** This first prototype admits only an actually accepted ordinary append through TextInput or Space with the collapsed cursor at document end, or an actually accepted unmodified Backspace suffix deletion from document end. Both sides must remain collapsed, one-line and left-aligned, under current qualified focus/Capture/window/domain/concrete mount and unchanged viewport, scale, font/metrics, alignment and scroll. The complete compared sources must contain only printable ASCII U+0020–U+007E, so mixed/unknown bidi is not inferred from TextDirection::detect's first-strong result. This is an explicit prototype admission bound, not a change to editing support: all other text continues to edit and paint canonically. Shape the complete old and current strings through the same existing text owner; require equal actual face/spacing binding, metrics and origin, exact unchanged surviving-prefix glyph IDs/positions/advances and a separable actual source-cluster join. A cluster crossing the edit join, changed surviving shaping, missing exact evidence or nonfinite geometry requires canonical snap. No substring shaping or glyph-ordinal source inference is permitted.

**INV-101-CB-SPATIAL.** The reveal/conceal is an expressly admitted SPATIAL ink clip, not a per-source-range foreground mask or proof of glyph-ink isolation. Partial glyph ink at the moving clip edge, including overhang across that edge, is the defined wipe. Source clusters crossing the edit join or changed surviving shaping still fail qualification; advance bounds MUST NOT be asserted to be ink bounds. Preserve the existing complete-string shape and raster backing. Emit one complete current string for append, or one complete owned historical string for conceal, under an existing rectangular ClipNode intersected with the ordinary text viewport. Emit the text-native caret separately outside that moving clip but inside the ordinary viewport, with its left edge and the clip's right edge equal to the same physically snapped absolute B. Caret-only background Z is strictly above glyph Z. No layered old/new run overlay, background-color eraser, widget glyph injection or new renderer primitive is admitted.

**INV-101-CB-SOURCE.** For append, the visible source is the complete current document; inserted ink is revealed behind B as B moves right. For Backspace, the owner may retain at most one owned complete pre-deletion source string, its exact original revision and current-prefix association, solely as nonsemantic transient visual evidence. Its complete shape is regenerated ephemerally by the existing text owner; no historical glyph, layout, cluster or context reference persists. The current logical document must remain an exact qualified surviving prefix of that source and determine the latest target B. As B moves left, the historical suffix is concealed. At settlement the historical source is dropped and only the current document is drawn. The constant-count source copy is bounded by that admitted existing document's byte length, does not grow during concealment, and is not history, undo state, a semantic value or an independently reusable text source.

**INV-101-CB-LATEST.** Own at most one pending latest descriptor and one finite active transition. The prototype boundary uses one 55 ms Once/EaseOut leg in the existing Timeline under §11.5; this is the TextField end-edit duration under §8.4.5, not a framework-wide animation default. Multiple qualified same-direction operations coalesce to the latest current target and retarget from the current qualified sampled B, never from an old logical endpoint. For repeated Backspace, reuse the single still-qualified historical source while the current document remains its matching prefix; do not retain a sequence of deleted strings. Direction reversal, incompatible source/geometry or an operation that cannot coalesce coherently snaps to current canonical text/caret and discards the old visual transition. No pending characters, per-event legs, catch-up replay or inferred TextInput repeat provenance exist. Rejected/no-op edits create no new transition; independently required snap boundaries still apply.

**INV-101-CB-SAFETY-PHASE.** Existing event/property owners may record only accepted source/range/revision and bounded intent. Only ordinary Update starts, samples, replaces or cancels the mount-qualified Timeline handle and publishes immutable B/source qualification. Read-only Paint shapes complete sources ephemerally, qualifies current geometry/source and emits the existing scene/clip/text operations. Changed samples invalidate only the TextField paint and required ancestor composition, not layout. Changed shaping, invalid cache, IME/preedit/commit, selection or pointer placement/drag, Delete/word-delete, clipboard/undo/redo/programmatic replacement, scrolling/reveal, geometry/font/theme/scale, availability, focus/Capture/domain or mount changes suppress the effect at first consumption and cancel/release it at the next lawful existing owner boundary. All known animation failure or missing capability uses full current canonical text/caret. No requalification or idle frame resurrects an old leg. No new clock, phase, callback, sampling owner or lifetime mechanism is admitted. Capture-qualified blink remains unchanged.

**FORBID-101-CB.** No caret chasing already fully exposed inserted characters, delayed logical edits, repeat backlog, semantic use of deleted visual text, stale source rebinding, retained shaped output, substring reshaping, guessed crossing-cluster geometry, whole-field opacity, input transport, public text schema, renderer backend, general animation facility, other-widget or G1 change. Disabled mode retains ordinary behavior and no effect handle/source allocation or continuing demand. Existing renderer clipping, whole-string shaping and complete-text realization laws remain intact. Native JC visual approval is required; compilation or smoke success is not visual acceptance.

P6 attribution — 2026-10-29: JC's caret-driven reveal/conceal experiment; exact HAL-101-CB-REVEAL-CONCEAL-P6-20261029 subsection explicitly approved by the supervising technical reviewer, with JC's final veto retained. P6 takes priority over P2/P4 only for its qualified opt-in transitions. Implementation and native visual acceptance are separate; the proposed P5 opacity route remains inactive.

Candidate duration amendment — 2026-10-29: JC explicitly directed “make it 60”. The P6 reveal/conceal candidate is therefore 60 ms Once/EaseOut; all other P6 rules and the separate 65 ms navigation candidate remain unchanged.

### 8.4.3 Experimental bounded range editing — P7

**INV-101-RANGE-SCOPE.** This subsection extends only a motion-enabled ordinary standalone, non-password, non-embedded, single-line, left-aligned TextField. P6's existing end-append/unmodified end-Backspace presentation and 55 ms candidate under §11.5 remain unchanged by this subsection. For operations actually qualified below, this subsection additionally excepts §8.4.1/§8.4.2's selection-bearing, word-delete and clipboard snap rules, caret-only/no-character-position restriction and prohibition of multiple old/current text draws, solely to admit the two complementary whole-string presentation draws defined here. All other P2/P4/P6 logical, safety, disabled, phase, Capture and lifetime laws remain binding.

**INV-101-RANGE-EDIT.** Eligibility derives only from an actually accepted existing owner branch: macOS Option+Backspace without Ctrl/Meta/Shift, Windows/Linux Ctrl+Backspace without Alt/Meta/Shift; deletion of an existing selected range by Backspace, Delete or the existing Cut command; or the existing Paste command inserting into a collapsed selection. Platform names qualify presentation only and do not change which existing commands edit. Word deletion and selected deletion must be pure deletion; Paste must be pure insertion after the existing normalization/admission. Selection-replacing Paste, forward word-Delete, undo/redo, IME, programmatic replacement and other compound operations remain canonical snap boundaries. Clipboard writes/reads, document, cursor, selection collapse, history, callbacks and hit testing occur immediately under their existing owners.

**INV-101-RANGE-BOUND.** Both complete compared sources must be printable ASCII U+0020–U+007E. The total inserted or removed span represented by one transition must contain from 1 through 32 bytes, and its canonical horizontal extent must not exceed one half of the current text viewport width. These are explicit prototype admission limits, not editing limits. Both edit joins must be unambiguous actual cluster boundaries. Nonfinite, longer, Unicode, complex/bidi, wrapping, ligature-crossing, changed-surviving-shaping or otherwise ambiguous cases edit normally and snap to complete canonical text/caret. No best-effort partial eligibility is admitted.

**INV-101-RANGE-SOURCE.** Record the accepted operation, exact old/new revisions and byte ranges, current concrete mount and compatible geometry/Capture facts. The new source must equal the exact unchanged old prefix and suffix around that operation, with only its admitted insertion or deletion between them. Shape each complete string through the same existing text owner and require identical actual face/spacing binding and metrics. Require identical surviving-prefix glyph/source attribution and positions, and identical surviving-suffix glyph/source attribution, IDs, y and advances with only the exact constant x difference caused by the edit. Glyph-to-source attribution must come from the text owner's actual full-string shaping cluster data; glyph ordinal, glyph count, nearest x or guessed source association is insufficient. A cluster crossing either join or any mismatch suppresses presentation.

**INV-101-RANGE-ORIGIN.** A new range transition may start only from an owned previously qualified static projection of the same pre-edit active caret, revision, concrete mount and coordinate space, or from the same qualified active range transition when coalescing below. For deletion the old active caret/projection must be at the removed range's right endpoint; for collapsed Paste it must be at the insertion join. A backward selection whose active caret is already at the surviving left endpoint therefore snaps; no synthetic jump to the far end is admitted. An ordinary Update may record the owned static active-head geometry/facts of a selected TextField for this qualification without creating a motion handle or demand. Pointer/selection establishment must first reconcile statically. An admitted range edit may bypass only its own expected modifier/clipboard/selection-collapse snap; it must never clear a pending unrelated safety snap or resurrect a disqualified origin.

**INV-101-RANGE-PRESENTATION.** Let B be the one published text-space moving boundary, n the current canonical post-edit join/caret x and d = B - n. Convert B through the unchanged text origin/scroll to one physically snapped absolute clip/caret edge. For deletion, draw the complete retained old source at the ordinary text origin only to the left of that edge, and draw the complete current source at ordinary x plus d only to its right. For insertion, use the complete current source for both complementary draws, the left at ordinary x and the right at ordinary x plus d. The surviving suffix's translated join is n + d = B; at settlement d is zero and only the ordinary complete current draw remains. These are complementary, non-overlapping rectangular clips within the ordinary viewport, not overlaid whole runs or substring shapes. Uniform suffix presentation translation is the only additionally admitted position change; canonical glyph advances, shaping, layout and all interaction geometry remain unchanged. This is the same explicitly spatial ink wipe as P6: partial ink/overhang at the moving edge is intentional, and advance bounds are not asserted to be ink bounds. Draw the existing separate text-native caret at that same edge above both glyph draws. No new primitive, text schema, glyph injection, opacity effect or eraser is admitted.

**INV-101-RANGE-LATEST.** Use one pending latest descriptor, one existing mount-qualified Timeline handle and one finite Once/EaseOut leg whose range-only duration is governed by §8.4.4. At most one owned complete original source may be retained, bounded by that already admitted document's byte length; the current source remains the canonical document. No shaped output persists. Adjacent repeated backward-word deletions may extend the same original deletion span, and adjacent collapsed Pastes may extend the same insertion span, only while the exact original prefix/suffix association, direction, origin and aggregate limits remain qualified. Retarget from the current qualified sampled B; never queue operations or retain a series of source strings. Direction reversal, mixed operation kinds, incompatible association or exceeded bounds snaps and retires the transient source. A rejected/no-op operation creates no motion; existing independently required snap boundaries still apply.

**INV-101-RANGE-SAFETY.** Event/property owners record only existing immediate editing plus bounded owned intent. Only the ordinary Update reconciles, starts, samples, replaces or cancels the one Timeline leg and publishes immutable presentation. Read-only Paint requalifies complete current/old sources and geometry before emitting existing text/clip operations. A later invalidity causes the first consuming Paint to draw full current canonical text/caret; the next lawful Update cancels the handle and drops transient state. All P6 IME, pointer, scroll/reveal, font/theme/scale/viewport, availability, Capture/domain, mount/replacement and failure boundaries remain. No stale leg resumes after requalification. Disabled mode acquires no range source, shaping work, handle or continuing demand. Completion releases the source and motion demand.

**FORBID-101-RANGE.** No delayed semantic edit, clipboard/history replay, noncanonical hit/selection/IME, repeat backlog, unbounded source retention, substring reshaping, guessed cluster attribution, arbitrary transform, source-range ink-isolation claim, new clock/phase, input/renderer transport, public text API, other-widget or G1 change. Unsupported presentation always preserves the supported canonical edit. Compilation/focused smoke does not establish native visual acceptance.

P7 proposal status — 2026-10-29: the bounded range-editing candidate required explicit supervisory technical acceptance under JC's retained final ownership and veto. The proposal alone was inactive; P6's existing 60 ms baseline was unchanged.

Adoption — 2026-10-29: the supervising technical reviewer explicitly accepted the complete HAL-101-EDITING-RANGE-P7-20261029 packet, including its bounds, source qualification, complementary whole-string presentation and coordinated private HAL-048 helper. These scoped additions are governing; implementation and native JC visual acceptance remain separate, and JC retains final ownership/veto.

### 8.4.4 Experimental distance-qualified range timing — P8

**INV-101-RANGE-TIME-SCOPE.** Only the already qualified §8.4.3 range descriptor uses this subsection. The §8.4.2 P6 end-edit candidate is exactly 55 ms Once/EaseOut under §11.5, and the separately admitted navigation candidate remains65ms. Operation provenance, source/cluster/geometry proofs, 1–32-byte and half-viewport limits, one retained original source and all canonical editing/safety/lifetime laws are unchanged. Neither text length nor command naming admits a range.

**INV-101-RANGE-TIME-DISTANCE.** At the ordinary Update cut for an accepted changed latest range target, first sample the preceding qualified handle at the current engine frame timestamp, or use the qualified static origin if no active leg exists. Let from be that displayable published boundary and to the fresh qualified current canonical join. Define d=abs(to-from) in LOGICAL TEXT-SPACE PIXELS. The unchanged draw origin makes this the remaining owner-published visual travel. It is not a physical-pixel, GPU or display-readback measurement; ordinary physical snapping remains unchanged. Nonfinite or incompatible evidence snaps canonically. Equal endpoints start no new leg.

**INV-101-RANGE-TIME-CANDIDATE.** This prototype's proposed candidate, effective only upon explicit owner acceptance, is T_ms(d)=clamp(80+1.25*d,100,220) for a finite positive d, converted once to positive finite Timeline seconds. The finite minimum is100ms and maximum220ms. Use the existing Once mode and Easing::EaseOut with E(t)=1-(1-t)^2. These are one attributable native-evaluation candidate, not a universal timing law or an executor-adjustable default. No character-count multiplier, per-byte duration, delay, alternate easing, overshoot or duration accumulation is admitted.

**INV-101-RANGE-TIME-RETARGET.** Fix that duration in the admitted descriptor at start. An active unchanged target keeps its existing handle, duration and deadline; idle samples never recalculate them. A genuinely changed qualified latest target samples current B, cancels the superseded leg and derives one new bounded duration from its remaining travel. Publish exact from on that start frame. Coalescing, no-backlog and all failure/snap rules remain unchanged. With no further accepted retarget, the first admitted sample at or after the selected duration exposes the exact current target and releases the old source and motion demand.

**FORBID-101-RANGE-TIME.** No P6 duration change, independent clock/advance/dt interpolation, semantic lag, queued paths, Paint-side timing mutation, default enablement outside §8.4.5, source-bound relaxation, other-widget or G1 change. Compilation and focused smoke remain separate from JC native visual acceptance.

P8 attribution —2026-10-29: proposed for JC's native feedback that fixed60ms range motion is too fast; requires explicit supervising technical acceptance, with JC final veto. Existing P6 native-approved60ms is preserved.

Adoption — 2026-10-29: the supervising technical reviewer explicitly approved exact Part A of HAL-101-P8-RANGE-TIMING-AND-DEFAULT-POLICY-20261029, including the formula, bounds and existing Once/EaseOut. JC retains final ownership/veto and native visual acceptance. Part B was initially accepted only as policy direction; that deferral is superseded by JC's explicit activation recorded in §8.4.5.

### 8.4.5 Standard TextField motion eligibility and plain compatibility

**INV-101-MOTION-DEFAULT-ACTIVATION.** JC's explicit direct owner activation on 2026-10-29 makes ordinary TextField construction and Default enable the existing admitted motion policy unless explicitly opted out. No further approval or preceding native-evaluation gate is required for this activation. This is the sole scoped exception to the former disabled-default/explicit-opt-in wording in §2.3 and §§8.4.1–8.4.4, their automatic-enablement prohibitions, and the corresponding opt-in scope phrase of HAL-048's accepted typography contract. Unrelated widget policies and Draft/Proposed statuses do not change.

**INV-101-MOTION-DEFAULT-SCOPE.** Default enablement, when activated, covers the CURRENT admitted P2 navigation, P6 end edits and P7 range classes, each under its own qualification. It does not silently impose P6/P7's ASCII/left-alignment/editable restrictions on navigation. Thus read-only, Unicode and non-left-aligned fields may retain their already admitted nonrepeat navigation smoothing when otherwise qualified, while their content edits remain restricted or canonical according to existing policy. Property-cell text/integer/float editors and agent-built ordinary TextFields are ordinary consumers of this same rule; no surrounding UI or factory name grants an undocumented exemption. Password and explicitly embedded editors remain excluded; IME/composition, Capture/focus/domain, geometry/scroll and ambiguous-source boundaries remain static under their current owners.

**INV-101-MOTION-PLAIN.** Preserve the public boolean builder, setter and getter. Explicit with_smooth_caret_prototype(false) or set_smooth_caret_prototype(false) selects ordinary static presentation, prevents new motion and persists until another explicit setting. Runtime opt-out suppresses obsolete projection at first consumption and cancels/releases at the next lawful Update. Mount, re-enable and requalification establish static canonical state before any accepted operation can animate. Explicit true retains the same admitted policy and does not bypass eligibility. Normal Showcase TextFields use ordinary default construction. JC's subsequent direct instruction removes the temporary ordinary/animated comparison row entirely; no demo-only opt-in substitutes for default activation.

**INV-101-MOTION-DEFAULT-NO-USE.** Default enablement is not a reason for unconditional source scans, full shaping, old-source allocation, Timeline admission, layout invalidation or continuing frames. An explicit plain or currently password/embedded/disabled/composing/non-Capture/inactive instance with no old channel to retire must short-circuit presentation work after only the necessary bounded policy/appearance checks, before motion-specific document scans, geometry qualification or shaping. Existing baseline editing/layout/paint/blink work is not removed or claimed to be absent. An instance losing qualification retires existing motion through its lawful Update/lifetime owner; only eligible active legs add motion demand. Focus retained after Capture release cannot sustain an invisible motion channel. No missing authority/publication may be treated as eligibility.

**FORBID-101-MOTION-DEFAULT.** No removal of plain mode, automatic re-enable after explicit opt-out, embedded/password/IME policy expansion, guessed source eligibility, new clock/phase, always-on motion demand, other-widget/G1 change or claim that a constructor flag flip alone proves performance/compatibility. Default activation is an intentional observable appearance change, not backward-identical rendering.

Activation attribution — 2026-10-29: JC explicitly ordered replacement of normal TextField behavior on that date. The prior default deferral was expressly corrected and is superseded. This activates the previously proposed §8.4.5 policy with the direct activation and comparison-removal decisions above. JC retains final ownership/veto. Existing P2 65 ms, P6 60 ms and P7/P8 adaptive timing/source restrictions remain unchanged; implementation proof and native visual evaluation remain distinct.

### 8.4.6 Shared TextField presentation in qualified composite editors

**INV-101-COMPOSITE-SCOPE.** Only the existing single-line text-entry regions of ComboBox, SpinBox, Slider and TagInput may consume the same admitted TextField navigation/end-edit/range presentation under this subsection. This is a scoped exception to the embedded/standalone and other-widget presentation exclusions in section 2.3 and sections 8.4.1-8.4.5, solely after all qualification and ownership requirements below are met. It does not admit TextArea, TextEdit, ActionPalette query editing, picker digit buffers, multiline text entry or a new editor family. The former exclusion remains in force for every unqualified embedded path.

**FORBID-101-COMPOSITE-SCOPE.** No embedded flag, host name, constructor default or factory installation alone grants presentation eligibility; no blanket other-widget or multiline expansion is admitted.

**INV-101-COMPOSITE-OWNER.** WidgetTree remains the sole owner of every Widget instance. Reuse shall follow one expressly admitted implementation path consistent with HAL-002 and HAL-087, with no host-owned peer Widget or cross-widget reference. Exactly one current mounted owner, one current active editing session and its actual text-entry region must qualify each presentation channel. A host mount or TreeWidgetId alone does not identify a replacement or reused editor session. The implementation and its owner-qualified session lifetime require explicit technical admission before code; this subsection does not select them or grant an exception to permanent ownership law.

**FORBID-101-COMPOSITE-OWNER.** No animation owner may be forged from a widget-local ID, recyclable NodeId, bare host ID or invented child mount. Current direct ownership of a TextField Widget is not legalized by this presentation exception.

**INV-101-COMPOSITE-POLICY.** Qualified composite text entry uses the existing TextField motion policy by default, respecting an explicitly selected plain setting without automatic re-enable on edit entry. Preserve all existing operation, repeat, source, shaping, alignment, range, timing and no-backlog conditions. P6/P7 content presentation still requires left alignment and a compatible unchanged text origin; numeric centering is not changed or silently admitted. Existing nonrepeat navigation retains its own broader qualification. An ineligible content edit remains immediate and canonical.

**FORBID-101-COMPOSITE-POLICY.** No duration/easing variant, alignment change, new semantic edit class, automatic motion on programmatic value synchronization, or independent host implementation of TextField motion is admitted.

**INV-101-COMPOSITE-CAPTURE.** Eligibility derives from current framework-qualified Capture, focus, window/domain activity and availability of the actual editing owner, together with the host's already-authorized active text-entry mode. Capture of a composite for popup navigation, numeric stepping, track drag, chip selection or another internal target does not imply text-entry eligibility. The existing HAL-328 owner routes input and releases Capture; a presentation channel cannot acquire, prolong, transfer or resurrect Capture. PM-014 geometric forwarding, cache and cleanup requirements remain binding.

**FORBID-101-COMPOSITE-CAPTURE.** No cached focus boolean, forwarded mode flag, popup state, primary pointer watch or retained previous Capture fact may substitute for current source-qualified keyboard authority.

**INV-101-COMPOSITE-GEOMETRY.** Start or retarget only with the actual text-entry region, text origin, viewport/clip, alignment, scroll, font/scale/theme metrics, fresh document/cache/cluster evidence and concrete owner/session qualification. A host outer rect is not the input-region rect. Layout supplies geometry through the existing lawful geometry owner; no presentation mutation or new timing phase occurs there. Paint requalifies its actual region and draws canonically on any mismatch. Chip insertion/removal/reflow, popup/editor relocation, resize, alignment/scroll/font change or missing geometry suppresses the first obsolete projection and causes cancellation at the next lawful Update. Requalification first establishes a static canonical origin and never resumes an old leg.

**FORBID-101-COMPOSITE-GEOMETRY.** No invented current geometry, copying a host context unchanged for a different field rect, stale cache acceptance, Paint-side reconciliation or guessed cluster join is admitted.

**INV-101-COMPOSITE-UPDATE.** The existing canonical timed Update owner reconciles the shared presentation channel once during its own already-scheduled Update, using the same engine frame time and existing Timeline. Event/property owners retain immediate existing editing and bounded latest intent only. Update alone starts, samples, replaces or cancels the channel and publishes immutable presentation; Layout and Paint consume it read-only. Preserve one latest transition, one bounded source and the existing failure-to-canonical behavior. Only changed presentation invalidates the affected owner paint and required ancestor composition.

**FORBID-101-COMPOSITE-UPDATE.** No second Widget traversal, duplicate update, host clock, per-editor Timeline, paint-started leg, new callback phase or generic animation bus is admitted.

**INV-101-COMPOSITE-LIFETIME.** Edit-session exit, target change/reuse, commit/cancel, field hiding, clearing/replacement, Capture/focus/domain/availability loss, IME, unsafe selection/scroll/geometry change and owner retirement suppress obsolete presentation at first consumption and retire its handle/source through the lawful Update or engine lifetime owner. Same-instance layout take/restore preserves its existing mount qualification; true replacement/removal cannot leave a predecessor handle or let a successor inherit it. Session re-entry begins static. Changes in committed values or selected items remain immediately canonical and cannot reuse deleted visual text as meaning.

**FORBID-101-COMPOSITE-LIFETIME.** No old-source/session resurrection, successor cancellation by a predecessor, retained shaped output, semantic use of transient text or unbounded retired-session history is admitted.

**INV-101-COMPOSITE-SEMANTICS.** Shared motion changes only text/caret presentation. Existing document, cursor, selection, clipboard, undo/redo, hit testing, IME, filtering, resolver inputs/decisions, numeric parsing/clamping, tag creation/selection, notifications and commit/cancel rules remain immediate under their existing owners. Host-intercepted commands do not become accepted TextField operations. Read-only display, increment/decrement, slider movement and popup item changes do not animate toward a value through this exception.

**FORBID-101-COMPOSITE-SEMANTICS.** No delayed emission or commit, inferred intent, text-input provenance guess, filtering/clipboard replay or presentation-controlled numeric/resolver state is admitted.

**INV-101-COMPOSITE-NO-USE.** Plain, password, inactive, nonediting, non-Capture, unavailable, composing, multiline or otherwise unqualified entry with no channel to retire short-circuits motion work before motion-specific source scans, shaping, source allocation or Timeline admission. A qualified unsettled channel may contribute only its actual existing frame demand; settlement or retirement ends that contribution. Baseline editing, layout, blink and independent host effects retain their existing owners.

**FORBID-101-COMPOSITE-NO-USE.** No idle embedded-motion demand, unconditional per-host shaping, always-present transition/source allocation or claim of zero total widget work is admitted.

---

## 9. Edge Cases & Constraints

1. **Empty text**:

    * Hit-test always returns position `0`.
    * Caret at `0`.
    * Placeholder displayed if configured.

2. **Max length**:

    * If `max_len_bytes` is set and insertion would exceed:

        * Either reject whole insert, or truncate to available space.
        * SPEC decision: simplest first — reject entire insert; do not partially accept.

3. **Non-printable characters**:

    * Editing substrate may allow them; TextField does not add them intentionally.
    * If present (e.g. pasted later when clipboard is wired): they are shaped and drawn as usual.

4. **Scroll clamping**:

    * If content shrinks (deletions), after layout rebuild, `scroll_x` must be clamped again to `[0, max_scroll]`.

5. **Read-only**:

    * All selection and caret movement allowed.
    * Any mutation call becomes a no-op, but still keeps invariants valid.

---

## 10. Implementation Plan (Stages for Claude)

This is the part you hand directly to the coding agent.
Every stage must compile and keep all existing tests green.

### Stage 1 – Minimal TextFieldState Skeleton

* Add `TextFieldState` and `TextFieldConfig` structs with fields from §2.

* No behavior yet, just `new()` and basic defaults:

    * `padding = 4.0`
    * `is_enabled = true`
    * `is_read_only = false`
    * `blink_interval = 530ms` (match current).
    * Default colors via some `TextFieldColors::default()` from palette.

* No widget implementation yet; just types.

### Stage 2 – Widget Shell & Layout

* Add `TextField` struct wrapping `TextFieldState` and an internal `WidgetInternalId`.

* Implement `Widget for TextField`:

    * `layout`:

        * Rebuild `layout` when `document.version()` or width changes.
        * Compute `content_width = layout.content_width()`.
        * Set `bounds` and return `LayoutResult` with min one-line height.

    * `paint`:

        * Background + border only for now.
        * No text/selection/caret yet.

* Add constructors:

    * `TextField::new()`
    * `with_text`, `with_placeholder`, `with_password`, `with_max_len`, `with_align`.

### Stage 3 – Hit-Testing and Pointer Down/Up

* Implement `hit_test_local_to_index` as in §3.

* Implement:

    * `handle_pointer_down(local_x, local_y, click_count, modifiers, cx)`.
    * `handle_pointer_move(local_x, local_y)`.
    * `handle_pointer_up(cx)`.

* Wire them into `Widget::handle_event` for `Event::Pointer`:

    * Use `cx.click_count()` for click count.
    * Support `PointerEventKind::DoubleClick` explicitly as `click_count=2`.
    * Triple-click via `click_count=3` from fusion layer (no separate kind required).

* Implement drag modes per §5, but do not paint yet.

### Stage 4 – Keyboard Handling (No Mutation)

* Implement `handle_key`:

    * Movement only: left/right, home/end, Ctrl+left/right, plus Shift variants.

    * Use existing editing.rs functions:

        * `move_left_collapse`, `move_left_select`, etc.

    * No deletion or insertion yet.

* Wire `handle_key` from `Widget::handle_event` when focused.

* Ensure selection invariants still hold after movement.

### Stage 5 – Caret & Selection Painting

* Implement `paint_selection` and `paint_caret` per §8.

* Integrate them into `Widget::paint` after background.

* Add caret blinking:

    * `update_blink()` called from `layout`.
    * `reset_blink()` called on any movement or click.

* Make selection behavior match SPEC-HAL-052 exactly.

### Stage 6 – Text Insertion & Deletion

* Hook character input:

    * `KeyCode::Character(ch)` → insert if allowed.
    * `KeyCode::Space` → insert `" "` if allowed.
    * `KeyCode::Backspace` / `Delete` → use `apply_edit_op` / `apply_delete_word_*` with cluster vector.

* Respect `max_len_bytes` when inserting.

* After mutation:

    * `document.mark_modified()`
    * `layout.invalidate()`
    * `cx.invalidate_layout()`
    * `reset_blink()`

### Stage 7 – Horizontal Scrolling

* Implement `scroll_x` logic:

    * Compute `content_width` from `layout`.
    * Adjust `scroll_x` on:

        * Layout changes.
        * Any caret movement or click that moves caret.
        * Clamp to `[0, max_scroll]`.

* Modify hit-test and paint to account for `scroll_x` exactly as in §7.

* Add small margin logic around caret when scrolling.

### Stage 8 – Commit-on-Enter & Read-Only

* Implement `commit_on_enter`:

    * If `config.commit_on_enter` and Enter pressed:

        * Emit an action or call a callback via Action System.
        * No mutation to document.

* Implement `is_read_only`:

    * Block all mutation paths when true (insert/delete/word-delete).

* Implement `is_enabled`:

    * Disable focus acquisition and pointer capture when false.
    * Paint disabled colors.

## 11. Smooth-caret prototype amendment — 2026-10-29

**Authority status:** Adopted / Binding additive exception, limited to the two inserts identified below.  
**Approval:** Delegated technical acceptance of HAL-101-SMOOTH-CARET-P1-20261029, under JC's explicit TextField experiment authorization. JC retains final ownership and veto.

The exact approved Placement A is integrated after the existing §2.3 configuration text, and the exact approved Placement B is integrated as §8.4.1 after the complete existing §8.4. All pre-existing text remains intact. The original in-thread proposal remains historical source; only these two additions are adopted. HAL-002/002A and the statuses of HAL-097/097A, PM-020 and other documents are unchanged.

**Implementation and evidence status:** This adoption record establishes the scoped contract, not implemented smooth-caret behavior, passing tests or native evaluation. Concrete-instance replacement/unmount animation cleanup remains a required implementation proof under §8.4.1. No protected concurrent G1 source is authorized for modification by this record.

### 11.1 Candidate duration decision — 2026-10-29

**Authority:** After observing the opted-in Showcase field on 2026-10-29, JC requested a 30 ms candidate to make the motion faster. JC retains final ownership and veto. This records the historical candidate, not a replacement for later timing amendments.

This attributable experiment decision changes only the §8.4.1 candidate duration and its terminal deadline from 60 ms to 30 ms. Explicit opt-in, disabled-by-default behavior, Once/EaseOut, immediate logical editing, current-value retargeting, all snap boundaries, engine timing/ownership and proof obligations remain unchanged. The recording is evidence of JC's evaluation of the preceding 60 ms candidate, not native acceptance of the new 30 ms candidate. The new duration requires its own source-bound verification and visual evaluation; it is not a universal default or release claim.

**Subsequent candidate decision — 2026-10-29:** JC directly requested “now change it to 45ms” in the same Actual chat. The current §8.4.1 candidate and exact terminal deadline are therefore 45 ms, superseding the preceding 30 ms trial. All other contract clauses and proof limits remain unchanged; this decision is not a claim of native evaluation.

**Subsequent candidate decision — 2026-10-29:** JC directly requested “make it 50” in the same Actual chat. The §8.4.1 candidate and terminal deadline are now 50 ms, superseding the 45 ms trial; all other contract clauses and proof limits remain unchanged.

**Subsequent candidate decision — 2026-10-29:** JC directly requested “back to 60” in the same Actual chat. The §8.4.1 candidate and terminal deadline return to 60 ms, superseding the 50 ms trial; all other contract clauses and proof limits remain unchanged.

**Subsequent candidate decision — 2026-10-29:** JC directly requested “make it 75” in the same Actual chat. The §8.4.1 candidate and terminal deadline are now 75 ms, superseding the 60 ms trial; all other contract clauses and proof limits remain unchanged.

**Subsequent candidate decision — 2026-10-29:** JC directly requested “make it 65” in the same Actual chat. The §8.4.1 candidate and terminal deadline are now 65 ms, superseding the 75 ms trial; all other contract clauses and proof limits remain unchanged.

### 11.2 Verified-key repeat amendment — 2026-10-29

**Authority status:** Adopted / Binding, limited to the exact HAL-101-SMOOTH-CARET-REPEAT-P2-20261029 line898 replacement and INV-101-SC-REPEAT / FORBID-101-SC-REPEAT insertion. The delegated technical reviewer explicitly accepted those supplied texts; JC retains final authority and veto. The 65 ms discrete-motion candidate is unchanged. This amendment admits only the TextField correction for verified KeyEvent.is_repeat under qualified Capture; it does not admit event-pipeline or other-widget changes.

### 11.3 Held-insertion provenance — accepted missing requirement

**Status:** The requirement below is accepted as a documented missing requirement only. It is not an adopted event representation, compatibility decision or permission to change the input pipeline. Held character insertion remains OPEN pending a separate input-owner representation/compatibility decision.

**INV-101-SC-TEXT-PROVENANCE (documented missing requirement; no input implementation authority).** A text insertion may use the verified-repeat exception only when the owning input path supplies immutable repeat/discrete qualification derived from the exact native keyboard event that produced that text and preserves that association through the existing raw, fused and routed text admission to the consuming TextField. A native keyboard-derived repeated insertion uses the same static-caret boundary as INV-101-SC-REPEAT. Qualification must distinguish known repeat, known discrete and unavailable origin; these are semantic categories, not chosen Rust API spellings. Unavailable origin MUST NOT be inferred from cadence, timestamps, equality, string contents, previously observed keys or the existing key/text consumption-suppression heuristic. IME, clipboard and other non-keyboard text retain their existing owners and snap rules. No text operation may be delayed to wait for visual motion.

This requirement does not itself admit a public event/API representation or an input-pipeline implementation. Those require an explicit owning input decision and compatibility admission. Until then, the current TextInputEvent behavior remains unchanged; held insertion remains OPEN and no correction is claimed. No new transport, scheduler, clock, phase or generic bus is proposed.

### 11.4 P4 edit coherence — adopted 2026-10-29

**Authority:** JC's P4 direction and delegated technical acceptance. JC retains final authority and veto. The exact §8.4.1 eligible-source replacement and INV-101-SC-P4-EDIT-COHERENCE / FORBID-101-SC-P4 are Adopted / Binding. P2's verified-repeat static rule and rejected/equal-repeat precedence remain binding; its later eligible discrete operation now means only unchanged-document, unmodified, non-repeat Left/Right/Home/End navigation. All earlier content-edit interpolation eligibility is superseded by P4. The candidate remains 65 ms Once/EaseOut exclusively for that navigation.

P3's coordinated text-presentation/input-pipeline extension is REJECTED and not adopted. The text-origin requirement in §11.3 remains an inactive missing input capability, not implementation authority. P4 does not need repeat provenance for character insertion: every accepted content mutation uses the canonical static caret for every origin. Earlier statements that the insertion presentation path must remain unchanged are superseded only by this explicitly adopted P4 visual policy; the event representation, routing and editing authority remain unchanged. Native JC visual acceptance remains pending; natural text steps at repeat cadence are not claimed eliminated.

**Authoritative no-op clarification:** The subsequent P4 source-resolution decision permits comparison of the existing `before.text` snapshot with the actual post-delegate contents before `record_edit`. For byte-identical Backspace/Delete/ordinary TextInput results, no artificial document revision, edit history, text-cache rebuild, edit invalidation or `on_change` is produced solely because the event was accepted. Existing consumption and blink reset are preserved. An actual selection change remains a separate snap boundary and does not become a content mutation. This clarification does not change undo/redo, IME or another editor's semantics.

Scoped composite-editor adoption — 2026-10-29: the composite-editor additions and replacements in this document are Binding under the supervising technical reviewer's coordinated adoption, with JC's final ownership and veto retained. The exact three source bindings and adopted scope remain in the retained adoption record. No implementation or conformance result is asserted; all unrelated declared statuses remain unchanged.

### 11.5 P6 end-edit duration supersession — 2026-11-12

**Authority status:** Adopted / Binding scoped timing supersession by direct JC instruction on 2026-11-12. JC directed the character reveal/conceal duration to change from 60 ms to 55 ms. This is the presentation duration, not GPU render latency.

The active §8.4.2 single-character end-append/end-Backspace reveal/conceal duration is exactly **55 ms (0.055 Timeline seconds)** using the existing single Once/EaseOut leg. The same duration applies to its already-qualified mask-only caret-boundary end-edit sibling; this does not admit plaintext presentation in password mode or expand mask eligibility. With no accepted retarget, the first admitted sample at or after the stored 55 ms duration settles to the exact current canonical boundary and releases the transient source/handle under the existing lifecycle.

This decision supersedes only the prior 60 ms P6 duration and its operative cross-references. All dated 2026-10-29 decisions, including the 60 ms P6 attribution and the navigation trial chronology in §11.1, remain unchanged as history. Immediate logical edits, source/mask safety, qualification, coalescing, cancellation, default/plain policy and phase ownership are unchanged. P2 navigation remains 65 ms; P7/P8 range timing remains T_ms(d)=clamp(80+1.25*d,100,220). Source tests do not establish native JC visual acceptance.


## 12. Independent compact-input tree-owned text proof acceptance (2026-11-14)

**Disposition: ACCEPTED solely for HAL101-COMPACT-INPUT-TYPOGRAPHY-TREE-OWNED-TEXT-PROOF-20261114's test-only source and headless boundary.** This is an observation record under the existing adopted composite-editor contract, not adoption of the remaining proposed animation packages, product/physical acceptance, or a change in widget policy.

Primary independently reread the current `standard_inputs_preserve_metrics_and_routed_ownership` source, the section2.4/2.5 tree-owned editor law, the prior RED and final raw focused receipts. The current test source SHA256 is 8745c4f9abd6dbe79ab2e3d13a97d28e65eb9866b12b420854051d6a4667dfb0, with unchanged production app.rs SHA256 e4162c4745536b5a43603c94a3cd14a785003c1d6a48905fee563c108857bd8f. The accepted edit distinguishes the standalone TextField's local paint, the editable ComboBox's current sealed and mounted TextField child-local paint, and the noneditable ComboBox's own static caption. It validates child membership, canonical allocation, viewport, actual text-face font size and retained Arc membership in the host composed scene. The original post-initial-metric loop beginning at `let prefix = format!(` and all its existing input, popup, timing, readback and clean-retained checks are byte-identical to the retained beforeimage.

One installed native Grip focused test ran successfully: one selected library test passed, zero failed or ignored, 82.51 seconds; changed refresh/check also exited zero. All 16 original theme/scale/density configurations executed the original headless output path. Primary independently checked all 288 original PPM files against their manifest: zero missing, zero SHA256/size mismatches, exactly 18 stages in each of 16 configurations, 165892320 bytes total. These records verify headless readback identities and completed scenarios; Primary did not perform a new graphical/colourimetric assessment or repeat any build, check or renderer execution.

The original host-local `!output.is_empty()` RED was a read-owner assumption incompatible with the already adopted tree-owned TextField; the source correction restores the original expected behavior without adding a host text painter or a second editing owner. The preceding full App owner suite still has its retained 22-failure receipt; this scoped focused recovery and the separate earlier Slider focused recovery do not constitute a fresh full-suite count. The other historical App and Router RED selectors are not closed. The proposed HAL363/364 transient presentation scope remains inactive.

## 13. JC U1 — source-qualified text motion without arbitrary content ceilings (2026-11-15)

**Status: ADOPTED / BINDING scoped supersession by direct JC mandate.** Authority is `HAL-REMOVE-ARBITRARY-CAPABILITY-LIMITS-U1`, issued 2026-11-15T12:34:31.847Z and expressly resumed by `HAL-U1-RESUME-EXISTING-AUTHORIZED-UNIT-20261115`. This section records that selected owner decision; it is not an executor-selected expansion. Halcyon Primary retains architectural review and JC retains visual acceptance. Implementation and native/browser proof remain separate.

**INV-101-U1-CONTENT.** P6 end append/Backspace and P7/P8 already-admitted word/selection/range edit presentation SHALL NOT be excluded because the complete source is non-ASCII, Unicode, a particular script/language, more than 32 UTF-8 bytes, wider than half a viewport or longer than an arbitrary document quota. Those content-eligibility clauses in §§8.4.2–8.4.6, their associated exclusion lists and aggregate coalescing bounds are superseded. Historical text remains intact. This does not add or change a logical editing command; separate editing/IME/clipboard owners retain their existing authority.

**INV-101-U1-LOGICAL.** Every admitted logical edit, selection/cursor result, callback and history transaction remains immediate and canonical. Motion must never modify, decode differently, normalize, truncate, replace or delay source text. Explicit plain mode and existing availability, read-only, password privacy, IME, Focus/Capture, active editor instance/session/mount and domain rules remain authoritative. P2 navigation's accepted policy is unchanged.

**INV-101-U1-PROJECTION.** Generalize the existing complete-run, source-aware P6/P7 spatial presentation using the actual TextEngine source-cluster and glyph/face binding evidence. Shapes are computed from complete old/current sources through that owner; no substring shape, glyph-ordinal guess, byte-to-glyph assumption or invented bidi order is admitted. Correctly ordered actual visual subregions may share the one current transition. Shaping changes across an edit join are geometry to handle with the complete changed context, not a language blacklist or automatic whole-script suppression. Canonical caret, hit testing and text width remain unchanged.

**INV-101-U1-EVIDENCE.** Exact source revisions/association, valid grapheme/edit boundaries, complete actual source-to-glyph/face binding, finite layout/clip/DPR and current scroll/geometry remain required. When some geometry cannot safely be projected in a frame, render complete canonical current text immediately and identify the precise missing or incompatible evidence. An unsupported script's missing shaping or source mapping remains an open capability defect, never silently relabeled as accepted static policy. A correction crossing another accepted independent text-owner contract requires that owner's explicit disposition; this section does not adopt unrelated Draft/Proposed text policies or fallback fonts.

**INV-101-U1-VISIBLE-SPAN.** Large edits use the actual visible viewport and existing resource admission. Visible subregions animate coherently; offscreen source is not semantically removed, shortened or split by an arbitrary quota. Retain at most one current complete admitted transient edit origin, never a sequence of old documents or persistent shaped output. No unbounded rendering, memory retention, new renderer backend, DOM/CSS overlay, hidden global, generic animation engine or second scheduler is admitted. Existing resource exhaustion and invalid geometry remain real failures, not invented small payload ceilings.

**INV-101-U1-TIME-PHASE.** P6 remains exactly 55 ms Once/EaseOut under §11.5; P2 remains 65 ms; P7/P8 remains `clamp(80 + 1.25*d, 100, 220)` ms for actual remaining logical presentation travel. These parameters are not content-eligibility ceilings and must not be retuned to hide cadence defects. Events retain immediate edits plus owned intent; ordinary Update alone reconciles and samples the one existing Timeline handle; Layout/Paint remain read-only consumers. Equal/no-use/settled states create no motion work or continuing demand. Retargeting remains latest-only, never replay or backlog.

**INV-101-U1-PARITY.** The same accepted text/edit/source must receive feature-equivalent native and browser presentation through the same TextField owner. WASM is not a lesser script class. Genuine source/backend/raster refusal remains observable under HAL360/361; compilation, a synthetic input attempt or a prepared frame is not physical pixel acceptance.

Qualification must cover ordinary ASCII, newly entered Cyrillic, mixed Latin/Cyrillic, combining marks, multi-codepoint emoji, Arabic/Hebrew and mixed-direction actual cluster order, contextual join changes, deletion beyond 32 UTF-8 bytes and spans beyond half the viewport. Use the same mounted owner and real Capture/edit path, immediate document/Undo/Redo, exact Timeline samples and complete shape evidence; distinguish physical native/browser observation from mechanical proof. S7's pre-winit automated-input loss must not be “repaired” by replaying or inventing input. Clipboard and uncaptured-browser default policy are separate owners.
