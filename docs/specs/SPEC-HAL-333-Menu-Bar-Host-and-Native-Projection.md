# SPEC-HAL-333: Menu Bar Host and Native Projection

**Status:** Binding
**Revision:** 4 — supersedes Revision 3 in full (see §6)
**Layer:** Host / Platform Projection / Application Integration / Interaction Contract
**Scope:** Menu host layer — the application main menu, its macOS native projection, its framework-rendered in-window realization, and its application-level integration. Excludes context menus, dropdowns, popovers, menu surfaces, menu items, and in-menu interaction (SPEC-HAL-122, SPEC-HAL-222, SPEC-HAL-223).
**Ground Truth:** SPEC-HAL-001, SPEC-HAL-002, SPEC-HAL-018, SPEC-HAL-020 / 020A–D, SPEC-HAL-047, SPEC-HAL-103, SPEC-HAL-122, SPEC-HAL-132, SPEC-HAL-139, SPEC-HAL-158, SPEC-HAL-176, SPEC-HAL-222, SPEC-HAL-223, SPEC-HAL-269, SPEC-HAL-332

> Revision 4 restores macOS native projection for the application main menu only. Native projection is the same integration class as native text backends: a target-gated platform backend behind a normalized Halcyon contract. Halcyon remains the semantic authority.

---

## 0. Purpose

This SPEC defines the contract for the **menu host**: the layer that realizes the application main menu either as a macOS-native `NSMenu` projection or as a framework-rendered in-window bar. The selection is platform-specific and follows the native text backend pattern: one normalized model, target-gated backends, and no platform semantic authority.

This SPEC exists to prevent:

- menu bars built from generic Button primitives
- OS menu APIs entering the render graph, layout tree, paint pipeline, routing authority, focus authority, edit authority, or model authority
- a second (or third) command or shortcut registry parallel to the Action System
- host-local item state diverging from the model
- divergent menu definitions across platforms
- a menu strip occupying window space when no menu exists
- macOS users receiving a non-native main menu without a structural need
- native main-menu roles bypassing Halcyon's Action System, clipboard system, lifecycle policy, or window/domain authority

Menu content and in-menu behavior remain owned by SPEC-HAL-222 / SPEC-HAL-223 / SPEC-HAL-122. Command identity, execution, and shortcuts remain owned by the Action System (SPEC-HAL-020 / 020A–D).

---

## 1. Conceptual Definition

**MenuModel** — the single source of truth for the application menu. It owns exactly three tables:

1. **Structure** — an ordered set of top-level titles, each bound to a `MenuItem` tree (SPEC-HAL-222). Presentation data: labels, icons, separators, submenu shape.
2. **Bindings** — `MenuItemId → ActionId`. Command identity per activatable item, resolved against the Action System.
3. **Live state** — `MenuItemId → MenuItemState`. Runtime enabled / checked state.

It carries no rendering, geometry, theme, focus, or platform knowledge.

**MenuItemState** — the live state of one item:

```rust
pub struct MenuItemState {
    pub enabled: bool,
    /// None = not checkable (no check slot rendered).
    /// Some(b) = checkable, current value b.
    pub checked: Option<bool>,
}
```

State is owned by the MenuModel and *provided externally* to MenuItem — satisfying SPEC-HAL-222 INV-A2 literally. SPEC-HAL-222 and the `MenuItem` data shape are untouched by this SPEC.

**MenuHost** — the active realization of a `MenuModel` on the current platform. Exactly one exists per application process. It holds no authoritative menu state. It may present, mirror, and report; it may not decide, mutate, route, validate, focus, edit, or own state.

**MacOsMenuHost** — a platform backend under `src/platform/`, target-gated to macOS. It projects the `MenuModel` into `NSApp.mainMenu`, occupies no in-window space, and buffers `MenuItemId` activations back to AppRoot.

**InWindowMenuHost** — the framework-rendered fallback host. It projects the `MenuModel` into the in-window `MenuBar` widget through the shared **MenuBarMailbox** (the SPEC-HAL-269 ToastManager shared-state pattern). The WidgetTree remains the sole owner of the widget instance.

**MenuBar** — the in-window bar widget (Class D). Mounted only when the active host kind is in-window; claims the top strip via the root container's strip allocation (Class G).

**MenuHostKind** — the output of host resolution. Resolution is the menu equivalent of `FontManagerTextEngine::new_with_system_ui()` / native text backend selection: target-aware, deterministic, and separate from construction.

---

## 2. Sovereignty

### INV-S1: Surface Sovereignty
The wgpu surface is sovereign. No OS menu API renders into, queries, or mutates the render graph, the layout tree, or the paint pipeline.

### INV-S2: Native Menu Projection Is Outward Only
On macOS, the native main menu is an outward projection of Halcyon state. AppKit owns pixels and menu tracking for the system menu bar; Halcyon owns content, state, command identity, dispatch, lifecycle policy, text editing, clipboard semantics, focus, routing, and window/domain interpretation.

### INV-S3: Sovereign Fallback Is Total
The in-window host renders entirely within the framework, with zero OS menu involvement, and is available on every target. Windows, Linux, WASM, and unsupported platforms use this host unless a later SPEC explicitly adds a native backend.

---

## Invariant Class A — Model & Authority

### INV-A1: Model Purity
MenuModel is built only on `MenuItem` (SPEC-HAL-222), `MenuItemId`, `ActionId` bindings, and `MenuItemState`. It holds no pixels, geometry, theme, focus, or platform state.

### INV-A2: Single Source of Truth
The MenuModel owns all live item state — enabled, checked, label. The host holds no authoritative item state and does not define, augment, or reorder menu content. Structural change occurs by replacing the model; state change occurs only through the MenuModel mutation API.

### INV-A3: Content Authority Unchanged
In-window menu-surface layout, hover, keyboard navigation, submenu expansion, and dismissal remain owned by MenuSurface (SPEC-HAL-223) under the interaction contract (SPEC-HAL-122). Native macOS menu interaction is OS-owned presentation behavior only; it is not Halcyon semantic authority. This SPEC governs only the main-menu host, host selection, projection, and application integration.

### INV-A4: Action Binding
Every activatable leaf item binds `MenuItemId → ActionId`. Separators and submenu-carrier items bind nothing. There is no menu-local command registry and no menu-local keymap; command identity, execution, and shortcuts belong to the Action System (SPEC-HAL-020 / 020A–D).

### INV-A5: Mutation Discipline
Application code mutates the model only — `set_enabled`, `set_checked`, `set_label`. A mutation applies to the model immediately and queues the item id as dirty. AppRoot drains dirty ids each frame and mirrors them to the host via `sync_item`. `sync_item` is a projection refresh of an already-applied model change — never an independent edit channel. Direct host or mailbox mutation by application code is forbidden. Model and projection are structurally unable to diverge: `install` re-realizes the model; it reverts nothing because nothing host-local exists to revert.

### INV-A6: Shortcut Data Has One Authority
Structured shortcuts live on `Action` (SPEC-HAL-020). `MenuModel::bind` derives the display hint from the bound Action's structured `Shortcut` via `Shortcut::to_display_string`, writes it into the item's hint slot, and replaces any authored hint unconditionally; an unregistered action clears the slot.

Native macOS key equivalents are sourced at host install time from `ActionRegistry` through `MenuModel` bindings. `MenuModel` stores no `Shortcut`; hosts store only the OS projection required to mirror the installed menu. This preserves one shortcut authority, exact display/key-equivalent agreement, and no menu-local key dispatch table. If SPEC-HAL-020 ever gains runtime shortcut mutation, this clause must be amended in the same change.

---

## Invariant Class B — Host Resolution

### INV-B1: Exactly One Host
Resolution yields exactly one `MenuHostKind`; construction yields exactly one active `MenuHost`. Resolution never fails. Absence of a native capability yields `InWindow`.

### INV-B2: Backend Pattern Matches Native Text
Host selection follows the native text backend pattern (SPEC-HAL-047 / SPEC-HAL-103): platform-specific backends live behind one normalized Halcyon contract. Applications provide one `MenuModel`. The active backend realizes it without leaking platform details into widgets, layout, rendering, or application code.

### INV-B3: Per-Platform Host Policy

| Platform | Host |
|---|---|
| macOS | Native main menu (`NSMenu`) |
| Windows | In-window menu bar |
| Linux | In-window menu bar |
| WASM / other | In-window menu bar |

Windows native `HMENU` is not a host under this SPEC. Linux native global menus are not a host under this SPEC; DBus appmenu and Wayland appmenu protocols require separate empirical SPECs before re-entry.

### INV-B4: Resolution Is Split From Construction
`resolve_menu_host_kind()` is a deterministic probe returning the kind only. It performs no host construction, requires no widget tree access, and does not mutate the OS. `App::run` constructs the active host after window creation and hands it to AppRoot.

### INV-B5: Host Kind Is Process-Stable
The host kind is resolved once at startup and remains fixed for the process lifetime. Runtime platform changes do not re-resolve the main-menu host.

---

## Invariant Class C — Native macOS Projection

### INV-C1: Zero In-Window Footprint
When the active host is native macOS, AppRoot creates no `MenuBar` node and no top menu strip exists in the widget tree. The application content receives the same root layout it would receive with no menu bar.

### INV-C2: Projection Only
`MacOsMenuHost` translates the `MenuModel` into `NSMenu` / `NSMenuItem` objects and mirrors live item state from the model on `install` and `sync_item`. It holds no authoritative content or state. Reinstalling from the model replaces the projection; it does not merge host-local edits.

### INV-C3: Thin Projection Law
The macOS host may present, mirror, and report. It may not decide, mutate, route, validate, focus, edit, or own state. No AppKit callback may directly mutate Halcyon state. AppKit callbacks buffer `MenuItemId` activations only; AppRoot drains and dispatches in the mutation-legal frame phase.

### INV-C4: Standard App Roles
Native selectors are allowed only for narrowly scoped application-system roles whose semantics are already OS-owned or lifecycle-mediated: About, Hide, Hide Others, Show All, and a Quit request that enters Halcyon's lifecycle/shutdown policy unless the implementation proves direct `NSApp.terminate` preserves that policy.

Settings, Help, Window, and all document/application commands are ordinary Halcyon actions. They project as menu items but activate through `MenuItemId -> ActionId`.

### INV-C5: Edit Menu Is Halcyon-Owned
The Edit menu must never use AppKit responder-chain editing selectors for Cut, Copy, Paste, Undo, Redo, Select All, or related text commands. Halcyon's text editing, clipboard, undo, and selection systems are sovereign inside the wgpu surface. Edit items route through the Action System exactly like any other command.

### INV-C6: Services Are Capability-Mediated
macOS Services may exist only through explicit Halcyon capabilities. The obvious first bridge is clipboard/selection mediation through Halcyon's clipboard and text-selection contracts. No service may read, write, or mutate selection, clipboard, focus, or document state except through Halcyon-owned capability APIs.

### INV-C7: Window Menu Reflects Halcyon Window/Domain State
The native Window menu may only reflect Halcyon window/domain state. It may not create an independent window registry, decide focus, or mutate window/domain state outside Halcyon's lifecycle and focus contracts. Until multi-window focus/domain projection is implemented, Window-menu entries beyond inert standard presentation are optional and must not lie.

### INV-C8: Key Equivalents Re-Enter Through Activation
Native key equivalents are installed from structured `Action::shortcut` data. When AppKit consumes a key equivalent, the native target/action buffers the corresponding `MenuItemId`; the raw key path must not also dispatch the same command. Every shortcut dispatches at most once and always through AppRoot's menu activation drain.

### INV-C9: Main Thread
`MacOsMenuHost` is main-thread-only. Install, sync, and target/action callbacks occur on the main thread. This mirrors AppKit ownership and keeps UI mutation confined to the existing frame phase.

### INV-C10: Tracking Stall Accepted
Native macOS menu tracking may enter an OS-modal run loop and stall Halcyon's frame loop while a menu is open. This is accepted platform behavior for native main-menu projection and must not be filed as a defect unless state dispatch escapes the drain contract.

---

## Invariant Class D — In-Window Menu Bar

### INV-D1: Composition Over Reinvention
The bar drives one MenuSurface (SPEC-HAL-223) per open title. It does not implement menu list layout, hover, or dismissal internally.

### INV-D2: Bar-Level Ownership Only
The bar owns: title open and close, hover-switch between titles while a menu is open, horizontal Left/Right traversal, and title chrome state (rest, hover, active-open). All vertical and in-menu behavior delegates to SPEC-HAL-122 / SPEC-HAL-223.

### INV-D3: Title Chrome
Titles are flat: no button fill at rest, no caret, no disclosure indicator. Hover paints `controls.background_hover`. **The active-open title paints the popover surface (`surfaces.level3`) so title and popup read as one connected surface** — the open state must never be a near-invisible step off the bar color.

### INV-D4: Square Corners
The bar and its title cells use zero corner radius (SPEC-HAL-332). `push_rect`, never rounded.

### INV-D5: Theme-Resolved Color
All bar colors are resolved from the active theme at paint. No baked or hardcoded color values.

### INV-D6: Empty Projection Occupies Nothing
With zero titles, the bar lays out at zero height and paints no chrome. No model ⇒ no strip: an application that never supplies a MenuModel carries no menu footprint. (The strip follows the bar's reported height — Class G.)

Mounting, strip allocation, and per-frame duties for the bar are owned by Class G. Class D applies only when the active host is in-window.

> Popup-side emphasis (panel on `surfaces.level3`, selection-grade highlight, strong border) was corrected in MenuSurfaceOverlay during this revision cycle and applies to all its consumers. Those tokens are SPEC-HAL-223 surface territory; codifying them there is a queued 223 amendment. The light-theme `surfaces.level3` value remains a SPEC-HAL-332 §9 recalibration item.

---

## Invariant Class E — Activation & Commands

### INV-E1: Uniform Dispatch Through the Action System
Every non-standard-role activation, native or in-window, resolves `MenuItemId → ActionId` through the model's binding table and executes through the **existing** action execution model (SPEC-HAL-020): the registered handler is extracted and invoked with `&mut AppContext` in AppRoot's mutation-legal phase. No new dispatch machinery exists.

**Menu bindings are app-scope commands.** Engine-ranged ActionIds (SPEC-HAL-176) are keyboard-router territory; a menu binding one is an authoring error — rejected at dispatch with a diagnostic, never half-dispatched.

### INV-E2: Drain Re-Entry
Activations are buffered by the active host. AppRoot drains `drain_activations()` once per frame on the UI loop, in the same per-frame phase as existing action flushing, and dispatches per INV-E1. There is no push path.

### INV-E3: Single Shortcut Authority
Structured shortcuts live on `Action` (SPEC-HAL-020). The `BindingRegistry` remains the only framework dispatch table. Native key equivalents are projection data only; in-window hints are projection data only. No host owns a shortcut dispatch table.

---

## Invariant Class F — Dependencies & Module Boundaries

### INV-F1: No GTK, No General UI Toolkit
No menu host links GTK or any C/general-purpose UI toolkit. Native integration uses direct platform bindings only.

### INV-F2: Target-Gated Native Dependencies
macOS native menu dependencies must be declared under `[target.'cfg(target_os = "macos")'.dependencies]`, matching the native font backend pattern. The intended family is `objc2`, `objc2-foundation`, and `objc2-app-kit`, version-aligned with the tree already compatible with winit. No macOS menu dependency may be visible to Windows, Linux, WASM, or unsupported targets.

### INV-F3: Module Placement
- `src/model/menu_model.rs` — `MenuModel`, `MenuItemState`.
- `src/platform/menu_host.rs` — `MenuHost` trait, `MenuHostKind`, and `resolve_menu_host_kind()`.
- `src/platform/native_menu_macos.rs` — `MacOsMenuHost`, `cfg(target_os = "macos")`.
- `src/widgets/menu_bar.rs` — the `MenuBar` widget, `MenuBarMailbox`, and `InWindowMenuHost`. The in-window host lives in the widget layer because its data contract is the widget's mailbox.

Native hosts are platform backends, parallel to native text/font backends. Widgets never import AppKit types.

---

## Invariant Class G — Application Integration

### INV-G1: Host Ownership
AppRoot owns the active host: `menu_host: Option<Box<dyn MenuHost>>`, sibling of `toast_widget_node`, with `menu_bar_node: Option<NodeId>` alongside for the in-window host only. `App::run` resolves the host kind, constructs the host after window creation, and hands it to AppRoot via `set_menu_host`. The application supplies the MenuModel through `AppContext::set_menu_model`; AppRoot installs it on the host at startup and on every model replacement (install-once flag cleared by `set_menu_model`).

### INV-G2: In-Window Mount and Strip Mechanics
When the active host is in-window, `App::run` mounts the `MenuBar` widget as a child of the root `ScreenHost` and retains its `NodeId` — the SPEC-HAL-269 toast ownership pattern. Data flows host ↔ widget exclusively through the shared `MenuBarMailbox`; **no tree access crosses the host bridge**. `ScreenHost` is told the bar's `NodeId` (`set_menu_bar_child`) and allocates the top strip: the bar is laid out first at full width with loose height; every other child takes the remainder below the strip. With no registered bar — or a zero-height bar (INV-D6) — layout is identical to the pre-menu behavior.

Consequences, recorded: modal subtrees and the toast overlay lay out below the strip; the bar stays visible above modal scrims; modal-first input routing (SPEC-HAL-137) keeps the bar inert while a modal is mounted.

### INV-G3: Native Means No Bar Node
When the active host is native macOS, AppRoot creates no bar node and does not register a menu-bar strip child with `ScreenHost`. This is the sole owner of INV-C1's footprint guarantee.

### INV-G4: Per-Frame Duties
Each frame, in AppRoot's existing flush phase (`flush_pending_actions_all`, after `sync_toast_widget`) — never during layout or paint:

1. install the model on the host if newly supplied or replaced (INV-G1);
2. drain MenuModel dirty ids → `host.sync_item` (INV-A5);
3. drain host activations → dispatch per Class E;
4. if and only if `menu_bar_node` exists, apply the mailbox to the bar widget (`apply_mailbox`) and, when it reports a change, mark the bar node layout-dirty — **dirty authority lives in AppRoot, never in the widget** (SPEC-HAL-158).

SPEC-HAL-002's phase separation and deferred-mutation semantics are preserved: all dispatch occurs in the mutation-legal phase.

---

## 3. Interface

### MenuModel

Interface excerpts below show public API only; all fields are private unless shown.

```rust
pub struct MenuItemState {
    pub enabled: bool,
    pub checked: Option<bool>,
}

pub struct MenuModel;

impl MenuModel {
    // Build (application setup):
    pub fn add_title(&mut self, title: &str, items: Vec<MenuItem>);
    /// Binds an activatable leaf to its Action; derives the display hint
    /// from the Action's structured Shortcut (INV-A6). No Shortcut is stored.
    pub fn bind(&mut self, id: MenuItemId, action: ActionId, registry: &ActionRegistry);
    /// Declares an item checkable; initial value false.
    pub fn set_checkable(&mut self, id: MenuItemId);

    // Mutation (application runtime — INV-A5):
    pub fn set_enabled(&mut self, id: MenuItemId, enabled: bool);
    pub fn set_checked(&mut self, id: MenuItemId, checked: bool);
    pub fn set_label(&mut self, id: MenuItemId, label: String);

    // Drain & queries (AppRoot / host):
    pub fn drain_dirty(&mut self) -> Vec<MenuItemId>;
    pub fn item_state(&self, id: MenuItemId) -> Option<MenuItemState>;
    pub fn item_label(&self, id: MenuItemId) -> Option<&str>;
    pub fn bound_action(&self, id: MenuItemId) -> Option<ActionId>;
}
```

### MenuHost

```rust
pub trait MenuHost {
    /// Realize the full model: structure, bindings, live state, and
    /// host-specific projection data such as native key equivalents.
    fn install(&mut self, model: &MenuModel, registry: &ActionRegistry);

    /// Mirror one item's current model state into the projection. The model
    /// is the only source (INV-A5). Implementations mirror enabled + label;
    /// checked state is mirrored where the active host supports a check slot.
    fn sync_item(&mut self, model: &MenuModel, id: MenuItemId);

    /// Drain activations buffered since the last call (INV-E2).
    fn drain_activations(&mut self) -> Vec<MenuItemId>;

    /// True only for the in-window host.
    fn occupies_in_window_bar(&self) -> bool;
}
```

### Resolution

```rust
pub enum MenuHostKind {
    MacOsNative,
    InWindow,
}

pub fn resolve_menu_host_kind() -> MenuHostKind {
    #[cfg(target_os = "macos")]
    {
        return MenuHostKind::MacOsNative;
    }

    #[cfg(not(target_os = "macos"))]
    {
        return MenuHostKind::InWindow;
    }
}
```

### InWindowMenuHost

```rust
pub struct InWindowMenuHost;

impl InWindowMenuHost {
    pub fn new(mailbox: Rc<RefCell<MenuBarMailbox>>) -> Self;
}

impl MenuHost for InWindowMenuHost {
    fn install(&mut self, model: &MenuModel, registry: &ActionRegistry);
    fn sync_item(&mut self, model: &MenuModel, id: MenuItemId);
    fn drain_activations(&mut self) -> Vec<MenuItemId>;
    fn occupies_in_window_bar(&self) -> bool;
}
```

### MacOsMenuHost

```rust
#[cfg(target_os = "macos")]
pub struct MacOsMenuHost;

#[cfg(target_os = "macos")]
impl MacOsMenuHost {
    pub fn new() -> Self;
}

#[cfg(target_os = "macos")]
impl MenuHost for MacOsMenuHost {
    fn install(&mut self, model: &MenuModel, registry: &ActionRegistry);
    fn sync_item(&mut self, model: &MenuModel, id: MenuItemId);
    fn drain_activations(&mut self) -> Vec<MenuItemId>;
    fn occupies_in_window_bar(&self) -> bool;
}
```

### Application entry points

```rust
impl AppContext {
    /// Supply or replace the application menu model (INV-G1).
    pub fn set_menu_model(&mut self, model: MenuModel);
    /// Runtime mutation access (INV-A5).
    pub fn menu_model_mut(&mut self) -> Option<&mut MenuModel>;
}
```

---

## 4. Policy Surface

The in-window bar declares its interaction policy. Native macOS is exempt; AppKit owns menu tracking and presentation behavior, while Halcyon owns state and dispatch.

```rust
pub struct MenuBarPolicy {
    /// How a closed menu opens.
    pub open_trigger: OpenTrigger,
    /// With one menu open, hovering another title switches to it.
    pub hover_switch_while_open: bool,
    /// Left/Right move between titles.
    pub horizontal_traversal: bool,
    /// Clicking the open title closes it.
    pub close_on_reactivate_title: bool,
    /// Conditions that dismiss the open menu.
    pub dismiss: DismissPolicy,
}

pub enum OpenTrigger {
    ClickToOpen,
    HoverToOpen,
}

pub struct DismissPolicy {
    pub on_click_outside: bool,
    pub on_escape: bool,
    pub on_activation: bool,
}
```

Canonical defaults: `open_trigger = ClickToOpen`, `hover_switch_while_open = true`, `horizontal_traversal = true`, `close_on_reactivate_title = true`, and every `DismissPolicy` field `true`.

Naming note, recorded: `widgets::menu_bar::DismissPolicy` shares its name with the SPEC-HAL-127 type in `core/overlay_root.rs`; it stays module-scoped (not re-exported). A rename remains available as a one-line errata if ruled.

---

## 5. Out of Scope

- Windows native `HMENU` menu bars.
- Linux dbusmenu / `_KDE_NET_WM_APPMENU_*` / Wayland `org_kde_kwin_appmenu`.
- Tray, dock, and status-item menus.
- Dynamic menu morphing beyond MenuModel replacement and state sync.
- Checked-state popup visuals — the named SPEC-HAL-223 gap; `MenuItemState.checked` and `set_checkable` exist in the model and await the 223 amendment.
- Submenu expansion in in-window bar popups — pending MenuSurfaceOverlay verification (SPEC-HAL-223 territory). Native macOS submenus are projection of the existing `MenuItem` tree and do not validate in-window submenu behavior.
- Alt-mnemonic bar activation — named candidate §4 extension, not yet ruled.
- macOS native behavior beyond this SPEC's named main-menu roles and projection rules.
- Any change to SPEC-HAL-020: `Action` remains a stateless definition; the registry remains stateless.
- Any change to SPEC-HAL-222: the `MenuItem` data shape is untouched.

---

## 6. Revision Record

**Revision 4 supersedes Revision 3 in full.** Ruling (JC, 2026-06-11): macOS returns to native main-menu projection; Windows/Linux/WASM keep the in-window host. The architecture follows native text backend precedent: one normalized Halcyon model, target-gated platform backends, deterministic host resolution, and strict projection-only OS authority. Changes:

- Restored Class B host resolution with `MacOsNative` and `InWindow`; Linux native probing remains out of scope and Windows native `HMENU` remains forbidden.
- Restored Class C as macOS-only native projection: `NSMenu` may present, mirror, and report, but may not decide, mutate, route, validate, focus, edit, or own state.
- Bound Edit menu commands unconditionally to Halcyon actions; AppKit responder-chain edit selectors are forbidden for Halcyon text.
- Bound Services to explicit Halcyon capabilities, starting with clipboard/selection mediation; no ambient service access is allowed.
- Bound Window menu projection to Halcyon window/domain state and forbade independent window authority.
- Reintroduced a `MenuHost` trait and host-kind resolution; `InWindowMenuHost` returns as the widget-layer backend and `MacOsMenuHost` becomes the macOS platform backend.
- Changed `MenuHost::install` to receive `ActionRegistry`, so native key equivalents derive from `Action::shortcut` without restoring `Shortcut` storage to `MenuModel`.
- Restored the native zero-footprint law: macOS native host creates no `MenuBar` node and no top strip.

**Revision 3 supersedes Revision 2 in full.** Ruling (JC, 2026-06-10): **in-house solution all the way** — the in-window Halcyon-rendered menu bar is the only menu host on every platform. Changes:

- Retired Class B (resolution) and Class C (native projection) in full: `MenuHostKind`, `resolve_menu_host_kind`, the Linux session/registrar probes, the per-platform host table, macOS/Linux projection law, threading law, and the tracking-stall record. Class letters B, C, and invariant G4 are not reused.
- Collapsed the `MenuHost` trait to a concrete bridge struct (single realization; the trait's `occupies_in_window_bar` answered a question only resolution asked). `src/platform/menu_host.rs` deleted; `zbus` removed from the dependency graph (INV-F2: the menu system introduces zero crates).
- INV-A6 rewritten: display hints derive at bind time; the per-binding `Shortcut` snapshot and `bound_shortcut` query — whose only purpose was native key equivalents — are removed. No `Shortcut` is stored anywhere in the menu system.
- INV-G2 corrected against the implementation: the host bridge addresses the widget through the shared `MenuBarMailbox` (SPEC-HAL-269 pattern), never through the tree; `ScreenHost` owns strip allocation via the registered bar child; AppRoot owns layout-dirty marking (SPEC-HAL-158). Revision 2's "addressing the widget through the tree" wording was wrong on both mechanism and authority.
- INV-E1 extended with the implemented dispatch law: menu bindings are app-scope; engine-ranged ActionIds are rejected with a diagnostic.
- Codified from validated implementation: INV-D6 (empty projection occupies nothing — no model, no strip) and the INV-D3 open-title emphasis contract (active-open title paints `surfaces.level3`, connecting visually to the popup). Popup-side token corrections live in MenuSurfaceOverlay pending the queued SPEC-HAL-223 amendment; light-theme `level3` remains a SPEC-HAL-332 §9 item.

**Revision 2 superseded Revision 1 in full.** Corrections of Type-D character (internally contradictory or unimplementable clauses), resolved before implementation: defined `MenuItemState` and fixed its ownership; bound dispatch and shortcut authority to the existing Action System; rewrote macOS projection around NSApp standard roles; replaced the nonexistent platform event channel with buffered drain; split resolution from construction; gated the Linux probe on session type; added threading law, the macOS tracking-stall record, and Class G.

## 7. Native standard Quit must return through the caller - 2026-11-14

**Status: ADOPTED / ACTIVE, scoped correction of INV-C4.**
This section governs ordinary `App::run` on macOS. It amends only the
standard-application-role clauses identified below; the remaining Revision 4
contract, HAL002/002A, ordinary command dispatch and non-macOS behavior remain
unchanged. It does not adopt Draft HAL234.

### 7.1 Verified boundary and required outcome

In the originating application, the initial screen supplied no MenuModel. The
resolved winit default menu therefore remained installed and its Quit selected
`terminate:`. After another screen installed a model, Halcyon's standard Quit
also targeted `NSApplication.terminate:`. The application's original cleanup
owner was retained outside `App::run` and explicitly invoked after that call
returned. Winit's termination notification/LoopExiting is not that return or a
cleanup acknowledgement. Source qualification established a missing guarantee,
not the cause of any historical exit.

Required outcome: choosing standard application Quit, including its Cmd-Q
equivalent, on ordinary `App::run` must become a bounded framework-owned
termination request, perform the existing native exit preparation, stop the
original event loop normally and return to the caller. Captured Screen/session
owners then follow ordinary Rust destruction; the caller can execute its
existing cleanup. Halcyon neither joins application/backend workers nor claims
their cleanup succeeded.

### 7.2 One native standard menu from startup

The ordinary App path must not expose winit's unmediated Quit before a product
MenuModel exists. Use a crate-private `create_app_event_loop()` in
`platform/window.rs`: preserve the existing builder configuration, but on macOS
set the existing winit `with_default_menu(false)` for this App-owned factory.
`App::run` uses it. Preserve the public low-level `create_event_loop()` contract
and its callers; this is ownership-specific construction, not a user toggle,
environment flag or alternative product mode.

Initialize the active native host's standard application roles during ordinary
AppRoot host attachment, before entering the event loop. No fabricated product
MenuModel or MenuItemId is introduced. The same standard-role builder is used
when a real model is installed/replaced, without clearing a pending Quit.

Retain the existing native system roles available at startup: About, Services
menu registration, Hide, Hide Others, Show All and Quit, with their conventional
existing key equivalents. This supplies no new Services/content/clipboard
capability; do not add responder-chain editing or document access. Standard
Quit alone targets the owned buffered request callback, never `terminate:`.
Do not lose product menu titles, enabled/check/label mirroring, normal activation
IDs or structured shortcuts when adding the standard-role shell.

### 7.3 Exact host boundary and request custody

Extend the existing MenuHost trait with these additive defaulted methods:
`initialize_application_roles(&mut self, wake: Option<UiWakeHandle>)` defaults
to no action; `take_quit_request(&mut self) -> bool` defaults to false.
Existing in-window and external implementors retain their current behavior.
AppRoot::set_menu_host supplies its existing context waker and invokes
initialization once for the newly attached host. Ordinary App::run must supply
its actual live waker; absence in an owner-only test is not native execution.

The native implementation holds one private main-thread-local, host-owned Quit
request latch shared with its retained native target. It has no reference to
AppRoot, Screen, WidgetTree, model, worker or another domain. The native callback
only records the pending standard role and wakes through the existing
UiWakeHandle. Publish the request before wake, coalesce duplicate activations,
and do not allocate an unbounded activation list for Quit. It may neither invoke
application cleanup nor mutate framework UI, navigation, focus or selection.

Keep ordinary MenuItemId activations on the original buffered path. Quit is a
named standard lifecycle role, not a magic/sentinel MenuItemId, duplicate
ActionRegistry or second raw-key shortcut. Keep the existing reserved Cmd-Q
projection rule, so an AppKit-consumed equivalent cannot dispatch twice.

The pending latch survives menu installation/replacement and is consumed once by
AppRoot. Retire the native callback target with its host; no stale projection
may command another App instance. Wake failure retains the pending request and
produces one bounded explicit diagnostic for that failed transition, never a
direct-terminate fallback or a successful-cleanup claim. No panic may unwind
through the Objective-C callback.

### 7.4 Terminal lifecycle drain, not a rendering dependency

Add one private AppRoot operation to consume `take_quit_request` and call the
existing `AppContext::request_exit`. Reuse it from the normal menu synchronization
cutoff and from the native unit-UserEvent terminal-lifecycle handling, after
the native callback has returned. The native callback itself never runs this
operation.

The native event-loop owner must settle that buffered termination before any
visibility, redraw, throttle, configure, acquire or text-realization admission
can prevent it. Use the same existing unit EventLoopProxy wake; do not add a
new thread, timer, periodic poll, event bus or secondary event loop. A non-Quit
unit wake retains its original behavior.

If that drain requests exit, reuse the existing primary native exit preparation
(relative-pointer cancellation, pending-relative-input discard and cursor
restoration), then call the original `ActiveEventLoop::exit()` exactly once.
Factor the existing preparation rather than inventing a second teardown policy.
Preserve primary CloseRequested and the existing app-action request_exit route;
child-window close remains its existing dialog operation.

This is the terminal control path already represented by primary close, not an
additional UI-update phase. It must not dispatch ordinary menu commands, flush
Screens, rebuild/layout/paint, or run arbitrary callbacks from native menu
callbacks or from a hidden-window workaround. No widget mutation permission is
created by entering UpdateAuthorityGuard. No change to the HAL002 frame order,
no synthetic CloseRequested event and no direct NSApplication stop/terminate
call is admitted in the native target callback.

A hidden/minimized/zero-size or idle ordinary App must still be able to quit.
Do not unhide, activate or render the window merely to drain Quit. Do not require
another user event, a model result, a caret animation or a successful frame.
Normal stop releases the original event-handler closure and returns from
App::run. Do not append process::exit after the loop or move caller cleanup into
Drop-only or native notification code.

### 7.5 Invariants and exact clause reconciliation

- INV-333-QUIT-RETURN: the normal standard Quit follows the framework stop/return
  route; native termination notification alone is not accepted proof.
- INV-333-QUIT-OWNER: callbacks only enqueue the owned standard role; AppRoot and
  the native App loop own admission and termination.
- INV-333-QUIT-ONCE: one pending role, one consumption, no duplicate shortcut
  dispatch or callback into a retired App.
- INV-333-QUIT-NO-RENDER-DEPENDENCY: visibility and GPU/text output are not
  prerequisites for processing an already admitted Quit.
- INV-333-QUIT-TRUTH: return, Screen destruction, caller cleanup, process exit and
  worker/resource acknowledgement are separate facts.

INV-C3's MenuItemId-only wording gains this one coalesced standard Quit latch;
all ordinary command activation remains unchanged. INV-C4's optional direct
terminate exception is NOT admitted for the ordinary App path under this section.
INV-C8/E3 retain single shortcut authority with the existing standard Cmd-Q
reservation. INV-G1 gains the no-model standard-role initialization. INV-E2/G4
retain ordinary per-frame menu draining; the separately named terminal control
drain may stop before rendering, as primary close already does. No broad native
event-to-UI mutation permission follows.

Scope excludes forced process termination, signals, OS logout/restart policy,
Dock/session termination, new save/veto dialogs and arbitrary application-delegate
replacement. These must not be claimed covered by a main-menu proof. Preserve
existing platform/dependency behavior outside the specified ordinary App route.

### 7.6 Proof and consumer return

Use nearest existing native-menu and app-menu tests for callback versus drain,
duplicate coalescing, no-model startup, model replacement, normal item dispatch,
reserved Cmd-Q and target retirement. Preserve existing test assertions.

Extend the existing `tests/app_frame_realization_native.rs` owner, with a
macOS-only sibling `tests/app_frame_realization_native/native_quit.rs`, for three
explicit child cases: initial no-model menu, projected model menu and hidden
window. Each uses actual App::run and the actual installed NSMenu action/target
on that process's main thread. The fixture may invoke that real native action
programmatically; it must not call request_exit or the new latch directly to
stand in for the behavior being tested. Verify callback return precedes core
consumption, Screen destruction, App::run return and caller-owned cleanup
sentinel exactly once. The hidden case must not depend on another rendered
frame. A parent must require the post-return receipt; a child exit0 without it
fails, including the old direct-terminate path. This is native lifecycle proof,
not human keyboard input, scanout or an application-worker cleanup receipt.

Run focused menu/quit tests and the dependent ordinary application build without
changing consumer source, backend runtime, fonts or screen composition. Reuse
unchanged accepted source/Metal and backend evidence. No broad renderer, font or
installed-backend fixture replay is justified by this unit.

Independent framework review is a prerequisite for the remaining consumer native
scenario when foreground/input ownership is available. That scenario must
observe the application's original shutdown return and real retirement/resource
result; successful generic native return is not a substitute. The three-request,
clipboard and physical-evidence limits remain explicit.

### 7.7 Framework and consumer ownership

The framework unit consists of the native Quit correction and its native
return proof. Application integration and application-owned shutdown acceptance
remain separate; neither changes framework ownership or authorizes an
unrelated source writer.

Permanent scope: the existing MenuHost/native menu, App/bootstrap owner and
nearest tests; mechanically necessary own-package release metadata only.
No generic lifecycle service, new public cleanup hook, product workaround,
dependency upgrade or additional internal instrumentation is authorized.

### 7.8 Independent source and native-return acceptance - 2026-11-14

**Status: ACCEPTED for the selected framework and native-return boundary.**
Independent review covered MenuHost, the native target, ordinary App
construction/drain/exit preparation, tests and retained original records. All
nine delivered files matched the recorded tested source identities. The accepted
framework package was 0.235.0-beta. No test, native application or backend request
was repeated during this review.

The original native RED actually selected terminate: in initial, projected and
hidden child cases. Each child exited0 without the post-action/return/cleanup
sequence and the parent returned101. This proves the old path in those owned
fixtures, not the cause of a historical application exit. The final three original
native logs each record the installed action returning, core consumption, native
stop with exiting=true, original Screen destruction, App::run return and explicit
caller cleanup once, in order. The parent returned0. Latest frame identity,
paint count and Screen-flush count remained unchanged after the action; the
hidden case used its own NSWindow and did not activate or render to quit.

Independent review also checked original native_menu_macos 4/4,
app_menu_host_tests 7/7, package check with 0 errors/127 warnings and successful
dependent ordinary application build. The dependent lock delta was only the
local Halcyon package version. Successful build is not a native consumer request
or resource-retirement receipt.

The reviewed code keeps callback request custody separate from core stop and
caller cleanup, preserves standard application roles from startup and ordinary
menu activations, retains pending Quit across model projection, and makes a
retired target inert. Public low-level event-loop construction stays unchanged.
The exiting-loop guard prevents a consumed role from admitting another frame.

The original candidate source identities, initial/projected/hidden native red
and green records, focused menu-test results, package check, dependent build
and lockfile comparison were retained privately outside build output. They are
historical review evidence, not public reproduction artifacts linked by this
document.

This framework acceptance does not establish human Cmd-Q, the consumer's
three-request foreground/input/pixel scenario, physical clipboard or actual
application/backend shutdown. The framework unit is complete; its acceptance
does not reopen closed instrumentation work or alter unrelated rendering work.

### 7.9 Return to the original consumer acceptance, not a new product design

The original consumer scenario continued against the reviewed lifecycle
correction. This historical acceptance record selects no new application
behavior, visual composition, desktop policy, backend, source access or storage.
Unresolved product-design choices remain separate; no layout feature is
selected here.

Temporary passive observation was confined to the consumer's original request
screen and ordinary application entry point. No framework source
instrumentation, internal router probe, public observer API or alternate entry
was admitted. Exact beforeimages had to be preserved and restored; accepted
consumer/backend/runtime semantics remained unchanged.

The later five-file route investigation is closed. For this consumer-only
acceptance, retain the original post-route staging contract rather
than requiring private App/router/TextField records from the closed loan.
Before the one fixed-text post, require current owned foreground/window,
measured click ingress at the live enabled writable editor and returned
consumer flush. Do not claim those facts alone prove internal Capture.
Before Send, require actual post-route handoff bytes equal the exact selected
request, unchanged current editor identity, deferred enabled Send, and the
separately inspected original staged image. Successful editor mutation is the
current effective-input evidence. No mutation or forced Capture is admitted.

Keep the corrected complete event-local flags policy and strict numeric
completed==1 versus completed_total==turn from the retained controller.
Adapt only its record-schema consumption to the two consumer sites; no invented
private-route records. Qualify that adaptation offline before native launch.
A staged image must be returned and visually inspected before a separate
subsequent Send tool call. No same-block capture/display-and-Send shortcut.

Run the original three fixed nonpersonal requests once in one fresh ordinary
App, followed by the unsent still responsive draft. Preserve same resident and
exact correlation, source/document identity, real rendered outcome, original
token budget and nonpreferred model output without retry. Use the existing
owned-window transport and one measured transform correction only. Foreground
and current ownership must pass before effects; one existing bounded activation
request is permitted, not focus-stealing loops. On denied ownership or foreign
input stop scripted actuation; do not restart another application to retry.

Use one original guarded native Quit and record App::run return followed by the
result of the original outer shutdown call exactly once. That loaded-resident
call's actual Ok is owner-qualified through the unchanged resident/scheduler
shutdown and released-resource checks, not a generic fixture sentinel. Retain
an original Err explicitly; exit0/PID absence is corroboration, not replacement.
No extra generation or changes to cleanup to manufacture acknowledgement.

Keep the original 240s owned watchdog and 512-record/262144-byte content-free
consumer trace; log relevant state changes and selected-event flush return,
not every frame or pointer motion. Retain original own-window images and exact
results privately outside build output. Capture no unrelated windows or user
text. After any outcome restore both source files and rebuild the ordinary
application. Run only checks affected by the temporary consumer observation,
not unchanged framework or backend fixtures. No parallel builds against active
framework work. Busy execution ownership is not a cancellation grant or
permission to introduce another writer.

This record is admission only. Queue delivery, executor observation, native
execution and independent consumer acceptance require their own evidence.

### 7.10 Historical partial cleanup and capture evidence - 2026-11-14

**Status: partial evidence ACCEPTED; the same consumer scenario remained OPEN.**
Independent review covered the original native run, observed boundaries,
temporary observation diff, helper source, restoration and focused verification
records. The first selected request reached the current editor and enabled
submission, but capture refused its new path before producing an image. The
helper still restricted captures to an earlier evidence directory. No
inspected-stage scripted submission occurred.

The later uncommanded request produced an actual selected Local(Metal) reply
and left worker27962/generation1 admitted and Ready. Original native.log ends
with app_run_return followed by outer_shutdown ok=true. The executed main
observation records the result of the original shutdown.shutdown() call, not
a replacement sentinel. Its unchanged model/runtime owners qualify actual
resident and scheduler closure and released resources. This accepts an
ordinary loaded-resident caller-join acknowledgement. It does NOT identify the
exit actor/selector or establish controlled Quit, visible answer pixels,
three prescribed turns or physical clipboard. Those boundaries stay distinct.
The two consumer sources and original model/runtime match restored beforeimages.

Section7.9 remained the complete consumer scenario after this partial result.
Its continuation did not reopen framework implementation or authorize additional
framework source instrumentation.

Only the private verification helper's evidence binding was corrected. The
original evidence and pinned helper remained unchanged. The corrected copy used
a newly owned private directory, created exclusively without overwriting, and
bound its evidence constant to that exact directory. The copy had its own
recorded identity; it could not claim the original helper hash. Existing
capture-path guards, authenticated own-window checks, event builders, flags,
source/tap, ordering, payloads and controller predicates remained unchanged.
No original archive became writable.

Before any application launch, factor the EXISTING capture-path Boolean into
one private pure predicate shared by capture and an added no-post path check.
Keep its actual prefix/no-traversal/no-existing-file conditions; do not weaken
or replace them with a success assumption. Qualify the root as a real owned
mode0700 nonsymlink directory, and planned capture outputs as nonexistent
direct PNG children. Check all stage1/turn1/stage2/turn2/stage3/turn3/responsive
and bounded failure captures used by the actual controller. The SAME predicate
must accept planned fresh paths and reject the prior archive, outside-root,
traversal and an existing file. No screenshot, OS input or model invocation
occurs during this check. Review all controller/helper root references and
run the unchanged consumer-schema offline check before launching. Reuse valid
unchanged event-family and predicate proofs; this is not another tooling project.

After that changed prerequisite passes, execute section7.9's one fresh
ordinary run, exactly three fixed requests and the unsent responsive draft.
Capture, return and inspect each stage image BEFORE a separate Send call.
Never let successful path validation substitute for actual image creation,
inspection, enabled controls or correlated completion. Retain the original
foreground/ownership/permission checks, current measured transform, numeric
per-turn1/lifetime1-to3 evidence, source/document equality, real native Quit
and actual outer shutdown result. No fourth generation or preferred-output retry.
Foreign input stops actuation. A concrete failure is retained, not silently fixed
inside the live run or followed by another launch.

Only the same two passive consumer sources may be temporarily reconstructed.
Reuse their byte-matching focused evidence; build the current candidate, then
restore both exact beforeimages and rebuild the ordinary executable after any
outcome. Preserve accepted functionality and unrelated current work. The
existing source-qualified Agentic API research does not authorize enabling a
server, replacing OS-derived input or relabeling functional tests as this
native gate. No visual/desktop, model, storage, lifecycle or input-policy change
is selected. The coherent result is the original consumer scenario, not a
helper-only success. Delivery and execution require separate receipts.

### 7.11 Original scripted consumer scenario accepted - 2026-11-14

**Status: ACCEPTED and CLOSED at the selected scripted ordinary-window boundary.**

Independent review covered the original native log, per-request/staging records,
all 27 helper effects, separate controller results, ordered visual-review
record, seven original capture identities, source restoration and
candidate/restored build records. No test, application, screenshot or backend
request was rerun during acceptance. Image inspection was the executor's
recorded individual review; independent acceptance checked original hashes and
correlation, not a separate pixel reinspection.

One fresh ordinary zero-argument application completed the three prescribed
requests. Exact staged inputs of 57/55/44 bytes and current editor identities
preceded separate submission calls. Capture and submission were distinct
controller invocations; the retained inspection record placed each stage review
before submission. The third staged image recorded recognizable U+770B rather
than the earlier missing-cluster box.

Actual selected Q6_K Local(Metal) replies used the same worker29189/native
generation1, registry1/revision1. Per-turn completed remains1; lifetime completed
advances1,2,3. Original token/stop results are14/8/7, all EndOfSequence.
Original raw/document lengths are64/64,24/24,13/3. The last pair is the existing
single-terminal-marker projection, not a raw-receipt rewrite. All three actual
documents match their qualified visible bodies and mounted identities, with
no reported layout failure. The first reply did not satisfy the requested
heading/bullet formatting; it was preserved without retry. The second renders
the fenced Rust block and the third renders U+770B in the recorded images.

The fourth exact16-byte still responsive draft remained UNSENT, with nonce3,
lifetime3, the same resident and third document retained. The original trace
contains four selected text events and sixteen pointer edges matching the
authored effects; no unexpected text/pointer ingress, overflow, forced stop
or fourth generation is recorded.

Effect027 is one controlled original native Quit with both intended Command
key edges. The original native trace then records App::run return followed by
the actual application-owned shutdown result Ok. The unchanged resident/scheduler
owners qualify resource retirement through that join. The close-controller
and supervisor return0 and reap the child; those process results corroborate,
rather than replace, the owner acknowledgement. This closes the controlled
loaded-resident caller-cleanup gap left open in sections7.9-7.10.

The corrected helper copy preserves the original event/ownership guards and
qualifies its current private capture root before launch. The original archive
remains unchanged. Both temporary consumer files are byte-identical to their
preserved beforeimages and current source; original backend/runtime/presentation/
font owners are unchanged. The ordinary build succeeded after restoration and
contains no temporary observation marker.

The original native log, 27 effect records, four stage records, three request
records, ordered visual review, native-proof summary, shutdown controller,
supervisor, restoration, helper change, capture-path checks and
candidate/restored builds were retained privately. These records establish the
historical acceptance described here; they are not public reproduction
artifacts linked by this document.

This closes the selected original consumer objective. It does not certify
human-operated input, physical clipboard, manual concurrent live-drag resize
or an exact renderer-attempt-to-WindowServer scanout join. Those limitations
neither reopen this completed scripted unit nor justify unchanged repeat runs.
No Agent server activation, new proof subsystem, desktop/Stage composition,
browser integration or other product behavior is selected. The unresolved
human-owned visual-concept discussion remains separate. Existing unrelated
authorized work retains its owner and is not interrupted by this closure.
