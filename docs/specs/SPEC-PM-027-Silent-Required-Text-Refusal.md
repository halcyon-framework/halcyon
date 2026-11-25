# SPEC-PM-027: Silent Required-Text Refusal Masquerading as an Application Hang

**Status:** Active
**Owner:** Core frame orchestration and renderer
**Applies to:** HAL360/361 callers and native text presentation
**Date:** 2026-11-13

## Observed pathology

A native application accepted a real resident-backend response and replaced the
mounted answer and status TextBlocks. Native receipt 873 for SurfaceId(1),
FrameEpoch(798), then reported MissingGlyph at primitive 14/glyph 0 with output,
submission and present-call all false. Repeated failures left the earlier pending
frame visible. An independent user-supplied log from a different process recorded
the same failure indices. Their attempts/processes must not be merged.

The exact responsible character/font is not yet established. The typed HAL360
refusal is correct protection, not evidence that incomplete text may be drawn.
The engineering failure is letting the refusal remain silent on the ordinary
application path and treating successful UI mutation or idle threads as evidence
that presentation succeeded. Temporary high-volume tracing overflowed, which
also prevented complete-run qualification.

## Prevention rules

RULE-PM-027-1: UI mutation, successful inference, wake acceptance, Compile success
and visible presentation are separate claims. Require the original boundary's
evidence for each. AppFrameCompileOutcome::Compiled can contain typed unavailable
text; it is not a successful output.

RULE-PM-027-2: enforce HAL361 section 6 on the normal path. Report the first typed
required-text refusal without an opt-in observer; retain unresolved fault state
through unchanged/skipped frames; record recovery only from real same-surface
successful output. Keep the guard surface-scoped and bounded.

RULE-PM-027-3: a programming path claiming output from the same refused Compile
must trip a named runtime invariant. Unsupported text is an expected typed error,
not an excuse to crash the whole application or silently replace characters.

RULE-PM-027-4: regressions involving dynamic text need real text realization at
the renderer boundary. A CPU-only UI-loop proving TextBlock content does not
prove glyph availability, atlas admission or native output.

## Forbidden patterns

No Unicode deletion, whitespace trimming, fallback-font policy activation,
constant redraw loop or synthetic success to conceal refusal. No unbounded
event log or whole-frame spam in place of a first-failure guard. No claim of a
deadlock solely from stale pixels, or of physical recovery from method return.
Do not promote a partial/overflowed capture into full product acceptance.

## Governing implementation contract

[SPEC-HAL-361 section 6](SPEC-HAL-361-App-Frame-Realization-Observation.md)
owns the selected safeguard. [SPEC-HAL-360](SPEC-HAL-360-Complete-Text-Realization.md)
continues to own shaping/raster/atlas admission and surface independence.
This pathology record does not define a new text engine or fallback mechanism.
