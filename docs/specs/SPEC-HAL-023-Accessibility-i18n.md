# SPEC-HAL-023: Application Localization

**Author:** JC
**Date:** 2026-10-14
**Status:** DRAFT / FOR JC REVIEW
**Implementation:** Not Implemented
**Backend / message format:** Undecided
**Scope:** Explicit application localization context, messages, regional formatting,
locale publication, retained text dependencies, and consumer ownership.
**Source baseline:** Halcyon 0.213.0-beta at
b303927f754976a1fad4a6f108b0c9e8b42bc12e.
**Authority:** JC's direction to begin with specifications and his accepted
framework/application ownership split. The detailed contract below is proposed, not adopted.

The SPEC ID and existing path are retained. Existing references to accessibility
in HAL-023 refer to the deferred material preserved in Appendix A. This revision
does not amend HAL-132 or HAL-125, adopt accessibility, or alter their owners.
HAL-034's localization reference still identifies this document; it does not
adopt this Draft or change HAL-034's scope.

## Pre-Flight

- Objective: a complete localization Draft in the existing HAL-023, with its original
  text preserved verbatim; no runtime implementation.
- Mandatory reading: full Work Doctrine triplet, PROJECT-ENTRY, Constitution/002A
  and advisory Design Intelligence retained at matching live hashes; full current
  HAL-023/007/020/048/309/354 read. HAL-132 C.2/F and HAL-034's localization
  exclusion read for existing-reference compatibility.
- Governing authority: JC's specification mandate and accepted ownership split;
  HAL-002 supreme, HAL-002A Permanent, HAL-309 Adopted, HAL-354 Binding with its
  unadopted 363/364 clauses excluded. HAL-048 retains its declared status and
  scoped typography contract. HAL-007 remains Draft and HAL-020 Design / Binding;
  neither supplies localization implementation authority.
- Authority change: rewrite this Draft only; no adoption, constitutional amendment,
  backend selection, or permission to implement its proposed requirements.
- Verified target: original Halcyon source at the recorded baseline.
- Owned file: only docs/specs/SPEC-HAL-023-Accessibility-i18n.md; no path rename,
  companion SPEC, guide, index, ENTRY, consumer, or release-metadata edits.
- Invariants: App-local explicit state, stable identity, retained tree ownership,
  queued external ingress, deferred publication, phase isolation, immutable logical
  output, ephemeral shaping, narrow invalidation, and preserved editing.
- Existing mechanisms: AppConfig/UiThreadConfig declarations, synchronized
  ingress, TextEngine/paragraph owner, and HAL-354 retained/update owners.
  Exact TextProvider and Locale symbol inspection returned not found; that is not
  a universal claim that the repository contains no language-related code.
- Forbidden: hidden global locale, parallel locale/shaper/renderer/input engines,
  a new message DSL or framework for every translation backend, inferred
  commit/cancel, stale fragments, and read-phase or cross-thread semantic mutation.
- Dependent boundaries: product catalogs and semantics stay with the application;
  text/font/layout/interaction owners retain their authority. HAL-034/132/125
  and unadopted HAL-363/364 are not changed or promoted.
- Test ownership: documentation only. Section 11 declares future functional
  obligations; no tests, fixtures, Rust paths, or executable test names are created.
- Verification: complete Grip read/hash/diff/status below; local byte comparison
  of Appendix A against the beforeimage, LF/final newline, real links and statuses,
  and confirmation that the owned diff is the only tracked change.
- Completeness: self-contained Draft, explicit unresolved decisions, original
  historical bytes, one reviewed owned diff and one ordinary local signed commit.
- SemVer/changelog: N/A; unadopted documentation only. Cargo manifests/lock,
  0.213.0-beta and CHANGELOG remain unchanged.
- Git/external effects: exact-path local commit under standing JC authority;
  commit is not adoption. No push, amend, reset, new branch/worktree, or signing
  configuration change.
- Out of scope: implementation, dependency changes, catalogs/translations,
  Options/PropertyEditor work, automatic translation, accessibility/input/bidi/font
  implementation, backend experiments, builds, tests, and native launches.

The document verification block, executed after the edit, is:

    ws halcyon
    read docs/specs/SPEC-HAL-023-Accessibility-i18n.md max-lines 1600
    read docs/specs/SPEC-HAL-023-Accessibility-i18n.md hash
    git diff path docs/specs/SPEC-HAL-023-Accessibility-i18n.md
    git status

Any paginated read is continued to its end. Verification results and the final
commit identity are reported separately; this block is not a claim that they
have already run.

### Halcyon Pre-Flight

- Governing status: Constitution/002A above all; this document Draft; existing
  HAL-048 text, HAL-354 retained, and HAL-309 documentation owners respected.
- Widget policy: no new widget behavior. Preservation obligations describe
  proposed localization, not a change to interaction policy.
- Interaction added: none; PointerDriven, Navigation/Capture and Escape unchanged.
- Applicable prohibitions: hidden globals, cross-phase mutation, persistent UI or
  shaping references, inferred commit/cancel, and stale retained reuse.
- Reuse: existing App context, queued ingress, deferred mutation, text/font/
  paragraph pipeline and retained dependencies; no second scheduler or binding graph.
- Demo/example compliance: no demo or native action. Appendix A's old interface
  sketches are historical quotations, not current APIs or runnable examples.

## 1. Authority, Scope, and Current Evidence

JC has accepted that reusable localization infrastructure belongs to Halcyon,
while each application owns its product translations and language choices.
Configuration and options UI topology remains an application decision; this SPEC
does not implement or redefine that UI.

All MUST/SHALL requirements in Sections 2-12 describe this proposed contract if
adopted. They do not upgrade this Draft, create APIs, select a library, or
authorize runtime work. The original combined Draft was also a Draft; its
preservation does not turn it into superseded adopted authority.

At the source baseline, the inspected AppConfig and UiThreadConfig have no
locale/provider declaration. Exact TextProvider and Locale symbols were not
found. TextEngine exists in src/core/text_engine.rs and remains the text owner.
These observations establish the inspected boundary, not an exhaustive absence
claim or a completed portability audit.

This SPEC does not include screen-reader bridges, accessibility trees, keyboard
navigation, an input mode, a bidi implementation, font discovery, or layout
mirroring. It describes their boundaries only where localized text depends on
the existing owners.

## 2. Explicit Context and Ownership

Each App that opts into localization SHALL have one explicit App-owned context.
Context is App-local, not a mutable process singleton or an implicit call to
setlocale. Independent opted-in Apps may select different contexts. Immutable
catalog and locale-data assets may be shared safely without sharing mutable
language selection.

Apps using ordinary literal text need not supply a locale, catalog or provider.
They do not initialize localization backend/data or change their existing behavior
merely because localization is available. No process/system locale is implicitly
selected for them.

The context distinguishes:

- requested message language/locale and the actually resolved message locale;
- regional formatting preferences and their actually resolved data;
- time zone used for applicable date/time presentation.

A product may deliberately initialize related defaults together, but one choice
MUST NOT silently determine the others. OS/browser language, region and time-zone
preferences are advisory inputs admitted through the existing owner, not
authority to switch a running App behind the user's choice.

Halcyon owns reusable resolution/formatting/publication mechanics and a namespace
for its own framework messages. Each application owns its product catalogs and
domain terminology, available languages, defaults, user selection, and persistence
policy. This SPEC chooses no default language, storage format or fixed
supported-locale list.

Message namespaces SHALL have explicit ownership and collision behavior.
Localizing a label MUST NOT replace a domain enum, ActionId, property identity,
widget ID, binding meaning, or imported fact. Framework catalogs do not acquire
application content merely because a framework widget displays it.

## 3. Message Identity, Arguments, and Formatting

A stable message ID identifies a message independently of its displayed text.
It is distinct from ActionId, widget/domain identity and translated labels;
changing language does not change a command or property target.

The proposed message contract supports value-typed arguments, grammatical
plural/select resolution, and number/date/time presentation with explicit
options. Concrete Rust signatures, module layout, macros, catalog syntax and
format-option enums remain undecided until the backend/message-format decision.

Numeric arguments SHALL retain the precision and meaning required by their
domain. Converting a number to display text before plural resolution or
silently narrowing it to an incompatible numeric type is invalid. Display
rounding is an explicit presentation choice, not mutation of the domain value.
Date/time inputs retain their declared temporal meaning; a civil date and an
instant are not silently interchanged by choosing a time zone.

Each resolved message SHALL identify its actual message locale, including when
message fallback is used. Language-dependent grammatical plural/ordinal selection
follows that actual message language, not the independently selected regional
number/date display locale or an untranslated requested locale. Non-language
selectors retain their explicit typed meaning. Backend-defined numeric options,
rounding and operand rules remain explicit; this contract does not replace them
with its own plural algorithm.

Regional number/date/time display uses the declared resolved formatting locale,
locale-data revision, options and, where applicable, time zone. A backend limitation
in separating message grammar from regional display MUST be reported rather than
silently merging the choices. This is a requirement for the eventual backend
assessment, not a claim that either candidate already implements it.

Unsupported argument types/options are explicit outcomes, not guesses.
Localization does not translate user-authored values, imported source strings or
key identity unless their product owner has separately declared that meaning.

Messages are data, not executable commands. They MUST NOT dispatch actions,
evaluate arbitrary code, allocate domain identity or mutate App/widget state.
Any backend formatting functions admitted later must have explicitly bounded
formatting semantics and no authority over product or UI behavior.

## 4. Negotiation, Fallback, Errors, and Capabilities

Requested and resolved settings SHALL be separately observable. Negotiation
uses a declared, finite, deterministic policy over actually supported resources.
It MUST NOT use ad hoc language-tag truncation or choose an arbitrary available
language. The selected standards/backend policy and fallback order require a
later recorded decision.

Fallback may be explicitly declared for missing messages or unavailable locale
data. Its use and actual source remain distinguishable from an exact match.
No available fallback is an explicit failure with product-selected presentation;
it is not fabricated success, an unexplained empty string, or an arbitrary
language choice.

The contract distinguishes at least:

| Outcome | Required treatment |
|---|---|
| Unsupported requested locale/capability | Report negotiation failure or an explicitly allowed resolved alternative. |
| Missing message | Apply only declared message fallback; otherwise report unresolved message. |
| Missing or wrong argument | Report the argument problem separately from a missing message. |
| Invalid catalog | Reject that candidate revision; do not publish a partially parsed resource set. |
| Unavailable formatting data/options | Report formatter capability failure or declared formatting fallback. |
| Stale/cancelled completion | Discard it without replacing the admitted context. |

Diagnostics retain message/request/resource identity and reason without gaining
semantic control. Product error display and development diagnostics are separate
from a successful formatted-message result. This Draft fixes no UI wording for
errors or hardcoded fallback language.

Catalog and formatting-data admission SHALL validate the capabilities required
by the declared target languages. Translation availability alone does not prove
the text owner's glyph, shaping, bidi or font support. A capability gap remains
explicit; i18n does not install a replacement text engine to conceal it.

A revision may intentionally contain messages resolved through a declared
fallback catalog. That is one coherent resolution snapshot, not permission to
mix resource generations or silently accept only part of a failed update.

## 5. Preparation, Requests, and Publication

Loading, parsing, validation and message formatting SHALL occur outside Layout,
Scene build and Paint. Required formatter/data preparation also precedes their
consumption. Existing lawful TextEngine shaping in its own phases is unchanged:
message formatting and glyph shaping are different operations.

Immutable catalogs, locale data and formed logical text may live between frames.
Widget/tree/context borrows, glyph runs, shaping references and paragraph/layout
borrows may not be retained across frames. Any cache uses stable owned value
keys under its existing bounded owner.

The lifecycle distinguishes the latest requested context, pending preparation,
and the currently admitted context, if any. Already-prepared or embedded resources
may be validated and published through the same lawful App owner without a worker,
asynchronous job or extra scheduling.

1. An explicit owner request identifies the App lifetime and a new request
   generation, desired settings and resource identities.
2. Preparation constructs and validates a complete candidate without mutating
   App state, widgets, the tree, navigation, focus or Capture.
3. When background preparation is used, completion enters through the existing
   synchronized pending ingress. For either preparation path, the lawful App/
   update/deferred owner checks App lifetime, latest request generation and
   candidate/resource identity.
4. Only a ready, valid, still-current candidate may establish the first context
   or replace the admitted context at the legal publication boundary. Outputs
   needed by affected consumers are resolved before Layout observes them.
5. Rejected preparation terminates that request with an explicit outcome.
   Cancelled, superseded or obsolete completions cannot publish. App teardown
   retires the request; a reused identity cannot receive its completion.

For a replacement, a newer request supersedes pending preparation, not the
current coherent display. A failed request does not silently relabel the old
admitted locale as the requested one. Until a replacement is ready, the old
coherent context remains active.

Before first admission, no old revision exists to retain. Initially required
localized content is pending/unavailable until a valid context is ready and
admitted. Readiness and explicit failure follow the existing startup/application
owner and declared product policy. An empty successful catalog or default language
MUST NOT be fabricated, and a failed request terminates with an explicit outcome
rather than hanging in pending. This is a localization boundary, not a new
visibility/readiness protocol.

Retry/cancellation/pending-status presentation belongs to the existing application
owner; no second scheduler or request framework is introduced.

After lawful publication, the first admitted successor frame SHALL use
consistent logical text, metrics, layout and scene inputs from that admitted
context. The request does not promise an immediate visible switch before resources
are ready and cannot force an extra frame outside engine cadence. Newly visible
consumers resolve under the same admitted revision before their Layout/Paint.

## 6. Retained Dependencies, Equality, and Bounds

Resolution/cache dependencies include message identity, effective locale choices,
catalog and locale-data revisions, argument values, formatting options and time
zone where relevant. Text consumers additionally retain their existing typography,
font, geometry, direction/isolation and other real presentation dependencies.

A changed dependency requires checking the effective result; it does not
automatically require a whole-tree rebuild or global cache flush. Equality means
the exact effective logical output and relevant text/typography/direction inputs
are unchanged, not merely the same visible-looking label. Dependency bookkeeping
still advances lawfully when output is equal.

Equal effective results SHALL avoid unnecessary shaping, layout, repaint and
rebuild. Actual changes invalidate only affected owners and necessary ancestor
composition under HAL-354. Text/typography changes that may alter geometry use
the existing LayoutAndPaint authority; known stable-geometry presentation changes
use the narrower existing class. Clean unrelated siblings remain reusable.

Locale publication MUST NOT recreate the WidgetTree to reset state, introduce
a second reactive provider/binding graph, or retain removed mount references.
Catalog/output caches have declared capacity and lifetime bounds and retire
obsolete generations when no bounded owner needs them.

No blanket O(1) guarantee is made for variable-length message formatting or
output. No blanket allocation prohibition is imposed on every frame-loop
operation. Future proof reports bounded preparation/resolution work, cache
retention, actual affected text/layout/scene work, and absence of backend work
for consumers that do not use localization. Paint does not load, parse or format
messages, and unchanged output does not cause repeated formatting work without
a changed dependency.

## 7. Editing, Identity, and Interaction Preservation

A locale publication SHALL preserve the current page, retained identities,
user-entered text, edit drafts, selection, caret and IME composition. It MUST NOT
commit, cancel, reparse or replace an edit merely because its surrounding labels
change.

The publication itself does not change focus or Capture, arm a new Capture owner,
restore released Capture, or reinterpret an input. Real user interaction,
interruption and teardown still follow the existing framework owners; this
SPEC neither bypasses them nor changes PointerDriven/Navigation/Capture/Escape.

In particular, a numeric/date editor's current parsing transaction does not
inherit a new parse locale from translated labels. Its owner preserves the
existing input meaning. Any explicit product change to parsing policy needs
its own declared transaction behavior; this SPEC chooses neither parser rules
nor product storage semantics.

Localized captions and validation explanations are projections. They do not
replace the committed value or modify a binding/action target. Equal labels
cannot merge identities or redirect an existing pending edit.

## 8. Text, Direction, and Rendering Boundary

The exact resolved logical text and typography inputs measured for layout SHALL
be the ones supplied to the existing TextEngine/paragraph/rendering path.
HAL-048 remains the text owner. No separate measure_text_localized shortcut,
second shaper, font resolver, layout solver or renderer is introduced.

Message-level directional isolation is separate from glyph coverage, complex
shaping, bidirectional ordering and layout mirroring. Unicode isolation controls
MUST NOT be silently stripped or changed between resolution, measurement and
drawing. Backend isolation choices and text-owner support must be explicit.

The ability to store a translation or expose Ltr/Rtl/Auto metadata does not
prove usable RTL, CJK or font fallback. Language admission requires evidence for
the actual text path; unsupported capability is reported truthfully. This Draft
does not adopt other text/font proposals or implement those capabilities.

## 9. Portable Semantics and Platform Boundary

Windows, macOS and Linux consumers target one declared localization semantics.
Future mobile/web consumers use that same contract; their mention is not a
claim of completed ports, FFI/WASM integration or native/browser proof.

A common catalog format alone does not prove identical Rust/JavaScript output.
Any later parity claim identifies message/runtime versions, locale-data versions,
formatting options, time-zone data where used, supported functions and real
target evidence. OS/JS Intl may be an explicitly selected implementation
resource later; it MUST NOT become a hidden policy source or implicit override
of the App context.

Resource packaging, data subset/footprint and loading strategy are explicit
backend/deployment decisions. They cannot require I/O in Paint or platform
callbacks mutating UI semantics. This unit supplies no packaging implementation.

## 10. Backend Decision and Dated External Evidence

No backend or message syntax is selected. Fluent with any necessary ICU4X
components, and Unicode MessageFormat 2 with a suitable implementation, remain
discussion candidates. This Draft prescribes no FTL/MF2 schema, macros or
concrete library classes. Backend-neutral requirements do not mean implementing
multiple backends or an abstraction framework for all translation libraries.

A later decision must record translator/tooling needs, message/formatting
coverage, error/fallback behavior, maintenance and license compatibility,
data footprint, target availability, and how claimed cross-target parity will
be verified. Standard conformance and maturity of a particular crate are
separate questions. These are unresolved decisions, not invented runtime defaults.

The following primary-source observations were checked on 2026-10-14.
Mutable main/latest links are dated evidence, not evergreen compatibility proof
or dependency selections:

| Source and bound observation | Limit |
|---|---|
| [fluent-rs bundle source](https://github.com/projectfluent/fluent-rs/blob/main/fluent-bundle/src/bundle.rs): add_builtins registers NUMBER and leaves DATETIME as TODO in the observed main. | No complete native date/time formatting claim or ready Fluent/ICU4X integration. |
| [fluent-rs custom_formatter example](https://github.com/projectfluent/fluent-rs/blob/main/fluent-bundle/examples/custom_formatter.rs): its default number example explicitly describes non-locale-aware output before a custom formatter. | Hook availability is not locale-formatting coverage or Rust/JS parity. |
| [ICU4X icu 2.3.1 documentation](https://docs.rs/icu/2.3.1/icu/): locale-driven components, formatters and data providers. | Version-pinned documentation; not an application message catalog or an integration result. |
| [Unicode LDML MessageFormat](https://unicode.org/reports/tr35/tr35-messageFormat.html): message syntax, data model and processing/conformance contract. | The report URL is mutable; the standard does not certify a crate's maturity. |
| [Rust mf2](https://docs.rs/mf2/latest/mf2/), [mf2-runtime](https://docs.rs/mf2-runtime/latest/mf2_runtime/) and the [Rust MF2 book](https://evancarroll.github.io/rust-mf2/): Rust implementation documentation exists; the observed docs.rs pages identify 3.0.0. | Latest/book links are mutable. Existence is not our maturity, platform-support or parity acceptance. |

Only the ICU4X documentation above is release-pinned here. Before a backend
decision, mutable evidence must be re-bound to the selected release/source and
actual target requirements. No license, maintenance, platform or parity
conclusion is implied by these links; no experiment or benchmark ran in this unit.

## 11. Future Functional Acceptance Obligations

Implementation acceptance, if separately authorized, requires focused behavior
through the normal public application/text/update path:

1. Literal-text Apps retain existing behavior without localization backend/data;
   two opted-in App contexts use different locales while safely sharing immutable
   assets, and switching one does not change the other.
2. Value-typed arguments, plurals/selectors and formatting options produce correct
   results, including different message/regional locales and message fallback;
   grammar follows the actual message language, and fallback/errors remain
   observable without executing actions.
3. Already-prepared resources admit without a mandatory worker; initial readiness/
   failure is explicit, while invalid replacement preparation and stale completion
   preserve the prior coherent revision. A valid current publication occurs once.
4. Switching around a real editor preserves identity, draft, selection/caret/IME
   and input meaning without an inferred commit/cancel or restored Capture.
5. The first admitted successor uses the exact resolved logical text and matching
   metrics/scene; equal effective output avoids extra work and unrelated owners
   remain reusable.
6. Once a backend is chosen, representative declared target fixtures cover
   language/data/options and claimed desktop or web/mobile parity. Resource and
   cache bounds are reported separately from source or native visual proof.

These are contract obligations, not new test names, Rust fixtures, aesthetic
snapshots, executed tests, build results or implementation-completion claims.

## 12. Constitution and Existing-Owner Compliance

The proposed implementation must satisfy the following unchanged owners:

| Rule / owner | Localization boundary |
|---|---|
| HAL-002 Pillars A/C; sections 3.1, 3.4, 4.6 and 4.9 | Tree owns widgets; stable IDs survive publication; no retained widget references or reset-by-rebuild. |
| HAL-002 Pillars B/D/E; sections 3.2, 3.6, 3.8, 4.2, 4.7, 4.10 and 5 | Deferred state publication; no load/format or semantic mutation in Layout/Scene/Paint; immutable scene inputs. |
| HAL-002 section 3.3 | Workers return data through synchronized pending ingress, never publish widget/App state directly. |
| HAL-002 section 3.10 | Locale is explicit App context, never a hidden process-global mutable setting. |
| HAL-002 section 3.11 and 4.11 | Caption changes cannot reinterpret user intent or infer commit/cancel. |
| HAL-002 sections 3.9, 4.8 and 7; HAL-048 sections 2-3 | Owned logical strings may persist; ephemeral shaping/layout references may not; existing TextEngine remains the sole shaping source. |
| HAL-002 Pillar F; HAL-354 sections 1-6 | Advisory platform ingress, engine cadence, narrow invalidation and consistent successor scene; no second frame scheduler. |
| HAL-309 sections 2/7 and INV-309-2/5 | Draft intent and implementation evidence remain distinct; this documentation commit is not adoption or a behavioral release. |

HAL-354's proposed 363/364 amendments remain unadopted. HAL-048's unrelated
Draft/Proposed dependencies are not promoted. This table does not rewrite
contradictions elsewhere or amend the Constitution. Runtime implementation
requires a later explicit task and the then-current accepted contract.

## 13. Disposition of Original Sections 1-11

The original 2025-11-17 document remains quoted in Appendix A. This table
describes how this new Draft treats its content; neither version is adopted
merely by being present.

| Original section | Disposition |
|---|---|
| 1. Philosophy | Retain optional explicit integration; replace trading-only and indefinite-deferral framing. |
| 2. What This SPEC Does NOT Do | Retain no automatic translation; replace hook-only scope with this proposal; accessibility/bidi implementation remains deferred. |
| 3. Architectural Hooks | Deferred historical accessibility material, outside this contract. |
| 4. Keyboard-Only Operability | Deferred historical material; not input/focus authority or an amendment to current interaction owners. |
| 5. Localization Minimal Hooks | Retain App-supplied resources and explicit switching; replace the unimplemented get(key) sketch and unconditional rebuild instruction. |
| 6. Text Measurement | Retain identical resolved text for measurement/drawing; incorrect HAL-007 reference replaced by the existing HAL-048/TextEngine boundary, without capability promises. |
| 7. Optional RTL Hooks | Retain explicit direction boundary; historical enum sketch does not implement bidi, glyph support or mirroring. |
| 8. Settings Integration | Retain explicit choice; replace invented storage/automatic HAL-020 profile claim with consumer-owned persistence. |
| 9. Performance Requirements | Retain bounded optional work; replace blanket O(1)/allocation claims; accessibility-thread proposal remains historical. |
| 10. Implementation Checklist | Historical unexecuted proposal, not this unit's implementation plan or permission. |
| 11. Result | Historical assertion, not evidence that the sketched interfaces exist. |

## Appendix A: Original Draft Verbatim

**Original date/status:** 2025-11-17 / Draft.
**Beforeimage:** 5459 UTF-8 bytes, 287 LF-terminated lines.
**SHA-256:** 5535758a01bf93674fd14c04c29c8ed954a8a3a9b3761cb86c08dbf64fa31672.

The complete original bytes follow inside a four-backtick Markdown quotation.
They are non-normative historical/deferred material, including the accessibility
proposals and incorrect references. Its sketched interfaces are not current APIs;
its implementation checklist and closing claims do not prove implementation.
The quoted content is not a runnable example and grants no new authority.

````markdown
**SPEC-HAL-023: Halcyon — Accessibility & Localization Layer (Optional but Future-Proof)**
**Author:** JC
**Date:** 2025-11-17
**Status:** Draft
**Scope:** Accessibility tree, keyboard-only operability, localization hooks, text measurement for multi-locale UI, optional but structured.
**Purpose:** Provide a minimal blueprint so Halcyon can *eventually* support accessibility and localization without architectural disruption — but **without** implementing any of this now.

This SPEC exists so future work does not require redesigning core systems.

---

# 1. Philosophy

Accessibility and localization are **non-core** for your trading stack today.
But if Halcyon becomes a long-lived personal UI framework, it must remain *extendable* toward:

* accessible tree representation
* screen reader hooks
* full keyboard navigation
* internationalized text layout

The goal is to define **hooks and structure**, not full implementations.

Halcyon must not get bloated.

---

# 2. What This SPEC Does NOT Do

No screen reader bridges.
No ARIA tree.
No automatic translations.
No bidi layout.
No global i18n framework.

We define interfaces so that **if you decide later**, nothing breaks.

---

# 3. Architectural Hooks (Zero Cost Until Used)

### 3.1 AccessibilityNode (Optional per-widget metadata)

Every widget may optionally expose an accessibility node:

```rust
pub struct AccessibilityNode {
    pub role: AccessibilityRole,
    pub label: Option<String>,
    pub description: Option<String>,
    pub value: Option<String>,
    pub bounds: (f32, f32, f32, f32),
    pub focusable: bool,
}
```

Role enum:

```rust
pub enum AccessibilityRole {
    Container,
    Button,
    Text,
    TextField,
    Slider,
    List,
    Custom,
}
```

Widget trait (strictly **optional**, default no-op):

```rust
fn accessibility(&self) -> Option<AccessibilityNode> {
    None
}
```

### 3.2 Accessibility Tree

At frame-build time:

```
WidgetTree → AccessibilityTree (if enabled)
```

Stored separately, off the hot path.

Halcyon only generates this if `settings.accessibility.enabled = true`.

---

# 4. Keyboard-Only Operability

Halcyon already supports:

* focus manager
* tab order
* key event bubbling

Accessibility requires one more abstraction:

### 4.1 Focus Order Metadata

Each widget can optionally specify:

```rust
fn focus_priority(&self) -> Option<u32> {
    None
}
```

Halcyon uses these to compute stable tab order.

If not specified, default ordering = widget tree order.

### 4.2 Keyboard Activation Hook

Widgets must implement:

```rust
fn activate_via_keyboard(&mut self) { /* default empty */ }
```

This enables:

* Space key activates Button
* Enter triggers default action
* ESC closes modal, etc.

---

# 5. Localization (i18n) Minimal Hooks

No global localization system is implemented.
But Halcyon must allow **user-supplied text providers**.

### 5.1 TextProvider Trait

```rust
pub trait TextProvider {
    fn get(&self, key: &str) -> &str;
}
```

App implements:

```rust
struct MyTextProvider { /* language maps */ }
```

Halcyon runtime stores:

```rust
runtime.text_provider: Box<dyn TextProvider>
```

Widgets ask for:

```rust
label(text_provider.get("login.button"))
```

### 5.2 Locale Switching

If locale changes:

```
runtime.text_provider = new_locale_provider;
rebuild UI;
```

No magic. No runtime patching.

---

# 6. Text Measurement for Locales

Halcyon’s text system (SPEC-HAL-007) must expose:

```rust
fn measure_text_localized(
    &self,
    content: &str,
    locale: &Locale
) -> TextMetrics;
```

Even if initially identical to normal text measurement, this hook ensures proper:

* CJK width
* RTL shaping
* ligatures
* fallback fonts

without needing to redesign layout later.

---

# 7. Optional RTL Hooks

RTL is not implemented, but we reserve the flags:

```rust
pub enum TextDirection {
    Ltr,
    Rtl,
    Auto,
}
```

Widget styles may contain:

```rust
text_direction: TextDirection,
```

This is harmless now but essential later if needed.

---

# 8. Settings Integration

In `settings.json`:

```json
{
  "accessibility": {
    "enabled": false,
    "high_contrast": false
  },
  "locale": "en-US"
}
```

Profile system from SPEC-HAL-020 handles this automatically.

---

# 9. Performance Requirements

* Accessibility tree generated **off main thread**
* Localization lookup must be O(1)
* Zero cost for apps not using accessibility/localization
* No allocations inside frame loop

All implemented lazily.

---

# 10. Implementation Checklist

### Phase 1 (Stub)

* [ ] Add `AccessibilityNode` struct
* [ ] Add optional `Widget::accessibility()`
* [ ] Add `TextProvider` trait
* [ ] Add `Locale` struct (lightweight)
* [ ] Add settings entries for `accessibility.enabled` and `locale`

### Phase 2 (Enable Localized Labels)

* [ ] Wire `TextProvider` into runtime
* [ ] Widgets use text provider when label text is a key

### Phase 3 (Focus/Keyboard Enhancements)

* [ ] Add `focus_priority()`
* [ ] Add `activate_via_keyboard()`

### Phase 4 (Optional Future)

* [ ] Build accessibility tree
* [ ] Export tree through platform API
* [ ] Basic screen-reader support (only if needed)

---

# 11. Result

This SPEC ensures that **if someday you want Halcyon to have accessibility and localization**, the architecture won’t block you.

But it does not burden you now.
The system stays lightweight, fast, and code-first.

You have the hooks.
You don’t have the baggage.
````
