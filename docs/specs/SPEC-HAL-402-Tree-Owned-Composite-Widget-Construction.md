# SPEC-HAL-402: Tree-Owned Composite Widget Construction

**Author:** JC  
**Date:** 2026-11-28  
**Status:** Withdrawn / Historical record; never adopted or implemented.  
**Withdrawal:** 2026-11-28, maintainer decision.  
**Revision:** Preserved post-control-audit proposal, with withdrawal rationale.

## Withdrawal decision — 2026-11-28

The maintainer has withdrawn this initiative in its entirety. HAL-402 is
retained to preserve the problem investigation, proposed design, review
findings and reasons not to proceed. It is not a deferred implementation
task, an active Draft awaiting another revision, a successor contract, or
authority to alter any API or implementation. Its identifier is retained.

The original objective was legitimate: constructing a composite widget
should not require an unrelated widget to be mounted and replaced merely
to obtain the intended parent's location. The mistake was to turn that
construction inconvenience into a new architecture before establishing
why the existing construction contracts and mechanisms were insufficient.
Withdrawal is a decision against this proposal, not against improving
composition ergonomics or investigating real structural integrity defects.

### Historical origin and the distinction between operations

The early retained WidgetTree model already distinguished tree ownership
from structural references held by a composite. The November 17, 2025
widget-system Draft, then recorded as RFC-HAL-010, section 6.1, proposed
mounting a Panel, adding its children and completing its NodeId fields
afterward. That was a design proposal, not evidence that the illustrated
implementation was qualified.

The first Tier-1 implementation on November 17 stored explicit child
NodeIds in Button, Container, Panel and ScrollArea and child lists in
Row and Column. In its initial Showcase, logical child references and
actual tree membership did not consistently describe the same hierarchy.
The tree also had take_widget/restore_widget, introduced to permit layout
of an extracted widget with mutable access to the tree. Its intended
layout use was to return the same object.

On November 18, the introduction of event routing and hit-testing
required actual structural membership to agree with logical composition.
The first confirmed Showcase construction replacement created a temporary
Container, added a Label beneath it, dropped the Container and restored a
new Button into the same node. The Label was made non-hit-testable so
pointer targeting could reach the Button. Subsequent corrections repaired
root reachability, nested parent/child membership, coordinate calculation
and duplicate painting. Replacement was a practical way to establish
correct topology despite the constructor's dependency on a child NodeId;
it was not introduced as a general requirement of retained ownership.

Flex, introduced on November 21, illustrates a different solution.
Its ordinary children are supplied by the actual tree during layout.
It need not be replaced merely because its child collection is not known
at object construction. This does not by itself resolve the problem of
a composite that requires a particular immutable structural role.

On December 5, HAL-087 documented the distinction between tree-managed
and widget-internal identities and the need to preserve registered node
identity. It records an earlier identity-mapping failure; the precise
intermediate source containing that failure was not recovered during
this research. On December 10, replace_widget! encapsulated the existing
take/drop/restore construction sequence and the examples were normalized
to use it. It did not introduce a new composition mechanism.

On December 29, ScrollArea::new_from_children temporarily addressed
unknown content by discovering the first child during layout, using a
sentinel and allowing empty layout when no child existed. That approach
was removed. On December 30, HAL-154 established the binding Deferred
wiring and first-layout sealing contract. The following day List and
ScrollArea were migrated, the use of a fake initial NodeId followed by
a second assignment caused a double-set failure, and the construction
path was corrected to use an explicitly unwired object. First-layout
sealing was transferred to the engine. Historical paint/event guard
coverage was not equivalent to full all-build conformance.

On January 8, 2026, the old replacement macro's use of restore_widget
with a new rect violated HAL-141/145 geometry authority during build.
replace_widget_only was introduced specifically to replace the object
without assigning geometry. This was a correction of an existing
construction path, not a decision that every composite must use it.

On May 6, standalone constructors introduced a separate Button text
mode and the independently writable optional child/text fields. This
explains the later content-representation exposure without making it a
necessary property of composite construction. On October 29, concrete
mount lineage, explicit predecessor retirement and successor mount
hooks gave genuine replacement its modern lifetime semantics; the
layout take/restore pair became crate-internal.

Historical evidence was inspected read-only in the retained development
history. Useful archival revision locators, in chronological order:
c3a96f73, 050ae5cd, 49ca06a5, 7191ca72, ad066daa, 9bf2b674,
28441032, 6509cafd, f331c45d, eb11cf0b, ebbe92ad, 9cc7afcf,
8db3dc25, d17f487f, 234eabe8, 7a7838fb, b42fc8be and b3dd1d0f.
The relevant historical source areas are the widget-system and identity
documents, widget_tree.rs, widget.rs, button.rs, flex.rs,
scroll_area.rs, deferred_wiring.rs, macros.rs and the early composition
examples. These locators are historical provenance, not a promise that
the archived revisions are available from the public source repository.
No historical executable or runtime conformance was established.

Three different operations must therefore remain distinct:

- Ordinary construction establishes the intended widgets and their
  actual tree relationships.
- The internal layout lease temporarily moves and restores the same
  concrete widget without ending its mount.
- Genuine replacement substitutes a different concrete widget at an
  existing registered node, with the appropriate lifetime transition.

The placeholder-based construction pattern uses genuine replacement
to solve an ordering dependency. It is a historically established
construction idiom, not inherently an invalid use of Rust or evidence
that replacement itself is defective. Rust's exclusive-borrow rules
explain the layout lease; the constructor/installation ordering
dependency follows from the chosen API and is not imposed by Rust.
No material performance penalty was established.

### Why the proposed architecture was rejected

1. **Existing authority was not given its proper design weight.**
   HAL-154 expressly permits necessary structural references to be
   unknown during object construction and resolved after mount,
   subject to explicit wiring, single-assignment, the first-layout
   seal and failure rules. The preserved proposal treated completeness
   before the actual host's first mount as the necessary design target
   (sections 1.1, 3, 4.2 and 14), rather than establishing why the
   existing contract cannot solve the actual need. The new private
   two-slot allocation plan, sealed pair API and separate App-origin
   construction witness have no demonstrated necessity relative to that
   baseline. This is not proof that preallocation is inherently invalid;
   its additional complexity is unjustified for the admitted objective.
   Nor does withdrawal authorize extending HAL-101's scoped editor
   construction exception to arbitrary widgets.

2. **The proposal contradicted its own compatibility guarantee
   (HAL-402-B01, High).** Sections 8.1 and 10 required mutually
   incompatible behavior. Section 8.1 would refuse the existing
   Button::new(NodeId(0)) placeholder insertion before any effect,
   while section 10 preserved existing placeholder-based callers and
   disallowed broad migration. Concrete callers include
   examples/card.rs:267,443,459, examples/split_pane.rs:170-171,
   examples/flow_grid_layouts.rs:545,586,622,
   examples/showcase/tab_inputs.rs:434,452,470 and existing SDF
   Button GPU fixtures in src/core/app/sdf_button_v3_gpu_tests.rs.
   Their construction sequences would no longer reach the subsequent
   replacement under the proposed admission rules. Rejecting these
   paths was not an authorized compatibility decision.

3. **The configuration redesign exceeded the construction need
   (HAL-402-B02, High).** Section 5.2 introduced ButtonConfig,
   covering ordinary defaults, binding and callbacks, focus, typography,
   appearance/product selection and SDF/material requests. Although
   the revised Draft forbade duplicated semantics, it did not establish
   a complete justified initialization/runtime boundary for the existing
   owner mechanisms. The original construction problem does not
   demonstrate a need to reorganize those configuration paths. The
   scope had expanded from resolving structural order to changing the
   initialization surface of an existing control.

4. **The proposed construction scope remained ambiguous
   (HAL-402-B03, Medium).** The representative Showcase Button is
   beneath ordinary Flex containers within Panel and ScrollArea
   composition (examples/showcase/main.rs:1028-1045 and
   examples/showcase/tab_overlays.rs:470-577). The proposal did not
   establish an unambiguous distinction between prohibited mutation
   of a protected structural role and lawful construction of ordinary
   descendants inside its content subtree. The exact scope could not be
   delegated as a settled implementation decision.

5. **The lifetime restriction was stronger than its justification
   (HAL-402-B04, Medium).** Section 8.2 proposed rejecting every
   concrete replacement of a required child beneath a surviving Button.
   Protecting a required relation from removal or stale slot reuse is
   different from prohibiting an otherwise lawful replacement at the
   same registered child node. A structural role, a registered node and
   its concrete occupant are distinct facts. The closed editor-role
   constraints in HAL-101 do not establish an equivalent general
   prohibition for ordinary Button content. The necessary restriction
   must be derived from the actual owning role and governing authority.

The control review therefore found real incompatibility and unnecessary
architectural expansion, not merely wording that needed another revision.
An implementation of this proposal would impose new obligations on
WidgetTree, App admission and Button initialization without a demonstrated
need or sufficient compatibility qualification. The initiative is
withdrawn rather than carried forward as a narrower version of the same
architecture.

### Retained findings and limits of this decision

The source-level observations in sections 1.3 and 8.5 remain relevant
evidence, not adopted requirements and not claims of reproduced failures.
In particular, the independently writable Button child/text modes, the
required-child relationship and recycled NodeId exposure (A01), and
the separate SignalStore pending-work/subscription lifetime exposure
(A03) are not resolved by withdrawal. They must not be silently
dismissed or treated as authorization for the proposed fixes.

The original A02/A04/A05/A06 design review also retains useful
distinctions: actual construction authority must not be inferred from
public type names; configuration must remain with its owner; a Label
is not necessarily passive; and equal final membership or appearance
does not prove equal intermediate lifecycle effects. These findings
remain available for independent work if a real supported capability
requires it. None revives the withdrawn pair API.

Genuine replacement remains an existing architectural capability under
HAL-087 and HAL-354, with geometry governed by HAL-141 and construction
governed by HAL-154 where applicable. Existing lawful callers,
functionality and owner boundaries are preserved. Withdrawal adopts
no new construction API, no new required-child policy, no migration,
no performance claim and no constitutional or governing SPEC change.

The historical review did not establish why Deferred wiring was not
extended to Button, nor whether it is the appropriate solution for
every composite. Any future investigation must begin from the actual
governing contracts and reachable source behavior, distinguish
ordinary tree-defined child collections from required structural roles,
and establish a concrete deficiency before proposing a new mechanism.
There is no active successor initiative and no automatic follow-on work.

---

**Preserved proposal (historical, non-binding).** Everything from the
original Owners and Scope metadata through sections 0-14 below is
retained as the exact substance of the withdrawn Draft. Its imperatives,
proposed invariants, acceptance gates, statuses, and references to a
future implementation are historical records only. They are not active
instructions, do not grant adoption or execution authority, and do not
supersede this withdrawal decision.

**Owners:** WidgetTree owns installation, membership and concrete mounts. App owns construction admission. Widgets own their configuration and ordinary behavior.  
**Scope:** Common, bounded construction of a host and one required direct child. First admitted role: an ordinary child-backed Button and its Label caption. The enduring relationship correction in section 8 is a necessary, explicitly proposed compatibility decision.

## 0. Decision and adoption boundary

The original Draft proposed a specialized mount_label_button API. Its control
audit identified six material deficiencies. This revision replaces that
proposal, not the existing framework or any adopted authority.

The required result is the existing Showcase Open Context Menu control built
as a real Button with a real Label, without mounting and replacing an
artificial predecessor. It must retain its ordinary presentation, identity,
lifecycle and action behavior. Construction convenience does not justify
an unsafe enduring child relationship.

This document proposes a design. No implementation, migration, publication,
or change of a binding RFC follows from writing it. In particular, the
public Button content and structural mutation restrictions in section 8
require explicit maintainer adoption at G0. If that decision is withheld,
the new operation remains unadopted. Do not deliver a reduced pilot under
a false structural-integrity promise.

## 1. Problem and source baseline

### 1.1 Existing construction

WidgetTree owns arena nodes and maintains the registered TreeWidgetId lookup
index. NodeId is a reusable storage location, not a persistent identity.
See src/core/widget_tree.rs:588-614,1393-1408.

WidgetTree::add_child receives the actual Widget and an existing parent
NodeId. It allocates a slot and tree identity, establishes a concrete mount,
invokes the ordinary mount hook, installs membership and performs applicable
registration. See src/core/widget_tree.rs:1504-1575.

Button::new(child: NodeId) needs its child's location at construction.
Button::with_text instead uses a standalone caption. The child-backed
layout uses the stored child location and paint consumes its text
presentation. See src/widgets/button.rs:506-580,1094-1161,1363-1390.

The existing construction sequence for Showcase's Open Context Menu is:

1. Mount an empty Label as the intended Button host.
2. Mount the actual caption Label beneath it.
3. Construct the actual Button with that child's NodeId.
4. Replace the temporary host with the Button.

Sources: examples/showcase/tab_overlays.rs:561-577;
src/widgets/notification_log.rs:129-149. The replace_widget macro delegates
to replace_widget_only (src/utils/macros.rs:1-9).

Genuine replacement retains the admitted node's registered identity and
topology, retires its predecessor's concrete mount, invokes the predecessor's
unmount hook, installs the successor and establishes a fresh concrete mount
(src/core/widget_tree.rs:3985-4053). The temporary host is consequently a
real mounted object. Its mount, registration, retirement and unmount are
not private allocation planning.

The ordering dependency is real: the host needs its child ID before
construction, while ordinary child installation needs an existing host.
The legitimate property is that the actual host is constructed with the
required relationship determined. Do not replace this with a mounted
incomplete Button that is repaired afterward.

The objective is coherent construction semantics and developer ergonomics,
not an asserted speed improvement. Hiding the existing placeholder sequence
behind a helper does not eliminate its lifecycle.

### 1.2 Distinct facts

Construction creates the actual Widget object from owned inputs.
Assembly determines relationships. Installation admits the objects into
WidgetTree. An ordinary update changes authorized widget state. Genuine
replacement ends one concrete mount and begins another at the retained
registered node. Removal ends registered membership and the applicable
subtree lifetime.

A temporary initialization value is not a Widget or second tree. A private
allocation plan is not an installed node. TreeWidgetId is not a concrete
mount token. WidgetInternalId is not routing authority. WidgetKey does not
implicitly reconcile arbitrary entities. A NodeId does not identify its
former occupant.

### 1.3 Existing source-level integrity exposures

Button currently exposes independently writable child and text fields
(src/widgets/button.rs:360-366). Layout uses child directly, while paint
can choose text when both are present. Ordinary removal immediately
releases the child's arena slot; the generic layout_child lookup does not
itself establish that the requested node is the actual direct child.

See src/widgets/button.rs:1115-1136,1363-1383;
src/core/widget_tree.rs:1583-1699; src/core/widget/mod.rs:376-445.

SignalStore has an independent raw-NodeId lifetime exposure described in
section 8.5. These observations precede the proposed construction path.
Neither is claimed as a newly reproduced production incident.

## 2. Governing authority

| Authority | Preserved boundary |
| --- | --- |
| HAL-002/002A; HAL-002 pillars A and C, 3.1, 3.4, 4.1, 4.6 | WidgetTree alone owns installed widgets; no persistent cross-widget references, widget-owned Widgets or second hierarchy. |
| HAL-002 pillar B, 3.2, 3.3, 3.6-3.8, 4.2, 5 | Synchronous structural work at an admitted existing construction boundary, not event/update/layout/paint authority. |
| HAL-002 3.5, 4.9, 4.10; HAL-087; HAL-148 | Explicit ownership, stable registered identity and concrete lifetime; no identity derived from slot reuse. |
| HAL-154 | Existing Deferred and first-layout sealing govern required deferred wiring; no unwired reads, post-seal repair or generalization of closed-editor exceptions. |
| HAL-354 | Existing mount-qualified work, retained-fragment lifetime and invalidation. |
| HAL-112; HAL-128; HAL-207 and applicable input/availability contracts | Existing Button activation, capture, focus, cancellation and action semantics. |
| HAL-141; HAL-158/158B | Existing geometry authority remains unchanged. |

HAL-154 sections A-F and G are Binding. It does not require placeholder
replacement. Its scoped HAL-101 closed-editor assembly additions do not
authorize general tree construction. The initial pair resolves its child
before constructing the real host; it does not need a Deferred field.
Where a future independently admitted role genuinely requires deferred
wiring, apply HAL-154 in full, including its actual engine first-layout
enforcement. Do not introduce a second seal mechanism.

HAL-018, HAL-010, HAL-037, HAL-031 and HAL-315 retain their actual declared
statuses. The HAL-018 Draft and public context names do not establish a
private runtime permission. Section 6 grounds the proposed permission in
actual runtime entry points. This Draft changes no existing binding SPEC
and proposes no constitutional amendment.

## 3. Proposed invariants

**INV-402-OWNER:** WidgetTree alone owns installed widgets, registered
membership, identity and concrete mounts. Uninstalled owned inputs
have no routing, layout, rendering or live structural authority.

**INV-402-REAL:** Only intended real widgets receive concrete mounts.
No temporary mounted parent, placeholder, fabricated ID or artificial
replacement serves as allocation.

**INV-402-COMPLETE:** The actual host is constructed with its required
relationship already determined. No unresolved required role is mounted
or repaired after mount.

**INV-402-ROLE:** During an installed host's concrete lifetime its
required child relationship is immutable. Supported structural operations
protect it before effects, and required-child consumption verifies actual
identity and membership.

**INV-402-PHASE:** New construction originates in the actual engine-owned
Screen build and is qualified to that tree, root and newly built scope.
Public methods, contexts and raw indices alone confer no permission.

**INV-402-IDENTITY:** Every actual node gets fresh registered identity
and concrete mount. No synthetic predecessor, identity transfer or
implicit remapping.

**INV-402-REFUSAL:** A recoverable refusal precedes semantic installation.
It changes no existing membership, registrations, identities, mounts,
dirty state or geometry and causes no mount, unmount or action.

**INV-402-LIFECYCLE:** Prepared ownership, hooks, live membership,
registration and completed publication are separate facts with the
exact ordering specified for this pair.

**INV-402-NORMAL:** The installed pair uses ordinary Button and Label
behavior. No assembly-specific retained flag, wrapper, traversal,
polling, renderer or extra update/layout/paint path.

**INV-402-COMPAT:** Preserve all existing valid constructors, widget
configuration, interaction, genuine replacement and removal except the
explicitly proposed restrictions in section 8. No silent compatibility
change, migration, or substitution with a standalone text Button.

## 4. Bounded tree-owned construction

### 4.1 Common operation and admitted roles

The common operation installs a parent and one required direct child
under an existing ordinary parent. WidgetTree resolves final identities,
constructs the real host from its initialization input, and owns all
installation and registration.

Use one entry, not a family of widget-specific mount_label_button,
mount_group_box or mount_scroll_area methods. A sealed built-in role
implementation supplies pure validation and actual host construction
from typed owned input and a private final child reference. It cannot
allocate, register, dispatch or grant topology authority.

The first admitted pair is ButtonConfig plus Label. It is a qualification
case of the common operation, not the permanent name or shape of that
operation. No arbitrary Widget/Widget pair, user factory, recursive
hierarchy builder, multi-role DSL, runtime editor, plugin or transaction
service follows. Extension requires separately accepted role authority
and demonstrated reuse of the actual problem.

### 4.2 Design selection

A macro hiding placeholder replacement fails INV-402-REAL.
A caller factory receiving prospective raw NodeIds allows unqualified
locators to escape, including on refusal. A public late-wiring setter
introduces an incomplete host and sequencing obligation. HAL-154 remains
lawful when wiring is genuinely deferred, but this pair need not defer
it. An independently live off-tree hierarchy introduces unnecessary
identity translation, ownership and installation problems.

A special LabelButtonOptions duplicates ordinary Button configuration.
Instead use one canonical Button initialization value shared by its
existing constructors and the new operation. The tree owns structural
installation, not widget-specific configuration policy.

### 4.3 Private final allocation plan

Under exclusive tree ownership, plan exactly two distinct final slots
and two fresh TreeWidgetIds in the existing allocator order, parent
first and child second: free-list pop, then arena growth. Planning and
consumption use the same canonical allocator rules, factored only as
necessary. Verify the actual candidate locations and mapping state.
Precheck slot arithmetic and identity/mount generation capacity.
Never admit identity wrap or reuse. Bounded explicit fallible storage
reservation precedes semantic installation.

There is no separate allocator, fictitious WidgetNode, persistent
reservation record or public prospective handle. No callback, arbitrary
factory, event, await or other allocation intervenes between planning
and installation. Ordinary allocator abort and violated engine invariants
remain fatal; they are not claimed as recoverable errors.

## 5. Proposed public contract

### 5.1 One typed fixed-pair entry

Proposed Rust API shape (not an existing compiled API):

~~~rust
pub fn mount_composite<P, C>(
    &mut self,
    parent: NodeId,
    host: P,
    child: C,
    key: Option<WidgetKey>,
    z_order: i16,
) -> Result<MountedComposite, CompositeConstructionError>
where
    P: CompositeParent<C>,
    C: Widget + 'static;
~~~

The method belongs to ScreenFrameContext; the tree implementation is
private. CompositeParent<C> is a public sealed bound. Its private
supertrait/implementation supplies role validation and real parent
construction, without a publicly callable installation method or
prospective-ID API. ButtonConfig plus Label is the sole initially
admitted implementation.

MountedComposite contains public copied NodeIds named root and child.
They describe live registered slots after success, not enduring or
asynchronous handles. The root receives the supplied key and z_order.
Its actual single direct child receives key None and z_order 0.
Use existing tree identity lookup for registered TreeWidgetIds.

For the existing representative consumer, the call shape is:

~~~rust
let mounted = cx.mount_composite(
    button_row_a,
    ButtonConfig::default()
        .with_binding("overlay_open_context_menu", writer.clone()),
    Label::new("Open Context Menu"),
    None,
    0,
).expect("static Showcase menu composite must be admitted");
self.overlay_menu_button_id = Some(mounted.root);
~~~

This illustrates the proposed API at the actual Screen::build return
boundary, not currently compilable Halcyon code. The static Showcase
inputs and their admitted scope must be established by qualification;
their refusal is an example construction invariant failure. Application
callers with recoverable input retain the typed Result and handle it
without silently choosing another construction route.

The initial parent is an ordinary live Flex within the current newly
built scope of section 6, not the scope anchor itself, an old screen,
closed host, virtualized owner or special surface. The input NodeId
is a current observation from this tree/build. The method verifies
actual current membership but cannot turn arbitrary copied raw slot
numbers into cross-tree or generation-safe identities.

### 5.2 One canonical Button initialization owner

ButtonConfig is the ordinary initialization value of Button, not a
Label-specific options subset. It does not implement Widget, own a
Label, or contain live widget state. The actual host is constructed
after its content mode and required identity are known.

Use one canonical initializer and default source for existing
Button::new(NodeId), existing Button::with_text, the proposed
standalone no-caption constructor and the admitted pair. Existing
constructors and configuration methods retain signatures and valid
semantics. Shared validation/application functions remain owned by
Button; WidgetTree does not implement configuration policy.

Preserve the existing supported initial configuration, including:

- enabled policy, pointer cursor, optional Padding and uniform
  with_padding convenience;
- on_click and ChangeWriter binding with the existing interning
  and emission semantics;
- caption alignment, finite caption Y offset and focus-cue modality;
- typography selection and its existing child-backed-mode refusal;
- applicable checked initial appearance/product selection and
  material/SDF requests, with their existing precedence and owner rules.

The initialization value exposes the ordinary initial-configuration
vocabulary, including with_enabled, with_pointer_cursor, with_padding,
on_click, with_binding, with_text_offset_y, with_caption_alignment and
with_focus_cue_modality. Reuse existing checked types and validation
for fallible typography, appearance and material inputs. Do not
fabricate renderer admission or duplicate a runtime revision stream.
Mode-dependent validation occurs before constructing the actual parent.

Consumed initialization data moves into ordinary Button fields/state.
Do not retain a mirrored configuration, duplicate defaults, independent
style resolution, event policy or action executor. Do not expose live
mounts, child IDs, geometry or runtime interaction state as initialization
inputs. Existing nonstructural public configuration capabilities stay
available.

Explicit caller configuration, such as ordinary binding-name interning,
may already have effects before construction is attempted. Those are
not action emission and are not rolled back by tree refusal. Nor does
the refusal promise to undo arbitrary external effects of dropping
owned input values.

If any existing initial capability cannot use this canonical owner
without a competing implementation, resolve that exact initialization
boundary before G1. Do not deliver a partial options interface.

### 5.3 Initial Label role

A Label's Rust type alone does not establish passivity. The new initial
pair requires the actual existing passive Label configuration:
selection disabled, no associated target, not hit-testable, not
focusable and not capture-eligible. The Label owner supplies the
predicate using its actual policy and selection/association state
(src/widgets/label.rs:80-130,242-258,409-445). Refuse initially
selectable or associated Labels without altering their behavior.

Preserve the admitted Label's lawful presentation configuration.
Do not silently disable a policy, substitute standalone Button text,
or invent another input route.

This initial predicate is not a permanent new Label policy. Section 8
protects the enduring identity and membership of the required child.
Subsequent explicit Label state/policy changes retain their existing
authority; later enabling independent selection/association is not
covered by the initial passive-caption equivalence claim. No new
interaction permission or automatic suppression follows.

### 5.4 Recoverable errors

- OutsideConstruction: no actual current admitted build scope.
- ParentUnavailable: requested parent or captured anchor cannot be
  established as current in the exact tree/scope.
- UnsupportedParent: actual parent type, owner or ancestry is not
  admitted for the initial pair.
- InvalidRole: the actual child fails its admitted role predicate.
- InvalidConfiguration: existing applicable validation or the
  selected content-mode contract refuses the input.
- CapacityOrIdentityExhausted: explicit bounded reserve failure or
  inability to represent required slots, identities or mount generations.

No error implies arbitrary allocator, panic, destructor or external
state recovery. Refusal does not silently repair or switch to another
construction route.

## 6. Actual runtime construction authority

### 6.1 Source and authority distinction

ScreenFrameContext's app, tree and parent_node fields are public
(src/core/app.rs:8786-8792). AppRoot::new and rebuild_active_view
are public (src/core/app/app_root.rs:7-26,370-503). A public context,
public rebuild method, raw index or absence of a layout guard does
not prove an actual runtime build.

Production rebuild callers:
src/core/app/run.rs:377,541,1768; src/core/app/ui_loop.rs:643.
Actual base and modal Screen::build invocations:
src/core/app/app_root.rs:404-412,481-489.

### 6.2 Positive private scope

Use a private runtime-owned entry to the same existing AppRoot rebuild
body. Existing public rebuild_active_view retains its ordinary
low-level behavior but cannot mint this new permission merely by
being invoked. Factor the actual admission boundary, not a second
screen/modal stack, teardown or rebuild implementation.

Every ordinary supported native/browser startup and subsequent real
runtime rebuild enters the private owner route. Immediately around
each actual base/modal Screen::build, create one private witness of:

- the exact current WidgetTree lifetime;
- the actual ScreenHost/root's registered TreeWidgetId and concrete
  mount qualification;
- the fresh-tree-identity allocation cutoff at this build's entry.

The actual runtime scope owns the sole strong UI-local Rc witness.
WidgetTree holds only a private Weak observation during that build.
The new entry upgrades and checks it while exclusively borrowing
the actual tree. No witness or handle is exposed to application
code or retained by a widget.

Reject nested minting for the same tree. A nested public/manual
AppRoot::rebuild_active_view on a tree with a live build witness
must refuse before teardown, update or build effects; it must not
borrow the current witness as another runtime entry. The actual
runtime shall not enter a nested event, update, layout or paint
phase with this witness live. Admission also checks the existing
layout/paint authority guards and refuses while either phase is
active; their absence alone never grants construction authority.
This is a local positive-scope invariant, not a new global phase
tracker.

On ordinary return clear the observation. On unwind, where supported,
dropping the runtime's strong owner revokes authority even if the
caller has moved the tree value; temporary upgrades cannot escape
the synchronous operation. An expired observation is never permission.
The witness does not cover old subtree removal, ordinary Screen
update, action flushing, event routing, layout, paint, another
Screen build or later frames. No global, thread-local mode, public
begin/end switch, caller-supplied permission field or new frame phase.

Retain the existing public ScreenFrameContext shape and Screen::build
signature. A helper operating on the same actual admitted tree/build
may use the new entry. A forged context outside that scope cannot.
The existing manually driven public AppRoot remains available for
legacy behavior; the new operation is initially admitted only at
actual runtime builds. Admitting an independent manual runtime
requires separately adopted authority. This restriction is not a
hidden product switch: all ordinary supported real builds use the
same positive admission route.

### 6.3 Exact parent scope

On every new pair admission:

1. Match the witness with the current tree lifetime.
2. Resolve the exact captured current root identity and mount.
   Replacement or removal cannot substitute another anchor.
3. Require a live ordinary Flex parent without active layout lease.
4. Walk its actual parent chain to the captured anchor. Every node
   below the anchor must have a fresh registered tree identity
   allocated since this build's cutoff. The anchor itself is not
   an eligible target. No old screen, existing modal or previous
   sibling subtree may qualify.
5. Verify the actual live direct links, including the branch
   beneath the anchor; refuse inconsistent, missing or cyclic
   ancestry instead of guessing membership.

Use nonreused registered identity allocation order, not NodeId
range. Arithmetic exhaustion or a wrapped counter cannot establish
eligibility. Preserve the AppRoot rule of one new root for each
screen/modal. The operation does not authorize an additional screen
root or unrelated topology changes.

The witness exists only for actual builds, and the qualification
runs only at the new construction entry. No new work on a retained
frame that does not build. This narrow permission does not assert
that every existing low-level public mutation API is phase-safe.

## 7. Exact construction and lifecycle

### 7.1 Preparation

Under actual admitted exclusive tree access:

1. Qualify scope, actual parent, supplied child role and ordinary
   initialization with their existing owner validators.
   No lifecycle hook, action or tree mutation runs.
2. Plan two distinct slots and fresh TreeWidgetIds; precheck
   mount generation/slot arithmetic and explicit bounded reserve
   capacity. Do not consume logical allocator state before all
   recoverable checks have succeeded.
3. Privately determine the final child reference, including
   slot and fresh TreeWidgetId in the actual tree. Construct the
   real Button with this complete bound reference and the
   canonical initialization input. The Label is already the
   actual intended child. No incomplete mounted Button is created.
4. Exhaust recoverable refusal conditions before consuming
   the plan. The remaining installation is a closed internal
   operation; it does not return an ordinary Err.

No prospective ID reaches a caller or user factory. Capacity
reservation alone may affect storage capacity, not logical
membership or identity.

### 7.2 Installation

Consume the canonical plan and allocate the two concrete mount
generations. Prepare the two real owning WidgetNodes with ordinary
initial state and their complete privately determined relationships:

- composite root's parent is the qualified requested parent;
- root's first child is the actual caption;
- caption's parent is the actual Button, with no sibling or
  child of its own;
- root uses normal key, z-order and sibling insertion semantics.

The operation owns the prepared objects; neither is yet publicly
reachable live membership. Obtain applicable registration inputs
using existing owner queries at their actual required sampling
points. In particular, preserve availability facts sampled
before mount, not a duplicate availability policy.

Invoke the two ordinary on_mount hooks exactly once, Label first
and Button second, with complete private identity/mount/role facts.
Neither object is publicly reachable through live lookup yet.
Hooks retain ordinary self access, not structural authority.
Preserve Button's neutral animation/activation reset
(src/widgets/button.rs:1794-1808) and Label's ordinary hook.

Install both actual owning nodes with their complete identity
mapping and parent/child membership links. Then apply each
applicable existing registration once, parent then child,
using the canonical existing dispatch and indexing. Preserve
availability, authority, scroll, custom render and composite-editor
registration rules according to actual applicability. No
fabricated registration or duplicate dispatch tables. For the
admitted concrete pair these hooks/registrations must be
source-qualified not to require an earlier partial live
publication or change the pair's topology.

All structural links must be complete before a registration
that reads them. No public observer, event, user action,
arbitrary callback, layout, scene construction or renderer
preparation may run between internal steps.

Apply the ordinary topology generation, geometry-origin and
affected layout/composition/hit invalidation and parent dirty
effects of two real installations. Do not invent a new
generation protocol or geometry. Nodes begin with ordinary
unlaid-out geometry and dirty state. No artificial predecessor
retirement, scene eviction, transition, focus transfer or pending
removal is caused by the new construction.

Return both actual live NodeIds only after all installation
and registration work succeeds.

### 7.3 Lifecycle equivalence has a precise boundary

The legacy path installs/registers the caption below a temporary
parent before the real Button's replacement mount hook. The new
path invokes both real hooks before final live membership and
applies applicable registration effects after complete membership.
Intermediate histories are intentionally not identical.

Preserve relative real hook order, exactly one hook per intended
object, actual local hook effects, required registrations and
completed ordinary behavior. Qualify actual hook/registration
semantics, including the intermediate availability publication
timing difference. Do not infer lifecycle equivalence from pixels.

A later role whose hook requires earlier membership, external
effects, another order or a fallible prerequisite is not admitted
automatically. It requires an explicit owner-specific lifecycle
contract. No new on_mount structural authority is granted.

## 8. Enduring required-child integrity

### 8.1 One canonical content representation

Completing construction is insufficient if the required child
can subsequently be removed and its slot reused beneath a
surviving host. Address this through a scoped correction to the
ordinary child-backed Button contract, shared by the legacy and
new installation routes, subject to the G0 decision.

Replace independently public child and text fields with one
private canonical Button content representation:

- Standalone(Option<String>): ordinary direct caption or its
  explicit absence.
- ChildCandidate(NodeId): unmounted legacy Button::new input;
  the supplied slot is known but not yet qualified against the
  actual intended host.
- ChildBound { slot: NodeId, owner: TreeWidgetId }: the single
  immutable required child relationship of an admitted host.

ChildBound is one qualified relationship: registered identity is
authority; slot is the existing storage locator. They cannot
be changed independently. There is no persistent role registry
or second content authority.

The new path constructs the actual Button directly in ChildBound.
Existing Button::new(NodeId) keeps its signature. At genuine
legacy installation into the retained host node, qualify the
actual supplied current slot against the actual live direct child
of that host before successor mount; capture that child's
TreeWidgetId. This is part of successor installation, not
post-mount repair. No unqualified ChildCandidate may become
live. The raw legacy constructor cannot identify an arbitrary
former slot occupant; caller must supply its current intended
slot in the same tree.

Both routes use the same bound representation and ordinary
behavior after installation. At every existing generic installation
boundary (create_root, add_child and replace_widget_only), validate
the Button content mode before any allocator, registration, dirty,
mount or topology effect. An ordinary single-node insertion cannot
satisfy a required direct-child relationship for a not-yet-installed
host; create_root and add_child therefore reject such a child-backed
candidate before effects. A bound host is admitted only by the actual
qualified composite installation or the genuine replacement path
that validates its relation against the retained host. Preserve the
ordinary standalone routes. Use existing all-build invariant-failure
policy for forbidden input at legacy non-Result entry points.

Preserve existing lawful legacy child-content capability; the new
pair's Label-only/passive initial admission does not retroactively
restrict every legacy constructor to Label. The legacy reference
must nonetheless describe an actual live direct child of the
intended host.

### 8.2 Supported mutation and use protection

Before existing Button layout or paint consumes the required
child, verify that the stored locator resolves to its stored
TreeWidgetId and the actual host's direct child membership.
Reuse existing tree/node and traversal facts, without an
additional index or full-tree scan. Missing or mismatched
relationship is an all-build invariant failure before consuming
another object's layout, cached size, geometry, text or paint
suppression. No plausible-child search, fallback to text, silent
rebinding or layout-time repair.

Fold this local check into the common ordinary required-child
access. It is a structural-correctness requirement shared by
new and legacy installation, not an assembly-provenance branch.
No additional idle traversal or layout pass.

The structural owner protects the required relation before
supported mutation effects:

- remove_node of a required child while its bound host survives
  refuses before unlink, dirtying, retirement or releasing a
  slot. The existing sealed ScrollArea guard identifies the
  owner boundary; do not transfer ScrollArea's role policy.
- clear_children prequalifies the requested direct-child set
  before removing any member. No partial clear before finding
  a protected child.
- replace_widget_only on a required child while its host survives
  refuses before predecessor retirement or successor effects.
  Retained TreeWidgetId alone does not qualify another concrete
  child.
- Ordinary removal of a subtree including the host retires
  its required child together with it. Internal recursive
  retirement of that subtree is not an independent forbidden
  child removal.
- Genuine host replacement remains valid. Qualify the successor's
  own content relation against the actual retained node and
  children before retiring the predecessor. The predecessor
  role ends with its concrete mount. An admitted successor may
  use a current direct child or its own ordinary standalone
  mode. Do not destroy retained child membership implicitly.
- No lawful helper may bypass this protection. No new reparent
  API is proposed. If an existing admitted owner operation
  detaches a required child from a surviving host, enforce
  the same pre-effect boundary.

The existing layout take/restore stores DummyWidget temporarily
and marks widget_layout_lease (src/core/widget_tree.rs:3918-3937,
3953-3983). This is not genuine replacement and must not hide
the role from structural mutation. Extend the existing lease
observation with the copied immutable bound-child locator and
identity of a leased child-backed Button; ordinary leases
retain their semantics. Capture it from the actual object at
take and retire it on restore of that same Box. This is a
temporary observation of the single canonical role during
the existing lease, not a second enduring authority, registry
or mount. The required-child mutation guard uses this observation
when the parent Box is leased. Never infer absence of the role
from DummyWidget.

Do not authorize mutation during an active borrow or retirement
of a leased object. Preserve the actual independently admitted
specialized layout-owner operations. Qualify the real reachable
mutation paths at G1; any lawful route touching this relationship
must enforce the exact protection before effects.

Keep existing void tree mutation signatures and their all-build
invariant-failure policy for forbidden required-child operations.
No silent no-op or delayed repair. Existing ordinary extra child
membership beneath a legacy Button is not newly prohibited
unless it destroys, substitutes or rebinds the required child.
The initial pair contains exactly its two intended actual nodes.

HAL-087 already excludes whole-node/whole-concrete-value substitution
through an ordinary mutable downcast. This Draft does not legalize
such use or claim that all arbitrary Rust mutable-borrow misuse
becomes mechanically impossible. The supported owner mutation
and required-child use boundary is mechanical. If G1 identifies
an admitted route that evades it, resolve that concrete route
before implementation. No general public-borrow redesign follows.

### 8.3 Explicit compatibility decision

Unrestricted public child/text mutation and a mechanically
immutable single content role cannot both be promised. Do not
retain competing fields with a shadow role or silently ignore
writes.

The proposed change removes direct public access to Button::child
and Button::text, replacing it with:

~~~rust
pub fn child_node(&self) -> Option<NodeId>;
pub fn text(&self) -> Option<&str>;
pub fn without_text() -> Self;
pub fn set_text(&mut self, text: Option<String>) -> Result<bool, ButtonContentError>;
~~~

child_node is a copied slot observation, not a structural setter
or generation-safe handle. text exposes the current standalone
caption. without_text creates ordinary standalone absence.
set_text changes only Standalone's Option<String>; true on actual
change and false on equality. Child-backed/candidate modes refuse
with ButtonContentError::NotStandalone and no effects.
Preserve None versus Some(empty). Existing ordinary caller-owned
layout invalidation after an actual standalone text change remains,
including Screen::update with a tree borrow. No hidden invalidation
service or public mode-converting setter.

Preserve existing Button::new, Button::with_text, fluent configuration
and other valid behavior. Reinspect actual semantic references
when preparing implementation. Known direct child/text assignments
that construct a test caption:
src/tests/retained_value_updates.rs:479-480 and
src/tests/retained_value_updates/button_appearance.rs:3456-3457.
Convert those exact fixture operations to lawful existing
constructor/replacement or actual new qualified construction,
preserving test content, typography, geometry and assertions.

Known standalone text update/absence evidence:
src/tests/retained_value_updates/button_appearance.rs:2956;
src/core/app/transient_presentation/tests.rs:718,840.
Preserve these observable capabilities using the explicit text
operation, not deleting assertions or replacing None with empty.

This is a source-compatibility change, including the required-child
removal/replacement restriction. External use is not claimed to
be exhaustively known. Explicit maintainer approval is required
at G0, and eventual implementation must follow existing release
classification and compatibility communication. No version or
release action is authorized now.

If direct unrestricted field mutation must remain legal, G0 is
blocked. Do not deliver a false fixed-role guarantee, introduce
a competing authority or silently change an existing public API.

### 8.4 Identity and lifetime summary

| Fact | Preparation | Success | Update | Retirement/replacement |
| --- | --- | --- | --- | --- |
| NodeId | Private planned slot | Live locator | Same for surviving node | Retained on true replacement; reusable after removal |
| TreeWidgetId | Private fresh owner | Registered identity | Stable | Retained on true replacement; retired on removal |
| WidgetInternalId | Actual object's local ID | Same local ID | Stable | Successor's own ID |
| WidgetKey | Caller input | Root key | Ordinary semantics | Ordinary replacement semantics |
| Concrete mount | Privately prepared | One per real node | Preserved including layout lease | Predecessor retires; successor fresh |
| Required child | Privately resolved | Bound actual direct child | Immutable relation | Host retirement ends role; independent child retirement refused |

Preserve HAL-354 current mount-qualified work and retained
fragment eviction. The operation introduces no former occupant's
mount transfer or new raw-index queued-work mechanism. This is
not a claim that every existing subsystem is generation-safe.

### 8.5 Independent SignalStore exposure (A03)

Current SignalStore uses HashSet<NodeId> for subscriptions and
Vec<NodeId> for pending dirty work. Current removal queues the
old raw NodeId and immediately releases the slot. App can rebuild
before draining raw removal cleanup and later applies pending
dirty work by raw slot.

Evidence: src/core/widget_tree.rs:1680-1693,2418-2421;
src/model/signal_store.rs:35-44,82-131;
src/core/app/ui_loop.rs:643-719,783-786;
src/core/app/run.rs:1768 onward for corresponding native rebuild
and cleanup. LayoutContext::watch forwards the current NodeId
(src/core/widget/mod.rs:200-210). The semantic reference search
identified that forwarding and direct SignalStore tests, not
demonstrated widespread production use.

Source-derived sequence: remove old node; reuse its slot;
subscribe successor before raw removal cleanup; remove subscribers
using the former occupant's slot. The successor can lose its
subscription. A pending old dirty slot can likewise target a new
occupant. Actual supported-consumer reachability of each sequence
requires separate reproduction. No runtime reproduction or observed
production impact is claimed.

Record this as an independent open source-level finding belonging
to SignalStore and App lifecycle/invalidation. HAL-031 remains
Draft. HAL-087/HAL-148 identity constraints remain authoritative.
Do not redesign signals, change ordinary allocator reuse, add slot
quarantine, flush a foreign owner's queues, or migrate global IDs
in HAL-402.

The new operation must not introduce earlier unsupported subscription,
prospective-slot exposure, raw queued work or altered cleanup cutoff.
G1 checks the actual selected consumer against the precise existing
exposure. If the proposed path adds or depends on an actual incorrect
raw-index path, hold it until its owning contract resolves the
necessary boundary. Otherwise the independent finding remains
open without blocking qualified unrelated construction work.
No global stale-work safety claim follows.

## 9. Failure and cost

Recoverable refusal precedes logical allocation-plan consumption
and semantic installation. Previously existing objects, membership,
registration, identity, mount, dirty state and geometry remain
unchanged. No mount/unmount, action, pending-removal or current-fragment
retirement results from refusal. Capacity changes without logical
membership effects are permitted. Owned inputs drop normally.

After installation begins, no ordinary Err. The closed sequence
contains only the admitted built-in objects and qualified owner
functions, not arbitrary caller factories, external actions, IO,
await or destruction of a synthetic predecessor.

Panic, allocator abort, WASM panic-abort or engine invariant failure
is not recoverable refusal. Preserve the actual fatal policy.
No general rollback of arbitrary fatal or external effects.
The permission expires on unwind where supported; correctness
must not depend on unwinding in WASM.

The operation installs two real nodes and two concrete mounts.
Allocation planning is bounded to two slots and reuses the
existing allocator. Ancestor verification and sibling insertion
have their actual costs; do not claim constant-time insertion or
performance improvement without evidence.

No assembly state remains after success. The permission exists
only during actual build, and section 8's necessary local checks
and lease observation serve the common enduring relationship,
not assembly provenance. No new permanent role registry, service
or traversal. Measure two real installations/two mount hooks
and zero construction-time unmounts, replacements or synthetic
pending removals against the actual legacy baseline. Separate
construction effects from steady-state costs.

## 10. Explicit exclusions and retained behavior

Preserve genuine replace_widget_only as the concrete mount boundary,
including registered identity, valid retained topology, predecessor
retirement and fresh successor mount. Protecting a surviving
required role does not abolish true replacement. Preserve ordinary
whole-subtree retirement and layout take/restore of the same Box.

Existing placeholder-based callers and replace_widget macro remain
available. No broad migration. HAL-154/HAL-101 closed-role admission,
virtualized owner operations, availability/input/animation/layout/
rendering contracts stay authoritative. No new Widget trait,
global identity types, Screen rebuild policy, renderer, event router,
App state model, theme, layout model or Deferred primitive.
No new feature gate or alternate supported execution mode.

## 11. Initial scope after adoption

The owner boundaries of the first implementation are limited to:

- WidgetTree allocation, insertion, required-child mutation checks
  and the existing layout lease observation;
- actual runtime admission in src/core/app/run.rs, ui_loop.rs,
  app_root.rs, and ScreenFrameContext in src/core/app.rs;
- ordinary Button initialization and private canonical content
  relationship in src/widgets/button.rs;
- actual passive-caption admission in src/widgets/label.rs;
- nearest existing owner-local tests/public integration tests,
  necessary compatibility fixtures, the single real Showcase
  consumer and corresponding public API documentation.

Representative consumer: examples/showcase/tab_overlays.rs:561-577,
the existing Open Context Menu. Replace only construction scaffolding.
Preserve actual caption, parent, binding, order and behavior,
other controls/examples and existing Escape/navigation rules.

Any implementation task requires G0 adoption and full current
Work Doctrine/Halcyon pre-flight, applicable binding authority
and PM reading, actual source/owner inspection and exact runnable
verification. Do not invent existing test names. Use nearest
existing declared test surfaces, not a separate test root,
test-only permission, synthetic action or alternate renderer.

Rustdoc and affected guides describe the actual qualified API
under HAL-309, with Draft/implementation distinguished.
No issue publication, release or implementation authority
is granted by this document.

## 12. Qualification and acceptance

### 12.1 Structural, lifecycle and authority evidence

| Case | Required evidence |
| --- | --- |
| Real pair | Actual admitted Flex parent, exact two intended types and complete direct links, key/order/z-order, correct registrations; no artificial mount. |
| Initialization | One canonical defaults/validation owner; ordinary enabled/cursor/padding/caption settings, action binding/callback, applicable appearance/material inputs; no silent option loss or action during preparation. |
| Allocation | Distinct slots and fresh registered identities/mounts, reuse/growth/mixed cases, checked exhaustion/refusal; no former concrete mount becomes current. |
| Lifecycle | Exact child-first hooks before final membership, applicable registrations once after links complete, ordinary neutral state, no construction-time unmount/replacement. |
| Actual authority | Real owned base and modal startup/rebuild paths accept; forged public context/manual root outside scope, unrelated subtree, retired anchor or wrong tree cannot grant permission. |
| Scope lifetime | Ordinary return and supported unwind revoke permission; subsequent scope distinct. No WASM panic-abort recovery claim. |
| Refusal | Actual invalid parent/role/configuration or bounded reserve failure causes no membership, registration, identity, mount, dirty or lifecycle effect. |
| Enduring role | Direct child removal, clear_children and required-child replacement fail before effects. Whole subtree retirement and valid host replacement remain ordinary. A layout lease cannot hide the protected role. |
| Required-child access | Actual stale/mismatched occupant is not consumed for cached layout, paint suppression or text lookup; no repair/fallback. |
| Compatibility | Legacy constructor/replacement, standalone caption/absence, retained text changes and genuine replacement preserve behavior; only explicitly adopted restrictions change. |
| Label policy | Initially passive caption accepted; initially selectable/associated Label refused without rewriting policy. |
| Current lifetime | Existing retained mount/fragment qualification and layout take/restore remain; no global SignalStore claim. |

Use actual production owner structures and entry points.
Reuse existing proof, including
widget_mount_tokens_change_on_replacement_and_node_slot_reuse
in src/core/widget_tree.rs, where its evidence remains applicable.
Extend the nearest existing owner surface where necessary. Avoid
duplicated tests and unsupported-space rejection campaigns.

### 12.2 Actual consumer equivalence

Compare the new pair against the actual legacy sequence with
matched inputs, placement, theme, constraints, viewport, DPR
and interactions. Normalize only intentionally different numeric
identities and lifecycle history.

Required results:

- Same ordinary first layout/paint and constrained resize:
  natural size, padding, caption metrics, clipping, placement,
  child paint suppression, focus/hit geometry and visual order.
- Existing pointer press/release, exit, outside release, cancel,
  disabled behavior, keyboard Enter/Space/focus and applicable
  touch semantics remain. No duplicate activation.
- Actual existing ChangeWriter binding and callback semantics
  preserved. Open Context Menu opens its real menu through
  the ordinary action, not a synthetic substitute.
- Retained data/appearance update, animation, identical clean
  frame and resize preserve actual mounted objects without
  assembly-specific runtime work or unintended replacement.
- Ordinary rebuild/navigation away and back creates/retires
  correct identities and registrations under existing Screen
  policy. No leaked new registrations or obsolete mount-qualified
  work. Existing standalone/legacy Buttons and affected sealed
  composites remain correct.

Run affected locked native/WASM builds/tests and actual Showcase
interaction/rendering. Compilation is not runtime/presentation
or physical-input evidence. Distinguish backend/browser output,
physical input, scanout and maintainer visual acceptance.
Do not infer general platform qualification from one paired result.

### 12.3 Gates

**G0 — Adoption.** Explicit maintainer adoption of this exact
architecture, initial role/API and public content/mutation
compatibility decision in section 8.3. G0 has not occurred.

**G1 — Source/owner qualification.** Establish actual canonical
configuration and allocator ownership, positive runtime scope,
exact lifecycle/registration order, current lawful mutation paths
touching the required relation, and the actual consumer's
interaction with section 8.5. Resolve concrete adopted-contract
conflicts before dependent implementation.

**G2 — Mechanical proof.** Exact implementation satisfies section
12.1, including all-build authority and role enforcement and
preserved existing capabilities.

**G3 — Real consumer.** Actual representative Showcase control
satisfies section 12.2 and required construction/cost boundary.

**G4 — Maintainer result/API review.** Review qualified real
developer-facing API and compatibility communication. Further
roles or migration need separate authorization.

A failed gate blocks the affected proposed objective, not an
invitation to weaken it, create another mode or invent work.

## 13. Control audit disposition

| Finding | Design disposition |
| --- | --- |
| HAL-402-A01 High | Sections 8.1-8.4: one qualified private content representation, pre-effect protection, actual-use identity/membership checks, visibility during layout lease. Exact compatibility adoption remains required at G0. |
| HAL-402-A02 High | Section 6: actual runtime-originated permission, tree/root/mount/new-subtree qualification and revocation; public names/context do not grant authority. |
| HAL-402-A03 High | Section 8.5: narrow original universal queued-work promise; independently record actual source exposure, owner and interaction gate. No SignalStore redesign/adoption. |
| HAL-402-A04 Medium | Sections 4-5: sealed common pair operation and canonical ordinary Button initializer, no specialized tree entry or parallel partial options. |
| HAL-402-A05 Medium | Section 5.3: real initial passive Label predicate; no silent disabling or permanent passive promise. |
| HAL-402-A06 Medium | Section 7: exact prepared ownership, hook ordering, membership, registration and observation; intermediate lifecycle equivalence not claimed. |

These are design dispositions, not claims of corrected source or
runtime conformance. Proposed technical decisions are the common
pair API, shared Button initialization, explicit scoped content/
mutation correction, runtime witness and subtree scope, and
initial passive-caption admission.

HAL-402 remains on HOLD for implementation pending explicit
adoption. The independent SignalStore finding remains open.

## 14. Forbidden behavior

**FORBID-402-1:** Artificially mount and replace an unrelated object
to obtain IDs for the new operation.

**FORBID-402-2:** Second live tree, persistent widget references,
Widget ownership of another Widget, fabricated IDs or caller
exposure of prospective raw locations.

**FORBID-402-3:** Publicly reachable incomplete composition,
unresolved required role, post-mount repair or new hook authority.

**FORBID-402-4:** Caller-created permission, global/thread-local
mode, public names as positive authority, unrelated subtree
admission or permission surviving actual build.

**FORBID-402-5:** Competing writable content authority,
silent required-child removal/replacement/rebinding or treating
a retained registered identity as proof of the same concrete mount.

**FORBID-402-6:** Widget-specific tree method family, duplicate
options/default/style/action policy, arbitrary factories or
general transaction service for this fixed-pair problem.

**FORBID-402-7:** Assembly-specific retained runtime, hidden
mode, extra frame traversal, second layout/paint path or
geometry authority.

**FORBID-402-8:** Silent compatibility break, weakened HAL-154
or existing closed-role authority, unapproved migration or
publication.

**FORBID-402-9:** General rollback/panic recovery, universal
stale raw-ID safety or lifecycle equivalence inferred from pixels.

**FORBID-402-10:** Treating this Draft, source reasoning or
test-only permission as implementation, adoption or runtime proof.

The accepted result must be the same real ordinary composition,
with coherent developer-facing construction, its actual required
relationship determined before the first real mount and protected
afterward, one installed lifetime per intended object, and no
artificial predecessor. Construction machinery serves this
result; it does not become the product architecture.
