# SPEC-HAL-345: ListView and TreeView Performance Diagnostic and Remediation

**Status:** Complete
**Layer:** Showcase dev build profile / ListView / TreeView / Text shaping hot path
**Owner:** Core Architecture
**Applies To:** `examples/showcase/tab_list_view.rs`, `examples/showcase/tab_tree_view.rs`, `src/widgets/list_view/`, `src/widgets/tree_view/`, `src/widgets/tabular_view/`, shared render and profiling paths proven by probes
**Related:** SPEC-HAL-179, SPEC-HAL-250, SPEC-HAL-259, SPEC-HAL-262, SPEC-HAL-344

---

# 1. Problem

Showcase List View and Tree View have a drastic performance regression in the active Showcase app.

Observed user-facing state on 2026-06-14:

1. Showcase List View with 10k rows showed `14 fps`, CPU around `104.6%`, and memory around `279 MB`.
2. Showcase Tree View was reported laggy. Screenshot showed `63 fps`, CPU around `106.7%`, and memory around `290 MB`.
3. Follow-up clarification: the low FPS is triggered by fast scrolling, not by a static idle view.

The symptom is not resolved unless the active Showcase List View and Tree View no longer sustain the reported high CPU / low responsiveness state during fast scrolling under the same demo views.

---

# 2. Mandatory Constraints

1. Probe first. Do not infer the root from ListView, TreeView, or renderer naming.
2. Use existing frame-stage profiler hooks when available.
3. Temporary probes must be environment-gated or removed before completion.
4. Do not introduce a new renderer.
5. Do not introduce worker threads.
6. Do not change public APIs unless a measured fix requires it.
7. Do not bundle unrelated visual, interaction, or architecture changes.
8. ListView and TreeView may share a fix only if a probe proves a shared active bottleneck.
9. Preserve SPEC-HAL-344's renderer metrics-cache fix.
10. Final verification must include `grip:check_health` with zero errors.

---

# 3. First Iteration Scope

The first diagnostic pass must establish the active layer for each view:

1. Run Showcase List View with profiling enabled during fast scrolling.
2. Run Showcase Tree View with profiling enabled during fast scrolling.
3. Record frame-stage distribution: `build_scene`, `draw_plan`, `compile`, `render`, `submit`, `layout`, and `text` if emitted.
4. If a stage dominates, instrument only that stage.
5. If ListView and TreeView share the same dominant stage, investigate the shared code path before widget-specific code.
6. If the dominant stages differ, diagnose and fix them separately.

---

# 4. First Iteration Audit

The first fast-scroll probes used `HAL_PROFILE_RESIZE=1 cargo run --example showcase`
with a temporary environment-gated auto-scroll probe. The probe was removed before
completion.

Measured ListView fast-scroll state before the fix:

1. `fps=11-13`.
2. `build_scene=980-1046ms/sec`.
3. `paint=960-1026ms/sec` after paint substage instrumentation.
4. `layout=17-19ms/sec`.
5. Layout-transition substage timers stayed near zero after the first second.

Measured TreeView fast-scroll state before the fix:

1. Initial fast-scroll samples ranged from `fps=44-95` while CPU remained high.
2. `build_scene=728-841ms/sec`.
3. `paint=697-831ms/sec`.
4. `layout=14-26ms/sec`.
5. Layout-transition substage timers stayed near zero after the first second.

First audit result:

1. Broken virtualization was rejected: visible row realization remained
   viewport-bounded.
2. Data-source delay was rejected: Showcase ListView source is synchronous and
   O(1) in `examples/showcase/tab_list_view.rs`; TreeView source is O(1) per
   realized cell in `examples/showcase/tab_tree_view.rs`.
3. Layout and layout-transition binding were rejected as primary causes by the
   measured substage timings.
4. The active layer was text work during fast-scroll scene construction /
   renderer compile.

The first pass identified the measured layer as:

1. ListView: text shaping for newly exposed visible cell text.
2. TreeView: shared text-heavy paint/compile work under fast scrolling, with
   text shaping contributing and root debug-build cost dominating CPU.

---

# 5. Diagnostic Deepening

Backend text-engine probes proved the active text path:

1. `src/platform/font_manager_text_engine.rs` probe showed zero shape calls in
   this Showcase path, so FontManagerTextEngine was rejected.
2. `src/platform/backend_text_engine.rs::shape_internal` was active.
3. ListView fast-scroll before the fix measured `shape=887-938ms/sec`,
   `shape_miss=~830/sec`, and `compile` or `paint` owning the frame depending
   on whether widgets pre-shaped text.
4. Splitting `shape_internal` showed `font_metrics=23-25ms/sec` and
   `shaper=865-896ms/sec` in the failing ListView run.
5. Source inspection verified `TextShaper::shape` calls `rustybuzz::shape` in
   `src/core/text_shaper.rs`, and renderer compile calls
   `BackendTextEngine::shape_with_keys` from
   `src/renderer/wgpu_renderer.rs::build_text_quads_from_surface_primitives`.

Rejected candidates:

1. Widget-side metrics-only `GlyphRun` construction moved cost from
   `build_scene/paint` to renderer `compile`; ListView stayed at `fps=11`.
2. `TextShaper` shape-plan caching and buffer reuse compiled cleanly but
   ListView stayed at `fps=13-14`; the wrapper optimization was not sufficient.
3. `[profile.dev.package.rustybuzz] opt-level = 3` recompiled `rustybuzz` but
   ListView stayed at `fps=13`.

Final proven fix:

1. `CARGO_PROFILE_DEV_OPT_LEVEL=1` was tested before changing the manifest.
2. The same ListView fast-scroll probe jumped to `fps=103-121`.
3. The same TreeView fast-scroll probe held `fps=117-121`.
4. The committed manifest equivalent is `[profile.dev] opt-level = 1` in
   `Cargo.toml`.
5. This fix changes dev build code generation only. It does not change
   ListView, TreeView, data-source, scroll, renderer, text, or public API
   behavior.

---

# 6. Final Audit

**INV-345-0:** ListView and TreeView performance claims must be backed by a real probe or verified source line.

**INV-345-1:** Visible row realization remains viewport-bounded.

**INV-345-2:** Content height for fixed row-height data remains arithmetic, not dependent on materializing all rows.

**INV-345-3:** Scroll, selection, focus, expansion, column geometry, and row identity authorities remain unchanged unless the measured fix requires a SPEC update.

**INV-345-4:** Renderer changes are allowed only at the proven renderer layer and must preserve draw ordering, clipping, glyph rasterization, and text geometry.

**INV-345-5:** Temporary diagnostics and temporary Showcase startup toggles must not remain in final source.

Final verification after the fix:

1. ListView fast-scroll with `HAL_PROFILE_RESIZE=1 HAL_PROBE_AUTO_SCROLL_PX=720 cargo run --example showcase` and final manifest profile:
   `fps=102-121`, `compile=30.9-37.7ms/sec`, `layout=0.5-18.8ms/sec`.
2. TreeView fast-scroll with the same command and final manifest profile:
   `fps=117-121`, `compile=34.4-38.8ms/sec`, `layout=6.0-8.2ms/sec`.
3. Temporary `HAL_PROBE_AUTO_SCROLL_PX` code and startup-section toggles were
   removed after verification.
4. Final `grip:check_health` is required to report zero errors.

---

# 7. Completion Criteria

This task is complete only when:

1. A dedicated probe records the active ListView bottleneck.
2. A dedicated probe records the active TreeView bottleneck.
3. The final diagnosis names exact functions or source paths verified by code inspection.
4. The fix targets only the proven layer.
5. Showcase List View and Tree View are re-run with fast-scroll profiler output after the fix.
6. `grip:check_health` reports zero errors.
7. Temporary diagnostic code and temporary startup toggles are removed.

Completion status:

1. Dedicated ListView bottleneck probe: complete.
2. Dedicated TreeView bottleneck probe: complete.
3. Exact source diagnosis: complete.
4. Fix targets only the proven dev-build layer: complete.
5. Post-fix ListView and TreeView fast-scroll profiler output: complete.
6. `grip:check_health`: complete, zero errors.
7. Temporary diagnostics and startup toggles: removed.
