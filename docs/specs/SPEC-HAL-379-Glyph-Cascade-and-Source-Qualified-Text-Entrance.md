# SPEC-HAL-379: Glyph Cascade and Source-Qualified Text Entrance

**Author:** JC, with product intent recorded through delegated technical review, 2026-11-05.
**Status:** DRAFT / INACTIVE / NOT BINDING.
**Technical authority:** None newly adopted. No source S2, shader change, public API, implicit animation policy or native capability is authorized by this document.
**Implementation:** This exact capability is unimplemented/unverified; related existing features have only their respective scope.
**Owner:** Halcyon text shaping/retained paint, HAL021 Update timing and actual glyph atlas/compositor; consumer supplies source text and optional stylistic request
**Supreme law:** HAL002 Constitution, HAL002A amendment procedure, adopted HAL309 documentation law.
**Related sources:** HAL021, HAL047/048/050/066, HAL067 (Design), HAL068 (Design), HAL069 (Design), HAL104, HAL359/360; HAL363/364 remain Proposed

## Historical contract scope

- Objective: record the product-visible effect, exact separate ownership, existing source constraints and S1 decisions that remain open.
- Standing objective: source-qualified, native and responsive application presentation through Halcyon; not a static viewer, screenshot trick or infrastructure-for-infrastructure's-sake.
- Review authority: JC owns product goal and native visual acceptance; the delegated architectural reviewer makes explicit technical selections and independently verifies; implementation of selected S1 requires separate authorization.
- Adoption and implementation: draft is not a runtime authority; select and adopt exact technical S1 with code/owner evidence before issuing an implementation task. Existing HAL372 S2 continues independently.
- Governing and status: no change to HAL002, HAL002A, HAL021, HAL355, HAL357, HAL372, HAL068/069 or any Draft/Proposed text. Those contracts retain their actual stated statuses.
- Authority change: NONE. No implicit adoption, new renderer/consumer behavior or additional Scene writer.
- Existing implementation: HAL048/050/066 and HAL360 provide qualified text shaping, atlas, glyph quads and real compositor delivery. HAL067 SDF/MSDF and LCD paths are Design/Not Implemented; HAL068 static text shadows/outline/glow and HAL069 animated text are likewise Design/Not Implemented. They cannot be treated as implemented prerequisites or newly binding. No current SPEC names a source-safe per-glyph Glyph Cascade staged appearance.
- Owned scope: only this new Markdown file; no code, existing SPEC, assets, tests or foreign working changes.
- Invariants and prohibited cases: sections 3 and 4. Reuse existing source/target, Update, Timeline, GPU, text and layout owners rather than creating any competing machinery.
- Dependent boundary: The application owns real product evidence, physical meaning, user configuration and authoritative semantic updates; HAL owns appearance, projection, frame timing, routing, target and renderer mechanics.
- Verification now: file content, classification, source/authority separation and intact reference names. This is a document-only check, not Cargo/native proof.
- Test owner after adoption: actual nearest source/backend/current native App test paths, selected after S1 source inspection.
- Completion boundary: user-facing goal and safety/ownership intent with explicit unresolved technical design, not a prematurely executable implementation plan.
- Version: N/A, documentation-only inactive Draft with no public behavior effect.
- External/destructive effects: NONE.
- Out of scope: new text shaping engine, a mandatory SDF/MSDF implementation, rewriting HAL068/069 Drafts, per-character fake glyph assumptions, new global animation system

## 1. Intended user-visible outcome

Reveal a real shaped text run as a coherent sequence of visual appearances, optionally with source-qualified trailing highlight or glow, while keeping words readable and original text, caret, selection and hit mapping immediately correct. Text supports real complex shaping, combining marks, ligatures and bidirectional scripts rather than being split into arbitrary Unicode scalar glyph guesses.

## 2. Verified existing boundary

HAL048/050/066 and HAL360 provide qualified text shaping, atlas, glyph quads and real compositor delivery. HAL067 SDF/MSDF and LCD paths are Design/Not Implemented; HAL068 static text shadows/outline/glow and HAL069 animated text are likewise Design/Not Implemented. They cannot be treated as implemented prerequisites or newly binding. No current SPEC names a source-safe per-glyph Glyph Cascade staged appearance.

An existing animation or render program does not imply this source-qualified effect is supported. Do not reclassify drafted generalized presentation mechanisms or technical proposals as active simply to fill a gap.

## 3. Owner-correct data flow and required invariants

The consumer supplies text identity, accepted value and presentation request; the established shaping owner produces stable cluster/glyph source provenance and actual composed glyph geometry. A future admitted presentation maps a finite sequence to cluster-safe reveal order, evaluated through the sole HAL021 engine time and lowered into the existing retained text GPU pipeline. A reveal may affect only visual coverage/appearance, not text value, input eligibility, shaping geometry, baseline, selection, IME or accessibility text.

INV-379-TEXT: original string, grapheme clusters, shaped glyph source and bidirectional order preserved. INV-379-METRICS: HAL104 pixel-stable baseline and glyph positions unchanged. INV-379-LAYOUT: ephemeral shaping/layout and stable retained identity. INV-379-INPUT: caret/selection/IME/accessibility available as currently supported, no waiting for completion. INV-379-TIME: HAL021 only. INV-379-RESOURCE: bounded glyph/atlas, frame and invalidation work.

Unknown data, unqualified motion, unowned presentation transitions and failures must remain explicit. A declined effect must not invalidate a fully admitted ordinary source or silently redraw a stale previous scene.

## 4. Forbidden substitutions

FORBID-379-SPLIT-LIGATURE, FORBID-379-CODEPOINT-AS-GRAPHEME, FORBID-379-REGENERATE-ATLAS-PER-TICK, FORBID-379-LAYOUT-FROM-EFFECT, FORBID-379-SECOND-GLOBAL-CLOCK, FORBID-379-DELAY-TEXT-SEMANTICS, FORBID-379-IMPLY-HAL067-069-ADOPTION.

No second WidgetTree, renderer, frame clock, global effects registry, independently driven scheduler, shader with unverified source math, speculative assembly engine, or OS-controlled semantic operation. Input and visual presentation are separate from domain changes.

## 5. OPEN technical decision register: S1 cannot be inferred

Decide exact source cluster/run identity and visually staged order across bidi/ligatures; eligibility of reshaping/reflow/async fonts/selection/IME changes; original R8 grayscale path and whether any concrete shadow/glow needs separately accepted HAL068/HAL067 (not assumed); pixel coverage/blend and per-cluster interpolation path in HAL359/360; bounded glyph count/time/atlas resources; source/value replacement retarget/cancel; disabled/reduced-motion; lifetime and no-effect parity; accessibility text reading during reveal.

The delegated architectural reviewer must make and record these exact choices after inspecting current owner source. Implementation cannot decide between alternatives. The unresolved design is not an excuse to mark the effect implemented or invent missing behavior.

## 6. Required causal proof after a separately adopted S1

Real scripts combining Latin ligatures, combining accents, Cyrillic, Arabic or Hebrew bidi and emoji in actual supported fonts; ensure value metrics/layout/selection/IME unchanged, per-cluster reveal has true original source mapping and finite duration, retarget/resize/reflow and font replacement safe, real GPU text pixels and native text editing while appearance is active.

Actual backend pixels, native App/Screen effects, physical OS input and JC visual acceptance require their own demonstrated evidence, not a headless proxy or a successful render submission.

## 7. Status and adjacent contracts

This is the distinct Glyph Cascade product profile, not a second generic Text FX engine. HAL068 and HAL069 retain Design/Not Implemented. Technical S1 must decide precisely how this capability relies on or extends them without silently adopting their entire proposals.

**DRAFT / INACTIVE / NOT BINDING**, 2026-11-05. No executable S2 work begins from this document alone. A later S1 must explicitly resolve all relevant behavior and compatibility with current binding owners, within constitutional limits, before technical implementation.
