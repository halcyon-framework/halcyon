# SPEC-HAL-390: Native Read-Only Markdown Viewer

**Author:** JC **Product authority:** JC.
**Date:** 2026-11-13.
**Status:** ADOPTED / ACTIVE within section 14; sections 1-13 retain historical proposal context.
**Owner:** Halcyon document presentation and existing text, layout and input owners.
**Originating use case:** Render application-supplied response text as a native
read-only document. Framework delivery and application integration have separate
acceptance boundaries.

## 1. Purpose, mandate and limits

JC requested a native Markdown viewer independently of the MissingGlyph repair
and subsequently directed continued design work after reviewing that repair.
This historical proposal does not activate the entire Draft document/editor
corpus. Its original complete viewing contract required scoped adoption before
implementation; section 14 records that later adoption and its precedence.

The human result is readable native answers and supplied documents: actual
headings, paragraphs, emphasis, lists, quotations, code and tables rather than
raw Markdown delimiters in one uniform TextBlock. The component is reusable
across applications. It is a reader, not an editor, browser, model runtime,
universal document store, resource fetcher or independent rendering engine.

The font repair remains separate accepted evidence. HAL048/049 supply actual
registered face bindings; HAL360/361 distinguish Complete, renderable Degraded
and Unavailable. A Markdown document must inherit those mechanisms unchanged.
This design progression did not close the then-open native three-request
consumer acceptance boundary.

## 2. Pre-flight and status of existing mechanisms

Read authority and source before preparing this candidate:
- PROJECT-ENTRY; supreme HAL002 and permanent appendix HAL002A.
- Scoped accepted HAL048 typography and S5 additions, HAL049 sections 15-16,
  HAL360/361 systemic missing-cluster clauses and binding HAL354.
- HAL315 read-only selection is Draft as a whole. Its actual selection substrate
  and TextBlock use were inspected; this candidate does not adopt the whole Draft.
- HAL060/061 describe Draft editable rich text and rich clipboard directions,
  not an implemented Markdown reader. They are not prerequisites to this reader.
- HAL309 separates a specification from a usage guide and implementation proof.
- Design Intelligence was used as advisory composition guidance, not authority.

Verified starting points:
- No Markdown-specific source module or specification was found in the inspected
  Halcyon source/specification inventory on 2026-11-13.
- Paragraph owns a String and styled Span ranges. TextStyle currently supports
  family, size, weight, italic, color and cluster gap. Do not invent underline or
  background fields from the Draft RichTextField examples.
- ParagraphLayoutEngine::try_layout_with_catalog already returns the existing
  Lines with actual bound face runs. TextBlock's S5 implementation preserves
  those run bindings through measurement, selection and paint.
- ReadOnlyTextSelection and TextSelectionPolicy already provide bounded
  selection/copy state, policy checks and routed gestures.
- ScrollArea owns scroll translation, extent and scrollbars. A document reader
  must not introduce a second scroll scheduler or intercept native input directly.
- Existing TextBlock accepts plain text and is not a rich-span document renderer.
  Keep its public behavior; do not add Markdown interpretation to TextBlock::new.
- Cargo.toml currently has no Markdown parser dependency.

Widget policy proposed here is READ-ONLY DOCUMENT with explicitly enabled
selection/copy. It admits no text insertion, deletion, task-checkbox mutation,
IME editing, undo system or execution of document content. No existing
display-only widget gains new interaction implicitly.

## 3. Responsibilities and the one production path

The proposed path is:

    exact supplied UTF-8 source
      -> bounded pure Markdown parse
      -> immutable source-bearing document and reading projection
      -> MarkdownView layout through the existing Paragraph owner
      -> existing bound text/geometry scene emission
      -> ordinary Build/Compile/Execute and HAL360/361 outcomes

Ownership stays separate:
- The caller owns where source came from, its lifetime, replacement decisions,
  permitted recipients, storage and any external-resource access.
- MarkdownDocument owns one immutable input plus its verified parsed structure.
  It contains no widget, native font handle, GPU object or model receipt.
- MarkdownView owns only presentation/selection state for that supplied document.
  WidgetTree remains the sole owner of widget instances.
- Paragraph, font catalog, TextEngine and renderer retain their current owners.
- The caller's existing worker/lifecycle may prepare a document off the UI thread.
  The viewer does not create a hidden thread, global runtime or parse service.

There is no HTML intermediate, WebView, JavaScript evaluator, CSS layout system,
alternate glyph renderer, parallel FontManager or application-specific painter.

## 4. Proposed syntax profile and parser boundary

The proposed syntax baseline is CommonMark 0.31.2 plus exactly tables,
strikethrough and task-list markers. This is a named native viewing profile,
not a claim of complete GitHub product rendering.

The proposed parser is pulldown-cmark 0.13.4, consumed as Rust events plus source
ranges through into_offset_iter. The package/version/features and direct API
must receive the bounded dependency/source qualification in section 12 before
adoption. Do not select another parser or upgrade unrelated dependencies in
the executor. The parser's HTML output module is not part of the rendering path.

Only ENABLE_TABLES, ENABLE_STRIKETHROUGH and ENABLE_TASKLISTS are proposed.
Do not use Options::all, ENABLE_GFM as a supposed blanket profile, smart
punctuation, heading attributes, executable extensions, front matter, math,
footnotes, wiki links or syntax highlighting by implication. Additional syntax
remains literal according to the selected parser/profile.

Render these source meanings explicitly:
- Paragraphs, ATX/Setext headings, thematic breaks, block quotes and nested lists.
- Strong/emphasis/strike, inline code, soft/hard breaks and fenced/indented code.
- Table header/body cells and their declared alignments.
- Task markers as read-only geometric status marks, not interactive Checkboxes.
- Link labels as text with preserved destination metadata. S1 grants no link
  activation, external open or resource operation.
- Image syntax as a clearly non-loaded image placeholder containing its parsed
  alternative text. Keep its destination in the document, but perform no load.
- Raw HTML as inert literal source text, never interpreted as tags or styling.

An unclosed code fence or other unfinished model answer is parsed as the
selected grammar specifies. It is not automatically a parser error, and the
viewer must not invent closing syntax or silently complete source text.

Language labels on code fences are informational. They do not authorize a
language server, external tool, code execution or unselected highlighter.

## 5. Exact source versus reading projection

Source and presentation have different representations and must never be
confused. Preserve the input Arc<str> byte-for-byte, including original newline
forms, escapes, delimiters, spelling and normalization. Parsing must not write
back into it.

The immutable document contains:
- the exact supplied source;
- structured block/inline records with document-local numeric identities;
- actual parser source ranges and nesting, checked within that source;
- a deterministic decoded reading-text projection;
- a mapping between each reading segment and its source range(s);
- metadata needed for presentation, including list starts, table alignment,
  code fence information, link destinations and image alternative text.

These are document values, not UI-object references or a durable knowledge graph.
An identity is qualified by this exact document instance/revision; the same
numeric node index in another document is not the same node.

Parser event offsets are provenance ranges, NOT an assumed one-byte-to-one-byte
mapping. Entity decoding, escaped punctuation, code-span whitespace and removed
syntax can change length. Distinguish exact-copy segments from decoded segments
and generated structural separators. A decoded segment may map to a larger
source interval; never derive its raw byte position by adding a display offset.

Reading projection rules:
- Use decoded parser text for ordinary inline content.
- A soft break becomes one space; a hard break becomes LF.
- Separate adjacent top-level blocks by two LFs, without a fabricated trailing
  block separator. Preserve line endings within the parsed code literal.
- List items retain a deterministic textual list marker in the reading projection;
  use the parser's actual ordered-list start and item order.
- Use LF between table rows and TAB between cells. Do not put table border art
  into copied reading text.
- A task marker contributes its explicit [ ] or [x] state to reading copy.
- A thematic break contributes a block boundary, not decorative glyph content.
- Image placeholder alternative text participates in reading copy, with its
  non-loaded status explicitly represented. Do not invent image content.
- Raw HTML contributes its literal source representation.

CommonMark-defined decoding is a presentation transformation, not permission
to replace unsupported Unicode. Record transformations in the source mapping.
In particular, parser handling of U+0000 does not erase the original source.

Two copy meanings remain distinct:
1. Ordinary selection copy returns the selected reading-text range, with no
   soft-wrap line breaks inserted by layout.
2. Copy-source returns the exact supplied Markdown source or an explicitly
   selected source range. It is never reconstructed from the rendered tree.

Code literal copy is the parsed code content, not an assertion that stripping
fences/indentation preserved every raw source byte. Exact raw source remains
separately accessible. Syntax coloring, language names and soft-wrap indicators
never contaminate copied code.

## 6. Immutable preparation and revision lifetime

Proposed public responsibilities, to be pinned to exact signatures at adoption:
- MarkdownDocument parsing accepts owned immutable UTF-8 source and explicit
  parse limits, returning an immutable document or a typed preparation error.
- Accessors expose exact source and reading projection without mutation.
- MarkdownView is constructed from an Arc<MarkdownDocument>, a read-only
  selection policy and explicit/default appearance values.
- The document may be prepared on a caller-owned worker. All data handed to UI
  is immutable and Send-compatible; font/native handles stay out of that result.
- No source parsing occurs from layout, paint, pointer movement, renderer Compile
  or every frame. Source-equivalent reuse does not reparse the document.

A preparation request and result carry the caller's actual revision identity.
UI adoption checks it at the legal update boundary. Stale results are discarded,
not installed into a newer answer/document. The generic viewer does not invent
a global revision allocator.

This first profile supplies an immutable document to a view. Replacing it is
an explicit caller operation through existing lawful widget/update mechanisms.
A different document resets selection; identical document identity across
resize/theme/DPR changes preserves selection offsets. No implicit content diff,
cross-document selection migration or append-only streaming contract is claimed.

Proposed parse limits are explicit caller policy: maximum source bytes, node
count and nesting depth, checked with typed outcomes. Select exact defaults and
consumer values before implementation; no silent truncation is permitted.
Representation uses checked indices/arithmetic and bounded iteration, with no
application recursion proportional to untrusted nesting.

Preparation failures must not be presented as model failures or renderer
success. Preserve source and the actual error. The caller may display the
original text through its existing plain-text view with an explicit
Markdown-preparation error; it must not silently claim formatted success.
This is presentation fallback, not a second renderer or glyph-recovery policy.

## 7. Layout, typography and visual structure

Use existing Paragraph/Span/TextStyle and admitted catalog-based shaping.
Inline formatting creates styles over decoded reading ranges; every Paragraph's
spans must cover its text exactly without gaps, overlap or invalid byte edges.
Do not shape each scalar independently or split a shaped source cluster.

The viewer lays out block content on the UI layout phase using a finite content
width where supplied. Paint consumes the resulting exact styles, face/direction
bindings and positions. It must not join differently bound spans and reshape
the whole line with the default font.

Proposed presentation:
- Headings use a coherent relative size/weight hierarchy derived from the
  supplied base typography, with spacing before/after and no decorative glow.
- Lists use indentation and hanging item alignment. Numbering follows parsed
  starts; task indicators are geometry, not Unicode font assumptions.
- Quotes use a restrained rule and inset through existing geometry emission.
- Inline and block code request an actually admitted monospace face and use
  existing theme surface/foreground roles. No synthetic bold/italic or assumed
  font asset is introduced. Missing face availability must be explicit at
  design qualification, not silently solved by new OS font discovery.
- Tables have native row/cell layout, alignment and padding, not a screenshot
  or one line of monospaced pipe characters.
- Strike/underline or code-background decorations use existing paint geometry
  under the same clip/order; do not pretend unsupported TextStyle fields exist.
- Images remain the inert non-loaded presentation in section 4.

Resolve the exact available bold/italic/monospace assets and public typography
operations before adopting a visual promise. A requested weight and an actually
resolved weight must not be conflated. Do not silently replace the consumer's
default SystemUI face for every control.

The scroll owner remains ScrollArea. Select the exact existing composition
contract for the viewport and wide code/tables at section 12; do not add a
second scrolling/input model. The intended behavior is prose reflow, preserved
code indentation, reachable wide content and no clipping disguised as a complete
document. A logical code line and a visual wrapped line remain distinct.

All geometric/font caches follow existing stable-key invalidation: source
revision, width, resolved style, admitted catalog identity and scale. No cached
reference to another widget, SceneGraph node, borrowed glyph or native object.
Unchanged documents need no repeated parse or layout solely because time passed.
Visible rendering uses current clipping; no unverified virtualization guarantee
or new tabular data owner is implied by this first reader.

## 8. Read-only interaction and safe content

Reuse ReadOnlyTextSelection with one logical reading-text range across the
document. This is one widget's multi-run selection, not a cross-widget global
selection coordinator. Capture/focus remain Halcyon-owned under existing input
rules; OS activation is not semantic permission.

Hit positions and selection backgrounds use actual bound-font cluster geometry.
Maintain ordered anchor/head and cluster-safe boundaries across formatting,
paragraphs, code and table cells. Copy uses the logical reading order even when
visual direction differs. Copy never includes a renderer missing-glyph marker
instead of the original selected text.

Explicit selection policy controls interactivity. No widget becomes selectable
because the content happens to resemble Markdown. Preserve routed copy/select-all,
pointer drag and capture-leave behavior through the existing context/clipboard
owner. Do not add text editing, pasting, cut, IME or undo behavior.

Link destinations and image paths are inert document data in S1, including
file:, javascript:, data:, custom schemes and relative paths. No hover, parsing,
layout, image placeholder or copying triggers a network/OS/tool operation.
A later explicit link/image capability needs a separately adopted intent and
recipient/resource owner contract.

The viewer owns no credentials, file scanner, browser history, model prompt,
source permissions or retained memory. A document written by a model has no
extra authority compared with any other supplied document.

## 9. Rendering safety and framework invariants

Render through the normal retained scene, admitted font bindings and existing
renderer. No custom glyph atlas, renderer escape hatch or hidden completeness
override is allowed.

INV-MD-SOURCE: original source bytes remain immutable and recoverable.
INV-MD-REVISION: document, selection and source ranges belong to one exact
document revision; a stale parse cannot replace another document.
INV-MD-PHASE: parsing and application publication do not mutate UI during
layout/paint or from a background thread.
INV-MD-BINDING: measurement, hit-testing, selection and paint use the same actual
face/direction binding and source-cluster geometry.
INV-MD-PASSIVE: reading and rendering never execute links, images, HTML or code.
INV-MD-OUTCOME: parsed content, laid-out content, prepared text, produced output
and observed pixels remain different evidence boundaries.

HAL360 Degraded markers apply normally to unsupported clusters and do not
modify source, selection or copy. A real structural/raster/resource Unavailable
is still a refusal. HAL361 bounded first-fault reporting stays active. Never
claim Complete because a document parser or a UI-loop fixture succeeded.

No new timer, repeated redraw, debug-only working path, observer requirement,
source-bearing event spam, hidden global or renderer-owned document state.

## 10. First consumer boundary

A consuming application may use this component for a derived visible response.
Its original request, backend identity, token/stop metadata, pending state and
errors remain separate semantic content. They are not parsed as
response-authored Markdown.

Neither this document nor source-only qualification changes backend choice,
resident lifetime, token limit, source disclosure, tool access, retained memory,
conversation history, screen navigation or shutdown. Other callers can produce
a Markdown document without importing the originating application or backend.

## 11. Required proof before completion

Parser/document proof:
- Actual event/range parsing for every selected construct, nesting, escaped
  syntax, entities, CRLF, code whitespace, incomplete fences and empty input.
- Byte-identical source retrieval; explicit decoded/range relationships.
- Reading-copy rules across blocks/cells, not fake per-byte source offsets.
- Limit refusal before silent truncation; stale-revision rejection.
- HTML, image paths and hostile-looking link destinations have no effects.

Native owner/renderer proof:
- Actual Paragraph/registered-face path for mixed formatting, Latin/Cyrillic/CJK,
  RTL, combining clusters and uncovered-cluster Degraded containment.
- Read-only selection/copy crosses styled spans and blocks with exact source
  preservation; resize and DPR changes retain correct current geometry.
- Real Metal 1x/2x output for headings, code, nested lists and tables; clipping,
  scrolling, foreground/background ordering and visible marker pixels.
- Same document under unchanged inputs does not reparse on paint or input.
- Original unrelated TextBlock, selection, typography and font behavior remains.

Consumer proof:
- An ordinary application displays a real backend response through the same
  public viewer, with request/metadata separate and the original raw response
  untouched.
- Code and selected readable text can actually be copied; Unicode remains
  renderable or explicitly Degraded without freezing healthy controls.
- Cancellation, a new request and a stale presentation result retain original
  request identity and do not show an earlier response as a later one.
- Real native window observation is separate from scene/Compile/test evidence.

Use existing test modules/harnesses and nearest consumer tests; a showcase is
not regression proof. Preserve accepted unchanged S5 evidence rather than
repeating the whole investigation. No claimed test result exists from this Draft.

## 12. Exact design-adoption gates and next bounded review

Before selecting implementation, the historical proposal required resolution
of these source/contract facts; factual investigation alone did not select
replacement architecture:
1. Validate the proposed parser package 0.13.4, feature surface, offset semantics,
   dependency/licensing compatibility and whether the selected package is
   available through the existing permitted dependency process.
2. Pin the minimal public document/view API and typed errors against actual
   Widget/UpdateContext and selection APIs; define caller-visible parse limits.
3. Select the admitted bold/italic/monospace asset/typography path without
   font downloads, redistribution or hidden discovery.
4. Qualify the existing ScrollArea finite-width/wide-content composition and
   source/layout/copy mapping; select one coherent overflow policy.
5. Qualify the consumer's existing off-UI completion lifetime for CPU-only
   document preparation and matching-nonce UI adoption, without changing the
   original backend result.

These gates are bounded design inputs. They do not reopen the resolved
MissingGlyph root cause or authorize another model/native-input experiment.
No architecture choice is delegated merely because a source seam is absent.

Proposed future footprint: one native Markdown document/view module with nearest
tests; mechanically necessary exports, parser dependency and scoped owner
amendments only after adoption. Existing text, scrolling and rendering owners
remain the reusable substrate. Do not scaffold an unused engine while the
first consumer remains unconnected.

## 13. Source and external reference ledger

Current local source witnesses:
- src/core/paragraph.rs:112-259,415-422; SHA256
  fc64cc2361b29e3d1e129a1efd63904e33d5f7f802c2ae06697a5d44b04f8da1.
- src/widgets/text_block.rs:176-238,248-632; SHA256
  592991f69da4eff7c20955587b518860076ce997b7062c02eeeee197e302e041.
- src/core/text_selection.rs:28-44,87-302: existing policy/state owner.
- src/widgets/scroll_area.rs:139-395: current scroll ownership and wiring.
- Cargo.toml:12-60; SHA256
  427a4a0c01446ebed1c34d5d210c92acc8e76e15d9bee6de12427c89204ec34d.
- HAL315 SHA256 40d222aafeb248072f144231c9a4ae2fe523370c3aa73627133c76c92f26a8f7;
  Draft status is not silently promoted.
- HAL354 SHA256 6d3e0f2041d2af1aab92a0216c37afff481cb6b0012f0469b7ed25b36c288c82;
  existing binding retained-update rules remain governing.

External primary references inspected on 2026-11-13:
- CommonMark 0.31.2: https://spec.commonmark.org/0.31.2/
- pulldown-cmark guide and event/source-range API:
  https://pulldown-cmark.github.io/pulldown-cmark/
- Versioned options reference:
  https://docs.rs/pulldown-cmark/0.13.4/pulldown_cmark/struct.Options.html
- GitHub Flavored Markdown extension definitions: https://github.github.io/gfm/

External syntax/API references are design inputs, not an alternative to local
Halcyon authority or evidence that any viewer is already implemented.

## 14. Adopted native viewer and first-consumer unit - 2026-11-13

**Status: ADOPTED / ACTIVE / BINDING for this section and the requirements it
selects below.** This scoped adoption follows JC's continuing Markdown design
mandate. Sections 1-13 preserve the original proposal and factual ledger;
their no-implementation, proposed and unresolved gate wording is historical
for this exact selected scope. This section resolves section 12 and takes
precedence over contrary proposal wording. HAL002/002A remain supreme.
No unrelated Draft is adopted.

The result is one reusable native read-only Markdown widget, suitable for an
ordinary application's supplied response text. A parser-only delivery does not
complete this framework boundary. Consumer integration follows delivery and
independent review of the public framework API; consumer source ownership
remains separate from the framework implementation.

### 14.1 Parser and dependency selection

Select exactly:

    pulldown-cmark = { version = "=0.13.4", default-features = false }

This is a deliberately pinned reviewed version, not a claim of the latest
release. Independent review inspected the registry record and the 155043-byte
published archive in memory. SHA256
e9f068eba8e7071c5f9511831b44f32c740d5adf574e990f946ddb53db2f314e
matched the registry checksum; at that review the version was not yanked,
declared MIT and Rust 1.71.1, and used edition 2021. The default-disabled
HTML/getopts, serde, SIMD and gen-tests features were not selected. The ordinary
build script was inert without gen-tests. This reading did not install the crate
locally.

Cargo acquisition of this exact dependency and its required normal transitive
closure is now authorized through the existing build workflow. Preserve
compatible already resolved versions; no broad update, binary install, model/
font download, alternate registry or vendoring is selected. Cargo verifies
registry checksums. Keep the package's copyright/license notice in the owning
third-party notices when distributing the consumer. Add docs/THIRD-PARTY-NOTICES.md
for this dependency and its newly introduced closure, with exact upstream
notices and no claim about unrelated dependencies.

Parsing uses Parser::new_ext and into_offset_iter with exactly ENABLE_TABLES,
ENABLE_STRIKETHROUGH and ENABLE_TASKLISTS. Do not enable additional syntax.
The public event/range contract was independently read; decoded character
positions are not source offsets. Implementation must check exact received
package/API identity and execute the selected decoding/source-range regressions,
not infer byte fidelity from documentation.

### 14.2 Public API and immutable data

Export these types from `halcyon::widgets::markdown`, implemented under
`src/widgets/markdown/` with a directory `mod.rs`. Keep parser implementation
types private and do not expose pulldown-cmark in consumer signatures.

- `MarkdownLimits`, Copy, with public usize fields `max_source_bytes`,
  `max_reading_bytes`, `max_nodes`, `max_depth`, `max_table_columns`.
- `MarkdownLimitKind`: SourceBytes, ReadingBytes, Nodes, Depth, TableColumns.
- `MarkdownPrepareError`: InvalidLimits, LimitExceeded { kind, limit, observed },
  InvalidSourceRange, UnsupportedEvent, Canceled.
- `MarkdownDocument`, immutable and Send + Sync.
- `MarkdownStyle` and `MarkdownStyleError` (InvalidFamily, InvalidFontSize).
- `MarkdownView` and `MarkdownLayoutError` (UnsupportedTypography,
  RequiredFaceUnavailable, Text(TextInputError), InvalidGeometry).

Required signatures:

    MarkdownDocument::parse(source: Arc<str>, limits: MarkdownLimits)
        -> Result<Arc<MarkdownDocument>, MarkdownPrepareError>
    MarkdownDocument::parse_with_cancel(
        source: Arc<str>, limits: MarkdownLimits,
        canceled: impl FnMut() -> bool
    ) -> Result<Arc<MarkdownDocument>, MarkdownPrepareError>
    MarkdownDocument::source(&self) -> &str
    MarkdownDocument::reading_text(&self) -> &str
    MarkdownDocument::code_block_count(&self) -> usize
    MarkdownDocument::code_literal(&self, ordinal: usize) -> Option<&str>

    MarkdownStyle::new(prose_family: impl Into<String>, code_family: impl Into<String>)
        -> Result<MarkdownStyle, MarkdownStyleError>
    MarkdownStyle::with_size(self, size: f32)
        -> Result<MarkdownStyle, MarkdownStyleError>

    MarkdownView::new(document: Arc<MarkdownDocument>, style: MarkdownStyle) -> Self
    MarkdownView::with_selection_policy(self, policy: TextSelectionPolicy) -> Self
    MarkdownView::document(&self) -> &Arc<MarkdownDocument>
    MarkdownView::layout_failure(&self) -> Option<&MarkdownLayoutError>

MarkdownLimits::default selects respectively 262144, 1048576, 32768, 64 and 64.
All limits must be nonzero. Source length is checked before parser construction.
Node budget includes document nodes and provenance/projection segments; reject
before growing those records beyond the bound. Reading budget is checked before
each append, including generated markers/separators. Count actual open parser
containers for depth and actual parsed cells for columns. No partial document
is returned on refusal. Use checked arithmetic and iterative bounded traversal.

Check cancellation before constructing the parser, immediately afterward,
between events and before publication. This is cooperative CPU cancellation,
not a promise to interrupt inside an upstream parsing call. Source bounds also
apply before that indivisible call. The callback conveys caller cancellation
only; the parser owns no runtime, UI, timer or cancellation service.

Document structure and maps are private owned values. No font catalog, binding,
native handle, widget, engine, mutable cross-thread owner or renderer object
enters MarkdownDocument. Redacted Debug exposes counts/outcomes, never source,
reply bodies or font bytes.

### 14.3 Projection and source-map decision

Select section 5 with these precise qualifications. Exact source is retained
once as the supplied Arc<str>. The document may have an owned decoded reading
string, but it never changes the source. Each emitted reading segment carries
its whole parser source range, or an explicit structural node origin for
generated separators. Exact-copy segments may map bytes directly only after
equality is proved; decoded/normalized segments never claim a character-level
inverse mapping. Raw-source copy uses source(), never reverse reconstruction.

Paragraph/heading text uses parser-decoded events. SoftBreak becomes one space;
HardBreak becomes LF. Code events use the parser's decoded literal verbatim,
including its line-ending decisions. Literal raw HTML uses the actual source
range and is never executed. Do not feed raw-HTML text back into the parser.

Sequence projection joins adjacent nonempty block projections with two LFs.
List item projections are joined by one LF; prefix each item with "- " or its
actual consecutive ordered number plus ". ". Nesting adds two spaces per list
depth to the item prefix only, not to code literal storage. Blocks after the
first block inside an item use two LFs. Quotes retain their content without
inserting decorative quote bars. Tables use TAB between cells and LF between
rows. Task state contributes "[ ] " or "[x] " after its item prefix. An image
contributes "[image not loaded: " plus decoded alternative text plus "]".
A thematic break contributes separation only. Avoid a fabricated final block
separator; existing final LF in code literal remains part of that literal.

Metadata retains each code block's literal independently of structural prefixes
and separators. code_literal returns that value, never source with guessed fence
removal. Inline code and code-block tabs stay TAB in the reading/copy values.
For geometry, expand code tabs to the next four-column stop in a private visual
projection, with exact tab-to-reading mapping and no inserted spaces in copying.
Base tab-cell advance comes from the actual admitted code face's space glyph.
LF terminates a code line; CRLF/LF handling follows parser events, not a second
Markdown normalization pass. Tests must cover those differences explicitly.

Select all section 4 constructs. Inline code inherits enclosing strong/emphasis
style; fenced/indented code bodies are literal regular code style. Link labels
remain ordinary selectable text with inert destinations; task marks are
read-only geometry. Images do not load. No syntax highlighting, editing,
activation, fetching, or HTML/CSS interpretation is selected.

### 14.4 One view, existing text owners

MarkdownView is one WidgetTree-owned leaf, not a tree of independently selected
TextBlocks. One ReadOnlyTextSelection operates on its complete reading string.
Actual visual blocks/cells/runs map into that string. There are no cross-widget
selection links, global document manager or UI references inside document nodes.

This section and the coordinated HAL048/HAL049 addition expressly admit this
widget to the existing catalog/whole-cluster/bound-run path. Reuse
Paragraph::with_spans, ParagraphLayoutEngine::try_layout_with_catalog,
TextEngine bound metrics/shaping, actual source-cluster hit/highlight helpers
and ordinary logical scene text. No public generic run-painter API is needed.
A crate-private mechanical helper shared with TextBlock is allowed only to
factor its exact existing binding/shape operation without changing callers.

Every paragraph span covers a valid reading substring. Measurement, pointer
hit, selection geometry, paint and Compile preserve the same actual face,
direction, size and gap. Paint keeps each face run separate. Decorations are
geometry at producer-owned Z below or above the appropriate text; they are not
invented TextStyle fields or glyph substitution. Paragraph line-box height must
include the maximum actual run ascent/descent while keeping its canonical
baseline policy; there is no default-font approximation for larger styled runs.

No semantic source/selection mutation occurs during layout or paint. Geometry,
layout-result status and stable-key derived caches remain layout-owned. Do not
retain borrowed runs/glyphs or put them into SceneGraph. Cache keys include
document Arc identity, finite width, style, actual catalog generation, scale
and relevant theme values. Unchanged redraws do not reparse or relayout.

Size overrides are finite and strictly positive. With no override, resolve the
base size from Theme.typography.font_size at layout. Prose/heading/code text
uses colors.typography.primary; code/table-header surfaces use surfaces.level1;
table/quote/thematic rules use borders.subtle; selection uses the existing
accents.muted plus typography.selection_alpha recipe. No theme schema change.

Declared em geometry under INV-MD-GEOMETRY: block gap 0.75em, list indentation
1.5em, quote inset 1em, code padding 0.5em, table cell padding 0.5em horizontally
and 0.25em vertically. Heading multipliers are [1.75,1.5,1.25,1.125,1,1], weight
700, with preceding gap 1em except at the start. Body and code use the base size.
Rules use the existing theme border width; no per-application optical offsets.

The selected prose and code family names are explicit view configuration.
Validate each used primary weight/italic against the actual admitted catalog;
regular closest-match must not be described as bold/italic. Validate the code
primary as fixed-pitch from its admitted font metadata, not a name heuristic.
Coverage fallback may legitimately use a different face for unsupported
clusters; preserve its actual metrics rather than stretching it to pretend
monospacing. The application owns its explicit startup font-asset selection.

Missing typography capability/required style or invalid layout produces a typed
MarkdownLayoutError, not a panic or fake formatted success. Present a local
plain diagnostic using the existing default text path when that path is valid;
source remains available for exact copying. Failure of that underlying text
path retains HAL360's real Unavailable result. This introduces no renderer
recovery hook or consumer glyph handler.

### 14.5 Finite prose width and genuine overflow

Select ONE enclosing ScrollArea with vertical and horizontal scrolling enabled.
Its direct content is wired once before first layout. A tight finite viewport
must be supplied by the existing Flex allocation. The viewer receives the
actual finite child max_width; it does not ask ScrollArea for infinite width.

Prose/headings/quotes/list bodies wrap at available viewport width after their
explicit indentation/padding. Preformatted code lines do not wrap; only the
internal Paragraph measurement for those lines receives infinite max_width.
The widget reports their measured natural width in LayoutResult.content_size.

Tables are native block geometry, not TabularSubstrate data, extra child
widgets or pipe-art text. Determine each column's preferred width from the
maximum no-wrap cell width plus its two horizontal paddings. Its minimum is
min(preferred, max(6em, maximum actual cluster width + two paddings)).
For n columns and available W:
- If sum(preferred) <= W, add the spare width equally to the columns.
- If sum(minimum) <= W < sum(preferred), distribute W-sum(minimum) proportionally
  to each preferred-minus-minimum slack.
- Otherwise keep the minimums and report the genuine horizontal overflow.
Use checked finite arithmetic, including the all-zero-slack case. Wrap cell
paragraphs to their assigned inner widths; row height is the maximum laid-out
cell height plus vertical padding. Honor parsed cell alignment. Header cells
use actual bold prose style. No nested scrolling owner is introduced.

Return allocated size constrained by the parent separately from natural
content_size. Do not clamp away code/table extent or expand prose wrapping to
that wider extent. Existing Flex::layout_no_wrap aggregates child content_size,
so a consumer can keep a single wired column with separate request/view/metadata.
Do not change Flex or ScrollArea algorithms. A necessary mismatch in these
semantics is an owner finding, not permission for another engine.

### 14.6 Selection and explicit copy

Default selection remains Disabled. With Enabled, reuse routed capture/drag,
word/all selection and Cmd/Ctrl+C/A under actual focus/capture. Hits map through
actual bound source-cluster geometry into valid reading-string boundaries.
Do not let arbitrary byte offsets reach the selection helper: its length
clamping does not establish scalar or cluster legality. Generated tab geometry
maps to the original TAB interval; copied text has no visual expansion.

Cmd/Ctrl+Shift+C, delivered through the same existing Text event route while
this view is the eligible target, copies the entire exact Markdown source.
That source is the document supplied by the caller, including any accepted
application-side terminal-marker slice; it is not an upstream raw backend
receipt. Ordinary Cmd/Ctrl+C copies reading selection. No clipboard read, cut,
paste, link activation or implicit copy occurs. code_literal additionally
supports an explicit caller-owned code-copy action; no embedded Button is
required.

Resize/theme/DPR changes on the SAME mounted document preserve selection
indices. A replacement document or concrete mount resets selection. Remounting
the same Arc after Work navigation does not pretend to preserve destroyed
selection state. Original draft/Work continuity remains unchanged.

### 14.7 Outcome and acceptance boundary

Inherit HAL360/361 Complete/Degraded/Unavailable unchanged. A missing cluster
marker never replaces source or clipboard bytes. Do not introduce a viewer-only
font fallback, redraw loop, observer requirement, resource loader or alternate
renderer. A correctly parsed document is not a completed native frame.

Implement source and native proof as one coherent framework unit:
- `markdown_` tests inside the new directory module for parser/source/copy,
  cancellation/limits, the selected constructs and malformed-input distinctions;
- registered internal native/Metal tests in `src/tests/markdown_view.rs`,
  registered in the existing test module, with nonzero cases through the actual
  WidgetTree, Paragraph, bound scene, Compile and readback at 1x/2x;
- exact wide-code/table scroll reachability, same-document selection on resize,
  raw/reading/code-copy differences, real strong/emphasis faces, RTL/CJK and
  uncovered-cluster Degraded with same-surface routed input;
- one public integration target `tests/markdown_public.rs` proving the documented
  API without private/test-harness imports;
- a native `examples/markdown.rs` reader showing the same component and a
  deterministic nonpersonal document, preserving framework Escape ownership.

Parser/HTML/link tests do not execute external content. Do not add a broad
negative-feature test matrix or modify the original expensive Work suite.
Existing relevant typography/paragraph/S5 regressions run once after their
affected shared helper changes, not as an S4 restart. Existing passing evidence
may be reused when those owners remain unchanged.

Consumer integration completes the application objective after review of this
framework boundary. Deterministic document tests establish formatting coverage;
one genuine native response establishes the changed consumer route when
verified input is available. Do not repeat stochastic requests to obtain a
desired Markdown shape. Keep native input/pixel limits explicit.

The original additive framework delivery was classified minor and selected
Halcyon 0.234.0-beta with matching own-package metadata and a scoped Unreleased
note. It preserved independently owned rendering changes and could not lower
a greater existing version. It changed no consumer version and authorized no
remote publication, font redistribution, release build or unrelated application
change.

## 15. Independent framework-boundary review - 2026-11-13

Independent review accepted the delivered HAL390 public framework boundary
for consumer use. This is not acceptance of application integration or physical
gestures/clipboard.

The review inspected the actual document parser/projection and view sources,
public exports and public-only test, selected dependency declaration, example,
and the actual native/Metal tests. Document source, decoded reading and code
literals remain distinct. Preparation is bounded, cooperative and outside
frame callbacks. One mounted view uses registered bound runs, original cluster
geometry, the existing read-only selection owner and genuine content extents
for the original two-axis ScrollArea. Missing clusters inherit HAL360 Degraded
without rewriting document/copy bytes. No new framework owner was selected.

Inspected source identities: document.rs a9bd815178eefa57fec01c3266e20fd8fbac75b52ed9c98c9c8a93a222976c3c;
view.rs d8da6b6eb476fe8822805ee3cf52a46cf06a85fb460c149dc14185acdbc8660e;
public test 3d93a266b5e79a84caf5bbc4a977560a3593c13797d07d8e4bf89595fc954b08.
The preserved execution receipts were read independently, not rerun:
markdown-tests-4.txt records 11 internal passes and the selected public pass;
public-test-300.txt records the explicit public target exit0/1 pass;
metal-portable-evidence.txt records 3 affected internal passes;
check-final.txt records exit0, zero errors and 123 warnings; package-build.txt
and the example check/build record actual exit0. These are retained executor
runs independently reviewed, not new verification executions.

Privately retained native records identify an ordinary example process/window
and a Complete/produced/submitted/present-called first attempt, with no injected
input and owned termination by SIGTERM. The captured image remains executor
visual evidence; independent acceptance did not inspect its pixels again.
These records are historical evidence, not public reproduction artifacts
linked by this document. No physical selection/clipboard, exact scanout join,
application response or resident reuse is inferred; earlier font and rendering
investigations are not reopened.

The public framework prerequisite for consumer integration is satisfied.
Application integration remains separately scoped; Halcyon remains the sole
Markdown implementation owner.

## 16. Same-document reading-selection handoff for Stage navigation - 2026-11-14

**Status: ADOPTED / ACTIVE for this exact additive owner boundary.**
This section implements JC's requirement to preserve native reading selection
across screen navigation and switching. It is not adoption of the rest of
HAL315 or a new document/editor service.

### 16.1 Concrete gap and compatibility

The current MarkdownView privately owns ReadOnlyTextSelection. on_mount and
on_blur clear it. A preserved Arc<MarkdownDocument> is not a preserved selection,
and ScreenStack rebuilds returning widget subtrees. Exact same-document reading
selection across navigation therefore requires an owner-qualified handoff; a
consumer cannot implement it by inspecting private fields, saving glyphs,
keeping widgets outside the tree or fabricating selection events.

Section14.6's default concrete-mount reset remains the default. This section
adds an explicit ordinary public value handoff, and an explicit per-view blur
retention policy for the Stage consumer. This is normal widget configuration,
not a runtime switch, diagnostic mode or alternate renderer. Plain source,
document preparation, text coverage, input/capture and clipboard owners stay
unchanged. Default and all nonparticipating callers retain existing behavior.

### 16.2 Exact public contract

Export from halcyon::widgets::markdown:
- MarkdownSelectionRetention, Copy/Eq: ClearOnBlur (default), RetainOnBlur.
- MarkdownReadHandoff: opaque Clone value containing the actual immutable
  Arc<MarkdownDocument> and exact ordered reading-byte anchor/head. Only
  MarkdownView may construct it; Debug redacts content, offsets and addresses.
  No widget, mount, native handle, geometry, focus, capture or drag state.
- MarkdownReadHandoffError, Copy/Eq: SelectionDisabled, SelectionInProgress,
  LayoutUnavailable, DocumentMismatch, InvalidBoundary, UnsupportedTarget.
- MarkdownReadHandoffStatus, Copy/Eq: NotRequested, Pending, Applied,
  Rejected(MarkdownReadHandoffError).

Required ordinary public operations:
    MarkdownView::with_selection_retention(
        self, retention: MarkdownSelectionRetention
    ) -> Self
    MarkdownView::capture_read_handoff(&self)
        -> Result<MarkdownReadHandoff, MarkdownReadHandoffError>
    MarkdownView::try_stage_read_handoff(
        &mut self, handoff: &MarkdownReadHandoff
    ) -> Result<(), MarkdownReadHandoffError>
    MarkdownView::read_handoff_status(&self) -> MarkdownReadHandoffStatus
    MarkdownReadHandoff::document(&self) -> &Arc<MarkdownDocument>
    MarkdownReadHandoff::selection_anchor_byte(&self) -> usize
    MarkdownReadHandoff::selection_head_byte(&self) -> usize

Capture is read-only, from actual settled selection on an Enabled view. Refuse
an ongoing pointer/word drag, pending/unapplied handoff, or absent/failed actual
layout. Validate source offsets against the current document reading string and
actual current cache boundaries; no arbitrary public offset constructor or
clamping. Include exact collapsed selections as well as reverse selections.
A complete empty document is not a fictitious missing-layout success.

RetainOnBlur applies only with Enabled. On ordinary blur or eligible capture
leave, finish transient drag bookkeeping but keep the logical anchor/head;
never retain or regrant actual pointer/keyboard capture. ClearOnBlur and other
selection-policy behavior remain unchanged. Explicit selection-clear and
changing policy to Disabled still clear, including under RetainOnBlur.
This supplies a current observable selection after pointer activation of the
consumer's Views control; no last-selection shadow cache or inferred focus.

Staging is allowed only on a fresh unmounted MarkdownView, with Enabled,
RetainOnBlur, no input, existing pending request or prior application.
Require Arc::ptr_eq of the actual document and the handoff's document, not
source equality, labels, addresses serialized as IDs, or a reparse.
Validate exact reading bytes and both UTF-8 boundaries before storing an inert
owned pending value. Returning Ok means Pending only. Any refusal leaves
active selection and an earlier request unchanged. The caller retains its
independent original handoff until Applied is observed.

### 16.3 Existing deferred owner cut

Extend the EXISTING lazy, concrete-mount-qualified HAL383 handoff index and
existing pre-Layout engine cut to dispatch two explicit internal kinds:
ordinary TextField editing handoff and Markdown read-only handoff. This is not
a new queue service, a public generic state exporter or a new frame phase.
A narrowly named PendingTextHandoff record may pair the original WidgetMountToken
with its owner kind. Cover original create_root/add_child/replace_widget_only
indexing and retirement; consume each live request once, with no idle tree walk.
Keep original TextField validation and publication semantics byte-equivalent.

Both run_ui_loop and ordinary App::run must use that same cut after rebuild and
settled theme, before Layout/Paint. A mechanical naming correction of the
existing private handoff helpers/call sites is permitted. No other event-loop
ordering, window, modal, capture, application or renderer change.

The Markdown owner revalidates exact document, supported target, pending value,
reading text and scalar boundaries, then atomically restores ordered selection,
clears transient drag state and publishes Applied at that deferred cut.
A small crate-private atomic restore operation on ReadOnlyTextSelection is
permitted; do not expose mutable state or route synthetic gestures to restore.
Rejection changes no active selection; discard only the target's pending copy,
publish the typed cause, and leave the consumer's original value intact.
Retirement or replacement before the cut invalidates the concrete pending
instance and cannot apply to a reused node or different widget.

This transfers a logical interval in an IDENTICAL immutable reading string,
not an editable caret or original layout. Source selection was captured from
real cluster-safe hits. Target normal layout/paint keeps the existing actual
bound-run cluster-intersection highlighting, hit mapping and exact reading-copy
semantics under width/DPR changes; it never snaps saved byte endpoints or
claims identical geometry. No target shaping is invented at constructor time.
No publication in Layout/Paint, second-frame repair, timer, wake, parsing,
font acquisition or new text-engine API is admitted.

### 16.4 Coherent proof and footprint

Own only markdown/mod.rs and markdown/view.rs; the narrow private helper in
core/text_selection.rs; the original WidgetTree handoff index and its exact
App call sites; nearest existing markdown internal/public tests; scoped own
package version/lock and Unreleased note. HAL383 coordinated section11 keeps
its owner contract. No consumer/backend source, font asset, TextBlock, text
shaping, ScrollArea, independent rendering or original TextField algorithm
changes are authorized.

Extend src/tests/markdown_view.rs and tests/markdown_public.rs, not a new test
framework. Prove capture after real routed selection, reverse and collapsed
ranges, exact same-document staging, Pending before the cut, Applied before
first paint, continued routed selection/Copy after rebuild, and 1x/2x Metal
highlight output. Include a real public Screen cover/return mount cycle in
the existing harness. Test changed width using current layout and same source
interval, not old glyph coordinates. Copy goes to the existing isolated
test clipboard, not the personal system clipboard.

Prove default blur still clears; explicit retention preserves only logical
selection; explicit clear still clears. Different document Arc with identical
bytes, disabled/used/mounted targets, invalid private test-only snapshot,
active drag, replaced/retired mount and repeated processing cannot apply a
wrong/duplicate handoff. Use only live-confusable negative cases.

Run owning package markdown_ and edit_handoff targets, explicit public
markdown_public and text_field_handoff_public tests, and changed refresh/check.
The native and UI cuts require source qualification plus first-Paint evidence;
no physical gesture/clipboard claim or unrelated full-suite replay.
No model request is needed for this prerequisite.

The additive public boundary selects minor0.236.0-beta from inspected
0.235.0-beta, with its own-package lock record and scoped Unreleased entry.
If another authorized unit has advanced the package first, requalify that
serial boundary and apply the declared additive-release policy without lowering
it or modifying unrelated dependency resolutions.

Completion means implemented public handoff plus the stated owner/native proof
and independent review. It does not establish the separately scoped consumer
screen-navigation integration.

### 16.5 Implicit outside-down versus explicit clear - 2026-11-14

**ADOPTED clarification and narrow scope extension within the SAME
HAL-MARKDOWN-READING-STATE-HANDOFF-S1.** This is not a successor or a reduction
of the pointer-Views retention requirement.

Independent current-source review confirms that EventRouter's
clear_focused_read_only_selection_on_outside_down calls the existing
Widget::clear_read_only_text_selection before focus assignment/Blur
(event_router.rs:1118-1143,5709-5717; widget/mod.rs:3412-3418).
A blur-only policy therefore cannot preserve the interval after the Views
pointer click. Changing the existing clear hook into a retention veto would
also change explicit clear. Keep those operations distinct.

The selected public Widget extension is exactly one defaulted method:

```rust
fn clear_read_only_text_selection_on_outside_down(&mut self) -> bool {
    self.clear_read_only_text_selection()
}
```

The original clear_read_only_text_selection signature, default body and
unconditional clearing behavior of existing implementations remain intact.
It is the force-clear operation when explicitly invoked, independent of
retention. Its existing lifecycle callers in other widgets are unchanged.
Document the new method as the implicit outside-down operation; the bool
reports selection-state change, NOT input consumption, focus ownership or
permission to stop routing.

Change only the call inside EventRouter's existing outside-down helper to
invoke the new method. Preserve its current phase/order, focused-widget
lookup, descendant hit test, no-hit behavior, original non-touch/Primary/
!is_captured entry checks, and existing changed-state invalidation. Do not
add a second dispatch, downcast, widget-name test, application-screen/action-ID special
case, observer, queue, or new input/lifecycle state. The router determines
the reason from its actual route; the widget owns its selection policy.

MarkdownView overrides ONLY the new implicit hook as follows:
- Enabled plus RetainOnBlur leaves the current logical anchor/head unchanged
  and returns false. It neither consumes the outside event nor retains or
  regrants focus/capture. Subsequent genuine Blur/capture-leave follows
  section16.2 and ends transient drag bookkeeping without clearing the range.
- Every other policy/retention combination delegates to its original
  clear_read_only_text_selection implementation.

The original Markdown explicit clear always clears under RetainOnBlur too,
using its original return and transient-state semantics. Changing policy to
Disabled, new-document/reset semantics, pointer selection inside the view,
and the default ClearOnBlur behavior remain as previously selected. This
retention policy applies to all actual outside-down targets, including
nonfocusable background, not just the consumer's Views control. It confers
no input authority on an unfocused view and no cross-widget clipboard path.

**INV-MD-CLEAR-REASON:** implicit outside-down and explicit clear remain
distinct; retaining one cannot suppress the other.
**INV-MD-OUTSIDE-RETENTION:** a settled Enabled/RetainOnBlur interval survives
the original outside-down and subsequent Blur without changing routing.
**INV-MD-CLEAR-COMPATIBILITY:** default hook delegation preserves all existing
nonparticipating Widget implementations and their actual clear behavior.

The SAME unit's source scope additionally includes src/core/widget/mod.rs
for this one defaulted hook/documentation and src/core/event_router.rs for
this one call-site change. No other router algorithm is authorized. Owning
HAL328 section19 records the same boundary. Extend the existing markdown
tests; tests/focus_pointer_assignment.rs is additionally authorized only for
focused default-delegation/unchanged-routing proof. No TextBlock, Label,
Readout, consumer source or other widget implementation is to be changed.

Required proof must use the real EventRouter, not only direct on_blur calls:
settle a routed reverse Unicode selection on the actual MarkdownView, then
route a complete primary Down/Up to an ordinary outside Button representing
Views. Observe normal focus/activation exactly once and the same interval
from capture_read_handoff afterward. Continue that selected existing proof
through its Screen cover/remount and deferred handoff, rather than substituting
a separate copy of the selected text. The same outside click under default
ClearOnBlur clears. An explicit clear on the retained view also clears.
Check outside nonfocusable background separately because no Blur is required
there. Preserve the existing pointer-assignment tests, especially
test_left_click_on_non_focusable_clears_focused_read_only_selection; unchanged
default widgets must still call their original clearing implementation.

Add the exact public target to the existing final sequential verification:
`exec build timeout 300 output summary -- cargo test -p halcyon --test focus_pointer_assignment`.
Existing section16.4 Markdown/public-handoff/Metal and TextField gates remain.
Reuse unaffected proof and rerun only affected evidence at the coherent unit
boundary. No new model/native-consumer run or broad router suite is required.
The already selected minor0.236.0-beta remains the release for this unfinished
additive unit; this clarification alone does not select another increment.

### 16.6 Independent acceptance and consumer release - 2026-11-14

Independent review accepts the reading-state handoff at the public value/API,
routed selection, concrete-mount deferred application, real Screen cover/return
and selected Metal first-Paint boundaries. Final source identities match the
implementations reviewed during execution: MarkdownView capture/stage/application,
the atomic selection helper, the lazy WidgetTree owner-kind dispatch and the one
implicit-clear router replacement. The original App deferred call sites and
TextField algorithm are unchanged.

The original verification evidence was retained privately. The final raw
Markdown record has 14 internal and 2 public passes; edit_handoff has 9 passes.
The three explicit public/pointer targets, check and package build have actual
successful child outcomes. Earlier failed fixture records remain failures.
Independent review inspected assertions and original records, not a repeated
test execution. These are historical verification records, not public
reproduction artifacts linked by this document.

The exact accepted sequence is routed reverse Unicode selection, original
outside Button activation and focus transfer, real Screen retirement/remount,
Pending then Applied before first layout/paint, changed width and 1x/2x
Complete Metal highlight output. Copy is proved through the original reader
command and an isolated clipboard; this is not system-clipboard or physical
keyboard proof. Continued routed selection, explicit/default clear and
confusable stale/mismatched handoff cases retain their separate assertions.
No OS input, scanout or finished consumer integration is asserted.

HAL390 section16's public prerequisite for consumer reading-state preservation
is satisfied. Consumer implementation can use that accepted contract without
another handoff audit or unchanged test replay. The separately identified
captured-modifier ordering gap is governed by HAL328 section20; it does not
reopen this accepted work or prevent independent consumer development.

## 17. Scoped dependency security maintenance - 2026-11-25

**Status: ADOPTED / BINDING for dependency selection; scoped implementation
accepted as recorded below.** Under JC's public-beta dependency-security maintenance
mandate, Primary selects the following exact replacement for the parser version
in section 14.1:

    pulldown-cmark = { version = "=0.13.6", default-features = false }

This supersedes only the earlier exact 0.13.4 pin and acquisition permission.
RUSTSEC-2026-0414 and RUSTSEC-2026-0433 concern the disabled wiki-link extension;
the current selected profile is not affected by those paths. The compatible
patch release removes those flagged package versions from the distribution.
Independent review verified the 157338-byte archive against its registry
checksum, non-yanked status, MIT notice identical to the 0.13.4 archive, edition
2021, Rust 1.71.1, unchanged required normal dependency constraints, retained
Parser::new_ext and into_offset_iter signatures, and the three selected option
bits. The changed build script remains inert with gen-tests disabled. Newly
available definition-list syntax is not selected or adopted.

Preserve exactly ENABLE_TABLES, ENABLE_STRIKETHROUGH and ENABLE_TASKLISTS,
original source bytes/ranges, reading/code projections, cancellation, bounds,
inert HTML and links, public API and all existing view/selection semantics.
Cargo acquisition of this exact replacement is authorized through the ordinary
registry workflow. No alternate parser, syntax expansion or unrelated update
follows. Verify the original markdown_ internal and public tests against the
new resolved package, plus affected supported build targets. Preserve historical
0.13.4 evidence and record the newly executed proof separately. The identical
MIT notice remains required in the distribution. No constitutional change,
new widget behavior, consumer mutation, or publication is authorized.

**Scoped implementation acceptance - 2026-11-25.** The exact 0.13.6
parser pin with default features disabled is implemented and source-accepted.
The selected option profile and existing Markdown source paths are unchanged.
The retained HAL-SEC-DEPS-01 receipts establish 15 passing internal Markdown
tests, including native Metal cases, and two passing public tests, together
with successful native, WASM, Windows MSVC and iOS Simulator build checks.
The final dependency audit reports no vulnerability findings without advisory
suppression. The earlier 0.13.4 evidence remains historical; no expansion
of parser syntax or consumer-runtime acceptance is inferred.
