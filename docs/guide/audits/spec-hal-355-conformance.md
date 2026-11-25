---
doc-id: halcyon-audit-spec-hal-355-conformance
class: audit
authority: explanatory-non-binding
halcyon-base-revision: 7985f1ddb945108248d17c32efb4268bc4684a4d
candidate-manifest-sha256: 9ba710680016b000c4a3514ddb5a4b0cfa4c369b2573de70d78a2687da2c48b5
verified-at: 2026-08-23
governing-specs:
  - SPEC-HAL-355
  - SPEC-HAL-295A
  - SPEC-PM-007
---

## Pre-Flight

- SPEC: SPEC-HAL-355, SPEC-HAL-295A, and SPEC-PM-007, read before writing this task: YES
- Policy: N/A
- Interaction added: NO
- Forbidden behaviors checked: FORBID-355-RAW-GPU-ESCAPE, FORBID-355-2D-CONTAMINATION, FORBID-355-HIDDEN-CONTEXT, FORBID-355-FALSE-PUBLICATION, FORBID-355-UNLIMITED-BUDGET, FORBID-355-PORTABILITY-OVERCLAIM, FORBID-355-RECOVERY-OVERCLAIM
- Existing infrastructure: SPEC-HAL-266A custom-participant preparation, SPEC-HAL-295A custom-render publication, SPEC-HAL-263A production Image composition, renderer-owned target metrics, and the existing release Showcase were inspected and reused
- Reinvention risk: no new renderer, publication path, Image path, device, Queue, or benchmark policy was introduced; this record audits the existing implementation and names missing proof explicitly

# SPEC-HAL-355 Implementation and Backend Conformance Audit

This record separates adopted authority, implemented code, and backend support as
required by SPEC-HAL-355 §14.2. It does not declare a conforming backend.

## 1. Candidate Identity

The audited implementation is the uncommitted candidate based on
7985f1ddb945108248d17c32efb4268bc4684a4d, package version
0.194.0-beta, with candidate manifest SHA-256
9ba710680016b000c4a3514ddb5a4b0cfa4c369b2573de70d78a2687da2c48b5.
The digest is over path-qualified SHA-256 records for the candidate's manifests,
source, tests, and Showcase files, excluding this audit and changelog:

    shasum -a 256 Cargo.toml Cargo.lock examples/showcase/main.rs examples/showcase/tab_viewport_3d.rs src/renderer/batcher.rs src/renderer/custom_render_pass.rs src/renderer/wgpu_renderer.rs src/tests/custom_render_pass.rs src/tests/mod.rs src/tests/three_d_backend.rs src/tests/three_d_context.rs src/tests/three_d_raster.rs src/tests/three_d_viewport.rs src/three_d/content.rs src/three_d/context.rs src/three_d/mod.rs src/three_d/raster.rs src/three_d/viewport.rs src/widgets/panel.rs | shasum -a 256

The worktree is intentionally not represented as a commit. A later edit changes
the candidate and invalidates this digest and every result attributed to it.

## 2. Status

| Axis | Status | Exact meaning |
|---|---|---|
| SPEC-HAL-355 authority | Adopted / Binding | Architectural authority only. |
| Candidate implementation | Partial | The implemented 3D vertical and executed proof are substantial, but §16.4 obligations 25 and 26 are not complete and only Metal was executed. |
| Metal | **Implemented — Unverified** | Actual Apple M4 Max / Metal execution passed the recorded test set under Features::empty() and Limits::default(), but the full §16 contract is not complete. This is not a support claim. |
| Vulkan | **Implemented — Unverified** | Backend-generic implementation exists; no Vulkan host result is recorded. |
| DirectX 12 | **Implemented — Unverified** | Backend-generic implementation exists; no DirectX 12 host result is recorded. |
| Browser/WebGPU | **Unimplemented** | No browser vertical was built or executed; browser support remains unclaimed. |

Overall native implementation status is **partial**, not complete or conforming.

## 3. Executed Evidence

All commands ran from the Halcyon repository on 2026-08-23.

| Evidence | Result |
|---|---|
| Grip verify package halcyon test three_d_ refresh check timeout 600 | PASS: refresh completed; cargo check reported 0 errors and 57 existing warnings; 99 actual runnable tests passed, 0 failed, 0 ignored. |
| Grip verify package halcyon test custom_render_pass timeout 600 | PASS: 19 actual runnable tests passed, 0 failed, 0 ignored. The 2D structural test asserts exact zero generic custom-render and owner-bound 3D counters, including device-lifetime observation/callback counters, empty pending registration queues, and unchanged three-pass 2D topology. |
| Grip verify package halcyon test renderer_and_gpu_resources_stay_observer_free_until_shared_context_admission refresh check timeout 300 | PASS: 1 passed. Renderer and GpuResources construction plus rejected context admission leave observation/callback counters unchanged; the first valid context increments each once, a second context from another GpuResources snapshot increments neither, and both receive the same terminal event. |
| cargo test -q -p halcyon metal_backend_conformance_gate_requires_an_actual_default_profile_device -- --nocapture | PASS: 1 passed. Direct cargo was used only because Grip cannot expose test stdout. The gate fails instead of skipping when Metal is unavailable. |
| Metal adapter record | name="Apple M4 Max", vendor=0x0, device=0x0, type=IntegratedGpu, driver="", driver_info="", backend=Metal; the requested device asserts Features::empty() and Limits::default(). |
| cargo build --release --example showcase | PASS for halcyon v0.194.0-beta. Direct cargo was used because Grip does not express release example builds. |
| Release Showcase operator run | PASS for visible textured geometry and ordinary 2D composition at both 4x and 1x. Local non-versioned captures: /tmp/halcyon-showcase-p2-release-viewport.png and /tmp/halcyon-showcase-p2-release-viewport-1x.png. These captures are supplementary and do not replace executable proof. |

The three_d_ filter covers tests in the three_d_* modules, not merely files or
targets. The reported count is 99 runnable cases.

## 4. §16 Obligation Ledger

PASS means the named candidate evidence executed on the recorded Metal host.
PARTIAL and OPEN are not conformance.

| # | Status | Evidence or missing proof |
|---|---|---|
| 1 | PASS | three_d_math public scalar-role and non-finite boundary tests; public exports contain no third-party math type. |
| 2 | PASS | role_based_scalar_values_reject_every_non_finite_boundary, pose/projection tests, and positive_non_uniform_trs_preserves_winding_and_uses_inverse_transpose_normals. |
| 3 | PASS | Finite/infinite perspective and orthographic reverse-Z projection plus analytic-ray agreement tests. |
| 4 | PASS | Deterministic plane, sphere, AABB, triangle, and local-transform ray tests in three_d_math. |
| 5 | PASS | Byte-exact canonical geometry packing/default/index tests, semantic admission checks, and the actual winding oracle. |
| 6 | PASS | Context, kind, slot, generation, stale/freed, cross-context, and non-wrapping exhaustion tests fail before wgpu. |
| 7 | PASS | Snapshot leases and texture, sampler, geometry, and appearance replacement/retirement tests preserve exact generations. |
| 8 | PASS | Viewport success, failure, resize, inactivity, removal, registration failure, and destroyed-device tests expose distinct requested/committed truth; two contexts on one renderer-owned device receive the same immutable terminal event. |
| 9 | PASS | abi_sizes_dynamic_strides_and_fallback_binding_are_exact plus raster uniform-offset/alignment checks. |
| 10 | PASS | One bind-group layout serves textured and untextured appearances; fallback is the exact opaque-white 1x1 texture and canonical sampler. |
| 11 | PASS | Naga negative/adapter/span/IR/layout/round-trip/semantic tests execute against locked wgpu 25.0.2 and Naga 25.0.1. |
| 12 | PASS | Wrapper sanitization and alpha-mask discard tests cover finite clamp, non-finite zeroing, alpha one, and forbidden output shape. |
| 13 | PASS | Pipeline key and concurrent reservation tests prove correctness identity and single transactional publication. |
| 14 | PASS | actual_gpu_winding_oracle_accepts_ccw_and_culls_cw ran on the gated Metal device. |
| 15 | PASS | actual_gpu_reverse_z_near_over_far_holds_for_all_baseline_projections covers finite/infinite perspective and orthographic depth. |
| 16 | PASS | Actual 1x/4x raster proof covers private MSAA attachments and direct resolve; target compatibility tests cover both fronts. |
| 17 | PASS | Actual premultiplied edge/mask raster proof and target-bound production Image/Panel composition run at 1x and 4x. |
| 18 | PASS | Same-Queue worker/direct publication ordering, registration failure, and pending publication exhaustion preserve atomic tuple/front/lease commit. |
| 19 | PASS | Resize/removal/deferred-destroy tests prove published fronts never become writable backs and storage survives until unregistration drain. |
| 20 | PASS | Newer camera/model/revision, handle, budget, appearance, and registration failures retain compatible last-good truth; incompatible target changes withdraw it. |
| 21 | PASS | Shared-context viewport test proves shared immutable pipeline resources, private target/publication identity, and pre-wgpu cross-context rejection. |
| 22 | PASS | Per-surface DPR/resize tests cover 1.0, 1.5, and 2.0 metrics and isolate invalidation by SurfaceId. |
| 23 | PASS | three_d::audit and the generic custom-render audit are cfg(test) only. Specialized counters increment solely inside context/resource/raster/viewport owner boundaries; device-lifetime counters increment only on first successful consumer admission, never ordinary 2D traversal. |
| 24 | PASS | The 2D structural test reads all generic custom and specialized 3D counters as exactly zero. No 3D worker or job type exists; generic observer/callback/worker/pool/pass/encoder/submission/registration counters also remain zero. |
| 25 | PARTIAL | Static Showcase-style 2D topology, pass count, pending queues, and retained scene are covered. No paired animated-workload equality record for present decisions and submission/command-buffer counts exists yet. |
| 26 | OPEN | No versioned benchmark tool/document, measured noise floor, material-regression threshold, predeclared powered sample plan, randomized/alternated A/B run, raw samples, or confidence analysis exists. No timing claim is made. |
| 27 | PARTIAL | Metal executed under the default profile. Vulkan and DirectX 12 were not run and remain Implemented — Unverified. |
| 28 | PASS (Metal/source) | Device creation requests no optional feature, the Metal gate asserts the default profile, and zero-3D 2D proof executes without device-lifetime observation or 3D admission. Other native backends remain unverified. |
| 29 | OPEN | Browser/WebGPU build and real browser execution were not performed; browser support is unclaimed. |

## 5. Structural-Absence and Device-Lifetime Finding

The structural counter layer is compiled out of ordinary release builds and has
no branch in 2D traversal. WgpuRenderer construction and GpuResources retrieval
create no DeviceLifetimeState and install no device-lost callback. Rejected
Context3D admission also leaves both generic counters unchanged.

After configuration and ABI validation succeed, the first live Context3D asks
GpuResources for generic device-lifetime observation. A lazy OnceLock registry
then creates one shared immutable-first-failure state and installs one callback
under the same serialized acquisition. The registry retains only weak state and
keys the renderer-owned Arc<Device> identity shared by every GpuResources
snapshot. Later contexts on that device reuse the observation and do not replace
the callback. A real Device::destroy test proves two such contexts receive the
same terminal event. Moving a callback into each Context3D would be incorrect
because wgpu exposes one replaceable device-lost callback per device.

The first admitted context therefore records its owner-bound context counter and
the generic observation/callback pair. Target, buffer, texture, sampler, shader,
pipeline, encode, and publication counters remain zero until admitted demand
reaches their owners.

## 6. Blocking Proof Gap

SPEC-HAL-355 §16.4.26 deliberately requires measured policy rather than an
invented duration, noise floor, or threshold. Repository-wide exact-text search
found no implementation of that policy outside the SPEC. Therefore the correct
result is an explicit open obligation, not a fabricated benchmark or a Metal
Conforming label.

The next valid conformance step is an owner-approved, versioned uninstrumented
release A/B harness and policy satisfying every §16.4.26 condition, followed by
the missing animated 2D equality evidence. Vulkan, DirectX 12, and browser rows
require their own real hosts/verticals and the same semantic contract.
