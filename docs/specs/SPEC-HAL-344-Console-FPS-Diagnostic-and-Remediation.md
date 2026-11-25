# SPEC-HAL-344: Console FPS Diagnostic and Remediation

**Status:** Verified
**Layer:** Console Paint / Renderer Text Compile
**Owner:** Core Architecture
**Applies To:** `src/widgets/console.rs`, `src/renderer/wgpu_renderer.rs`, `examples/console.rs`, `examples/showcase/tab_console.rs`
**Related:** SPEC-HAL-305, SPEC-PM-010, SPEC-PM-016

---

# 1. Problem

The Showcase Console tab produced a sustained frame-rate drop on macOS while visible and streaming output.

Observed user-facing state on 2026-06-14:

- `35 fps` in the Showcase Console tab.
- `20 fps` in the Showcase Console tab after continued streaming.
- CPU around one full core while the Console tab remained active.

The symptom is not resolved unless the live Showcase Console tab recovers frame rate. Standalone Console paint improvement alone is insufficient proof.

---

# 2. Mandatory Constraints

The follow-up finding is binding:

1. Probe first. Do not guess from generic terminal-renderer patterns.
2. Do not add a separate Console renderer.
3. Do not add a monospace-only text renderer.
4. Do not add a text thread.
5. Do not break frame phase determinism.
6. Do not bundle separate Console correctness issues into the FPS fix.
7. Apply one targeted fix at the proven layer.

Alacritty's fixed grid, glyph atlas, instanced renderer, viewport-bounded emission, and coalescing model is useful as a reference pattern only. Halcyon must preserve its current renderer and phase model.

---

# 3. First Iteration: Console Paint Finding

`cargo run --example console` was run locally on 2026-06-14.

The existing `ConsoleBody::paint` probe fired repeatedly:

- cold samples: `46.08ms`, `22.25ms`, `18.57ms`, `15.34ms`
- sustained samples: repeated `2.00ms` to `6.53ms` paint frames

The split probe showed the first Console paint defect:

- `total=6.95ms`, `wrap=5.08ms`, `shape=0.39ms`, `draw=1.20ms`, `shape_misses=4`, `draw_calls=1098`
- `total=6.60ms`, `wrap=4.74ms`, `shape=0.35ms`, `draw=1.17ms`, `shape_misses=4`, `draw_calls=1027`
- retained frames also showed repeated wrap recomputation despite `shape_misses=0`

Authorized Console-side fixes:

1. Cache wrapped visual segmentation by exact entry text, available width, and font size.
2. Retain shaped runs beyond previous-frame-only eviction with a hard cap.
3. Avoid a second paint-side shape pass for unselected final text emission.

These fixes are retained because they remove real Console paint work and preserve exact wrap positions.

---

# 4. First Iteration Audit

The first iteration was incomplete for the reported Showcase symptom.

After Console paint fixes, the user supplied live Showcase screenshots still showing `35 fps` and `20 fps`.

Audit result:

1. The first iteration correctly reduced a Console paint subpath.
2. The first iteration incorrectly described the issue in past tense for the live Showcase symptom.
3. The next valid step was not another Console paint change. It was a Showcase-active frame-stage probe.

---

# 5. Showcase Frame-Stage Probe

The Showcase app was temporarily started on `Section::Console` only for profiling and then restored to normal startup.

Command:

```sh
env HAL_PROFILE_RESIZE=1 cargo run --example showcase
```

Pre-fix Console-active frame-stage profile:

- FPS: mostly `24` to `39`
- `build_scene`: roughly `50ms/sec` to `80ms/sec`
- `draw_plan`: roughly `10ms/sec`
- `compile`: roughly `895ms/sec` to `945ms/sec`
- `render`: roughly `9ms/sec` to `12ms/sec`
- `submit`: roughly `2ms/sec`
- `layout`: roughly `1ms/sec` to `2ms/sec`

Binding conclusion: the live Showcase FPS collapse was in renderer `compile_multi`, not `ConsoleBody::paint`, layout, draw-plan construction, GPU render, or submit.

---

# 6. Renderer Text-Compile Probe

Temporary text-compile instrumentation was added inside `WgpuRenderer::build_text_quads_from_surface_primitives` and removed after verification.

Probe result in the Console-active Showcase tab:

- text compile per frame: roughly `23ms` to `35ms`
- text primitives per frame: roughly `700` to `1080`
- glyphs per frame: roughly `3200` to `6070`
- raster misses: `0`
- shape time: roughly `0.02ms` to `0.17ms`
- glyph loop time: roughly `0.95ms` to `1.90ms`

Binding conclusion:

1. The glyph atlas was warm.
2. Shaping was not the active cost.
3. Glyph quad construction was not the active cost.
4. The remaining per-text-primitive cost in the measured loop was font metrics retrieval.

Verified code path:

- `WgpuRenderer::build_text_quads_from_surface_primitives` called `backend_mut().font_metrics(*font_size, scale_factor)` once per text primitive.
- On macOS, `CoreTextBackend::font_metrics` called `create_font(font_size)`, constructing a `CTFont` per call.

With hundreds to over a thousand text primitives per frame, this created the measured renderer compile cost.

---

# 7. Final Fix

`WgpuRenderer::build_text_quads_from_surface_primitives` keeps a local
`TextMetrics` front cache within one text-quad build pass and routes cache misses
through `BackendTextEngine`'s bounded cross-frame metrics cache.

Cache key:

- `font_size.to_bits()`
- `scale_factor.to_bits()`

Cache value:

- `TextMetrics`

Scope:

- local to one `build_text_quads_from_surface_primitives` call
- backed by engine-owned cross-frame reuse keyed by exact font identity, size, and scale
- no public API change
- no approximate measurement
- no glyph position changes
- no phase-order change

This fixes both proven layers: repeated per-primitive lookup within one renderer
compile and repeated CoreText font construction across unchanged compile passes.

---

# 8. Final Verification

Post-fix command:

```sh
env HAL_PROFILE_RESIZE=1 cargo run --example showcase
```

Post-fix Console-active frame-stage profile:

- FPS: held at roughly `120` to `121`
- `compile`: roughly `291ms/sec` to `413ms/sec`
- `build_scene`: roughly `169ms/sec` to `263ms/sec`
- `draw_plan`: roughly `31ms/sec` to `40ms/sec`
- `render`: roughly `249ms/sec` to `465ms/sec`
- `submit`: roughly `6ms/sec` to `12ms/sec`
- `layout`: roughly `2ms/sec` to `7ms/sec`

The target symptom is resolved: Showcase Console no longer remains in the `20fps` to `35fps` range under the same active-tab streaming profile.

Compiler verification:

```text
grip:check_health
0 errors, 97 warnings
cargo check passed.
```

---

# 9. Invariants

**INV-344-0:** Same application state produces the same Console frame output.

**INV-344-1:** Console paint cost is bounded by realized visible content, not total scrollback length.

**INV-344-2:** Unchanged visible text must not be reshaped every frame.

**INV-344-3:** Cached wrapped visual lines are keyed by exact entry text content, available width, and font size.

**INV-344-4:** Wrap cache keys preserve span boundaries exactly. A lossy hash-only key is invalid.

**INV-344-5:** Wrap cache values store wrap segmentation and segment widths only. GlyphRun ownership remains in `shape_cache`.

**INV-344-6:** Shape cache retention remains bounded by a hard maximum entry count.

**INV-344-7:** Renderer metrics use a local per-pass front cache backed by the
text engine's bounded cross-frame cache. Renderer code must not bypass that
authority through direct native-backend metric calls.

**INV-344-8:** Renderer metrics cache keys preserve exact float identity via `to_bits()`.

**INV-344-9:** Renderer metrics caching must not change text shaping, glyph rasterization, glyph atlas keys, clipping, background emission, or draw ordering.

**INV-344-10:** Temporary diagnostics and temporary Showcase startup toggles must not remain in committed code.

---

# 10. Separate Work Not Bundled

These are real but separate and are not part of this fix:

1. Replace the two O(n) `Console::layout` content-height sums with O(1) fixed-row arithmetic.
2. Correct the wrap-mode fixed-height oracle to exact wrapped-height accumulation.
3. Clean up hardcoded Warn/Error colors.

The follow-up explicitly separated these from the FPS root unless proven otherwise. They were not proven as the active Showcase FPS bottleneck.

---

# 11. Completion Criteria

This task is complete only when:

1. The first Console paint finding is preserved without claiming it solved the live Showcase symptom.
2. The Showcase-active frame-stage probe pins the live FPS collapse to renderer compile.
3. The renderer text-compile probe identifies per-primitive font metrics retrieval as the measured remaining cost.
4. The renderer fix caches metrics per `(font_size, scale_factor)` inside the text-quad build pass.
5. `grip:check_health` reports zero errors.
6. Console-active Showcase verification no longer shows the `20fps` to `35fps` failure band.
7. Temporary diagnostic code and temporary startup toggles are removed.
