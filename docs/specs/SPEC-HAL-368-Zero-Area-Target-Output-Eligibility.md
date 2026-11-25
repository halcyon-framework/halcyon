# SPEC-HAL-368: Zero-Area Target Output Eligibility — Inactive Review

**Status:** Proposed / inactive — non-binding, 2026-10-16.
**Review recommendation:** No reopening; do not adopt or activate this proposal.
**Author:** JC (direct assignment); the delegated technical reviewer owns the superseding disposition.

The subsequent governing-owner decision expressly supersedes the earlier
reopening instruction. HAL-081B INV-P2-007 requires the base surface
(`scene_key = 1`) to remain present and Visible. It does not require
successful GPU output when that target's actual physical extent is zero.
No reopening SPEC is needed solely for this case.

Keep the base registered and Visible. App retains actual current zero physical
extent as inactive; the existing target eligibility/Compile boundary omits the
unusable target before positive-size assertions, resource acquisition and
submission. Other valid child targets remain independently eligible. Stale
configured extents and fabricated positive dimensions cannot replace current
physical extent.

The original reopening draft is retained in signed documentation commit
`06f444e28915c932650c3f945f202d0aac3f8175` as history. It was never accepted.
The later withdrawal remains historical; the subsequent review explicitly keeps
this record Proposed/inactive. No new contract or HAL-081B amendment is proposed
or activated. HAL-024 records the current owner disposition, verified canonical
transport seam and the real zero-primary/positive-child/resume proof committed
in `8784377b31b38f56329b02c6ba48dc54eaf35da3`. Independent review remains with the delegated technical reviewer.
