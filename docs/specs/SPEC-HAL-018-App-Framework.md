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
