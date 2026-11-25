# SPEC-HAL-030A: Swapchain Queue Depth and High-Refresh Cadence

**Status:** Adopted / Binding
**Date:** 2026-08-28
**Amends:** SPEC-HAL-030
**Narrowly reopens:** SPEC-HAL-081B renderer-owned surface configuration
**Depends on:** SPEC-HAL-030, SPEC-HAL-042, SPEC-HAL-081B
**Runtime scope:** canonical `WgpuRenderer` window-surface presentation path

This amendment fixes one presentation-policy defect. It does not change the
engine frame loop, animation timing, custom-render publication, Viewport3D, or
shared spatial-domain composition.

---

## 1. Problem and Decision

The canonical renderer currently uses `wgpu::PresentMode::AutoVsync`. Its
`desired_maximum_frame_latency` is a separate queue-depth hint. WGPU documents
that a value of `1` can serialize CPU and GPU work at
`Surface::get_current_texture` on a backend that cannot wait on present. A value
of `2` admits at least one queued frame so the GPU work queue can remain fed.

Halcyon's binding decision is:

```text
present mode:                         existing renderer-selected AutoVsync
desired maximum frame latency:        2
configuration owner:                  WgpuRenderer
application sites:                    registration and every resize/reconfigure
```

The value is intentionally finite. It restores CPU/GPU overlap without
authorizing a deeper presentation backlog.

---

## 2. Ownership and Scope

`WgpuRenderer` remains the sole owner of complete
`wgpu::SurfaceConfiguration` values under SPEC-HAL-081B. The render-target
registry applies those values verbatim and does not choose presentation policy.

This amendment narrowly reopens SPEC-HAL-081B only for the renderer's
`desired_maximum_frame_latency` value and the construction path that keeps it
identical across registration and resize. It does not reopen batching,
`scene_key`, surface ordering, acquisition RAII, error recovery, attachment
sizing, or compositor authority.

SPEC-HAL-030 remains the owner of presentation semantics. SPEC-HAL-042 remains
the owner of frame admission and redraw cadence.

---

## 3. Binding Invariants

### INV-030A-001 — Canonical queue depth

Every window surface configured by `WgpuRenderer` MUST use
`desired_maximum_frame_latency == 2`.

### INV-030A-002 — Configuration-path identity

Initial registration and resize/reconfiguration MUST obtain the value from one
renderer-owned configuration construction path. A resize MUST NOT silently
restore a different queue depth.

### INV-030A-003 — Bounded presentation backlog

No Halcyon caller may increase the value above `2` or introduce a per-widget,
per-scene, per-animation, or per-Viewport3D override without new presentation
authority and cross-platform measurement.

### INV-030A-004 — Timing isolation

This queue-depth decision MUST NOT change engine `dt`, monitor-refresh
selection, `ContinuousFramePacer`, redraw admission, Timeline sampling, input
routing, camera/model state, or animation arithmetic.

### INV-030A-005 — Offscreen isolation

The setting applies only to OS window surfaces. A `CustomRenderPass` continues
to own exactly its separate front/back sampleable textures. Viewport3D's private
depth and optional MSAA attachments remain render-only and do not constitute a
third published offscreen buffer. No custom-render lease or committed-view
association is a swapchain slot.

### INV-030A-006 — Present-mode preservation

This amendment does not change the renderer-selected present mode or alpha,
format, usage, size, and view-format fields. The current `AutoVsync` policy
remains in force.

---

## 4. Failure and Lifecycle

Surface acquisition, lost/outdated recovery, zero-size suspension, and RAII
release retain their existing owners and behavior. When a non-zero resize
reconfigures a surface, INV-030A-001 applies before the next acquisition.

Backend clamping remains WGPU/platform behavior. Halcyon requests `2`; it does
not claim that every backend exposes or guarantees an identical native buffer
count.

---

## 5. Proof Obligations

Acceptance requires all of:

1. a production-path unit regression proving that the shared renderer
   configuration constructor preserves format, dimensions, present mode, alpha
   mode, and usage while setting the queue-depth hint to exactly `2`;
2. semantic/source evidence that both registration and resize use that shared
   constructor;
3. a focused Halcyon package test plus package check;
4. a live high-refresh measurement of the affected example without screen
   recording, reporting completed frames per interval and the renderer phase
   that previously blocked;
5. source evidence that Viewport3D/custom-render buffering remains separate and
   unchanged.

The motivating 120 Hz Metal measurement on 2026-08-28 is admissible runtime
evidence, not a universal performance guarantee: queue depth `1` sustained
approximately 60–66 completed frames/s in `spatial_domains_3d`, while changing
only the main-surface hint to `2` sustained 117–118 completed frames/s. The
supplied screen recording's variable frame cadence therefore does not by itself
measure the live renderer, although it is consistent with the observed defect.

---

## 6. Explicit Non-Goals

This amendment does not authorize:

- a present-mode abstraction or interactive present-mode switching;
- adaptive or platform-specific queue-depth selection;
- changes to frame pacing, animation, scene invalidation, or input;
- changes to Viewport3D, `CustomRenderPass`, SPEC-HAL-355, SPEC-HAL-356, or
  SPEC-HAL-357;
- material, lighting, geometry, or demo-content changes;
- claims about screen-recorder cadence or compositor behavior beyond measured
  evidence.

Any such change requires its own governing authority and proof.
