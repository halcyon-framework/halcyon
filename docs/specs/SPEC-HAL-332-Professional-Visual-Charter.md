# SPEC-HAL-332 — Professional Visual Charter

**Status:** REJECTED / RETIRED / INACTIVE / NOT BINDING
**Owner disposition:** JC, 2026-11-04 — HAL-REJECT-FRAMEWORK-WIDE-AESTHETIC-MOTION-BANS-01

## JC owner disposition — 2026-11-04

JC rejects this proposal as a compulsory framework-wide visual charter. Its claim
to govern every visual decision in Halcyon and all software built on Halcyon was
never adopted as universal law. The forced dense/austere professional genre,
consumer-style exclusions, categorical decoration and motion bans, genre-review
protocol, mandatory recalibration agenda and automatic visual Test of Adoption
are retired, not pending approval.

Applications own their artistic expression. Halcyon supplies
high-performance implementation mechanisms, not a compulsory design genre. Spring,
bounce, overshoot, rotation, scale and long transitions are not categorically
prohibited by Halcyon. This disposition creates no replacement aesthetic charter,
permission gate, approval system, curve/duration restriction, mode exception or
Showcase-only exemption. Products use the framework's actual capabilities; this
documentary disposition does not implement an unavailable capability, change
current defaults or authorize runtime work.

[HAL-002](SPEC-HAL-002-Constitution-and-Architectural-Invariants.md),
[HAL-002A](SPEC-HAL-002A-Constitution-Amendment-Guidelines.md), binding
[HAL-021](SPEC-HAL-021-Animations.md), and separately adopted component/domain
contracts retain their exact scopes. A scoped rule in another binding SPEC remains
that owner's rule; its citation of this charter does not adopt the charter's
universal genre or prohibitions. HAL-021A's existing status and pressed-local scope
are unchanged; HAL-363/364 remain Proposed. Historical dependencies, cross-references,
proposed amendments (including HAL-332A), adoption tests and implementation resemblance
cannot adopt or revive this rejected charter. The rejected
[HAL-097](SPEC-HAL-097-Motion-and-Micro-Animation-Rules.md) and
[HAL-097A](SPEC-HAL-097A-Motion-Authorization-and-Layout-Transition-Law.md)
supply no universal aesthetic or motion-approval authority either.

## Historical proposal — provenance only

The entire original proposal below, including its former Governs statement,
principles, anti-principles, decision protocol, recalibration items and Test of
Adoption, is historical nonbinding provenance. None is a current requirement,
pending implementation instruction or route to automatic adoption.

**SPEC-HAL-332**
**Professional Visual Charter — Genre Definition and Visual Authority**
**Author:** JC
**Historical proposal status:** PROPOSED; never adopted as universal framework law.
**Version:** 1.0

**Depends On:**

* SPEC-HAL-002 — Constitution and Architectural Invariants
* SPEC-HAL-012 — Theming
* SPEC-HAL-090 — Logical Grid System and Compact Layout Rules
* SPEC-HAL-091 — Adaptive Density Modes
* SPEC-HAL-092 — Unified Typography Rules
* SPEC-HAL-093 — Visual Appearance and Radii System
* SPEC-HAL-094 — Component Geometry and Layout Rules
* SPEC-HAL-095 — Iconography Rules and Pixel Grid
* SPEC-HAL-096 — Color System and Semantic Palette
* SPEC-HAL-097 — Motion and Micro-Animation Rules

**Governs:** every theme value, every default geometry token, every showcase composition, every visual design decision in Halcyon and in software built on Halcyon. Functional SPECs (color, geometry, typography, motion) implement individual dimensions of this charter. This document is the coordinating authority that says WHY those dimensions take the values they do.

---

## Implementation Status

*Not implemented.* This SPEC defines the design genre Halcyon belongs to, the audience it serves, and the principles every visual choice is checked against. It does not specify pixel values or hex codes — those live in the implementing SPECs (HAL-090 through HAL-097). It specifies the philosophy those SPECs must collectively express.

---

## 1. Motivation

Halcyon is a UI framework, but a framework's visual default is not a neutral choice. Every framework has a default look. Consumers of the framework either accept that default and inherit its character, or they fight it. Halcyon's default must therefore be opinionated and aligned with the kind of software Halcyon is intended to enable.

The market is saturated with frameworks defaulting to consumer-grade web aesthetics: rounded cards, generous whitespace, decorative gradients, friendly animation, soft shadows, large touch targets. These frameworks produce software that looks at home in a marketing page or a SaaS dashboard.

That is not the software Halcyon is for.

Halcyon is for **professional engineering software**: instruments, tools of trade, sustained-use applications that an expert sits in front of for eight to twelve hours a day. The aesthetic of that genre is distinct, codified by decades of convention, and **inversely correlated with consumer-app polish**. A framework that defaults to consumer aesthetics signals to its target audience that it was not designed for them.

This charter pins Halcyon's genre, articulates its principles, and prohibits drift toward consumer-app defaults.

---

## 2. Audience

Halcyon serves operators, engineers, traders, scientists, and analysts. Specifically:

* People who use one tool intensively for the bulk of their working day, not many tools casually.
* People whose work involves reading dense information — telemetry, market data, system state, simulation output, instrument readings — and acting on it quickly.
* People who notice and resent any UI element that wastes their attention or screen real estate.
* People in environments where wrong decisions have consequences: control rooms, trading floors, design reviews, machine-shop floors, flight ops, surgical theaters, financial back-offices.
* People who judge software the way a machinist judges a wrench: does it do the job, does it last, does it stay out of the way.

This audience does **not** include:

* Casual users browsing a product for the first time.
* Marketing audiences encountering the software in a video or screenshot.
* Designers evaluating the software for aesthetic novelty.
* Anyone for whom first-impression polish matters more than long-term productivity.

Visual decisions resolve in favor of the included audience. A first-launch screen that looks slightly austere to a marketer but loads dense data correctly for a trader is the right answer.

---

## 3. Genre

The genre Halcyon belongs to is established by these applications. Every design decision in Halcyon should be checked against the question: *would this look at home in this list?*

**Trading / finance:** Bloomberg Terminal, Refinitiv Eikon, FactSet, TradingView Pro, MetaTrader, NinjaTrader, Sierra Chart.

**Engineering CAD / simulation:** SOLIDWORKS, CATIA, Siemens NX, Creo, Fusion 360 (pro mode), ANSYS, COMSOL, MATLAB, LabVIEW, ParaView, KiCad.

**Audio / video / VFX:** DaVinci Resolve, Nuke, Houdini, Pro Tools, Reaper, Avid Media Composer.

**Software / reverse engineering:** IDA Pro, Ghidra, x64dbg, Wireshark, Burp Suite, Helix, Sublime Text, IntelliJ (dark theme), DBeaver (dark theme).

**Control / monitoring / SCADA:** Grafana (engineering dashboards), industrial HMIs, mission-control consoles, network operation center displays.

These applications differ in domain but share a visual genre. What they have in common:

* Dark default theme. Light theme present but secondary.
* Information density prioritized over breathing room.
* Restrained, semantic color palette. Color carries meaning.
* Minimal corner radii. Sharp lines.
* Hairline borders or none. No decorative outlines.
* Tactility through tonal shift, not shadow or gradient.
* Always-visible persistent status.
* Honest numeric representation.
* Monospace where data is being aligned.
* No "cards." No drop shadows for decoration. No animated micro-interactions for delight.

This is the genre. Halcyon belongs to it. The charter principles below codify what that genre requires.

---

## 4. Principles

### Principle 1 — Dark First

The default theme is dark. Light theme exists as a service mode for documentation screenshots, presentations, accessibility cases, and operators who personally prefer it. The default-launch experience presents the dark theme.

**Rationale.** Control rooms, trading floors, late-night engineering work, and screen-fatigue mitigation all favor dark UIs. Every reference application in §3 defaults to dark mode in current versions. A light default reads to the target audience as "consumer-app posture" before any feature is evaluated.

**Implementing SPEC.** SPEC-HAL-012 Theming. SPEC-HAL-096 Color System (default theme selection).

---

### Principle 2 — Tonal Hierarchy: Data Recesses Into Chrome

Data surfaces sit *in* the chrome, not *on* it. The frame around a data area is brighter and more saturated than the data area itself; the data area is the recessed reading plane.

In dark theme: chrome is at the lighter end of the dark range; data is darker than chrome.

In light theme: chrome is at the cooler/darker end of the light range; data may be at the same tone as chrome or fractionally darker, separated by a hairline border. Data must NOT be brighter than chrome.

**Anti-pattern (consumer-app convention) explicitly rejected:** pure-white data surfaces lifted off a gray panel surround. This is the "white card on gray page" pattern of every SaaS product. It signals consumer software. It is forbidden in Halcyon defaults.

**Rationale.** Pro tools recess data so the eye reads the chrome as application structure and the data as substance. The consumer inversion (white data popping out of gray chrome) draws attention to the container instead of the content — the opposite of what an instrument should do. SPEC-HAL-096 currently inherits the desktop-OS convention of white content on gray chrome; this charter overrides that for the default themes. SPEC-HAL-096 must be amended to reflect this principle.

**Implementing SPEC.** SPEC-HAL-096 Color System (surface hierarchy values, requires amendment).

---

### Principle 3 — Color Is Semantic, Never Decorative

The functional palette contains a small fixed set of color roles. Every color used in the UI must map to one of these roles. Decorative color (a blue accent because blue looks nice; a purple highlight because purple looks modern) is an architectural error.

**The functional palette:**

* Surface stack: 4-5 neutral tones forming the tonal hierarchy (chrome, panel, data, elevated, recessed).
* Text stack: primary, secondary, muted, inverse.
* Border stack: subtle, default, strong, focus.
* Accent: ONE accent color, used exclusively for focus, primary selection, and primary-action highlight.
* Semantic: success (green), warning (amber), error (red), info (often same as accent).

That is the complete palette. Anything else is decoration and forbidden by default. A widget that wants to introduce a new color must justify it against the functional list; if it does not fit, it does not exist.

**Implication for state colors.** Hover is a tonal modulation of the underlying surface (darken in light, lighten in dark), not a color shift. Pressed is the same modulation more pronounced. Focused is the accent color applied as a hairline border or stripe.

**Implementing SPEC.** SPEC-HAL-096 Color System.

---

### Principle 4 — Typographic Hierarchy via Weight and Case, Not Color

Section headers, table headers, status labels, and other hierarchically distinct text use *weight*, *size*, and *case* to differentiate themselves from body text. They do NOT use a different color tint.

A section header reading "PROPERTIES" in 11px bold all-caps is more legible as a hierarchy mark than "Properties" in 13px regular over a 5%-different background. The first reads as hierarchy instantly. The second relies on a tonal contrast below human perception threshold.

**Concrete patterns to adopt:**

* Section labels: bold, optionally all-caps small (10-11px), with slight letter-spacing increase. Color same as primary text.
* Table column headers: bold, optionally muted color (one role darker), regular case. Background distinct from data rows.
* Status labels in footers/inspectors: regular weight, muted color, smaller size.
* Body / data text: regular weight, primary color, default size.

**Implementing SPEC.** SPEC-HAL-092 Unified Typography Rules. SPEC-HAL-214 SectionHeader Primitive Invariants. SPEC-HAL-260A PropertyEditor section archetype.

---

### Principle 5 — Density Is the Default

Spacing, padding, row heights, and control sizes default to the dense end of the comfort range. Wasted space is wasted information.

**Concrete targets for the default density mode:**

* Row heights in data displays (List, TreeView, PropertyEditor, TableView): 22-26px.
* Toolbar / menu-bar button heights: 22-26px.
* Vertical padding inside interactive controls: 4-8px.
* Horizontal padding inside interactive controls: 8-12px.
* Spacing between sibling controls in a toolbar: 4-8px.
* Spacing between panels (separator thickness): 1px.
* Tab heights: 26-30px.

Halcyon already exposes Compact / Comfortable / Spacious density modes (SPEC-HAL-091). **Compact is the default.** Comfortable and Spacious exist for accessibility scaling and for occasional use cases where touch input or formal presentation requires it.

**Implementing SPEC.** SPEC-HAL-091 Adaptive Density Modes. SPEC-HAL-094 Component Geometry and Layout Rules.

---

### Principle 6 — Sharp Corners

Corner radii on functional UI elements are at most 2px. Most elements: 0-1px. The visual signature of professional instruments is sharp lines.

**Concrete defaults:**

* Data containers (panels, data surfaces, list bodies, table bodies): 0px.
* Interactive controls (Buttons, ComboBoxes, input fields): 1-2px maximum.
* Overlays (dropdowns, tooltips, popovers): 2-3px maximum.
* Tags, badges, status pills: 2px maximum.
* Avatars, circular icons: round (the exception — round is a deliberate semantic).

Radii ≥ 4px signal consumer-app design. They are forbidden in the default themes. A widget that needs a larger radius for semantic reasons (a chat-bubble widget in a messaging context, for example) is exempt only within its specific context.

**Implementing SPEC.** SPEC-HAL-093 Visual Appearance and Radii System.

---

### Principle 7 — Hairline Borders, Used Sparingly

Borders are 1 logical pixel. They appear only where they perform structural work:

* Around the outer perimeter of input fields (delineates input from chrome).
* Between major panels (delineates structural regions).
* Around the focused control (focus ring).
* Around overlay surfaces (delineates overlay from underlying canvas).

Borders do NOT appear:

* Inside data tables between cells (use tonal shift if separation is needed; usually no separation).
* Around every widget by default (the "outlined everything" enterprise-software trap).
* Decoratively, for visual texture.

Inside data tables, prefer zebra striping (alternating rows at 4-6% tonal difference) over cell borders. Even zebra is optional — many pro tools omit it.

**Implementing SPEC.** SPEC-HAL-093 Visual Appearance and Radii System. SPEC-HAL-096 Color System (border palette).

---

### Principle 8 — Tactility Through Tone, Not Shadow

Interactive states communicate through tonal shifts on the underlying surface. Shadows, glows, and gradients are reserved for elevated overlay surfaces (dropdowns, tooltips, modals) and even there, a 1px border may suffice in place of a shadow.

**State modulation defaults:**

* Hover: 4-8% tonal shift (darken in light, lighten in dark).
* Pressed: 10-15% tonal shift in the same direction as hover.
* Focused: 1-2px accent-color border, or 2-3px accent-color left-edge stripe. Never a glow.
* Disabled: text drops to muted role; background loses 50% of its hover affordance.
* Selected (list row, etc.): clearly distinct background (the row_selected role).

**No box-shadows** on flat surfaces, controls, cards, panels, or data containers. The only legitimate uses of shadow in Halcyon are: overlay surfaces (drop-shadow at the overlay edge to communicate elevation), and sculpted border effects (highlight/shadow pairs per SPEC-HAL-093, which simulate physical bevels at 1-pixel scale, not Material-Design floating shadows).

**Implementing SPEC.** SPEC-HAL-093A Interactive Element States and Micro-Feedback. SPEC-HAL-097 Motion and Micro-Animation Rules.

---

### Principle 9 — Persistent Status Density

A status bar at the bottom of the primary window is always present and packed with information: current mode, profile, performance counters, uptime, alerts, connection state, layout name, dataset cursor position, build version.

The status bar is not a "clean" UI element to hide for minimalism. It is a continuous readout of system state. Operators glance at it every few seconds without conscious effort. Removing or thinning it removes situational awareness.

A reasonable status bar in this genre packs 6-10 distinct fields. The showcase currently shows: `Ready | Items: 30 | Mode: A | v1.0 | 82 fps | uptime: 00:00:04 | GPU: active` — this is correctly pitched for the genre. Future widget catalog additions should add fields, not remove them.

**Implementing SPEC.** (no current dedicated SPEC; this charter establishes the requirement.)

---

### Principle 10 — Honest Numeric Representation

Display values are shown at the precision the underlying data supports. They are not rounded for prettiness.

* Pixel measurements: shown as exact values, including fractional pixels where present (23.7, not 24).
* Frame timings, latencies, throughput: full precision relevant to the measurement.
* Opacity, ratios, normalized values: full digits (0.94732, not 0.95) in inspector contexts.
* Currency, money: full decimal precision per the locale and asset.
* Coordinates, dimensions in design tools: full precision.

Rounded display is acceptable only in summary surfaces (headlines, dashboards intended for at-a-glance reading) and even there, the rounded value is captioned by the precise value on hover or in an inspector.

Operators in this genre interpret hidden precision as untrustworthy. Show the truth.

**Implementing SPEC.** No dedicated SPEC; this principle constrains every widget that displays a value. PropertyEditor (SPEC-HAL-260A), inspector cells, table cells, status fields all inherit this constraint.

---

### Principle 11 — Monospace for Aligned Data

Any column the operator will scan vertically to compare values uses a monospaced font. Labels and prose use the variable-width default.

**Monospace required:**

* Numeric columns in tables.
* Hex addresses, memory dumps, byte counts.
* Timestamps, durations, dates in tabular form.
* File sizes, paths, IDs, hashes, ports.
* Code blocks, log lines, command output.

**Variable-width by default:**

* Labels, captions, button text, headers.
* Prose, descriptions, tooltips, help text.
* Section names, column titles.

The principle is *alignment to scan*. If the eye will travel vertically through a column comparing values, the column requires monospace. If the field is being read once for its content, it does not.

**Implementing SPEC.** SPEC-HAL-092 Unified Typography Rules. SPEC-HAL-092B Font Roles and Semantic Typography Binding.

---

## 5. Anti-Principles

The following are explicitly rejected from Halcyon's default themes, default geometry, and showcase compositions. They may appear in user code only when justified by domain semantics; they may NOT appear in framework defaults.

* **Cards.** No element is presented as a "card" — a rounded rectangle with shadow lifted off the page surface. Cards are a web/SaaS pattern. Halcyon's data containers are flat panels with hairline borders.
* **Decorative gradients.** Backgrounds, button fills, headers, and section bands are solid colors. Gradients appear only where they communicate physical meaning (a Plot heatmap, a temperature scale, a Gauge fill).
* **Animated micro-interactions for delight.** Animations exist to communicate state transitions and to smooth abrupt visual changes. They do not exist to "add personality." Hover-bounce, click-ripple, ease-in-out-with-flourish: forbidden.
* **Large touch targets in non-touch contexts.** Default geometry is sized for mouse and keyboard. Touch density is a separate mode (SPEC-HAL-091), not a default.
* **Friendly empty states.** Empty data displays show "(empty)" or "No items" or nothing. They do not show illustrated cartoons, emoji, or "Don't worry, you can add one!" copy.
* **Onboarding overlays in default examples.** The showcase and example apps do not introduce themselves with welcome modals, tooltips, or feature highlights. The audience knows what they are looking at.
* **Marketing-grade typography.** No headings rendered at 40px+ for visual impact. The largest text in any default surface is the section header at 14-16px.
* **Color picked for vibe.** No color in the default palette is justified by "looks modern" or "feels professional." Every color has a functional role per Principle 3 or it does not exist.
* **Skeuomorphism.** No buttons styled to look like physical buttons (3D bevels, chrome-style highlights, leather-textured surfaces). No widgets styled to imitate physical instruments unless the widget IS a virtual instrument (a Gauge widget showing analog needle is a legitimate semantic, not skeuomorphism).
* **Hidden chrome.** No auto-hiding panels, scrollbars that vanish when idle, tab bars that collapse. Chrome stays visible because operators need to know where they are.

---

## 6. Decision Protocol

When any visual decision arises — a new widget's default appearance, a theme value, a showcase composition, a geometry token — the decision is checked against this protocol in order. The first failing check determines the outcome.

1. **Genre fit.** Would this design appear in any of the §3 reference applications? If clearly no, reject.
2. **Principle compliance.** Does the design comply with all 11 principles in §4? List any violations. If a violation, either reject or justify in writing under §7 (Decision Log).
3. **Anti-principle violation.** Does the design exhibit any of the §5 anti-principles? If yes, reject. Anti-principles are absolute — no exceptions in defaults.
4. **Existing SPEC consistency.** Does the design align with the implementing SPECs (HAL-090 through HAL-097)? If a SPEC conflicts with the charter, the charter wins; the SPEC requires amendment.
5. **Audience test.** Would an operator in §2 prefer this design over the available alternatives, for sustained eight-hour use? If unclear, prototype and ask. Do not guess.

Designs that pass all five checks are approved. Designs that fail any check are either rewritten or formally exempted with written justification.

---

## 7. Decision Log

Reserved. Future entries record visual decisions that pass the protocol with non-obvious justification, or that are exempted from a principle with stated rationale. This log is the historical record of how the charter was applied.

---

## 8. Alignment with Existing SPECs

The implementing SPECs already encode many of these principles in detail. The charter does not duplicate them — it coordinates them and surfaces conflicts.

**Aligned:**

* SPEC-HAL-091 Adaptive Density Modes — Compact density exists; charter affirms it as default.
* SPEC-HAL-092 Unified Typography Rules — typographic role system is consistent with Principle 4.
* SPEC-HAL-093 Visual Appearance and Radii System — sculpted borders, restraint on shadows.
* SPEC-HAL-094 Component Geometry — token-based geometry per density.
* SPEC-HAL-097 Motion and Micro-Animation Rules — restraint on decorative animation.
* SPEC-HAL-260A PropertyEditor — section archetype with header/subheader bands is the correct shape.

**Requires recalibration to align with charter:**

* **SPEC-HAL-012 Theming** — must set dark theme as the framework default (Principle 1).
* **SPEC-HAL-096 Color System** — surface hierarchy values need recalibration to satisfy Principle 2 (data recesses into chrome, never lifts above it). Specifically: light theme `surfaces.content` of pure white (0xFFFFFF) violates Principle 2 and must be replaced with a tone equal-to-or-darker-than the panel surface around it. Section band colors (`header_background`, `subheader_background`) must achieve at least 12-15 luminance levels of contrast against `row_background` per Principle 4 (otherwise hierarchy is invisible). The comment on `list.row_background` claiming it equals `surfaces.content` (currently false in light theme) must either be made true or removed.
* **SPEC-HAL-093 Visual Appearance and Radii System** — default control radii audit: must enforce ≤ 2px per Principle 6. Any default radius in the codebase above 2px requires reduction or written exemption.
* **SPEC-HAL-094 Component Geometry and Layout Rules** — toolbar button height, row height, and control padding defaults must be audited against Principle 5 targets.

---

## 9. Open Recalibration Items

These are the concrete actions that follow from this charter being adopted. Each becomes its own task or SPEC amendment; none are authored here.

1. **Theme default flip** — set dark theme as the framework default (SPEC-HAL-012 amendment).
2. **Light theme tonal recalibration** — rewrite `ColorSpec::light()` so chrome ≥ data, with section bands at perceptible contrast above data (SPEC-HAL-096 v4 amendment).
3. **Dark theme audit** — verify the dark theme already satisfies Principle 2 (it appears to, per current values, but a formal audit confirms it).
4. **Default geometry audit** — review every default value in `GeometrySpec`, `ListGeometrySpec`, `ScrollbarGeometrySpec`, and per-widget defaults against Principle 5 targets. List of widgets exceeding Principle 6 radius limit captured as exception list.
5. **Showcase reskin** — the showcase is Halcyon's primary public artifact. It must visibly embody this charter. Current Layout tab violates Principles 1, 2, 5, 6 visibly (per screenshot evidence from JC's review). A coordinated reskin pass aligns showcase with the charter once the underlying theme/geometry values are recalibrated.
6. **PropertyEditor section band visibility** — once SPEC-HAL-096 is recalibrated, PE Section/SubSection bands should read as distinct hierarchy without further widget changes. Re-verify after recalibration.
7. **Status bar adoption** — make a status bar the recommended bottom-of-window element in the public widget catalog (SPEC-HAL-XXX, new). The current showcase status bar is the reference implementation.

These items are listed in priority order. Items 1 and 2 are the load-bearing changes; items 3-7 follow as natural consequences.

---

## 10. Test of Adoption

This charter is adopted when:

* The dark theme is the framework default on a fresh launch (Principle 1 satisfied).
* The light theme presents data surfaces equal-to-or-darker than panel surfaces, with no pure white in default themes (Principle 2 satisfied).
* The showcase Layout tab, viewed side-by-side with a screenshot from any §3 reference application, reads as genre-consistent (genre test passed).
* No widget in the default catalog displays a corner radius above 2px (Principle 6 satisfied).
* No widget in the default catalog displays a decorative shadow (Principle 8 satisfied).
* PropertyEditor Section bands read as visually distinct hierarchy without squinting (Principles 2 and 4 satisfied).
* Every color in the default themes can be mapped to a functional role in Principle 3.

Adoption is binary. Partial adoption — flipping the default theme without recalibrating colors, for instance — leaves the framework in a self-contradictory state and is worse than no change.

---

**End of SPEC-HAL-332.**

## Disposition confirmation

The historical proposal remains REJECTED / RETIRED / INACTIVE / NOT BINDING.
Its former adoption tests and recalibration agenda cannot activate it, override
another owner, or impose a visual genre on Halcyon consumers.

