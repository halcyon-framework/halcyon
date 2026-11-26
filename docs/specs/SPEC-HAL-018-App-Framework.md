**SPEC-HAL-018: Halcyon App Framework
(Screens, Navigation, Modals, App Context, Global Shortcuts)**

**Author:** JC
**Date:** 2025-11-17
**Status:** Draft
**Scope:** High-level application structure for Halcyon: screen stack, routing, dialogs, global shortcuts, persistent context.
**Primary Use-Case:** Large trading applications (multi-screen dashboards, modal order tickets, context-aware hotkeys).

---

# 1. Purpose

Halcyon’s low-level primitives (widgets, events, scene) are solid.
But a real trading application **cannot** be built with raw widgets alone.

You need:

* screen navigation
* screen stack
* modals
* app-level context & services
* persistent settings/state
* global shortcuts (ESC, Cmd+S, etc.)
* predictable routing
* clear app lifecycle hooks

This SPEC defines the **minimal** high-level framework Halcyon must provide
without turning into a bloated React/Elm/XAML monstrosity.

---

# 2. Core Concepts

The Halcyon App Framework introduces:

1. **Screen**
   A widget + its own internal logic. Example: Login screen, Dashboard screen, Settings screen.

2. **ScreenStack**
   Classic push/pop navigation:

    * push(new screen)
    * replace(screen)
    * pop()

3. **Modal**
   A screen rendered “above” the current screen, blocking interaction behind.

4. **Global Shortcut Layer**
   Key handlers not tied to any widget:

    * ESC closes modals
    * F1 opens help
    * Ctrl+S saves workspace
    * etc.

5. **AppContext**
   A global object accessible to all screens:

    * persistent settings
    * workspace state
    * data connections
    * theme switching

6. **AppRoot**
   Top-level orchestrator: owns context + screen stack + routing.

---

# 3. Diagram

```
                 +--------------------+
                 |     Trading App    |
                 |--------------------|
                 |    AppRoot         |
                 |    AppContext      |
                 |    ScreenStack     |
                 |    ModalStack      |
                 |    GlobalShortcuts |
                 +--------------------+
                        |
                        v
                +-----------------+
                | Current Screen  |
                +-----------------+
                        |
                        v
                +-----------------+
                |   Halcyon UI   |
                +-----------------+
```

---

# 4. Screen API

A **Screen** is a small wrapper around a Halcyon widget tree:

```rust
pub trait Screen {
    fn build(&mut self, cx: &mut ScreenContext) -> &mut dyn Widget;

    fn on_enter(&mut self, _cx: &mut ScreenContext) {}
    fn on_leave(&mut self, _cx: &mut ScreenContext) {}

    fn handle_app_event(&mut self, _event: &AppEvent, _cx: &mut ScreenContext) {}
}
```

`build()` gives Halcyon the root widget for that screen.

`handle_app_event()` is for app-level events not routed through widget tree (e.g., “connection lost” or “new chart data arrived”).

---

# 5. ScreenContext (Context per Screen)

Allows screen to interact with the app environment:

```rust
pub struct ScreenContext<'a> {
    pub app: &'a mut AppContext,
    pub screens: &'a mut ScreenStack,
    pub modals: &'a mut ModalStack,
    pub shortcuts: &'a mut GlobalShortcuts,
}
```

A screen can:

```rust
cx.screens.push(Box::new(SettingsScreen::new()));
cx.modals.open(Box::new(ConfirmDialog::new("Delete user?")));
cx.app.set_workspace_layout(new_layout);
cx.shortcuts.register(...);
```

---

# 6. ScreenStack

Basics:

```rust
pub struct ScreenStack {
    stack: Vec<Box<dyn Screen>>,
}

impl ScreenStack {
    pub fn current(&mut self) -> &mut dyn Screen;
    pub fn push(&mut self, scr: Box<dyn Screen>);
    pub fn pop(&mut self);
    pub fn replace(&mut self, scr: Box<dyn Screen>);
}
```

Rules:

* Only **top screen** receives widget focus.
* When switching:

    * `on_leave()` is fired for old
    * `on_enter()` for new

This gives classic mobile/desktop navigation without any complexity.

---

# 7. Modal Stack

Halcyon supports **modal overlays**:

```rust
pub struct ModalStack {
    modals: Vec<Box<dyn Screen>>,   
}
```

Modal is just another screen, but:

* rendered **above** the main screen
* receives **all pointer + key events first**
* background is dimmed (Halcyon draws a translucent layer)
* ESC closes top modal (unless disabled)

This allows:

* confirmations
* order tickets
* input dialogs
* alerts
* symbol search

All without fighting the widget tree.

---

# 8. AppContext

A global “service + storage” container:

```rust
pub struct AppContext {
    pub theme: Theme,
    pub settings: Settings,
    pub workspace: WorkspaceState,
    pub data_bus: DataBus,
    pub api: TradingApi,
    pub session: SessionInfo,
}
```

Use-case examples:

* `data_bus`: shared streaming channels
* `workspace`: current layout (which charts, panes, symbols)
* `api`: ability to send orders
* `theme`: runtime theme switching

Every screen gets a mutable reference.

---

# 9. Global Shortcut Layer

A lightweight routing table:

```rust
pub struct GlobalShortcuts {
    bindings: Vec<(Shortcut, ShortcutHandler)>,
}

pub enum Shortcut {
    Key(KeyCode, Modifiers),
}

pub type ShortcutHandler = Box<dyn Fn(&mut ScreenContext) -> bool>;
```

Example registration:

```rust
cx.shortcuts.bind(
    Shortcut::Key(KeyCode::Escape, Modifiers::NONE),
    |cx| {
        if cx.modals.close_top() {
            return true;
        }
        cx.screens.pop();
        true
    },
);
```

Shortcut rules:

1. If a modal is open → modal screen gets events first
2. Else → focused widget
3. Else → global shortcuts

This covers all trading flows cleanly.

---

# 10. AppRoot

The top-level structure:

```rust
pub struct AppRoot {
    pub context: AppContext,
    pub screens: ScreenStack,
    pub modals: ModalStack,
    pub shortcuts: GlobalShortcuts,
}
```

It implements Halcyon’s `App` trait:

```rust
impl App for AppRoot {
    fn handle_event(&mut self, event: &Event) {
        let mut screen_cx = ScreenContext {
            app: &mut self.context,
            screens: &mut self.screens,
            modals: &mut self.modals,
            shortcuts: &mut self.shortcuts,
        };

        // 1. Modal first
        if let Some(modal) = self.modals.top_mut() {
            if modal.handle_event(event, &mut screen_cx) {
                return;
            }
        }

        // 2. Active screen
        if let Some(scr) = self.screens.current_mut() {
            scr.handle_event(event, &mut screen_cx);
        }

        // 3. Global shortcuts (keyboard only)
        if let Event::Key(k) = event {
            self.shortcuts.try_handle(k, &mut screen_cx);
        }
    }

    fn root_widget(&mut self) -> &mut dyn Widget {
        // Delegates to current screen’s widget
        self.screens.current_mut().build(&mut ScreenContext {
            app: &mut self.context,
            screens: &mut self.screens,
            modals: &mut self.modals,
            shortcuts: &mut self.shortcuts,
        })
    }

    fn build_scene(&mut self) -> Scene {
        // Halcyon builds scene from widget
        // Modal overlays draw last
        unimplemented!()
    }
}
```

This is the “engine room” of the app framework.

---

# 11. Navigation Patterns

### 11.1 Simple stack

```rust
WelcomeScreen
    → push(LoginScreen)
        → replace(DashboardScreen)
```

### 11.2 Modal

```rust
Dashboard
   + modal(OrderTicket)
```

### 11.3 Nested screens

You can embed ScreenStacks inside panels if you want sub-navigation.

---

# 12. Trading-Specific Flows Supported

### 12.1 Chart + Ladder + Blotter Workspace

Screen = WorkspaceScreen
WorkspaceScreen widget = DockingLayout → contains panels.

Modal = Order ticket / symbol search / preferences.

Global shortcuts:

* F1 help
* ESC close modal
* Ctrl+S save workspace
* 1/2/3 switch layout
* ` (backtick) show command palette (if you want)

### 12.2 Multi-account or multi-exchange navigation

Each “area” can be a Screen, switching as needed.

### 12.3 Alert dialogs, confirmations

Modals built cleanly over screen.

---

# 13. Implementation Checklist

Integration tasks:

### Framework

* [ ] Define `Screen` trait
* [ ] Implement `ScreenStack`
* [ ] Implement `ModalStack`
* [ ] Implement `AppContext`
* [ ] Implement `GlobalShortcuts`
* [ ] Implement `AppRoot`
* [ ] Extend Halcyon `App` runner to use these structures

### Rendering

* [ ] Support modal dimming layer in scene graph
* [ ] Support overlay rendering for dialogs

### Input

* [ ] Event routing rules matching the SPEC
* [ ] Integrate focus + shortcuts with HAL-009

### UX

* [ ] Provide basic widgets for modal frames, status bars, toolbars
* [ ] Provide helper screens (ConfirmDialog, TextPrompt, etc.)

---

# 14. Result

With this SPEC implemented:

* Halcyon becomes a **full application platform**, not just a widget library.
* You get **deterministic, simple navigation**, avoiding React/XAML hell.
* External chart integration becomes first-class and composable.
* Trading apps become clean to structure:

  ```
  AppRoot → Screens → Widgets → Scene → GPU
  ```
* Modals, dialogs, and workspace layouts become trivial to implement.
* No unnecessary complexity, no data-binding swamp, no magic.

This is the **final structural layer** before you start using Halcyon to build your real trading workstation.

---

# 15. Scoped Native Startup Fullscreen — Binding Addition (2026-10-10)

**Author:** JC. **Decision:** the delegated
HAL-STARTUP-FULLSCREEN-2026-10-10-F1 decision under JC's native-fullscreen
startup mandate. Only this addition is binding; the document-wide Draft status
and the unrelated HAL-018/HAL-082 proposals remain unchanged. HAL-002/002A,
HAL-082A's transport-only window boundary and HAL-024's coordinate authority
continue to govern.

`AppConfig.fullscreen: bool` defaults to `false` and is configured through
`with_fullscreen(bool)`. `with_size` continues to specify the requested windowed
logical size. Existing `Window::new` and `Window::with_endpoint` signatures and
default windowed behavior remain compatible.

The one native window creation uses a common attributes path. Opt-in startup
requests `Some(winit::window::Fullscreen::Borderless(None))` before
`create_window`; windowed startup requests `None`. This is native fullscreen,
not maximize, manual border removal, an exclusive video-mode takeover or a
second window. Title, requested windowed size, minimum size, signal endpoint,
cursor and scale handling retain their existing owners.

Initial renderer target dimensions use the obtained physical `window.inner_size()`.
The compositor, base surface and navigation viewport use that same obtained
extent normalized once with the admitted effective target ratio under §17 through
`physical_rect_to_logical` and `DpiScale::uniform`. Requested config dimensions
must not substitute for the obtained startup viewport. Subsequent resize and
scale events keep the existing projection/layout pipeline and single-root
lifetime; no synthetic resize or repair frame is introduced.

Startup configuration is applied once. Existing native fullscreen exit/re-entry,
Shift+F toggle and normal OS controls remain unchanged; redraw never re-forces
fullscreen and no new Escape or navigation behavior is added. Window remains
OS transport only and gains no compositor, tree, renderer or input authority.

**Implementation:** Candidate in 0.209.0-beta. Executor refresh/check, build and
four `startup_fullscreen_` tests passed on 2026-10-10; independent source review
is pending. Creation-attribute and obtained-extent tests establish the source
contract, not observed native fullscreen timing or platform quality.

RV1 source acceptance, 2026-10-10: the complete seven-file F1 candidate was independently source-reviewed and accepted. The recorded four-test/check/build receipt remains its implementation evidence. Native observation, integration into the original application workspace and consumer adoption remain separate.

# 16. Scoped First-Frame Visibility — Binding Addition (2026-10-14)

**Author:** JC. **Authority:** JC's explicit instruction to show the trainer only
when its first frame is ready, following the authorized native startup correction.
Only this addition is binding; the document-wide Draft status remains unchanged.
HAL-002/002A, HAL-042, HAL-082A, HAL-263 section 5.3 and HAL-361 retain their owners.

Canonical `App::run` creates its primary native window hidden through the common
creation-attributes path, with the configured fullscreen mode and obtained extent.
Public `Window::new` and `Window::with_endpoint` retain visible windowed defaults.
No second window, synthetic resize, repaired dimensions or delayed-show timer is
introduced.

`Screen::startup_ready(&self) -> Result<bool, String>` is a read-only declaration
of required initial content, defaulting to `Ok(true)`. `Ok(false)` means pending;
`Err` retains the producer's failure cause. Active Screen and mounted modal
requirements are combined; a terminal failure is not hidden by another pending
requirement. Producers retain their existing resource ownership, legal action/
Update settlement and wake ingress. While hidden, App also settles lower mounted
modal producers in the same ordinary Screen action phase; after reveal the
existing top-modal settlement policy remains unchanged. Registration and a delivered binding result
alone cannot certify that their required appearance is ready. The query performs
no registration, semantic mutation, layout, paint or scheduling.

Hidden startup admits the existing canonical frame body at `AboutToWait`, after
platform event intake, for initial work, existing resource wakes and deferred
layout continuation. Each admitted frame begins its existing ticker once and
preserves phase order. Native `RedrawRequested` uses that same body after reveal.
There is no second render pipeline or dependence on paint delivery to a hidden
window. Renderer admission remains after scene assembly; received evidence affects
only a later legal producer settlement and successor scene.

App samples readiness before scene preparation. A ready startup attempt ensures
the retained root produces a current scene without forcing another layout or
painting a widget twice. Readiness grants no renderer or presentation authority.
The same ready attempt must produce an actual primary-surface output. App retains
that output until it leaves the frame critical section. Renderer then submits its
commands and completes the submitted GPU work before App reveals the window; the
retained surface texture is presented immediately afterward, with the native
window already visible. A hidden-window Present followed by reveal is not the
startup path: native visibility may affect swap-chain presentation. Optional
HAL-361 observation is published after actual submission/presentation under its
existing contract; it is not required for the gate. A secondary output, compile
success or ticker completion cannot reveal the primary window. This guarantees
prepared content and issued presentation, not physical display scanout.

On Windows, transport disables DWM transitions for this first reveal, then waits
for the submitted presentation through `DwmFlush` and restores the fresh window's
default transition policy. This prevents show animation from inserting an empty
fullscreen/intermediate client state despite a prepared frame. The transport
operation owns no Scene, GPU handle, tree or input state. Other platforms retain
their native show policy. Frame completion counters and agent notification follow
the actual startup submission/presentation; optional observation still follows
the frame critical section.

A producer error, failed GPU/DWM completion or ready attempt with no primary output
fails startup with a diagnostic through the existing fatal startup convention.
It must not expose a placeholder or leave a hidden process waiting without work.
This adds no surface/device recovery policy. Subsequent frames, input, fullscreen
controls and ordinary shown-window scheduling retain their existing contracts.

Required proof: hidden creation attributes; real queued Image admission followed
by later Screen settlement; pending/ready/error aggregation; native windowed and
fullscreen startup remaining hidden until a qualifying primary frame; and the
existing complete-text and frame-observation contracts. Synthetic or headless
proof does not establish observed native visibility.

**Implementation proof (2026-10-14, 0.212.0-beta):** Eight `startup_` tests passed.
The native `--run-startup-native` and `--run-startup-native --fullscreen` probes
passed on Windows using the production visibility path, without probe-specific
transition suppression. Both observed a hidden HWND through queued admission,
later Screen settlement and the ready primary receipt, then sampled the expected
four image colors before the next shown scene. `--run-native` also passed its
existing complete/incomplete-text two-surface cases. These observations establish
the tested first-visible client content and ordering; they do not measure display
scanout or certify other platforms.

### 16.1 macOS native startup presentation — Binding Addition (2026-11-25)

**Status: Accepted / Binding.** The maintainer accepted the visible native
startup presentation after the bounded macOS trial. This addition supersedes
only the macOS visibility and initial drawable-availability requirements of
section 16. All remaining startup, renderer, and application contracts retain
their existing authority.

On macOS, replace the hidden-until-ready reveal guarantee in section 16
with the following scoped guarantee. Create the same primary native
window hidden, using its existing attributes, and install a passive,
GPU-independent native client presentation before showing it. Use the
initial App theme's root background and ordinary text color with one
centered static "Loading…" label. The native presentation fills the
client area and follows ordinary native resize. Show that same window;
do not wait for its first Halcyon drawable to reveal it.

Use the ordinary attached wgpu window surface. No additional window,
custom Metal presentation layer, alternate renderer, startup resource
pipeline, animation, progress estimate, or separate event loop is
admitted. The native presentation is private main-thread transport,
not a widget tree or another input authority. During preparation,
preserve native lifecycle, close/quit and geometry handling; do not
deliver application actions to content that is still concealed.

Retain the existing startup readiness and lower-modal settlement rules,
canonical frame body, resource admission, Build/Compile/Execute and
actual output custody. The existing startup-completion latch is
distinct from native visibility: visible loading is still startup.
Normal pending preparation may proceed behind the native presentation.
A qualifying primary output must still be prepared from the ready
scene and submitted with GPU completion before the existing Present
call. Keep the native presentation until that call succeeds; then
retire it once and complete startup. Do not report a skipped or
unsubmitted output as presented. Producer, preparation and other
startup errors retain their existing failure semantics.

An actual `RendererSurfaceError::Occluded` at acquisition of the
qualifying primary target is temporary unavailability, not fatal macOS startup
failure. Preserve that exact attempt-local
evidence from the existing renderer acquisition boundary, without
changing the public render API or adding global/persistent renderer
state. No output for another reason is not evidence of occlusion.
Leave the loading presentation and startup obligation pending. An
ordinary native `Occluded(false)` event wakes the existing frame
admission path; subsequent resource and geometry events retain their
existing owners. Do not introduce polling, sleep, retry timers, spin,
extra forced frame loops, synthetic input, or visibility spoofing.

Visible App children and existing low-level visible-window clients
may have no drawable on their initial acquisition. Keep existing
creation, registration, focus and signal semantics. Their actual
first output must be produced through the normal admitted frame after
native availability returns, not through a separate rendering route.
An ordinary unocclusion event requests the corresponding existing
redraw; document the temporary output absence for low-level callers.
Do not require first-attempt output regardless of actual availability.
Preserve per-target independence, ordinary subsequent rendering and
the existing handling of all other surface errors. No general public
surface-custody redesign is admitted.

All other platforms retain section 16. Use published wgpu/wgpu-hal 30.0.2;
retain the existing winit patch and qualified Windows dependency alignment.
No new dependency family or backend fork is required. This startup qualification
does not itself qualify a registry publication.

Implementation qualification: the macOS windowed and fullscreen native startup
probes observed the loading presentation during queued Image admission, real
primary GPU completion and Present, one native view removal, and subsequent
ordinary output. Windowed capture showed the ready image; fullscreen capture
showed the same ready scene after the next ordinary frame, before requesting a
distinct successor scene. These observations do not establish first-texture
fullscreen scanout or an absence of a transient visual gap. Both two-surface
native receipt modes preserved Complete/Degraded(MissingGlyph)/Complete text
realization and recovered an initially unavailable child after native
unocclusion. A low-level two-window probe also observed actual unocclusion and
subsequent output from both initially unavailable surfaces. The canonical App
primary was available before acquisition in these runs; its primary Occluded
deferral branch has not been exercised at native runtime. Native checks, five
focused startup tests, and the WASM library check passed. Windows native
runtime has not been requalified for this dependency selection.

# 17. App-local UI Scale — Accepted Scoped Addition (2026-10-16)

**Author:** JC. **Status:** Accepted / Binding; implementation proof pending.
JC's direct continuation mandate records independent technical acceptance of
HAL-024 Layer E at `ec307c537b0a430875ee0bb769a6971dfe903387` and authorizes its
reconciliation and implementation. This addition does not adopt the rest of
HAL-018's Draft scope, any new UI/IME behavior, or the proposed 363/364 package.
Constitution/002A remain supreme.

[HAL-024 Layer E](SPEC-HAL-024-Coordinate-Model.md) owns the complete bounded
scale contract. AppConfig supplies typed initial `UiScale`, default `1.0`, before
first Screen build/layout. `with_size` remains a requested OS-logical window size;
App scale does not change fullscreen/window creation semantics. Use obtained
physical extent and `E = D × S` once for the initial canonical viewport.

AppContext owns explicit runtime request and immutable requested/admitted/
effective readback. One pending slot, typed rejection, the post-old-batch Phase-2
checkpoint and no-op equality follow HAL-024 E.3. Requests do not remount Screens,
move focus, reset editing/keyboard Capture, or invent native IME support. Existing
128/362 cancellation retains its exact owner and ordering.

Each target uses its own coherent effective ratio/geometry/resource snapshot;
266/266A and 360 retain per-target preparation/output admission. No cross-target
atomic Present or global renderer gate is added. Existing §16 required-content
readiness and qualifying primary output still govern initial reveal. Admission
of an App value, target readiness, successful submission and observed display
remain distinct; implementation must report only the evidence it actually has.

# 18. GPU initialization failure — Binding Addition (2026-11-26)

**Status: Accepted for implementation.** This narrowly reopens the
initial renderer construction and initial window-surface creation
boundaries of HAL-081A/081B. Their ownership, registration, render
execution and presentation contracts remain unchanged. Sections 16 and
16.1 retain their successful startup behavior.

GPU availability is an external startup condition, not a framework
invariant. A returned failure to create a surface, obtain a compatible
adapter, or request a device, or an empty required surface capability
list, must not panic on the canonical App startup path.

The renderer owns the following public additive API, in
`renderer::wgpu_renderer`:

- `RendererInitError`, a typed error with variants
  `SurfaceCreation(wgpu::CreateSurfaceError)`,
  `AdapterUnavailable(wgpu::RequestAdapterError)`,
  `DeviceCreation(wgpu::RequestDeviceError)`,
  `NoSurfaceFormats` and `NoSurfaceAlphaModes`.
  Preserve the original typed causes through Error::source, with
  concise human-readable Display descriptions.
- `WgpuRenderer::try_new`, with the existing new constructor's
  arguments and `Result<Self, RendererInitError>` as its async result.
- `WgpuRenderer::try_create_surface_for_window`, with the existing
  surface constructor's arguments and
  `Result<wgpu::Surface<'static>, RendererInitError>` as its result.

The existing `new` and `create_surface_for_window` signatures remain
compatibility wrappers with their documented infallible/panic behavior;
they delegate to the fallible constructors. The canonical App uses the
fallible constructors for both initial surface creations. Headless
construction and ordinary later surface lifecycle are unchanged.
Preserve the current backend selection, adapter/device requirements and
nonempty capability-selection policy. Do not add another adapter query,
fallback renderer, retry, forced software adapter, or dependency change.

`App::run` retains its signature and scheduling semantics. Add
`App::run_with_gpu_init_error_handler<S: Screen + 'static>`, accepting
the screen and an owned `impl FnOnce(RendererInitError) + 'static`.
It follows the same App startup path and invokes that callback exactly
once on a GPU initialization refusal and never on successful startup.
The existing run delegates with a default diagnostic handler (error
tracing, with native stderr or browser console if no enabled subscriber).
No global callback, static error slot or AppContext authority is added.

An initialization refusal occurs before Screen construction/entry,
widget mounting, input-bridge installation, or presentation. Drop
partial renderer resources and the startup window/presentation.
On macOS request exit on the existing active event loop without
installing the runtime handler; tolerate the remaining exit callbacks.
Other native targets release their unstarted loop/window. On Web use
the existing bootstrap's callback slot and proxy to request terminal
exit on its own active event loop, without installing the App runtime.
Release its pending resources and remove only the canvas and sibling
overlay created for that App. An already closed loop needs no second
exit request. Deliver the original initialization error once, even if
the bootstrap is already closed. No pending loading UI, replay of input,
background frame loop, fake successful frame or automatic restart.

The caller owns failure presentation. Web Showcase supplies the typed
callback and displays a concise, comprehensible startup failure in its
existing host error panel; detailed underlying diagnostics go to the
browser console. Distinguish no compatible WebGPU adapter from failure
to create a rendering surface/device. Do not infer unsupported hardware
from an adapter refusal. Its HTML's existing absence-of-WebGPU check
also displays a readable message without a stack trace. Real unexpected
panics retain the existing diagnostics. No DOM policy is introduced into
the renderer.

Qualification: verify returned error classification and the empty
capability cases; exercise the canonical browser startup with WebGPU
API present but no available adapter, and the existing no-API case.
Observe the error once, no panic, no Screen entry, no installed App
runtime, and termination of the failed bootstrap. Verify ordinary
successful native and Web startup through the existing example paths.
Where a device/surface refusal is tested by a deterministic boundary
fixture rather than actual hardware, identify that evidence accordingly.
Do not require a new general fault-injection system. This addition
does not define recovery from device loss, runtime GPU validation
failures, or arbitrary programming errors.

# 19. Independent browser context-menu policy — Binding Addition (2026-11-26)

**Status: Accepted / Binding.** JC approved permitting the standard
browser context menu on Website while preserving existing application
input behavior.

Add the WASM-only `AppConfig::with_web_context_menu_enabled(self,
enabled: bool) -> Self`. Its default false retains the canonical
App's present contextmenu suppression. True permits the browser's
ordinary contextmenu default action on the App's own canvas. This
is a startup configuration carried to the existing window-creation
boundary, not application-global mutable state. The host may display
its own browser menu; Halcyon does not implement or synthesize that
menu. All current routed pointer events and application menu
semantics are unchanged. Existing native paths are unchanged.

The Winit Web transport owns default cancellation on its canvas.
Add the WASM `WindowAttributesExtWebSys::with_context_menu_enabled(
self, enabled: bool) -> Self` creation attribute to the existing
Halcyon derivative. Use an optional override, unset by default:
when unset, the contextmenu listener follows its existing
prevent_default value; when explicitly set, suppress that event
only when the selected menu policy is false. Pass the App's explicit
selection at canonical Web window creation. Low-level window callers
retain the previous default. Do not modify any other default-event
policy, including wheel, pointer, keyboard/clipboard, touch-action
or host choice controls. No capture listener which intercepts other
listeners, global prevent_default toggle, DOM overlay, or second
routing authority is admitted.

The published halcyon-winit 0.30.12 is immutable. The next local
derivative revision is halcyon-winit 0.30.13, based on the same upstream
winit 0.30.12, with the independent contextmenu option as its only
new transport behavior. Update its package metadata and lock and
Halcyon's WASM dependency pin consistently; native upstream winit
remains exactly 0.30.12. Retain the same Rust library identity and
existing patched input behavior. Document the derivative revision
and update current installation guidance where it identifies the
helper version, not historical evidence. This authorizes local
preparation only, not registry publication.

Qualification: compare ordinary real browser contextmenu event
cancellation on the canonical canvas with default and enabled App
configuration; verify the unchanged ordinary routed right-click and
primary activation paths and default wheel/key policy. Use the
existing browser test route; report event default-prevention evidence
as such, not physical browser-menu observation. Native and WASM
compilation must pass. No new general test harness or unrelated
transport changes.

## 19.1 Route-scoped browser context-menu policy (2026-11-26)

**Status: Accepted / Binding, scoped Website consumer correction.** The one-canvas Website now hosts both its ordinary Home and the live Dedicated Showcase. The earlier explicitly enabled browser context menu must remain available on Home, but the live Showcase must not display the host browser menu above its own routed context menu. Extend only the existing section 19 policy: the App startup value remains the default; a declared public WebNavigation route may explicitly override it.

Add WASM-only `WebNavigation::with_context_menu_enabled_for(self, route: NavigationNodeId, enabled: bool) -> Result<Self, WebContextMenuPolicyError>`. Define the separate checked error `WebContextMenuPolicyError::UnmappedRoute` for an ID absent from this configuration's already validated route map. A later explicit builder selection for the same route replaces that selection. The finite optional map is part of the existing immutable route declaration, not a second router. No selection means the AppConfig startup default. The existing AppWebNavigation owner synchronously projects the effective policy from the actual committed current NavigationNodeId at bind (including admitted direct entry) and after each successful route commit, before the newly active Screen receives ordinary input. A refused, pending or failed intent does not change policy. Never derive it from a raw location string or pending route. A retired binding must not act on the canvas; no new policy publication from stale callbacks. Existing no-navigation Apps, routes without an override, and native execution preserve their existing behavior.

The existing Winit Web canvas contextmenu listener remains the sole default-action cancellation owner. Store its existing optional override in a canvas-local Rc<Cell<Option<bool>>> shared with that listener. Add `WindowExtWebSys::set_context_menu_enabled(&self, enabled: bool)` on the existing Web derivative and the corresponding private Window dispatch, using the existing main-thread Window dispatcher; a main-thread setter updates synchronously. An unset low-level creation override continues to follow the current general prevent_default value until explicitly selected. Only contextmenu reads the independent override. Do not change the general prevent_default value, re-register listeners, intercept pointer events, install document/global listeners or introduce a second canvas, hit-test, router or application menu implementation. Preserve the existing pointer/keyboard/clipboard/wheel/touch and host input-overlay policies. Section 19's creation attribute and default remain compatible.

Website selects startup true and an explicit false override for WEB_SHOWCASE. Home keeps the ordinary browser context menu; the actual Showcase canvas permits only Halcyon's normal application context menus (except browser-reserved behavior outside ordinary cancelable events). Direct /showcase, ordinary transitions and browser Back/Forward must use the committed policy on that same canvas. No tab-specific event filter or browser detection is involved. Keep the published halcyon-winit 0.30.13 immutable: the new local derivative is 0.30.14 on the same upstream 0.30.12; reconcile its metadata, patch notice, canonical WASM dependency and lock. Native remains upstream 0.30.12, Halcyon stays on the current local 0.272.0-beta candidate. No publication.

Qualification: preserve both prior static policy cases, prove real ordinary contextmenu event cancellation on the same canvas following successful direct entry, accepted Home/Showcase transitions, trusted history traversal, and refused/pending navigation. Actual application right-click menu selection remains single and functional; primary activation and general default-cancellation behavior remain unchanged. Observe native/browser compilation and actual Website consumer behavior separately. Do not claim physical browser chrome observation from headless DOM cancellation alone.
