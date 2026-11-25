---
doc-id: halcyon-guide-drafts-readme
class: draft
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-24
governing-specs:
  - SPEC-HAL-309
---

# Drafts Area

Rules for `docs/guide/drafts/` (SPEC-HAL-309 §4-§5):

1. Drafts hold proposed text, unverified AI output, speculative
   architecture, and incomplete migration material. Nothing here is
   canonical reading.
2. Drafts are never linked as normal reading from `index.md`
   (FORBID-309-5); they appear in the registry only as bookkeeping rows.
3. Every draft still carries the §6 metadata block with
   `class: draft` (FORBID-309-3) and a base revision.
4. Promotion path: a draft becomes first-class only after every material
   claim is verified and cited under SPEC-HAL-309 §7 at a pinned
   revision, its registry row is reclassified, and it moves out of
   `drafts/`. Promotion never happens by linking a draft in place.
5. Contradictions found while drafting go to
   `audits/contradictions.md`, never harmonized into draft prose.
6. Stale drafts are deleted, not archived here; this directory is a
   working area, not a graveyard.
