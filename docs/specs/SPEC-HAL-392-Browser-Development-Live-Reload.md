# SPEC-HAL-392 — Browser Development Live Reload

**Status:** Draft design record / non-binding SPEC; V1 execution directly authorized by JC on 2026-11-17. **Owner:** Halcyon development tooling.
**Authority:** HAL-002/002A; existing web application, build, resource and renderer owners remain unchanged.

## 1. Objective

Keep one local development server and one open browser tab alive while Halcyon or a Rust/WASM consumer changes. Build only when the developer types `build` and presses Enter in the foreground session terminal. Reload the browser **only after a complete successful generation**. The current page remains usable during compilation or failure.

The implementation target is the ordinary `wasm32-unknown-unknown` WebGPU application path exercised by `examples/web_local_probe` and a separate Rust/WASM consumer application. This is a development convenience, not a product runtime feature.

## 2. V1 contract

1. A developer starts one foreground interactive session with a declared project, example/package, watch roots, build inputs, output directory and loopback address. Refuse noninteractive or background starts and occupied addresses. The developer owns the session lifetime and explicit stop.
2. An event-driven file watcher tracks source changes for build validity across relevant Rust sources, manifests, templates and assets, including a declared local Halcyon path dependency. Exclude build output, generated artifacts and unrelated projects. Startup, initial sync and file events never start Cargo or bindgen, publish or reload. Each terminal `build` followed by Enter requests one serial attempt on current input, with at most one pending explicit request during an active attempt. Repeated commands on unchanged inputs are valid.
3. Reuse the existing normal offline/locked Cargo WASM build and the matching installed `wasm-bindgen` stage. Capture diagnostics for the current attempt. If inputs change during a build, keep the current generation and wait for an explicit request. No downloads, implicit dependency updates or alternate compiler path.
4. Build and bind into a private generation directory. Publish a generation only after **all required HTML, JS, WASM and declared static assets** are present and complete. Immutable generation-qualified URLs prevent a browser from mixing old JS with new WASM if publication occurs between individual HTTP requests. The active-generation pointer changes atomically.
5. A development-only same-origin `EventSource` channel announces a newly published generation. The thin browser bootstrap then performs an ordinary page reload. On initial connection or reconnect it checks the current generation and reloads only when it differs. The browser UI remains the normal Halcyon canvas; the dev bootstrap does not create product controls or own application state.
6. A failed, superseded or cancelled build preserves the last successful generation and issues no reload. An explicit build after repair can recover without restarting Safari. Report build failures in the session terminal.
7. The server binds only to loopback and serves only explicit immutable output generations and the dev notification/status endpoints. Development URLs and scripts never enter production artifacts. No external telemetry, credentials, CMS, agent, public listener or deployment.
8. Stopping the development session terminates only its owned watcher/build/host. Browser tabs and unrelated processes remain under their own owners; no automatic Safari termination after a test. A bounded cleanup policy may retire unreferenced historical generations without touching the currently served generation.

## 3. Ownership and invariants

- This is a **development tool** around the existing build and WebGPU runtime, not a second renderer, a new Halcyon widget, a replacement resource manager or a new event-routing authority.
- A source change alone is not a successful generation. Build success alone is not evidence that the new page presented real GPU pixels. Never claim visual acceptance without an actual browser observation.
- The page reload creates a **new application instance**: volatile state such as an unsent form draft resets by default. V1 adds no persistence or hidden state handoff. Existing application semantics remain unchanged.
- Existing HAL-012 runtime appearance replacement remains a separate capability; this spec does not reinterpret it as a file-watcher or CSS-like authoring system.
- The watcher never changes the source, manifests, lockfile or application configuration as a side effect of rebuilding.
- The dev server must not seize or kill a foreign listener. Source/asset writes and build outputs must not cause a rebuild loop.

## 4. Minimum acceptance

- Start the session without compiling. Type `build` and press Enter for the first generation, then open Safari once. Edit a visible Rust caption and observe the page without a build or reload. Type `build` and press Enter and observe the changed Halcyon-rendered caption after publication.
- Edit a watched local Halcyon dependency and confirm the same idle-then-explicit-build sequence.
- Introduce a syntax error and request a build. Safari keeps the working page and counter. Restore the source and check that the page stays put until the next explicit build succeeds and reloads it.
- Change watched input during a build. The superseded attempt leaves the existing generation active and waits for an explicit request. Reuse the verified immutable generation coherence.
- Press Ctrl+C during an explicit build and observe the owned compiler, watcher and host stop. The Safari page remains interactive.
- Run only these focused end-to-end scenarios and the affected tool check. No broad renderer or product suite is required by this tooling change.

## 5. Deferred: hot swap

True in-place WASM/module replacement without page reload is **not** V1. It may be practical later, but requires an explicit protocol for compatible application-state snapshot/restore, widget identity, input/focus/capture ownership, GPU resource lifetime and failed-transition rollback. No such protocol is selected or authorized here. Treat it as a separate future proposal, not as a hidden extension to live reload.

**Scoped execution authority:** JC directly requested implementation of V1 under sections 2–4 after the existing owner's safe boundary. The Draft label adds no approval cycle. This does not authorize the deferred hot-swap protocol or change existing application/renderer authority.

## 6. Implemented V1

V1 was implemented as a local development utility with an explicit foreground build command, a native build supervisor, matching wasm-bindgen, and native kqueue notifications for source invalidation. The example and consumer configurations share the same generation protocol. This development infrastructure is retained privately rather than distributed as part of the framework. The public [browser probe](../../examples/web_local_probe/README.md) documents the ordinary manual build and serve route.

The original save-triggered V1 Safari run on 2026-11-17 established caption and local Halcyon dependency reloads, failed-build retention and recovery, coherent generation-qualified HTML/JS/WASM, and Ctrl+C cleanup during a build. That run remains historical evidence for the generation protocol; it does not establish the later explicit-build interaction.

One foreground probe session on 2026-11-18 verified the explicit-build correction in Safari. Startup and saves left the attempt count idle. Explicit caption and dependency builds reloaded the page. Syntax failure and saving the repair preserved the counter until an explicit recovery published. A superseded attempt preserved that generation, and Ctrl+C during an active compile left the page clickable.
