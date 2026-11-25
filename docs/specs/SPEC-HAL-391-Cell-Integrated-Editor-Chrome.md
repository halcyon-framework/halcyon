# SPEC-HAL-391 — Cell-Integrated Editor Chrome

**Status: ADOPTED / ACTIVE / BINDING — 2026-11-16, direct JC approval**\
**Date:** 2026-11-15\
**Scope clarification:** 2026-11-16 — JC: substrate-wide cell-hosting capability, not a ListView-only enhancement. §13 records the adopted mechanism and narrow closed-scope reconciliation.\
**Design intent:** JC — an editor in a cell occupies the cell; the cell edge is its frame.\
**Authority:** Direct maintainer approval for the framework-wide substrate capability and its implementation.\
**Implementation status:** Implementation and evidence are required by §13; this status change alone proves no runtime or visual acceptance. JC's acceptance of actual pixels remains open.

## 1. Purpose and status of requirements

This SPEC specifies a reusable cell-integrated presentation contract wherever qualified consumers of flat or hierarchical tabular substrates mount actual editor/control widgets. It does not belong to one named table widget. The genuine Showcase **List View → Choice** cells displaying Alpha, Beta, Gamma and Delta are the first visible acceptance case, not the product-scope boundary. The inspected baseline insets each ComboBox and draws its rounded input frame inside the cell. The adopted result extends to the cell edges, with the cell's border/separator/focus edge as the only exterior frame.

The adopted result is a change in embedding presentation, not a global restyling of TextField or ComboBox and not a substitute implementation of their editing behavior. Standalone controls retain their existing appearance and semantics.

“MUST”, “SHALL” and named invariants below are binding within the adopted scope. §13 supersedes the earlier unresolved carrier/adoption alternatives; historical observations and their hashes remain historical. Source observations and adoption do not prove completed conformance, runtime tests, native/WASM pixels or JC visual acceptance.

## 2. Scope

### 2.1 Qualified consumers

The adopted contract applies across accepted flat and hierarchical tabular cell-hosting paths. ListView, PropertyEditor and TreeView are current source-qualified consumers with real editor mounts (§3.2); they are not an exclusive list of consumers to which the contract may apply. Consumer readiness determines which existing embeddings can implement the contract, not whether another tabular consumer belongs to the product scope.

A particular consumer/family is ready for implementation only after identifying its accepted cell-allocation owner, actual WidgetTree mount and lifetime, lawful presentation-delivery path, current editor-state source for one cell edge, and existing hit/clip/overlay contract. A geometry projection alone supplies none of the missing hosting authority. A future or currently non-hosting consumer needs its own accepted hosting contract before mounting widgets; this RFC neither grants that capability nor excludes it permanently from the shared presentation contract.

This is an opt-in selection by the qualified cell host for each embedding, not an application runtime feature flag. It does not require every raw/text/read-only cell to become a widget, and it does not introduce factories or widget IDs into a geometry substrate. The current readiness inventory and unresolved delivery gates are explicit in §§3.2 and 6.

The mode applies to a **particular qualified embedding**, not to every instance of a widget type. A standalone TextField beside a table, a popup's own contents, a toolbar ComboBox and an unrelated descendant remain standalone unless independently qualified. Mere ancestry inside a table is not eligibility.

This proposal does not change:

- Cell values, resolver results, validation, edit triggers, commit/cancel policies, permissions or callback order.
- Row/column topology, selection semantics, column resize authority, virtualization or native/WASM input policy.
- Text shaping, fonts, density, control text sizes, caret/selection/IME, Undo/Redo, motion eligibility or timing.
- WidgetTree ownership, framework Focus/Capture, popup/overlay authority or renderer custody.

### 2.2 Control families

| Family | Proposed cell presentation | Qualification required before implementation acceptance |
| --- | --- | --- |
| TextField | Full cell editing surface; cell-owned exterior edge; existing text/caret/selection presentation inside. | Text viewport, focus/capture, selection, IME, history and clipping. |
| ComboBox | Full cell shell and trigger surface; content and arrow remain internally laid out; no independent outer or joined-popup frame inside the cell. | Editable/noneditable modes, tree-owned editor child, resolver states and popup anchoring. |
| SpinBox and other whole-surface numeric editors | Full cell shell; retain internal value viewport and button/step affordances. | Exact family owner, composite child/session, hit zones and numeric result transaction. |
| Checkbox, a color swatch and similar intrinsic affordances | The cell owns the exterior frame and cell-level focus cue; the actual checkbox box, check mark or swatch retains its natural geometry. | Intrinsic geometry and widget interaction cues; no stretching a checkbox into a text input. |
| Other compound editors | No automatic adoption from a name or shared painter. | Explicit family review of all shells, children, triggers and overlays. |

Supporting this contract in a shared input painter does not by itself qualify every caller. An unsupported embedding remains explicitly unqualified for runtime integration until its own readiness gates pass; it is not removed from the substrate-wide contract. It must not be advertised as integrated while silently painting a standalone shell.

The current shared default factory creates **TextField** for String, Integer and Float values (`src/widgets/cell_editor_factory.rs:122–150`), not SpinBox for numeric values. Naming SpinBox in this contract does not replace those current editors, create a new numeric mount path, or qualify every compound family automatically. Intrinsic marks such as a Checkbox box, color swatch or TreeView disclosure retain their accepted natural geometry; their enclosing cell surface and edge must not be confused with the mark itself.

## 3. Source-qualified starting point

The following original observations and §3.1 identities are preserved from the 2026-11-15 draft, including then-unfinalized browser work. They are historical inspection anchors, not assertions that every file hash or line number remains current. The 2026-11-16 scope qualification is recorded separately in §3.2.

| Owner | Inspected source and observation |
| --- | --- |
| Showcase composition | `examples/showcase/tab_list_view.rs:53,114–119,222–238`: Choice has a 120 logical-unit column, real Alpha/Beta/Gamma/Delta options and `set_column_combo_options`. It is not PropertyEditor or an HTML control. |
| ListView child allocation | `src/widgets/list_view/body.rs:446–498`: Layout publishes the current choice-role manifest; `padding_x` is applied to both outer X edges when constructing `CompositeChoiceRole.rect`. The comment identifies installation/synchronization at the next ordinary Phase 2 cut. |
| Cell layout | `src/engines/cell_mechanics/mod.rs:1328–1353`: layout consumes established mounted/readiness facts and the matching role rect, applies tight constraints and sets the actual child rect. It does not create the editor during Layout. |
| ComboBox shell | `src/widgets/combo_box.rs:1629–1778`: the host paints theme-resolved outer border, inner edge and fill, including a separate joined-popup path, followed by its arrow affordance. Resolver-state fills carry semantic information. |
| Shared input shell | `src/widgets/text_field.rs:3649–3717`: `paint_input_surface` paints rounded border, inner edge, fill and optional emphasized ring. Its own comment leaves geometry and Focus/Capture authority with the caller. |
| PropertyEditor cell cue | `src/widgets/property_editor/body.rs:733–875`: cell painting suppresses content/focus in editor or Checkbox cells because the child currently supplies its presentation. `is_edit_cell` includes the post-exit, pre-unmount CMS interval. The ordinary cell focus border is emitted at `LAYER_FOCUS`. |

Thus removing only ListView's horizontal padding would leave the nested rounded shell. Removing only the widget's frame would leave the inset allocation and could remove the only focus/capture cue. The proposed change must coordinate allocation, shell presentation and the cell edge in the same qualified embedding.

### 3.1 Inspected source identities

These identify the original source used for the RFC, not a current or accepted implementation manifest:

| Path | SHA256 |
| --- | --- |
| `examples/showcase/tab_list_view.rs` | `53c8dde8ff7bc9426dafc714ebdb67a6e8d8d6f7d4126e120170ce3e42ed573f` |
| `src/widgets/list_view/body.rs` | `8f9fcae0850f3ceea24ccddaa20de9028a8f293a7b28341b275a4732756bdb66` |
| `src/engines/cell_mechanics/mod.rs` | `c9334774d2134e7f8f3b9eaa7ac2ffb1f72357425721372235ac8c0fac7b2741` |
| `src/widgets/combo_box.rs` | `1968c8163654140d354190f7819d202cc16f076c74c4ea718e8b9e4421c5692a` |
| `src/widgets/text_field.rs` | `b6b4f19982a3884083acbe662dd08094d895811febfcea333132ee6b21dd64b4` |
| `src/widgets/property_editor/body.rs` | `2adc4362dd93798d4d44d2f81294c7b16022fbc21986c4bc16a53548a04c82a5` |

### 3.2 Current substrate and consumer readiness — 2026-11-16

This inventory was inspected at HEAD `52c63c10cc94879bfad4d46c77af8269c3e13f7a` with the existing working changes preserved. “Hosts today” describes the actual source, not acceptance of CellIntegrated presentation. No current path below implements this draft's mode.

| Layer / consumer | Actual geometry, hosting and presentation path | Readiness and exact boundary |
| --- | --- | --- |
| Flat `TabularSubstrate` / `tabular_view` module | `src/widgets/tabular_view/mod.rs:346,2635–2766`; `cell_content.rs:33–48` copies the cell rect into `CellContentProjection`. A Widget implementation exists: its layout ignores child input, its paint draws separators, and its event handler is a no-op. | A cell-geometry/projection source, not an existing general editor mount. No separate live `TabularView` widget host was found in this module. Do not infer a hosting API from the module name or the Widget implementation. Preserve its sealed topology and rect identity. |
| `HierarchicalTabularSubstrate` / `SharedHts` | `src/engines/hierarchical/mod.rs:8–10,61–79,102,146–148`: topology/geometry/virtualization/realization and the shared handle publish row/column geometry. | Not a WidgetTree host, renderer, presentation-state registry or editor factory. Flat ListView also currently consumes this shared machinery through CMS; flat versus hierarchical display is not a reason to duplicate the contract. |
| `CellMechanicsSubstrate` (CMS) | `src/engines/cell_mechanics/mod.rs:72–161,248–347,1328–1353,1497`: existing role manifests, phase-qualified composite work, Bool/Choice pools, edit session/current mount and factory; Layout consumes ready choice mounts with the published role rect. | Shared mounting/session mechanics above geometry, not another topology or cell-rect authority. This is a candidate common delivery seam, not permission to give HTS widget-host responsibilities. |
| ListView | `src/widgets/list_view/core.rs:116,236–239`; `body.rs:428,446–498,609`: CMS/factory, resident Bool and qualified Choice paths. Choice Layout publishes its role, then existing Phase 2 installs/synchronizes it; lines 462 and 475–490 add the exterior horizontal inset. | Real Bool/Choice hosting today; first visible acceptance is the genuine Choice column. No general on-demand TextField/SpinBox edit-session path was found in this consumer. Keep the closed composite; do not invent external widget injection to generalize presentation. Its current row selection/focus paint is not already a universal editor cell-edge resolver. |
| PropertyEditor | `src/widgets/property_editor/property_editor.rs:222–223`; `body.rs:383–406,733–875`: CMS/factory, resident Bool and session editors; snapped cell slots, content and focus paint are consumer-owned. Lines 750–778 include the post-exit/pre-unmount editor; lines 863–875 suppress the body cue when a child supplies it. | Real shared-factory editors today. Integrated edges must reconcile that suppression and exact allocation/edge geometry without a missing transition cue. Band/read-only cells do not become editors. Current numeric editors remain TextField. |
| TreeView | `src/widgets/tree_view/tree_view.rs:261–262,511`; `body.rs:631–677,906–939,1412`: CMS/factory and CellSource, real edit-session reconciliation and edit entry; current selection/focus paint is row-wide. | Genuine additional hierarchical editor host. Its row cue is not a cell-edge Capture cue. Consumer/family qualification must specify the integrated cell edge while preserving existing row behavior. Do not assume PropertyEditor's resident Bool path exists here: no `reconcile_always_mounted_editors` call was found in TreeView. |
| Shared default editor factory | `src/engines/cell_source/mod.rs:400–431`; `src/widgets/cell_editor_factory.rs:67–150`: existing Bool create/sync, Choice create/owner-qualified sync and general editor construction. General construction currently covers TextField, ComboBox, ColorPickerField and KeybindEditor according to value kind. | Reuse this real factory and current consumer mount paths. The factory is not a new public user-injected widget factory. Read-only/Badge and Bool special handling are not automatic whole-surface editor mounts. Each actual family needs shell/edge/overlay qualification. |
| Legacy `List` | `src/widgets/list.rs:1–18,53–57,833`: a real flat TabularSubstrate consumer with row selection and paint. No CMS/CellSource/editor-factory mount was found. | Substrate use alone does not qualify cell-integrated runtime behavior. New cell-widget hosting would need its own accepted contract first. |
| Similar names without the cell-host contract | `src/widgets/flow_grid.rs:1–24` is wrapping child layout; `src/widgets/readout.rs:455–465,1003` contains readout/status display types; `src/widgets/navigator/navigator.rs:1–5` explicitly does not use TabularSubstrate. | Neither “grid/table” naming nor arbitrary child layout establishes domain-qualified tabular editor cells. No general editable DataGrid/TableView/Spreadsheet host was found by the source inventory. Future names in comments are not implemented capabilities. |

For this inspection the shared seams were `cell_source/mod.rs` SHA256 `23e0d520011a5d9d06c5dd453f5d8cbab55ea35a6572cb4989b330998baa0c91`, `cell_editor_factory.rs` `ed2531e1f845f611d944a01aaa986cb5aa6011b56be60930b74c55421e97bc5a`, `tree_view/body.rs` `518d841e3a4326b7f16333d225c2ff3bef37a9ee81604f9cffedccaedfd00337`, `tabular_view/mod.rs` `c6a62190dc83960994c1f2246269db02fcd4c12d72505ba7ebb78b77cf9f9dea`, and `engines/hierarchical/mod.rs` `58de4c2ae77f56bca113e0bd0ec8a111db8122d8eb5b3eff4bf5e0e6dbf1956d`. Current TextField was `6778a1f0b652674693b1c6539ae8666243e4ea4f8bae844b629d9b2a5394543e`; the original §3.1 TextField hash is retained as history.

### 3.3 No transfer between geometry and hosting owners

The existing geometry owner publishes cell allocation; the qualified consumer/CMS mount path associates it with a concrete resident editor; the widget paints its semantic interior; the consumer paints the single exterior cell edge from current qualified facts. These are separate responsibilities even when one consumer currently holds a CMS handle. Presentation selection must not add a second rect, turn `CellContentProjection` into an editor factory, store WidgetTree state in HTS, or reinterpret a plain cell as an editor. A raw substrate consumer cannot become a hosting consumer merely by selecting the proposed presentation value.

## 4. Visual and geometry contract

### 4.1 Full cell surface

For a valid, realized cell hosting a qualified whole-surface editor, let `C` be the current cell rectangle published by its accepted geometry owner, in the coordinate space expected by the existing child-layout path. The editor's outer allocation and surface are `C`: no additional external X or Y inset, gap, margin, rounded shell or inner border is introduced by the embedding.

`C` must come from the existing cell/content projection and column-slot/row geometry owner. It must not be reconstructed by the editor from row indices, visible labels, a screenshot, the root/window size or a cached prior frame. This proposal does not redefine the substrate's `CellContentProjection` or create a second content-rect derivation rule.

For the Showcase Choice column, the visible result is Alpha/Beta/Gamma/Delta in full-width, full-row-height editor surfaces reaching the grid edges. There is no separate rounded rectangle floating inside each Choice cell. Column width, row height, text size and density remain those authored by the original Showcase and theme.

### 4.2 Exterior frame versus content padding

The cell owns the visible exterior frame. In integrated mode, the embedded whole-surface widget does not paint its standalone outer border, inner edge, exterior shadow/halo or independent rounded focus ring. Suppression includes every equivalent shell path, including ComboBox's joined-popup shell; suppressing only the closed-state painter is insufficient.

The widget continues to paint its semantic fill and content. Fill covers the surface up to the cell edge under the cell's existing clip and edge composition, without erasing the grid edge. No artificial rounded cutouts or body-colored strips may expose a gap at corners. The grid remains rectangular except where an independently accepted enclosing-grid boundary already defines another shape.

**Interior content padding is retained.** Theme/density-driven text inset, baseline, caret viewport, arrow/step-button spacing and intrinsic affordance spacing remain owned by the affected widget's layout. Zero exterior inset does not mean zero text padding. The change must not force a tiny font, enlarge an intrinsic mark, or enlarge the row to conceal an incorrect layout.

The active editor's geometry may change from today's inset allocation. Its internal content and hit regions must be recomputed from that actual new allocation; retaining stale inset hit geometry is a defect.

### 4.3 One visible edge and meaningful state

The consumer owns cell selection, grid separators and the cell-edge representation of focused/captured/hovered/disabled editor state. The widget remains the source of its actual editing/availability and semantic appearance facts. A cell cursor alone must not be presented as a widget keyboard Capture, and an active popup must not fabricate Capture in its parent.

Every qualified focused/captured editor must have the one appropriate **cell-edge** cue. Removing the child ring while leaving PropertyEditor's current suppression unconditional would produce a blank cue and is forbidden. Leaving both the cell cue and a child ring would produce two cues and is also forbidden.

Cell focus and actual widget Focus/Capture are distinct inputs to one resolved edge presentation. Existing accepted state classifiers and concrete theme/component appearance values remain the source of colors and emphasis. This SPEC proposes no RGB constants or new universal precedence for conflicting states. The implementation disposition must identify the concrete resolution for each adopted family, including disabled/read-only, hover, validation and popup states; an unresolved state combination is an adoption gate, not permission to layer unrelated borders.

Adjacent cells share the existing grid/separator ownership. A shared physical edge must not become two independently composited control strokes or a darker per-row seam. A focused/captured edge replaces or coherently composes the neutral edge through that owner; it is not a second inset or exterior frame. Existing declared paint layers and strict ordering remain authoritative.

Row/cell selection remains discernible while an embedded surface is present. The consumer and widget must compose their existing selection/fill facts so an opaque editor fill does not silently erase required selection feedback. This does not transfer cell selection or value mutation to the editor.

### 4.4 Extents, clipping and pixel alignment

Only current, qualified, finite cell extents may be used. Negative/nonfinite/unqualified extents are refused through the lawful owner path; zero-size or fully unavailable cells do not acquire invented bounds, active hit regions or a fabricated minimum GPU target. No clamp-to-one workaround or arbitrary new minimum column width is introduced.

A narrow positive cell uses the existing font, content layout, clip and affordance rules. Content that does not fit is clipped or withheld by its actual established availability rule; geometry is not guessed from a label and text is not shrunk to force a fit. Any genuinely unsupported positive-size projection is reported as a concrete geometry limitation for review, not hidden by falling back to a nested standalone frame.

HAL-024 traversal remains the owner of absolute placement, scroll and clipping. The mode adds no widget-authored cell clip, extra scroll transform or source-authored absolute coordinate path. Horizontal scroll, pinned-column boundaries and viewport clipping remain unchanged.

Cell allocation, edge snapping, content clipping and input conversion must use the same current scale and geometry snapshot. Snap shared absolute edges consistently at the renderer boundary; do not round each child's width independently into a seam. Logical geometry and physical pixels remain distinct. Native and WASM use this same contract at actual DPR, including fractional scale.

## 5. Ownership and input

| Concern | Sole existing authority retained by the proposal |
| --- | --- |
| Row/column topology and cell rect | Accepted substrate/HTS projection and consumer layout owner. |
| Grid, cell/row selection and exterior cell-edge presentation | Qualified tabular consumer, using current immutable appearance and framework interaction facts. |
| Widget storage, mount lifetime and composite editor child | WidgetTree and existing engine-owned composite transaction. |
| Text, selection, caret, IME, history, semantic edit and constraints | Actual TextField/editor document and its existing editing policy. |
| ComboBox matching, candidate validity and commit | Existing resolver/adapter and normal ComboBox contract. |
| Trigger, arrow, step buttons and internal content hit geometry | Actual mounted widget's layout/input owner. |
| Focus/Capture, reservations and once-only routing | Framework under HAL-328 and the existing qualified session/domain. |
| Popup lifetime, placement and escaping the cell clip | Existing overlay/popup owner. |

The full editor allocation is not a declaration that every point is a caret hit. The actual text viewport, arrow/trigger zone, intrinsic affordance and grid separator hit zones remain distinct. Pointer events continue through the existing routed path once, with the real coordinate conversion. No body-level synthetic event, second key dispatch, guessed Capture, host-created selection or direct peer mutation is admitted.

Expanding an editor into the former exterior gap intentionally expands its surface. Existing consumer arbitration must still distinguish cell selection, editor activation and column-resize handles. The widget may not resize a column, and the consumer may not turn an arrow click into a fabricated caret action. Precise boundary hits are part of acceptance.

### 5.1 Popups

The ComboBox popup anchor is derived from the current full cell shell through the existing popup-placement path. It follows lawful geometry changes, including column resize, scroll, DPR and viewport changes. No stale inset anchor may remain.

The popup can extend beyond the cell only through its existing overlay/clip authority and accepted stacking order. The cell body must not paint the popup outside its own clip or bypass pinned-column/scroll clipping. Popup rows, their own borders and their internal controls are not automatically cell-integrated by this proposal.

Open, close, highlight, commit, cancel, draft preservation and resolver semantics remain unchanged. Existing scroll/virtualization popup cancellation or retention policy is preserved; this SPEC does not select a new one. A popup-visible state must not restore a duplicate standalone shell around the embedded anchor.

## 6. Selected integration mechanism — §13 binding disposition

The selected mechanism is the internal strongly typed `CellEditorPresentation::{Standalone, CellIntegrated}`, selected by a qualified cell mount and consumed by the existing widget presentation owner. It is not a new public customization API. §13 fixes delivery through the existing CMS/factory/closed-composite transaction without changing public `CellEditorFactory` signatures.

The selected mechanism has the following constraints:

1. The consumer selects the mode only for a current qualified cell embedding. The selection travels with existing owner-qualified role/factory/session data, not an environmental global, renderer registry, parent-tree search or ad hoc ancestor downcast.
2. Presentation mode does not become a second stored cell rect. Fresh geometry still comes exclusively through the current accepted cell/layout projection. The host, row/column identity domain and concrete mounted editor must match before a presentation update applies.
3. The existing mount adapter and engine transaction deliver the value before the first paint that claims the integrated surface. Layout consumes readiness and geometry; Paint consumes immutable current presentation. Neither installs a child, synchronizes a document nor publishes canonical state.
4. For a compound host, the existing sealed composite association conveys only the necessary presentation to its actual tree-owned child. ComboBox/SpinBox must not store or directly borrow a TextField widget, duplicate its text paint, invoke an extra child Update or defer a result to a later frame to simulate atomicity.
5. The cell edge and child shell disposition settle coherently. No intermediate frame may have both frames or neither required focus cue. PropertyEditor's current post-exit/pre-unmount interval must remain covered until the actual editor mount/session transition is complete.
6. Current framework state needed for edge presentation must be projected through an owner-qualified immutable preparation path. Paint may not discover live mutable widgets or invent a second focus/capture store. If the existing carrier cannot express the necessary fact, Primary must select a narrow internal extension before implementation; this RFC does not create a generic event or state service.
7. Standalone remains the ordinary default for unqualified embeddings. A lawful change of presentation on a resident qualified cell uses its existing update/dirty path; it is not implemented by replacing the editor or switching widget identity.

Concrete implementation must follow §13's private adapter, current CMS membership and immutable edge preparation. A universal public styling API or extensible user-supplied cell-widget factory is not admitted.

### 6.1 Carrier at the existing mount boundary

One shared **presentation-only embedding value** is selected by the qualified consumer and delivered through the existing CMS/factory/role transaction. Keep geometry on its existing projection and keep mode out of `PropertyValue`/CellSource data semantics. Delivery qualifies the current domain/cell, mount and session; it is not a retained widget reference, executable callback, replacement factory or new host.

The concrete current seams and missing pieces are:

1. **Construction:** retain every existing public `CellEditorFactory` signature and supported custom implementation. A crate-private family adapter at the existing construction/role path applies presentation only to actual supported widget families. Unsupported custom widgets are reported as unsupported; they are not replaced, mutated by guessed type or advertised as integrated.
2. **Resident roles:** ListView's `CompositeChoiceRole`/manifest already carries exact role identity and geometry to phase-qualified work (`cell_mechanics/mod.rs:72–161`). Presentation follows that same publication/sync route. It introduces no second rect or Layout installation. Extend current CMS resident presentation sync narrowly where needed, with equality causing no work; do not add a public generic sync/factory method or remount to apply presentation.
3. **Compound editors:** `src/widgets/composite_edit_host.rs:29–85,103–145,169–205` already holds seed/presentation revision, validated child association, immutable applied snapshots and closed family dispatch. `src/core/composite_text_edit.rs:124–129,630–657` qualifies delegation against the actual relation and framework snapshot. These are the existing owner seams for any necessary presentation propagation; they are not a universal style API. Reuse the real sealed relation and propagate only facts actually required by the child. An already embedded text-only child must not acquire a second shell or text owner. `TextField::set_embedded` (`text_field.rs:3140–3145`) has existing compound-owner meaning; it is not a generic cell-mode setter and must not be repurposed by arbitrary consumers.

No new public enum, trait method or family customization API is admitted. The common capability must not be encoded as three unrelated ListView/PropertyEditor/TreeView styling conventions.

### 6.2 One cell edge requires current owner-qualified state

There are two kinds of existing inputs, neither sufficient alone:

- A consumer already has its own cell/row selection, slots, separators and edit-session/mount identity. These do not prove the editor's actual keyboard Capture, hover, availability or validation.
- `FrameworkAppearanceSnapshot` and `for_owner` (`src/core/appearance.rs:262–273,330–345`) provide immutable framework Focus/Capture facts for an exact owner. `PaintContext::appearance_facts` (`src/core/widget/mod.rs:1032–1043`) exposes the **current painting owner's** facts, not arbitrary child facts. Compound delegation preserves the distinction between host facts and the actual child holding editing authority. Widget semantic/availability/hover appearance still comes from that widget's current published owner result, not guessed framework facts.

The selected owner is the established engine/WidgetTree appearance/current CMS member handoff before consuming Paint. It prepares narrow immutable `CellEdgePresentation` from the consumer's current cell/mount/session and the existing framework appearance snapshot, with widget-owned semantic facts. The consumer resolves/paints the edge and the widget consumes the matching shell disposition. This is not a generic child fact bus or peer lookup from Paint. Keep actual child Capture distinct from host/cell cursor state, include the post-exit/pre-unmount interval, and settle edge and shell in the same lawful frame. Unavailable or stale qualification must not be redirected to another cell or filled by fabricated focus. §13 fixes state precedence and identity requirements.

### 6.3 Readiness is per actual mount and family

The first implementation disposition must enumerate current supported mounts, not label a whole substrate “implemented.” ListView's actual Choice/Bool roles, PropertyEditor's actual shared-factory sessions and TreeView's actual sessions have different readiness and existing cue paths (§3.2). Qualification must cover each affected family and resident refresh route. Non-hosting raw substrate paths remain unchanged until their own hosting authority exists; no topology amendment is needed or proposed to make the presentation contract reusable.

## 7. Identity, lifecycle and retained work

The semantic cell identity remains the existing domain-qualified `(RowId, ColumnId)`, with concrete host/mount/session qualification for delivery. Row ordinal, visible position and a reused numeric ID from another domain are not identity. Mode/state delivery for a retired or replaced mount must be rejected rather than redirected.

HAL-189 residency and interaction pins remain intact. Column resize, reorder, scroll, value changes, geometry changes or mode/state changes do not alone destroy a resident editor. Legitimate bounded offscreen eviction remains governed by HAL-189; active editing, Focus/Capture, IME and popup interaction pins must not be bypassed.

Use the existing closed dirty-flag vocabulary. A real allocation change is geometry work; an edge or shell appearance delta is presentation work; equal inputs cause no new work. Do not add a parallel dirty flag, invalidate unrelated siblings, remount controls, rebuild a Screen or manufacture a new scene to update chrome. A settled unchanged cell must reuse its clean retained fragments under HAL-354.

Entering/leaving editing, cancel/commit, hiding/removing a row/column and changing provider domain retain their exact existing consumer policies and lawful phases. This proposal neither unifies differing consumer blur policies nor resolves unrelated historical wording by reinterpretation. It changes no value, text, history entry or callback. All related child/session updates remain ordered by the existing composite engine transaction.

## 8. Authority reconciliation and explicit amendment gates

This is the **new SPEC required by the closed embedding scopes**. The matrix below preserves the original scope reconciliation; §13 records JC's adoption and the selected narrow presentation exceptions. It does not amend the source of those active documents or supersede unrelated behavior.

| Existing authority | Reconciliation proposed for adoption; exact gate |
| --- | --- |
| HAL-002 / HAL-002A | Preserve one owner, phase separation, explicit state and real input authority. No constitutional exception is proposed. |
| HAL-173 §§3,7,12, Implemented/Closed | Retain cell rect/content projection identity and body-pass clipping. This new SPEC must be explicitly adopted for the new embedding presentation. It must not change `content_rect == cell.rect`, add geometry authority or permit inline cross-cell paint. Popups remain under their separate existing overlay authority. |
| HAL-174 §§2,8,13, Implemented/Closed | Preserve single-cell edit lifecycle and geometry from the cell content projection. Adoption must explicitly admit this presentation/embedding extension without changing lifecycle or commit/cancel semantics. Any actual lifecycle departure needs a separately enumerated amendment. |
| HAL-192 §§2,3,6, Binding ListView closed composite | The closed internal organs and composition remain intact. Current source uses CMS/SharedHts (§3.2), whereas the historical document illustrates direct TabularSubstrate consumption; this draft does not silently rewrite that active document or select a topology migration. The proposed presentation feature must preserve the current qualified closed owner path. A generic injection point, externalized organ, authority wrapper or topology change would require an explicit architectural disposition; none is proposed here. |
| HAL-260A §9.1, §14.3, §16 and §17.5, Binding | Preserve one cell focus indicator and uniform cell-kind dispatch, not hardcoded column paint. Adoption must reconcile the current child-owned cue suppression with consumer-owned integrated edges, including the transition gap. The new narrow widget-presentation extension must be explicitly distinguished from HAL-260A §16's original “consumed only” widget scope. Sealed TabularSubstrate and HTS remain consumed, never modified. |
| HAL-259 / HAL-337 and current SharedHts source | Both inspected document headers say **Draft (Binding upon adoption)**. Their source-correlated engine/handle ownership and geometry-versus-consumer separation are described in §3.2, without promoting either document by this RFC. Preserve the current single geometry/topology owner, frame-ephemeral access and existing lawful mutation paths; add no presentation, widget registry, factory, observers or duplicate topology to the substrate. Any unresolved adoption status is for Primary to verify, not infer from implementation. |
| HAL-201 and HAL-101 §§2.4–2.5 | Add only a reviewed presentation contract; retain ComboBox resolver/commit behavior and the actual WidgetTree-owned TextField child. No host-held editor, copied text or duplicated lifecycle. |
| HAL-189 | Keep identity domains, bounded residency, interaction pins and the closed dirty flags. No new invalidation or remount semantics are adopted implicitly. |
| HAL-024, HAL-328, HAL-354 and accepted paint ordering | Keep one projection/clip owner, real Focus/Capture and once-only input, clean-fragment reuse and strict layer ordering. A new input priority or clip/overlay model is outside this RFC. |

HAL-331 is **Draft** and scoped to PropertyEditor policy. It is not adopted universal cell-embedding authority and is not a dependency that can silently introduce mount strategies or click precedence. HAL-332 is **Rejected / Retired / Inactive / Not Binding**; its historical code citations do not authorize this design. Proposed HAL-363/364 do not supply implementation authority here.

PM-014's four outcomes (real Capture, correct text measurement, qualified pointer geometry and complete exit cleanup) remain required through the current owner. Its historical host-managed mechanism must not be reintroduced: Binding PM-023 requires the real tree-owned editor and ordered engine transaction.

If implementation cannot satisfy the proposal without modifying sealed topology, externalizing ListView organs, changing lifecycle, adding a new keyboard priority or bypassing the composite owner, the exact conflict must return to Primary. Adoption of this presentation contract is not blanket permission for those architectural overrides.

## 9. Binding mechanically testable invariants

| Invariant | Required observable predicate after adoption |
| --- | --- |
| `INV-391-SHARED-CONTRACT` | The same presentation contract is usable by qualified flat and hierarchical cell hosts; each claimed consumer/family has an actual accepted mount and delivery path. Projection-only and plain cells do not acquire widget hosting implicitly. |
| `INV-391-QUALIFIED-MODE` | Every integrated surface traces to an explicit current consumer-qualified embedding and concrete mount; no mode is inferred from ancestry. |
| `INV-391-CELL-RECT` | Valid whole-surface editor allocation equals the current owner-published cell rect; exterior inset is zero on all sides. |
| `INV-391-SINGLE-EDGE` | Grid/separator and state presentation yield one visible cell-edge frame; no nested rounded shell, inner border or duplicate focus ring. |
| `INV-391-STATE-TRUTH` | Current actual Focus/Capture/availability/hover facts produce visible appropriate feedback through one edge; cell cursor is not fabricated Capture. |
| `INV-391-CONTENT-OWNER` | Actual editor owns source text/caret/selection/IME/history and semantic fills; host/cell paints no duplicate text. |
| `INV-391-HIT-GEOMETRY` | Text, arrow/step/intrinsic affordance and separator hit zones derive from the same current allocation used for paint; routed input is delivered once. |
| `INV-391-POPUP` | Existing overlay owns popup escape/stacking and the anchor follows the current full cell surface; no body clip bypass. |
| `INV-391-IDENTITY` | Presentation and geometry changes preserve resident identities and active session pins; stale domain/mount updates cannot apply elsewhere. |
| `INV-391-PHASE` | Qualified presentation and edge ownership settle before paint through existing lawful phases; no Layout installation, paint publication or later-frame repair. |
| `INV-391-RETAINED` | Equal dependencies preserve clean local/composed output; a cell delta does not repaint an unrelated sibling. |
| `INV-391-STANDALONE` | Unqualified/standalone controls retain their original geometry, chrome and behavior. |
| `INV-391-TARGET-PARITY` | Native and WASM use the same mode, owner geometry and interaction contract; font/backend differences are measured separately. |

## 10. Source-owner implementation and test plan

This is a plan for a later authorized implementation, not permission to edit or execute it now. Requalify current files and fixtures before selecting exact test commands. Extend nearest existing tests; do not invent a second widget/paint harness or an additional test framework.

### 10.1 Existing owner surfaces

| Boundary | Existing source/test surface to extend or qualify |
| --- | --- |
| ListView allocation, edge and role publication | `src/widgets/list_view/body.rs` and its inline `tests`; canonical `examples/showcase/tab_list_view.rs`. |
| Role readiness, mounted editor geometry and lifetime | `src/engines/cell_mechanics/mod.rs`, including its existing `try_enter_edit_tests` and `drag_reorder_editor_parking_tests`. |
| PropertyEditor cell frame and lifecycle | `src/widgets/property_editor/body.rs`, its `text_z_order_tests`, `src/tests/property_editor_shared_hts.rs` and `src/tests/property_editor_intent_deferral.rs`. |
| TreeView hosted editor and row/cell cue reconciliation | `src/widgets/tree_view/body.rs`, `tree_view.rs` and their existing routed/session fixtures; qualify actual editor families and preserve row/disclosure behavior. |
| Shared construction, resident refresh and compound propagation | `src/engines/cell_source/mod.rs`, `src/widgets/cell_editor_factory.rs`, current CMS role work and `src/widgets/composite_edit_host.rs` / `src/core/composite_text_edit.rs`; no new general factory or test harness. |
| Immutable editor-to-edge preparation | Existing `FrameworkAppearanceSnapshot`/per-owner facts and accepted mount/session validation; select the precise narrow preparation seam in §6.2 before adding a test for it. |
| ComboBox and actual child behavior | `src/widgets/combo_box.rs`, `src/tests/combo_box_input.rs` and `tests/composite_textfield_public.rs`. |
| Input/numeric shell and family qualification | Existing inline TextField/affected numeric-editor owner tests and existing public composite fixture; identify all shared-painter callers before changing any signature. |
| Scroll/resize/retained paint | `src/tests/interaction_fuzz/list_view_resize.rs` and existing real App/retained fixtures for the affected consumer. |

These locations identify owners, not a claim that each currently contains every required assertion. Family qualification must inspect the actual mounted owner and its retained child/composed output rather than assume all text belongs to the host's local fragment.

The readiness tests must distinguish real ListView roles, PropertyEditor and TreeView sessions, and shared factory families from projection-only TabularSubstrate/legacy List and unrelated grid layouts. Do not introduce widgets into plain/read-only cells to manufacture a second positive consumer. Equal presentation inputs must preserve existing resident identity; source-qualified family updates must reach the current sealed child only where necessary.

### 10.2 Geometry and owner proof

Prove the exact cell rect/child allocation relationship, theme content padding and actual snapped edges at 1x, 2x and a supported fractional scale. Cover a narrow positive cell, zero/unavailable extent, invalid extent refusal and partial horizontal/vertical visibility. No invented one-pixel allocation may stand in for a zero extent.

Verify column resize, pinned and scrollable columns, horizontal/vertical scroll, row reorder, provider-domain change, hide/remove and normal virtualization. Track stable RowId/ColumnId, actual widget/mount and sealed child relation, current role geometry and session. Preserve retained identities on ordinary changes and reject stale/retired deliveries. Include equal-input/no-op and unrelated-sibling Arc/output reuse assertions.

Exercise editor entry, active edit, commit/cancel and post-exit/pre-unmount frames. At each observable frame the correct cell or child presentation owns exactly one required edge; no duplicated or missing cue is repaired later.

### 10.3 Real input and semantic preservation

Use real mounted widgets and the existing routed input/engine transaction as positive proof. Test both existing UI interaction policies where applicable. Direct private writes to selection/Capture or a fake cell editor are not substitutes.

Cover first entry, text/caret hit at both edges, text selection, Unicode content, IME composition, Undo/Redo, read-only/disabled/password policy, focus/capture enter/leave and unavailable cells. Preserve current accepted text/motion contracts and do not add script/length eligibility restrictions for this chrome work.

For ComboBox test arrow versus text hits, actual open/close, navigation, highlight, verbatim commit/cancel, draft preservation and all existing semantic fill states. Test popup placement against the new full shell, including a clipped/scrolled cell and column resize. For SpinBox and intrinsic controls, test their genuine affordance zones and exactly-once value/callback behavior. Cell/row selection and column-resize handles must still work without stealing a child action or double delivery.

### 10.4 Pixel acceptance

Use original GPU output of the actual Showcase List View Choice cells, not HTML/CSS controls, screenshot widgets or a reconstructed scene. Capture full source-bound images and lossless 1:1 edge/content crops at measured DPR. Keep the original 120-unit Choice column, actual theme/density, real text and font provenance.

At minimum cover normal, hover, focused, captured, disabled, selected/unselected cell/row, validation where supported, and popup open/closed states. Inspect grid intersections and each cell edge for gaps, rounded cutouts, doubled strokes, dark seams and wrong overlap; inspect actual text baseline, content padding and arrow/step hit geometry. Pixel expectations must come from the accepted concrete appearance values, not a generic palette color or widened tolerance chosen to hide a mismatch.

Take equivalent standalone control captures to prove no incidental restyling. Include the relevant light/dark and density cases supported by the original controls. Native/WASM visual comparison must record source/artifact identity, theme, density, UI scale, CSS/physical viewport, DPR and font backend. Font-raster differences do not establish a cell-edge geometry difference, and equal compile output does not establish pixel parity.

The existing native Showcase and durable WASM Showcase must first show the genuine ListView Choice result and exercise every other consumer/family claimed by the implementation disposition, including separately qualified PropertyEditor and TreeView paths where admitted. Their exact current hosting/readiness differences must be recorded, not hidden behind ListView-only success. Verify cell input after real resize and scroll. Preserve unrelated browser Device errors as separate RED boundaries; neither old accepted browser pixels nor a new successful build certify the new cell presentation.

### 10.5 Required evidence and limits

The later implementation receipt must distinguish source review, child test exits, real routed semantics, GPU readback and actual native/browser pixels. Preserve all original assertions and failed receipts. A missing browser/IME/popup observation remains explicitly unobserved; no focused test result implies full framework acceptance.

## 11. Prohibitions

- No global theme override, implicit parent discovery, environment flag, hidden per-cell renderer state or general styling service to imitate the mode.
- No mutation of sealed TabularSubstrate/HTS or external widget injection through ListView.
- No host-held TextField, copied text, duplicate child lifecycle, fabricated focus/capture or new input replay path.
- No changing row height, column width, fonts, text semantics, callback/commit order or control policy to make a screenshot fit.
- No independent inset/rounded frame, double focus ring, blank focused editor or body-painted popup escape.
- No guessed rect, stale ordinal identity, remount-on-resize, unrelated subtree invalidation or later-frame synchronization repair.
- No native/WASM fork of the visual contract, CSS-rendered control or claimed pixel acceptance from compilation alone.

## 12. Adoption and completion gates

The source-qualified draft was independently reviewed and JC adopted the framework-wide contract on 2026-11-16. §13 records the selected mechanism. The following review/evidence obligations are retained; adoption is completed, while implementation, semantic/native/WASM proof and JC visual acceptance must not be inferred from it.

Implementation and independent acceptance must demonstrate conformance to these dispositions:

1. **Contract adoption — completed:** JC adopted the Halcyon-wide qualified cell-hosting presentation contract. ListView resident Choice/Bool and PropertyEditor/TreeView session paths are the selected current consumer paths; raw projection-only cells gain no hosting capability. Actual family readiness and implementation evidence must be reported precisely.
2. **Closed-scope reconciliation — selected:** §13 explicitly admits the narrow presentation exception under HAL-173/174, preserves HAL-192's current closed owner path, and replaces HAL-260A's child-cue suppression only where a qualified cell edge replaces it. HAL-259/337 Draft status and all unrelated authority remain unchanged.
3. **Mechanism — selected:** internal CellEditorPresentation, the existing CMS/factory/role transaction with unchanged public factory signatures, private actual-family adapter, closed compound propagation only where necessary, and narrow engine/WidgetTree immutable CellEdgePresentation preparation. No broader fact service or alternative architecture is authorized.
4. **Appearance — selected:** §13 fixes Disabled → actual Captured → actual Focused → actual hover/selection → neutral edge precedence with existing theme sources; validation semantic fills survive and popup-open is not Capture. Conformance still needs source/pixel proof.
5. **Implementation boundary — selected:** existing appearance/WidgetTree preparation, CMS/factory/closed composite, qualified widget shells and the three consumer geometry/edge owners with their nearest tests. A real contradiction returns to Primary; it does not authorize topology, public API, input or lifecycle changes.
6. **Independent acceptance — open until evidenced:** source-bound native GPU and WASM geometry/input/output evidence, standalone and retained preservation, honest physical-browser limits and JC's visual acceptance remain required. A build or adoption entry alone closes none of these gates.

This file is **ADOPTED / ACTIVE / BINDING** for the narrow presentation extension in §13. It authorizes no unrelated topology, editor semantic, input-priority, shader, theme or public API change. Runtime and visual completion require actual evidence under §10.

### 12.1 Coherent implementation objective

One coherent implementation objective delivers the shared presentation path at the **existing qualified cell-host mount boundary**, applies it to admitted actual families/consumers, and proves the original Showcase ListView Choice result first. It must prove reuse through PropertyEditor and qualified TreeView sessions, preserve Standalone and sealed compound ownership, and record per-consumer readiness rather than promise new raw-substrate hosting. Full allocation, widget shell disposition and one cell-edge state settle in one frame; removing ListView padding alone is insufficient. No generic factory, topology change, per-consumer visual fork or separate runtime flag is admitted.

## 13. JC adoption and Primary-selected mechanism — 2026-11-16

This section is the direct JC-authorized normative disposition of the earlier draft alternatives. It is the narrow explicit presentation extension required by HAL-173/174's closed embedding scopes. Cell geometry/projection and edit lifecycle remain unchanged. HAL-192's CURRENT closed ListView/CMS/SharedHts implementation remains closed; no topology migration or external widget injection is authorized. For qualified integrated embeddings only, this section replaces HAL-260A's nested child-chrome/cell-cue suppression with a single consumer-owned exterior cell edge. All unrelated clauses and existing editor semantics survive. HAL-259/337 Draft headers are not promoted.

1. **Mode and delivery.** One crate-private `CellEditorPresentation` has `Standalone` (ordinary default) and `CellIntegrated`. Qualified current ListView Choice/Bool roles and PropertyEditor/TreeView sessions select it through existing CMS/factory/role transactions. Preserve public factory signatures and custom implementations. The private adapter recognizes actual supported families; unsupported returned widgets remain explicitly unsupported. Intrinsic marks retain natural geometry. Numeric factory values remain TextField; SpinBox is admitted only through a genuine qualified existing composite mount.
2. **Allocation and shell.** Whole-surface allocation equals the current full cell rect, including ListView's exact column slot and row height; remove its exterior Choice padding without changing row/column/density/font. Semantic fill is square and edge-to-edge. Suppress only the standalone border, secondary edge, rounded outer frame/shadow/ring, including ComboBox's joined-popup shell. Preserve internal content padding, value baseline, arrow/step hit regions and semantic fills. Existing popup overlay/clip/anchor authority remains; no cell-body escape or new geometry/scroll writer.
3. **Prepared edge evidence.** The established engine/WidgetTree appearance/current CMS-member handoff prepares immutable `CellEdgePresentation` before Paint. Validate the actual interaction domain, `(RowId, ColumnId)`, concrete TreeWidgetId/mount and session where present. Use the existing immutable `FrameworkAppearanceSnapshot`, actual sealed relation/delegation and current widget-owned semantic availability/hover/validation facts. Delegated host Capture is distinguished from actual child Focus/Capture. No generic fact service, peer lookup from Paint, global registry or substrate state is admitted.
4. **One edge precedence.** Disabled/unavailable cannot appear editable; then actual Captured uses `input_chrome(Captured)`, then actual Focused uses `input_chrome(Focused)`, then actual hover/cell selection and existing neutral separators. Existing validation remains visible in semantic fill. Popup-open is not Capture. Use existing theme/component values, physical one-pixel shared-edge snapping and existing paint order; no new RGB constants. Preserve PE's qualified cue through exit-to-unmount and TreeView's row selection while giving only the actual editor cell its edge. Stale/retired/cross-domain/unavailable publications cannot lend an edge elsewhere.
5. **Lifetime and propagation.** Use current CMS roles, mounted identity, session, existing dirty vocabulary and closed composite seed/revision/association. Compound hosts pass only necessary presentation to their actual tree-owned child; no child borrow, duplicated text/Update or new phase. Direct cell TextField has its own presentation role; `set_embedded` keeps its original compound meaning. Equal mode/state produces no unrelated work, replacement or remount. Preserve Focus/Capture, input once-only, IME/history, S8 paste, U1 motion and all standalone behavior.
6. **Acceptance.** Deliver the real ListView Choice pixels, shared PropertyEditor and qualified TreeView use, semantic pointer/keyboard/popup and retained identity at DPR1/2/fractional scale, zero/narrow/clip cases and standalone controls. Require current native GPU readback and focused owner tests, native check, locked/offline WASM Showcase build and matching bindgen. Record source-identical served artifacts without altering the user's browser session or :8787. Physical native/browser evidence is separate; unavailable observations remain UNOBSERVED. Metadata follows existing release policy only for delivered capability. Adoption does not certify implementation, universal hosts or JC's visual acceptance.
