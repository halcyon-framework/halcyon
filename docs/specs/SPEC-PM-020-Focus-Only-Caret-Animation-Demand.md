# SPEC-PM-020: Focus-Only Caret Animation Demand

**Status:** Proposed pathology record pending independent technical review  
**Date:** 2026-10-25  
**Scope:** Confirmed standalone TextField and TextArea demand failures

## Observed failure

Both owners used their local `focused` flag to request caret animation after
keyboard Capture had ended. Capture release deliberately retains focus, while
their existing Paint paths require actual Capture to display the caret. The
engine consequently repainted the focused owner for an invisible caret.

- TextField's original-source regression failed on 2026-10-24 after genuine
  `RELEASE_CAPTURE`, with focus retained and `needs_animation()` still true.
  Its unchanged correction passed a fresh qualification on 2026-10-25,
  recorded_unix 1792915768. The earlier 20:20:37Z execution's transport outcome
  remains unknown; it is not that successful qualification.
- TextArea's original-source regression likewise failed on 2026-10-25 at the
  release-demand assertion, after real Capture entry, a visible caret and a
  530ms blink. Its scrolling channel was already settled at the failure.

## Distinct scrolling obligation

TextArea also owns a real ScrollContainer animation channel. Removing perpetual
focus demand alone can omit the last displayed scroll position: `update_visual`
may change its visual offset and settle before the engine queries remaining
animation demand. A changed terminal offset still requires owner-local paint
invalidation. Equal offsets require none. Document, cursor and selection remain
independent of these visual offsets under SPEC-HAL-109.

## Existing rules and reused mechanisms

This record introduces no governing behavior or new scheduling authority.
SPEC-HAL-328 owns current keyboard Capture; SPEC-HAL-354 owns animation demand,
changed-value invalidation and retained reuse; SPEC-HAL-012 owns Update/Paint
phase boundaries. The repairs derive an optional caret-demand result from
ordinary UpdateContext appearance publication. Absent publication preserves
the existing focused fallback. TextArea keeps its independent scroll-demand
expression and invalidates only its paint when the existing visual offset
changes, including settlement. No new clock, scheduler or editor policy is used.

## Evidence anchors and limits

- `text_field_caret_demand_stops_after_capture_release`
- `text_area_caret_demand_settles_after_release_without_stalling_scroll`

The TextArea regression uses genuine framework focus/Capture actions, retained
frames with nonzero dt and a routed pixel wheel. It checks a fractional scroll
target through its terminal snapped text projection and subsequent idle reuse.
These are bounded headless owner proofs, not native performance, all embedded
editor coverage, or general Paint conformance. SPEC-PM-019 and draft appearance
specifications are not adopted by this record.
