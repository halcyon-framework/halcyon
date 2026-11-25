# Halcyon Design Intelligence

**Revision:** 2026-11-23  
**Status:** Advisory design guidance; retained legacy document under SPEC-HAL-309.  
**Scope:** Professional UI-system quality, default controls, interaction design,
widget selection, and application composition. This is not an implementation
inventory, a new SPEC, or a claim of operating-system integration readiness.

## Purpose

Halcyon's objective is to become a professional, general-purpose UI system:
pleasant, precise, dependable, and coherent enough to be considered for the
everyday graphical environment of an operating system, as well as applications
for the general population and demanding professional tools.

The standard is not merely that widgets exist, tests pass, or a demonstration
runs. Ordinary controls should feel deliberately made, ordinary interactions
should be trustworthy, and repeated use should remain comfortable. A button,
text field, selector, and slider placed on a plain surface should already look
like members of one finished family. A clever screen arrangement cannot supply
quality missing from the controls themselves.

This document connects three distinct design responsibilities:

1. **Framework capability and behavior:** reusable controls, coherent semantics,
   correct rendering, dependable input, and explicit ownership.
2. **Default control design:** the visual and interactive craftsmanship of the
   standard family, before an application adds its own composition or skin.
3. **Application composition:** choosing the right controls and arranging them
   for a particular task and audience.

Professional does not mean austere, developer-only, oversized, glossy, or
decorated. It means understandable and satisfying without demanding attention
for its own sake. Beauty is welcome; decoration is not a substitute for clarity,
legibility, precision, or reliable behavior.

## 0. Objective, Authority, and Review Boundaries

### 0.1 The Operating-System Quality Benchmark

The comparison is a quality benchmark: would these controls be credible in
everyday settings, dialogs, file-oriented workflows, navigation surfaces, and
other frequently used desktop UI? Would they remain pleasant after hours of
use, with real content, different display scales, and repeated interactions?

It is not an instruction to build an operating system, window manager,
compositor, shell, or new platform backend. Those capabilities need their own
accepted authority and implementation evidence. A polished skin does not prove
that Halcyon is ready to supply an entire operating-system environment.

This objective concerns the quality experienced by users and the capabilities
available to application developers. Project participation and licensing are
documented separately in the [FAQ](guide/faq.md).

### 0.2 Guidance Is Not Framework Authority

The Constitution and accepted owning SPECs govern behavior. This document
guides design and review; it neither changes their status nor authorizes an
implementation that conflicts with them. A design goal is not evidence of a
present capability. A screenshot, comment, or existing helper is not authority.

Relevant owning boundaries include:

| Source | Boundary used here |
|---|---|
| [SPEC-HAL-002](specs/SPEC-HAL-002-Constitution-and-Architectural-Invariants.md) and [002A](specs/SPEC-HAL-002A-Constitution-Amendment-Guidelines.md) | Framework ownership, phase separation, identity, and amendment discipline. |
| [SPEC-HAL-012](specs/SPEC-HAL-012-Theming.md) | Appearance configuration and immutable resolved values; no transfer of semantic authority to skins. |
| [SPEC-HAL-208](specs/SPEC-HAL-208-Labeled-Controls-Ownership-Layout-and-Interaction-Invariants.md) and [208A](specs/SPEC-HAL-208A-Configurable-Label-Side.md) | Control-owned labels, unified hit regions, label-side policy, and the scoped default-selector ruling. |
| [SPEC-HAL-328](specs/SPEC-HAL-328-Keyboard-Authority-Focus-and-Capture.md) | Truthful framework focus/capture and application-policy-qualified visual cues. |
| [SPEC-HAL-354](specs/SPEC-HAL-354-Retained-Value-Updates-and-Scene-Fragments.md) | Retained updates and correct invalidation of changed presentation. |
| [SPEC-HAL-309](specs/SPEC-HAL-309-Documentation-System.md) | Separation of authority, implementation evidence, and explanatory documents. |

These references retain their declared statuses and scoped qualifications.
Referencing Proposed or Draft text does not adopt it. In particular, this
revision does not adopt SPEC-HAL-307's draft interactive-gauge or easing plans,
or the proposed 363/364 package. Read the exact owning contract for each task.
Canonical implementation references remain under `docs/guide/`; this file is
not a competing registry or a declaration that its quality goals are complete.

Guidance about the standard control family concerns its default design.
Applications retain their own artistic direction through the framework's actual
capabilities. The [rejected visual charter](specs/SPEC-HAL-332-Professional-Visual-Charter.md)
does not impose a universal genre or categorical motion and decoration bans.
Current component contracts retain their own scoped requirements.

### 0.3 Diagnose at the Correct Layer

Classify a finding before assigning work:

| Observation | First owner to examine |
|---|---|
| A button looks unfinished even in isolation | Its default appearance, typography, geometry, and rendering. |
| Individually refined controls look unrelated beside one another | Shared visual roles and family-level metrics. |
| Good controls form an awkward screen | Application composition and task hierarchy. |
| Correct state is displayed late or remains stale | Publication, invalidation, and retained presentation. |
| Clicking, editing, navigation, or cancellation is wrong | The owning interaction or model contract, not the skin. |
| A desired treatment is not supported by the rendering path | The rendering capability boundary, not an application workaround. |

The Showcase is a demonstration and inspection surface. Its incidental layout
is not the framework's intended product design. Neither a poor Showcase layout
nor a beautiful custom demo is sufficient evidence about the whole framework.
Keep widget review and application-composition review separate, then verify
that their results work together.

### 0.4 How to Use This Guidance

Use the interaction-first selection rules, four layout paradigms, spatial
guidance, composition patterns and decision sequence to reason about a screen.
Sections 10-12 address the default control family and review of the result.

Treat numeric thresholds as heuristics: option counts do not determine
semantics, a numeric range alone does not decide whether stepping is useful,
and no single layout suits every professional application. Control-owned labels
remain with their controls under 208/208A. Section 5 describes composition
patterns, not executable API examples.

## 1. Widget Selection — Interaction First

### 1.1 The Cardinal Rule

**Ask: "What would a human do with this value?"**

Not "what data type is it?" Does the person type or paste an exact value,
choose a named alternative, turn something on, adjust approximately, or merely
observe it? The action and meaning determine the widget. Determine when the
application applies the change as well; visual form must not promise behavior
the application does not provide.

### 1.2 TextField — Typed or Pasted Values

Prefer a TextField when the value is normally typed or pasted: port numbers,
IP addresses, URLs, hostnames, paths, UUIDs, API keys, tokens, large numbers,
and free-form text. Do not choose a numeric stepper just because storage is an
integer. Exact values need suitable validation through the owning contract,
not silent correction or substitution inferred from appearance.

### 1.3 SpinBox — Meaningful Incremental Adjustment

Prefer a SpinBox when stepping by the configured increment is a frequent,
natural action: font size, compression level, retry count, or a small thread
count. Roughly 50 meaningful values and roughly ten clicks to a likely nearby
value are review heuristics, not framework limits.

Ports, large connection limits, timestamps, versions, and values normally
pasted are poor choices merely because they are numeric. Duration or timeout
values require a decision about units, typical changes, precision, and typing;
their names alone do not settle the choice. Do not assume all users start at
zero or remove supported direct entry from an existing SpinBox.

### 1.4 Slider — Position and Feel

A Slider fits values where position in a range is meaningful: opacity,
volume, sensitivity, or visual dimensions. It supports "about here" adjustment.
Use an admitted numeric value editor alongside it when the workflow also needs
exact entry; both presentations must refer to the same value owner.

A port or arbitrary identifier is not a meaningful spatial range. A Slider
must not replace exact entry simply to make a form more visually varied.

### 1.5 ComboBox — Named Alternatives

Use a ComboBox to choose one value from a known set: environment, locale,
algorithm, or policy. Two named alternatives are still a choice, not necessarily
an on/off condition. AES-256 versus ChaCha20, for example, must not become a
boolean toggle merely because there are two entries.

Consider RadioGroup or SegmentedControl when a small set benefits from remaining
visible. For a long set, determine whether searching, filtering, or a larger
selection surface is needed and supported. Counts such as 15 or 100 are review
prompts, not automatic conversions to another control. Free text, suggestions,
and closed-list selection are distinct intents; do not substitute one silently.

### 1.6 Checkbox vs Switch — Application Timing

As a composition convention, prefer **Checkbox for a choice applied later**
and **Switch for an immediate on/off effect**. A form with Apply/Save normally
uses Checkboxes for its pending boolean preferences. A live theme or service
toggle normally uses a Switch.

The application owns persistence and application timing. A Checkbox does not
intrinsically own an unsaved-settings transaction, and a Switch does not own
the service it represents. Selection of items in a list is another valid
Checkbox use, not an Apply-gated preference. Read the control's owning SPEC.

Mixing an immediate Switch into an Apply-gated form requires an explicit,
understandable distinction. Do not imply "already changed" while secretly
waiting for Apply, or imply "not yet applied" while changing the system.

### 1.7 Gauge and LinearGauge — Read-Only Display

Gauges present externally supplied measured values. A radial Gauge suits a
prominent individual reading; a LinearGauge suits compact comparisons. A
configurable threshold belongs to an input control, not a display-only meter.

Do not infer input support from a draft or add drag interaction to improve a
demo. Choose a meaningful range, origin, units, and display size. Bidirectional
ranges and full-circle variants need intelligible labels rather than ordinary
percentage captions that disguise their meaning.

### 1.8 Sparkline — Trend at a Glance

Use a Sparkline to show the shape of a series over time. It is read-only.
Pair it with a readable identity and current value when the workflow needs
both. Do not imply precise values or time coverage that the presentation does
not communicate.

### 1.9 ProgressBar — Completion

Use a ProgressBar for progress toward a defined endpoint: download, migration,
or build. A normalized completion value is not interchangeable with ongoing
resource utilization. Prefer a Gauge or LinearGauge for capacity measurements.

### 1.10 BusyIndicator — Indeterminate Work

Use a BusyIndicator for actual ongoing work whose completion cannot presently
be quantified. It is not a decoration for an idle screen or a substitute for
an available determinate value. Do not animate an inactivity state to imply work.

### 1.11 RadioGroup, SegmentedControl, and Action Selectors

A RadioGroup makes mutually exclusive named options visible, including their
labels. A SegmentedControl suits a compact set of peer modes whose relationship
benefits from being shown together. Neither selected value nor selected mode is
the same fact as keyboard focus.

An action menu answers "what should happen?" A value selector answers "which
value?" Do not use DropdownMenu as a disguised value store or make it display
the last action as a current selection. Preserve the owning action contract.

## 2. Layout Paradigms

Choose a paradigm before placing controls. It establishes row structure, label
placement, hierarchy, grouping, and density. These are task-based patterns,
not claims that all operating systems or applications use one arrangement.

### 2.1 Settings Layout

For preferences and system settings, start with a vertical list of full-width
rows. Put explanatory content toward the leading edge and compact controls
toward the trailing edge. An optional description belongs below the principal
label; align the control with the principal line, not the whole paragraph.

Use plain section headings, subtle containers, or admitted GroupBox treatment
according to density and hierarchy. Immediate toggles and Apply-gated settings
remain distinct. Spacious does not mean forcing short tasks to scroll.

### 2.2 Form Layout

For data entry, dialogs, and Apply/Save configuration, group related fields.
Stacked labels above fields are a useful default. Place related short values
side by side; give addresses, paths, URLs, and descriptions enough width.

Two or three short fields can share a row when their labels and values fit.
This is not permission to force three columns into a narrow window. Keep a
stable action area and clearly distinguish accepting, cancelling, and applying
pending changes. Checkbox preference conventions follow section 1.6.

### 2.3 Dashboard Layout

Organize monitoring and analytical screens into information zones: prominent
readings, trends, and detailed data. Use read-only controls for measurements;
filters and date selectors are separate inputs. Priorities determine size.

Equal-priority instruments should use comparable, bounded dimensions. An
incomplete final row should not enlarge its gauges merely to consume width.
Do not let ornamental meters displace the information needed to make a decision.

### 2.4 Property Editor Layout

Inspector panels suit a compact label/control arrangement with aligned columns,
small regular gaps, sections, and overflow scrolling. Dense means economical,
not cramped, illegible, or difficult to hit. Long content needs an intentional
presentation rather than arbitrary truncation or a permanently squeezed editor.

### 2.5 Choosing the Paradigm

| Primary task | Starting pattern |
|---|---|
| Adjust preferences or system settings | Settings rows. |
| Enter related data and accept or cancel it | Form layout. |
| Observe measurements, trends, and status | Dashboard zones. |
| Inspect or edit properties beside other content | Compact property editor. |

A screen can contain subordinate regions with different purposes. Make that
hierarchy clear; do not mix patterns accidentally within equivalent rows.

### 2.6 Application Shells and Dialogs

Navigation, content, contextual tools, and status need distinct roles. Keep the
main task visible; do not surround it with equally prominent rails and cards.
Dialogs need a clear question or purpose, readable content, and explicit outcomes.
Their placement and dismissal must follow existing modal and input authority.
This guidance neither requires a new shell widget nor defines platform behavior.

## 3. Spatial Rules

### 3.1 Content-Aware Sizing

Distinguish intrinsic controls from space-consuming content. Buttons, selectors,
and compact numeric controls usually need a deliberate content-based width.
TextField, TextArea, and Slider often benefit from remaining space. Equivalent
controls may share widths for scanning; arbitrary equal division is not a rule.

Treat grow=0 for compact content and grow=1 for flexible content as a
composition starting point, not universal widget policy. Verify the actual
container and control behavior. Preserve intrinsic minimums, long content,
explicit fill-width variants, and resize behavior.

### 3.2 Horizontal Grouping

Useful form rows include first/last name, city/state/postcode, and port/protocol.
Long fields such as email, URL, path, search, and description normally need more
width. Start with at most three labeled fields, or four compact choices, per
row; reduce that count when actual content or available width demands it.

### 3.3 Vertical Rhythm and Density

Useful starting proportions are 8 logical units within sections, 16-24
between sections, 12 between related groups, and
8-12 vertical padding in comfortable settings rows. Compact inspectors may use
2-4 units between rows. These are design starting points, not new hardcoded
constants or promises that a density API already exists.

Choose a small coherent spacing scale for a recipe. Apply it through the
existing owning theme/layout mechanisms. A denser recipe must preserve text
metrics, hit areas, hierarchy, and usable spacing; it is not a geometric
shrink transform. Unexplained one-off offsets are a review finding.

### 3.4 Alignment

In settings rows, consistent trailing alignment gives compact controls a common
edge while labels share the leading edge. In forms and property editors, align
the appropriate fields and baselines. Do not confuse equal outer heights with
aligned text or a visually centered icon.

Control-owned labels remain owned by their controls. A desired aligned layout
is not permission to construct detached label/event pairs; section 5 describes
the required composition semantics.

### 3.5 Section Separation

Use the lightest separation that makes the structure clear: a text heading,
a subtle surface, or a bordered GroupBox with a legend. GroupBox is a legitimate
part of Halcyon's vocabulary, not something to remove because it looks traditional.

Use consistent treatment for equivalent groups. Different treatments may mark
different levels of hierarchy. Do not add a heading, border, shadow, and colored
slab to every section, or remove all boundaries in the name of minimalism.

### 3.6 Descriptions

Explain ambiguous settings and non-obvious consequences beside the relevant
control rather than requiring discovery through a tooltip. Omit redundant
descriptions for self-explanatory content or a deliberately compact inspector.

Use a secondary text role with adequate readability. An approximate
11-unit size is not a legibility guarantee or a universal requirement. Resolve
size, contrast, wrapping, and baseline from the chosen recipe and actual text.
Description text does not replace the control's principal owned label.

### 3.7 Contextual Warnings and Information

Use semantic roles deliberately: information for neutral context, warning for
caution, error for a condition needing correction, and success for confirmation.
Examples include restart requirements, disabled encryption, an occupied port,
and a verified connection.

State the meaning in text, not color alone. Keep the message associated with
its field without moving the editing target unpredictably. Display it when
relevant; permanently coloring ordinary controls reduces the distinction.

### 3.8 Required and Optional Fields

Choose one clear convention per form. When most fields are required, marking
the optional exceptions can be simpler; when most are optional, identify the
required ones. An asterisk needs an understandable meaning and an accessible
association, not merely a red glyph. Required does not mean erroneous.

The marker belongs to the field's labeling relationship. Do not create an
independent focus or event target merely to draw it. Implementation follows
the actual field/caption contract.

### 3.9 Value-Level Emphasis

Normal values use the ordinary content role. Capacity warnings, critical
conditions, and positive confirmations may use their semantic roles. Disabled
content remains identifiable and readable. Reserve emphasis for information
that deserves attention; not every value needs a different color.

## 4. Common-Sense Heuristics

### 4.1 Would a Human Step This Value?

Ask whether repeated nearby adjustment is natural. Retry count is a plausible
stepper; a pasted port or timestamp is not. Consider typing as well as stepping,
and do not base the decision on a fictitious journey from zero.

### 4.2 Is It Typed, Chosen, Adjusted, or Toggled?

Typed or pasted values suggest TextField. Named alternatives suggest an
appropriate selector. Spatial adjustment suggests Slider. A genuine on/off
condition suggests Checkbox or Switch with honest application timing.

### 4.3 Does It Need Precision or Feel?

Exactness and visual adjustment are different needs. When both matter, use an
admitted combined control arrangement rather than pretending one need does not
exist. Neither a color nor a formatting choice may alter the underlying value.

### 4.4 How Many Options, and What Do They Mean?

First distinguish a boolean condition from named alternatives. Then consider
visibility, label length, frequency, available space, and search needs. Cardinality
is one input to the decision; it is not the decision.

### 4.5 Is It Read-Only?

Choose a display control for observations: Gauge, LinearGauge, Sparkline,
ProgressBar, BusyIndicator, or Label according to meaning. Do not disguise a
read-only value as an input. An intentionally disabled input can show unavailable
editing, but is not the default representation of measured data.

## 5. Composition Patterns

These patterns describe composition. Implement each through verified existing
mechanisms; they do not define public APIs or establish missing capabilities. If the required layout is unavailable, identify the owning gap;
do not simulate label ownership or event routing in application code.

### 5.1 Settings Row — Owned Label and Compact Control

For a labeled Switch, the principal label and indicator remain one control
under 208/208A. A leading label can express the intended label-left/indicator-
right relationship. Distributing extra space across the row requires an admitted
control/layout contract; label-side selection alone is not a fill-width guarantee.
Do not assemble a separate principal Label, spacer, and unlabeled Switch.

### 5.2 Settings Row with Description

Keep the principal labeled control intact. Associate secondary descriptive text
below it through the admitted composition. The description explains the setting;
it does not become a substitute label or a proxy click handler. Align the
indicator with the principal line, not the description's vertical midpoint.

### 5.3 Settings Row with Warning

Show a named choice, such as an encryption algorithm, with its related warning
or restart note below the field. Two algorithms remain a named choice. The
warning has a semantic role and explicit wording, but no selection authority.

### 5.4 Required Field

Present the field label and required/optional indication together, followed by
the editor and any relevant validation. Preserve the label-to-editor association.
A marker is not a separate interactive element.

### 5.5 Stacked Labeled Field

Place an associated label above an editor for content such as a server address.
Use the owning caption/field mechanism where available, rather than assuming
any neighboring text establishes a semantic relationship.

### 5.6 Side-by-Side Fields

Give a long organization name the flexible portion of a row; size a neighboring
environment selector deliberately. Keep label placement consistent. Reflow
when the content no longer fits instead of preserving columns at unusable widths.

### 5.7 Toggle Group

Related Checkbox choices such as Use TLS, Verify certificates, and Compress may
share a row when they fit. Each Checkbox owns its label and full hit region.
Wrap or stack the group when labels grow; do not compress its indicators or text.

### 5.8 Lightweight Section Heading

Use a clear heading, optionally with a separator, when spacing and type provide
enough grouping. Do not require an unverified SectionHeader API; the pattern is
visual hierarchy implemented through the admitted vocabulary.

### 5.9 GroupBox Section

Use a GroupBox for an explicitly bounded group, especially a dense form. Keep
legend, border interruption, content padding, and adjacent groups consistent.
Preserve the established outlined-legend direction unless separately changed by
the owner; this document does not substitute another treatment.

### 5.10 Action Bar

Place Cancel and Apply/Save in a stable, clearly ordered action region. A trailing
action cluster is a useful form default. Distinguish the primary outcome without
making every action prominent. Labels describe the actual operation; placement
does not change keyboard activation, default action, or Escape ownership.

## 6. Anti-Patterns — What to Reject in Review

### 6.1 Numeric Storage Masquerading as Interaction Design

Ports, large counts, and timestamps do not become SpinBoxes because they are
numeric. Choose by intended interaction, not the serialized type.

### 6.2 Misleading Application Timing

A Switch that secretly waits for Apply, or a pending-looking preference that
changes the system immediately, presents the wrong expectation. Make timing
explicit and preserve application ownership.

### 6.3 Column Crunch

Do not keep a fixed column count once labels overlap, values become unusably
narrow, or related controls no longer read as a group. More columns do not
automatically mean higher useful density.

### 6.4 Gauge Without Size Constraint

A display meter should not expand to fill an entire screen merely because
space exists. Use a containing arrangement with intentional, bounded dimensions.
The final row's item count must not decide the instruments' importance.

### 6.5 Vertical Dump

One widget per row is not a universal composition rule. Related short fields
or choices can share space. Conversely, forcing unrelated settings into one
row is not an improvement. Group by meaning before optimizing density.

### 6.6 Universal Two-Option Toggle

Two alternatives are not necessarily a boolean. Never erase named meanings
such as algorithm A versus algorithm B to satisfy a visual shortcut.

### 6.7 One Paradigm Everywhere

Settings rows, forms, dashboards, and inspectors solve different tasks. Do not
apply a dense inspector recipe to ordinary preferences or a spacious settings
recipe to every data view.

### 6.8 Equal Flex Division for Mixed Content

A short selector and a long text entry rarely need equal shares simply because
they are siblings. Determine required content space, deliberate alignment, and
where flexibility belongs.

### 6.9 Showcase-Only Polish

Do not patch a demo's offsets, duplicate a painter, or build one-off wrappers to
hide a framework control defect. Fix the owning mechanism. Conversely, do not
rewrite a correct framework primitive merely to repair one application's layout.

### 6.10 Decoration Instead of Craftsmanship

Extra gradients, shadows, borders, animation, and oversized spacing cannot
repair weak proportions or ambiguous states. Equally, stripping every boundary
can make controls indistinguishable. Restraint is deliberate visual structure,
not emptiness or the minimum number of paint commands.

### 6.11 Screenshots or Tests Used as Universal Proof

A screenshot cannot prove editing, cancellation, or sustained operation. A
green test cannot establish that text is comfortable to read or that a button
looks finished. Do not let either kind of evidence stand in for the other.

## 7. Decision Sequence — Widget Selection

1. Is the content an observation, a command, or an editable value?
2. For an observation, choose the display according to current value, trend,
   determinate progress, or indeterminate work.
3. For a command, choose an action surface; do not invent a persistent selection.
4. For a value, distinguish typed/pasted content, named choice, approximate range,
   and genuine on/off condition.
5. Establish precision, application timing, availability, and expected content.
6. Select the existing control whose accepted contract matches those needs.
7. Choose the composition paradigm and size the control for the actual content.

Do not let the desired visual variety reverse this sequence.

## 8. Spatial Awareness and Review Protocol

### 8.1 Establish the Actual Surface

Establish window size, display scale, content length, theme and current state
from the actual source geometry and rendered surface. Do not invent measurements
from appearance. Use the display and input configuration relevant to the claim
being reviewed.

### 8.2 Choose the Task and Paradigm

Identify what the person is trying to accomplish and choose section 2's starting
pattern. Record whether the review targets a control, its visual family, or a
complete application surface. A Showcase inventory is not automatically a form.

### 8.3 Plan Hierarchy and Sections

Identify the principal task, supporting information, grouping, and actions.
Choose separation and density for those relationships. Do not fill empty space
merely to improve a utilization percentage.

### 8.4 Assign Widgets by Meaning

Apply section 7. Verify exact control authority before adding interaction,
changing activation timing, or turning a display into an input.

### 8.5 Assign Proportions and Behavior at Limits

Determine intrinsic content widths, flexible areas, and intentional bounds.
Plan narrow-window reflow or scrolling, long-label treatment, and enlarged text.
Keep these decisions in the correct layout/text owners, not paint-time patches.

### 8.6 Compose Through Existing Mechanisms

Reuse the admitted controls, layout, theming, and state ownership. Do not add
hidden child controls or a parallel visual-state engine to obtain a preferred
appearance. A missing mechanism is an explicit design question, not permission
to improvise a shortcut.

### 8.7 Evaluate the Rendered Result

Inspect the actual rendered result and exercise the intended interaction on
the relevant target. Review both resting appearance and state transitions.
Change the owning design when the result fails the objective; do not accept it
because the code matches a sketch or because a test passed.

## 9. Future Directions — Candidates, Not Capability Claims

Candidate design directions include content-based selector sizing,
responsive arrangements, reusable settings/form/dashboard patterns, density
recipes, named text-color roles, and associated required-field presentation.
Their presence here establishes neither current absence nor implementation.
Verify the current owner and supported surface before assigning work.

Do not build a template engine, automatic layout scorer, new theme registry,
or a density subsystem merely because this document names a design need. Use
existing mechanisms first. Any automation of composition must preserve explicit
application intent and must not infer semantic actions from visual patterns.

## 10. Default Control Craftsmanship

### 10.1 A Finished Family Before a Finished Screen

Review a basic Button, TextField, Checkbox, RadioGroup, Switch, Slider, and
selector together on an ordinary surface. Their type, spacing, indicator weight,
border treatment, and state emphasis should feel related without making all
controls geometrically identical. Then evaluate them in realistic layouts.

This family is Halcyon's default design, not the maximum expressiveness of the
framework. Product skins and custom painting may have a different character
through admitted appearance contracts while retaining semantic behavior and
ownership. A family-level correction should not require application-specific
copies of standard widget painters.

### 10.2 Resting Appearance Is a First-Class Result

The idle state should communicate what can be pressed, edited, selected, or only
read. Controls should look intentional before hover or animation supplies extra
feedback. Evaluate balanced text padding, visible but restrained boundaries,
proportionate indicators, and consistent placement of icons and adornments.

Do not make normal enabled controls resemble disabled ones. Do not make a
secondary action look unavailable merely to emphasize the primary action.
No state should require discovering a hover effect just to recognize its role.

### 10.3 Typography and Optical Balance

Establish related roles for control labels, values, secondary information,
section headings, and validation. Use the existing text metrics and shaping
owners; align baselines rather than compensating with unrelated widget offsets.

Review the actual glyphs and content: capitals and lowercase, digits, short and
long labels, fallback glyphs, and supported writing systems. Enlarging a control
must not leave its label visibly stranded; enlarging text must not clip its
ascent, descent, selection, or caret. Icons need compatible visual weight.

Optical review informs the design, but cannot bypass canonical layout or the
text pipeline with paint-only geometry corrections. A useful size relationship
belongs in the owning metrics, not in one Showcase instance.

### 10.4 Surfaces, Borders, and Geometry

Differentiate application background, editable surface, actionable surface,
selection, and overlay with a small intentional hierarchy. Use borders where
they clarify a boundary. Use subtle depth only where it clarifies hierarchy or
interaction; neither flatness nor simulated material is an automatic virtue.

Keep the established restrained-rounding and outlined-legend direction. Round
shapes remain appropriate for circular indicators and admitted tracks. Avoid
imposing one radius on every semantic shape or changing the default palette as
an incidental side effect of fixing a control.

At supported scales, equivalent strokes should have comparable visual weight;
joined edges, corners, clipping, and text/background layering should be clean.
Only the owning renderer and coordinate contracts determine how this is achieved.

### 10.5 State Feedback Without Visual Noise

Review the supported resting, hovered, pressed, selected, focused, captured,
disabled, and invalid conditions as distinct facts. Inspect the combinations
that actually occur. Do not build a fictitious exhaustive state matrix.

Feedback should be immediate enough to identify the current state, stable enough
to follow, and restrained enough not to compete with content. Hover should clear
when it no longer applies; a released or cancelled press should not remain
visually armed. Disabled content should remain readable while clearly inactive.

Motion may clarify a transition, not delay the semantic result or the only
required cue. It must settle through the existing timing and lifetime owner.
For standard controls, assess the purpose and ongoing cost of animation and
repaint demand; decorative motion alone does not establish a useful default. Reduced-motion needs must be assessed without
claiming a preference or API exists before it is verified.

### 10.6 Preserve the Default-Selector Ruling

Under SPEC-HAL-208 INV-D3, the default Checkbox,
RadioButton/RadioGroup options, and Switch have **no additional enclosing focus
rectangle around indicator, gap, and label**, and require no substitute
navigation-focus ornament. Preserve their square/tick, circle/dot, track/thumb,
and existing indicator modulation. Do not reintroduce the enclosure under
another name in the pursuit of consistency.

This does not equate selected value with focus, remove keyboard navigation,
split label hit regions, change capture, or suppress other controls' required
cues. Focus/capture eligibility and truthful projection remain governed by
208/208A/328, including PointerDriven behavior. Do not turn this exception into
a blanket rule that every control must lose its focus indication.

### 10.7 Review Each Common Control on Its Own Terms

| Family | Visible craftsmanship to establish |
|---|---|
| Buttons | Clear actionable face; balanced label/icon padding; related primary and secondary treatment; unambiguous hover, press, disabled, and required focus/capture states. |
| Text and numeric editors | A readable editing surface; balanced text insets; stable caret/selection; coherent attached buttons or units; clear validation without hiding required ownership cues. |
| Checkbox, radio options, Switch | Proportionate indicators; crisp value signs; owned, aligned labels; unified click region; no reinstated full-content enclosure. |
| Sliders and range controls | Legible track and thumb relationships; readable range/current value where admitted; aligned numeric editors; clear endpoint and active-thumb feedback. |
| Segments and tabs | Clear selected item; consistent separators and text; distinct internal focus target where required; deliberate overflow and no stale hover. |
| Menus, popups, and tooltips | Readable content and availability; appropriate separation from the underlying surface; coherent row spacing; correct placement, layering, and dismissal. |
| Groups, lists, trees, and inspectors | Consistent hierarchy and rhythm; aligned rows/columns; readable selection and empty states; clear association between content and controls. |
| Gauges and progress displays | Bounded size; readable values, units, scale and ticks; truthful determinate/indeterminate meaning; calm presentation when data is unchanged. |

The table names review outcomes, not new behavior specifications or claims that
all variants are implemented. Resolve missing authority before changing policy.

## 11. Behavioral Trust and General-Purpose Readiness

### 11.1 Complete Everyday Interactions

A professional control is more than its successful click path. Review the
admitted enter/edit/select/replace/accept/cancel sequence, focus transfer, pointer
release outside the target, and interruption of an in-progress interaction.
Retain input data and stable identity according to the owning contract. Never
interpret aesthetic polish as permission to change activation timing.

Text editing needs truthful selection, caret, clipboard, undo, and composition
behavior where supported. Menus and dialogs need clear outcomes and correct
ownership after dismissal. Missing capabilities must remain explicit gaps, not
be covered by an assertion that the UI is generally professional.

### 11.2 Legibility, Access, and Real Content

The general-population objective includes people who use larger text, keyboard
navigation, assistive technology, or supported alternative input paths. Review
names, roles, values, and state exposure at their actual framework/platform
boundary, not just the visible pixels. Color should not be the sole carrier of
important meaning. A tooltip should not be the only way to discover essential
instructions.

Use realistic content lengths and the supported scripts, text scales, display
scales, and themes. Distinguish supported behavior from unverified or absent
coverage. Do not claim accessibility or localization completeness, numerical
contrast compliance, or cross-platform parity without the corresponding proof.
This guide does not authorize new accessibility, text, or platform architecture.

### 11.3 Resize, Continuity, and Long Sessions

Ordinary resizing, narrow windows, scrolling, tab changes, and repeated overlay
use should not crash, lose input, detach selection from identity, or leave stale
pixels. At smaller sizes, use the admitted reflow/overflow behavior instead of
letting control parts overlap or vanish without explanation.

Repeated work should settle when it is finished. Stable screens should reuse
unchanged presentation; live screens should perform only their admitted work.
Measure input-to-visible response, frame pacing, sustained resource behavior,
and idle demand on the actual target when making those claims. A frame-rate
counter alone proves neither smoothness nor efficiency.

### 11.4 Framework Quality Must Survive Customization

The default recipe should be useful without per-application repair. An admitted
custom recipe should not require replacing semantic controls, inventing hidden
globals, or duplicating input and layout logic. Preserve typed boundaries and
honest unsupported-capability outcomes.

Demonstrate the intended degree of customization on the same controls with
preserved behavior before claiming it. This is not an instruction to implement
every material family, styling mechanism, platform, or future extension now.

## 12. Review Sequence and Acceptance

### 12.1 Start with Frequent Controls

Prioritize the controls people see and touch continually: buttons, text and
numeric editors, selectors, sliders, tabs, and menus. Establish their family
relationships before spending effort on decorative demonstrations. Then examine
complete settings/form workflows and dense professional surfaces.

Each unit should resolve one concrete quality issue at its correct owner while
preserving working behavior. A useful refinement can cross related controls;
a tiny edit is not automatically the right unit, and a broad redesign is not
automatically progress. Do not create an endless audit of hypothetical states.

### 12.2 Three Complementary Views

**Control view:** examine the actual resting control and its supported states on
ordinary surfaces. Judge typography, proportions, role clarity, and feedback.

**Task view:** exercise a small realistic workflow using those same controls,
such as changing and applying a preference or editing and cancelling a value.
Judge coherence, discoverability, input preservation, and outcomes. Do not
accept a separate one-off painter as proof of the framework control.

**Sustained-use view:** repeat meaningful transitions, resize and scroll within
the supported contract, and let the surface settle. Judge continuity and
resource behavior using appropriate evidence rather than an idle screenshot.

Use existing review surfaces and proof mechanisms where they cover the claim.

### 12.3 Evidence Must Match the Claim

| Claim | Necessary evidence boundary |
|---|---|
| The intended source change exists | Actual final source and scoped review. |
| Interaction and state preservation are correct | Executed, source-bound tests of the affected intended paths. |
| The intended primitives, geometry, and retained replacements occur | Actual scene/retained-path observations. |
| The controls look refined and readable | Review of actual rendered output at the relevant size, scale, theme, content, and state. |
| Motion and sustained use feel correct | Live observation and suitable timing/resource evidence on the intended target. |
| A platform or integration capability is supported | Its governing contract, implemented behavior and target-specific evidence. |

For a visual capability, record the source identity and semantic owner; the
renderer, target and timing owners; the source data and interaction behavior
that must remain unchanged; and the relevant resource, cancellation and
lifetime limits. Keep authority, implementation, backend execution, native
rendered output and physical interaction evidence distinct. The applicable
[3D substrate](specs/SPEC-HAL-355-Native-3D-Rendering-Substrate.md),
[selection outline](specs/SPEC-HAL-374-Source-Qualified-3D-Selection-Outline-and-Halo.md)
and [geometry admission](specs/SPEC-HAL-380-Nonblocking-PBR-Geometry-Certification-and-Admission.md)
contracts illustrate these ownership and evidence boundaries; their declared
statuses and exact scopes remain controlling.

Generated concepts and mockups can communicate a design direction; they do not
prove that Halcyon renders it. Headless primitive checks do not establish native
visual quality. Captures support visual review within their visible scope.

### 12.4 Acceptance Is Not an Average Score

Do not trade correctness for attractiveness, legibility for density, or clarity
for minimalism. A polished surface with broken editing is not finished. Neither
is a reliable control whose default appearance still looks accidental.

Record the result as one of: accepted for its named scope and evidence,
needs correction with a concrete finding, or unverified with the missing
evidence named. Do not call a whole widget, family, or framework complete from
one passing filter. Do not make a writing task an implementation-completion claim.

The maintainer defines the default visual direction and accepts changes to the
standard family. Application authors define their product's design. Reviewers
evaluate each scope against that direction and its governing contracts.

### 12.5 The Standard to Hold

Halcyon is approaching the intended standard when ordinary controls are refined
without special application fixes, related controls feel like one family,
complete everyday tasks behave predictably, and supported rendering and input
remain dependable during sustained use.

The practical question is: **would we choose these same controls, without
apology or a replacement skin to hide defects, for an interface people must
live with every day?** That is the direction of this document, not a claim
that the answer is already yes.