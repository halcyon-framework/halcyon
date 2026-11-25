# SPEC-PM-021: Telemetry Polling Is Not Animation

**Status:** Proposed pathology record pending independent technical review

**Date:** 2026-10-26

**Scope:** Showcase LiveStat idle demand

## Confirmed failure

LiveStat requested animation continuously to keep CPU/memory telemetry polling.
The ordinary engine consequently invalidated its retained local fragment even
when its text was identical. On native Inputs/Controls, independent process
sampling averaged 31.09% CPU; instrumentation observed about 120 surface
submissions/present calls and 122 local paints per second, with no layout passes.
The HUD's screen-update rate counted Screen::update, not presentations.

## Existing rule and correction

Binding SPEC-HAL-354's INV-354-CADENCE-1 separates polling from visual demand.
LiveStat now requests needs_update instead of needs_animation. Its existing
changed-text comparison and owner-local invalidation remain the paint authority.
No scheduler, display cadence, input, caret or animation policy changes.

With the same native diagnostic configuration, process CPU averaged 6.16%,
surface submissions/present calls and local paints fell to about six per second,
and updates remained about 120 per second. Real animated Gauges continue at the
display cadence. Logs and measurement boundaries are recorded in
target/halcyon-idle-cadence-status.txt. Native present calls do not prove displayed
pixels or GPU completion; physical interaction latency was not measured.

This record adds no governing behavior. SPEC-HAL-043 remains Draft, and the
separate Capture/caret cases in SPEC-PM-020 retain their own evidence and scope.
