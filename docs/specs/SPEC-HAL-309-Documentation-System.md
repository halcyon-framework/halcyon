# SPEC-HAL-309: **Documentation System**

### *AI-First Reference Documentation for Halcyon*

**Author:** JC
**Status:** Adopted
**Depends on:** SPEC-HAL-002 (Constitution), SPEC-HAL-000 §11 (Documentation Standards), SPEC-HAL-003 (Project Structure)
**Adopted:** 2026-07-24, owner decision, reconciled at revision `68216f25e43b532dbbd3cddec48c229cca913016` (halcyon `0.189.2-beta`)

---

# 1. Problem

Halcyon's implemented surface — the runtime, rendering pipeline, text engine,
layout system, widget corpus, tabular subsystem, Plot, and the agent server —
is not documented for consumers. Every new session, human or AI, starts with
rediscovery: reading `mod.rs`, grepping for symbols, guessing at APIs, asking
questions the codebase already answers.

The primary consumers of Halcyon documentation are AI agents building
applications on Halcyon. Human developers benefit from the same corpus, but
the structure is optimized for how LLMs consume information: dense, shallow,
searchable, revision-pinned, with complete type signatures and runnable
examples.

At adoption, the pre-existing material did not satisfy this:

- **SPECs** (`docs/specs/`) are design authority, not usage guides, and each
  carries exactly its declared status.
- **`docs/OVERVIEW.md`** is duplicated verbatim as the head of
  **`docs/WIDGET-CATALOG.md`**; both are unpinned legacy.
- **`docs/ENGINEERING.md`** is an engineering standards manual, not an API or
  architecture reference.
- **Fable/Opus survey documents** are research artifacts pinned to old
  revisions.
- **Rustdoc comments** exist on public items but are item-local, not
  connective.
- **Examples** exist but are not indexed or explained.

This SPEC owns the documentation system that fixes this: the canonical guide
corpus under `docs/guide/`, its document classes, its metadata and claim
contracts, and the replacement plan for the legacy documents.

---

# 2. Authority Model

Authority is separated and never collapses:

1. SPEC-HAL-002 and its amendment process are supreme ("If any SPEC or
   component contradicts this document, this document wins").
2. A governing SPEC carries exactly its declared status. A Draft is intent,
   not implemented truth.
3. Source, tests, compiler output, and runtime evidence establish implemented
   behavior at a particular revision.
4. `docs/guide/` explains and connects those sources. It never creates
   architecture and never silently resolves contradictions.
5. Rustdoc owns item-level public API documentation beside the code. Guides
   connect items; they do not replace item docs.
6. Examples demonstrate intended use; they do not prove general behavior.
7. Tests are executable evidence only when actually run against the cited
   revision.
8. Pre-contract guides and research are legacy or audit material until
   individually reverified and promoted. The adoption inventory is in §10.

---

# 3. Structure

SPECs stay in `docs/specs/` (unchanged). The canonical guide corpus lives in
`docs/guide/`:

```text
docs/guide/
├── index.md
├── overview.md
├── getting-started.md
├── runtime-and-frame-lifecycle.md
├── rendering-and-compositor.md
├── layout-scrolling-and-geometry.md
├── input-events-focus-and-actions.md
├── text-and-typography.md
├── theming-and-density.md
├── data-and-update-patterns.md
├── widget-catalog.md
├── tabular-and-hierarchical-views.md
├── plot-and-market-detail.md
├── agent-and-remote-control.md
├── examples-index.md
├── generated/
│   └── api-quick-reference.md
├── audits/
│   ├── implementation-status.md
│   └── contradictions.md
└── drafts/
    └── readme.md
```

Rules:

- Filenames are `lowercase-with-hyphens.md`.
- Canonical first-class guides stay flat directly under `docs/guide/`. No
  deeper nesting for canonical material.
- `generated/`, `audits/`, and `drafts/` are the only subdirectories. They
  physically separate non-canonical output so retrieval never mixes it with
  verified documentation. (This amends the earlier absolute flat-directory
  rule of this SPEC.)
- `CHANGELOG.md` stays at the workspace root.
- One doc per topic. Update existing docs instead of creating parallel ones.

---

# 4. Document Classes

**First-class** — architecture and runtime explanations, subsystem guides,
integration and data-flow guides, the widget catalog, getting-started
material, the examples index. Canonical reading.

**Generated** — public signature inventories, module/API inventories,
mechanically derived cross-reference tables. Generated does not mean
authoritative: its authority is exactly the source revision from which it was
produced, and each generated file records that revision and the generation
method.

**Audit** — implementation coverage, contradiction records, stale-document
detection, external review intake.

**Draft** — proposed text, unverified AI output, speculative architecture,
incomplete migration material. Never canonical reading.

**Operational doctrine** (outside the guide classes) — the internal development
rules and project-entry guidance govern repository work and verification. These
are separate from the public framework documentation. Product architecture
remains under the governing SPECs.

**Legacy** (transitional designation, not a class) — pre-contract documents
outside `docs/guide/`. The adoption inventory is recorded in §10.
`docs/DESIGN-INTELLIGENCE.md` remains legacy source material. Verify and promote
legacy material or retire it under §10.

---

# 5. Index Contract

`docs/guide/index.md` is the sole canonical entry point. It contains:

- the authority hierarchy (§2);
- recommended reading paths by task;
- the document registry: every file under `docs/guide/`, its class, verified
  revision, verification date, freshness state, and governing SPECs;
- the governing SPECs of the documentation system;
- the legacy/research register.

No competing manifest or secondary table of contents may duplicate it. Drafts
are never linked as normal reading; they appear in the registry only as
bookkeeping rows.

Freshness states: `planned` (registered, not yet written), `current`
(verified revision equals HEAD at last check), `stale` (HEAD has advanced
past the verified revision), `legacy` (§4). A stale guide is a true statement
about its pinned revision, not retroactively false.

---

# 6. Document Metadata

Every document under `docs/guide/` begins with:

```yaml
---
doc-id: halcyon-guide-<topic>
class: first-class | generated | audit | draft
authority: explanatory-non-binding
halcyon-revision: <full 40-character SHA>
verified-at: <YYYY-MM-DD>
governing-specs:
  - SPEC-HAL-...
---
```

- `doc-id` is stable and unique.
- `authority` is always `explanatory-non-binding`: nothing under
  `docs/guide/` binds; binding authority lives in `docs/specs/`.
- `halcyon-revision` is the exact revision the document's claims were
  verified against. It is never advanced silently (§8).
- `governing-specs` lists the SPECs whose areas the document explains.

Generated documents additionally record the generation method (exact command
or procedure).

---

# 7. Claims, Classification, and Citation

Every material claim is classified on two independent axes:

- **Authority:** `binding` | `draft-intent` | `none` | `contradictory`
- **Implementation:** `implemented` | `partial` | `absent` | `unknown` |
  `not-applicable`

The axes never collapse into a single status. "binding + partial" and
"draft-intent + implemented" are legal, meaningful states.

Citation forms:

```text
Authority:
docs/specs/<SPEC file>.md:<lines>
status: <declared SPEC status>
revision: <full SHA>

Implementation:
<src path>:<lines>
symbol: <exact symbol>
revision: <full SHA>

Executable evidence:
<test path>
test: <exact test name>
command: <exact command>
result: PASS|FAIL
revision: <full SHA>

Runtime evidence:
artifact: <path>
captured-at: <timestamp>
build-revision: <full SHA>
environment: <exact platform/configuration>
```

Rules:

- Missing evidence → the only valid implementation state is `unknown`.
- Authority and implementation disagree → mark `contradictory`, cite both
  sides, and record the contradiction in `audits/contradictions.md`. Never
  harmonize a contradiction into clean prose.
- Never infer implementation from a SPEC, filename, comment, example, or
  external survey.
- SPEC status is reported exactly as declared, never upgraded or
  paraphrased.

## 7.1 Public framework scope

Public framework documentation must explain Halcyon's behavior without requiring
knowledge of a private application, development conversation or execution task.

- Express consumer-driven requirements through the actual framework mechanism,
  ownership boundary and observable behavior. Applications retain their domain
  catalogs, business rules, visual choices and deployment policy.
- Preserve exact public type, method and variant names. Explain an unfamiliar
  identifier in descriptive prose; editorial cleanup does not rename an API.
- Keep normative contracts self-contained in their owning SPECs. When an adopted
  amendment is incorporated from a separate packet, integrate its complete accepted
  scope and reconcile references before retiring that packet from the public tree.
  Preserve later amendments, status boundaries and the original acceptance record.
- Public RFC proposals and decision rationale belong in the separate
  `halcyon-framework/rfcs` repository. The framework source repository keeps
  the complete effective contracts in `docs/specs/`; it does not maintain a
  competing `docs/proposals/` layer.
- Each RFC states its problem, scope, rationale, status and governing SPEC
  relationships. RFC discussion and acceptance do not establish implementation
  or conformance. Adoption integrates the complete accepted scope into the
  owning SPEC, with explicit supersession and a reference to the decision.
  No external RFC is required to recover a current contract's normative meaning.
  Existing SPEC identifiers and their declared status remain intact.
- Issues remain the intake for reports, questions and proposals. The maintainer
  owns RFC text and final decisions; this structure does not open external
  implementation pull requests or create a delivery commitment.
- Superseded draft packets, executor tasks, conversation routing, workstation
  paths, private receipt inventories and application-specific acceptance plans
  are development records. Preserve required provenance separately and exclude
  those originals from the clean public source snapshot.
- A public contract must not depend on a private sibling repository or an
  unpublished task for its normative meaning. Describe the necessary framework
  requirement locally and link to publicly available specifications where relevant.
- Preserve the exact identity, inputs, result and limitations of regression and
  conformance evidence. Removing a consumer name must not turn one historical
  run into a general success claim, replace a required original fixture with an
  arbitrary example, or present unavailable evidence as reproducible public proof.

---

# 8. Revision Discipline

Every canonical guide is a statement about one exact revision:

- Capture one base revision when work on a document starts. Never advance it
  silently mid-document.
- A newer HEAD makes a guide `stale`, not wrong. Re-verification against a
  new revision updates `halcyon-revision` and `verified-at` together.
- Performance or behavior numbers appear only with exact benchmark/runtime
  evidence at a named revision.

---

# 9. AI-First Design Principles

**9.1 Self-contained topics.** One file per topic. A guide must be
self-sufficient for its topic; cross-references add depth, they never
substitute for the critical information in place.

**9.2 Complete examples.** Every code example compiles if dropped into a
file. No pseudocode, no elided `use` lines, no `// ...` in critical paths.

**9.3 Shallow structure.** Canonical guides sit flat under `docs/guide/`;
only the three non-canonical subdirectories of §3 exist.

**9.4 Type signatures first.** In reference material, the signature precedes
the explanation.

**9.5 No stale docs without detection.** Staleness is not forbidden —
undetected staleness is. Revision metadata (§6) plus the audits make
staleness mechanically detectable. Work that changes a public API includes
the corresponding documentation update in its task definition.

**9.6 Changelog as memory.** Root `CHANGELOG.md` remains the chronological
record. One line per entry, precise and searchable.

---

# 10. Legacy Replacement Plan

Verified state at adoption (revision `68216f25e43b532dbbd3cddec48c229cca913016`):

- `docs/OVERVIEW.md` (387 lines) is duplicated verbatim as the first 387
  lines of `docs/WIDGET-CATALOG.md` (666 lines).
- Both are unpinned: no revision metadata, no claim classification.
- `docs/fable-5-codebase-survey.md` pins itself to 2026-06-10 at
  `0.27.9-alpha` — a research artifact, not a current guide.
- `docs/opus-4-8-survey.md` is a pasted research-session transcript — a
  research artifact, not a current guide.
- `docs/fable-5-external-review-ledger.md` declares itself non-authoritative
  ("It is not authority.") — audit intake.

Plan:

1. `docs/guide/overview.md` supersedes `docs/OVERVIEW.md`.
2. `docs/guide/widget-catalog.md` plus
   `docs/guide/tabular-and-hierarchical-views.md` supersede
   `docs/WIDGET-CATALOG.md`; the tabular/hierarchical material is extracted
   into its own guide.
3. No blind migration: legacy prose is source material only; every reused
   claim is independently reverified and cited under §7 before it enters a
   canonical guide.
4. Legacy files remain in place, registered as `legacy` in the index, until
   their superseding guides are written and verified; their retirement
   (deletion or archival) is a separate owner-ordered unit. External pointers
   to them are updated in that unit, not before.
5. The surveys and review ledger served as research and audit intake.

On 2026-11-21, the top-level overview and widget catalog were retired after a
historical claim audit accounted for their useful claims. The surveys and review
ledger were also retired. The original audit record is retained privately; the
[guide index](../guide/index.md) lists the remaining legacy material.

---

# 11. Adoption Sequence

1. Reconcile this SPEC; create `docs/guide/index.md`.
2. `overview.md`, `runtime-and-frame-lifecycle.md` — current architecture
   and the actual frame/update/presentation lifecycle.
3. `rendering-and-compositor.md`, `text-and-typography.md`,
   `theming-and-density.md`.
4. `layout-scrolling-and-geometry.md`, `input-events-focus-and-actions.md`,
   `data-and-update-patterns.md`, `tabular-and-hierarchical-views.md`.
5. `getting-started.md` and complete runnable examples; `examples-index.md`.
6. `widget-catalog.md` from live public modules, in coherent families.
7. `plot-and-market-detail.md` — only after the currently dirty
   market-detail unit is settled.
8. `agent-and-remote-control.md` — agent server, capability profiles,
   bounded querying, fused input.
9. `generated/api-quick-reference.md` and the example inventory,
   mechanically.
10. Adversarial audit: every major claim classified, cited, and checked
    against its governing SPEC and implementation (`audits/`).
11. Final integration pass; duplication removed; `index.md` updated once.

One writer owns the active documentation unit and its index rows. Parallel
writers are allowed only in separate worktrees, on disjoint guide files,
with one integrator owning `index.md`.

---

# 12. Ownership

- JC is sole owner and final authority; SPEC authorship is attributed to JC.
- Documentation work follows the maintainer's internal development rules and
  the repository's governing project authority.
- Generated documents are produced mechanically; the producing command is
  recorded in the file (§6).
- Tasks that change a public API carry the affected-guide update step
  (§9.5).

---

# 13. Invariants

**INV-309-0:** The union of `widget-catalog.md`,
`tabular-and-hierarchical-views.md`, and `plot-and-market-detail.md` must
cover every public widget module in `src/widgets/mod.rs` at the catalog's
verified revision. A public widget module absent from all of them means the
catalog is incomplete.

**INV-309-1:** Every code example in the docs must compile. No pseudocode,
no elisions in critical paths.

**INV-309-2:** Root `CHANGELOG.md` is updated with every commit that changes
behavior. Documentation-only commits are exempt.

**INV-309-3:** Docs are markdown. No HTML, no generated sites. AI agents
read markdown directly.

**INV-309-4:** Every document under `docs/guide/` carries the §6 metadata
block with an exact 40-character revision.

**INV-309-5:** Material claims carry the two-axis classification of §7; the
axes are never collapsed.

**INV-309-6:** Authority/implementation disagreements are recorded as
`contradictory` with both citations; prose never harmonizes them.

**INV-309-7:** `index.md` is the sole registry: every file under
`docs/guide/` has exactly one registry row, and no competing manifest
exists.

**INV-309-8:** Generated documents record their exact source revision and
generation method; they carry no authority beyond that revision.

---

# 14. Forbidden Behaviors

**FORBID-309-0:** No rustdoc HTML output as the primary guide corpus.
Rustdoc owns item-level API documentation beside the code (§2.5); the flat
markdown corpus is the primary connective documentation.

**FORBID-309-1:** No documentation that requires a build step to read. Docs
are plain text files in the repo.

**FORBID-309-2:** No "see X for details" deferral of critical information;
the current document must be self-sufficient for the reader's immediate
need. Cross-references are fine for depth.

**FORBID-309-3:** No document under `docs/guide/` without the §6 metadata
block.

**FORBID-309-4:** No silent advancement of a document's `halcyon-revision`.

**FORBID-309-5:** No draft linked as canonical reading from `index.md`.

**FORBID-309-6:** No blind migration of legacy prose into canonical guides.

**FORBID-309-7:** No collapse of the authority and implementation axes into
one status.

**FORBID-309-8:** No implementation claims inferred from SPECs, filenames,
comments, examples, or external surveys.

---

# 15. Consistency Matrix (SPEC-HAL-002A)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode Ownership | N/A — documentation, not code |
| B — Phase Separation | N/A |
| C — ID-based Identity | N/A |
| D — Pure Data | N/A |
| E — GPU-First | N/A |
| F — Sovereignty | N/A |

No permanent invariants affected. This SPEC governs documentation
infrastructure only. It is not a constitutional amendment: SPEC-HAL-002A
applies to changes to SPEC-HAL-002, which this SPEC does not touch.

---

# 16. Amendment Log

**2026-07-24 — Reconciliation and adoption** (owner decision, JC).
Reconciled the original Draft against live repository state at revision
`68216f25e43b532dbbd3cddec48c229cca913016`, halcyon `0.189.2-beta`:

- corrected the SPEC directory casing `docs/SPECs/` → `docs/specs/`;
- replaced the UPPERCASE flat guide inventory with the §3
  lowercase-with-hyphens structure;
- amended the absolute flat-directory rule: canonical guides stay flat,
  `generated/`, `audits/`, and `drafts/` are physically separated
  subdirectories;
- removed stale inventory claims (`docs/roadmap.md`, `docs/CHANGELOG.md`,
  widget/SPEC counts) and the stale `grip:symbols` tooling reference
  (superseded by the current internal verification workflow);
- replaced the stale actor-specific ownership table and corrected author
  attribution to JC;
- added: authority model (§2), document classes (§4), index contract (§5),
  metadata contract (§6), two-axis claim classification and citation forms
  (§7), revision discipline (§8), legacy replacement plan (§10), adoption
  sequence (§11), ownership (§12); extended invariants (INV-309-4…8) and
  forbidden behaviors (FORBID-309-3…8).

Status: Draft → Adopted. The guide corpus itself is under construction per
§11; per-document state is recorded in the `docs/guide/index.md` registry.

**2026-08-24 — Development authority consolidation (owner decision, JC).**
Updated §12 to identify one implementing-agent operating authority after
retiring the duplicate internal instruction file.

**2026-08-27 — Work Doctrine operational alignment (owner decision, JC).**
Classified internal development rules outside the guide classes, removed retired
role-file references, and aligned documentation ownership with the project entry.


**2026-11-23 — Public framework scope.**
Added §7.1 under the maintainer's documentation-cleanup mandate. Public contracts
must explain framework behavior independently of private consumers and execution
history. Exact API names, adopted requirements, original conformance obligations
and qualified evidence remain preserved. Historical packet retirement requires
complete integration of any incorporated normative scope.
