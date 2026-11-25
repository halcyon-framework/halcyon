---
doc-id: halcyon-guide-design-rationale
class: first-class
authority: explanatory-non-binding
halcyon-revision: 51beb5c3e5a1c86b3cc7b75e1c94cb4b4c9f6c71
verified-at: 2026-11-24
governing-specs:
  - SPEC-HAL-000
  - SPEC-HAL-001
  - SPEC-HAL-002
  - SPEC-HAL-002A
  - SPEC-HAL-010
  - SPEC-HAL-018
  - SPEC-HAL-266A
  - SPEC-HAL-295
  - SPEC-HAL-295A
  - SPEC-HAL-309
  - SPEC-HAL-354
  - SPEC-PM-000
  - SPEC-PM-001
  - SPEC-PM-027
---

# Halcyon: Design Philosophy and Architectural Rationale

Halcyon is a retained-mode, GPU-first UI framework written in Rust. Its
architecture emphasizes explicit ownership, predictable behavior, and the
ability to develop complex, long-lived applications without allowing individual
features to undermine the coherence of the system.

This document explains the reasons behind those choices. It is not a new
specification, a claim of superiority over other frameworks, or a promise
that every intended capability is already implemented.

## 1. Why another UI framework?

Halcyon began as a framework for the author's own applications, rather than
as an attempt to fill a perceived gap in the open-source ecosystem. Its
initial requirements came from demanding desktop applications, including
trading interfaces with frequently changing data, charts, large collections,
complex layouts, and sustained user interaction.

Another important consideration from the beginning was the development of
applications that people use throughout their working day. Such applications
place particular demands on responsiveness, interaction quality, visual
clarity, ergonomics, and stable resource consumption over extended periods
of continuous use.

Such applications raise questions that go beyond drawing controls. How does
an interface preserve selection when a list changes? How can rapidly arriving
data be reflected without repeatedly rebuilding unaffected structure? How do
focus, navigation, and rendering remain coherent while multiple subsystems
evolve independently?

These requirements shaped Halcyon's emphasis on stable identity, retained
structure, explicit state transitions, and GPU rendering.

From the outset, one of the benchmarks for Halcyon's maturity was whether
it could eventually merit serious consideration for use in financial and
trading applications, and even in medical applications. The significance of
these domains lies in the level of correctness, reliability, and
predictability they can demand, rather than in any restriction on the
framework's intended audience.

This is a long-term engineering aspiration, not a claim of present
suitability for safety-critical applications or compliance with regulatory
requirements. Such suitability must be established for each intended use.

Halcyon is intended as a general-purpose UI framework. The underlying
architectural challenges are common across many application domains, from
editors and monitoring tools to management applications and complex
data-intensive workbenches.

Visual quality and expressiveness are design goals in their own right.
The work already undertaken on 2D, 2.5D, and 3D rendering, physically
based materials, lighting, shadows, reflections, and glass effects reflects
the intention to support visually sophisticated applications alongside
conventional user interfaces.

This opens possibilities ranging from everyday consumer applications and
creative tools to interactive visualizations and immersive graphical
experiences. Games and game-related interfaces are not excluded from the
longer-term direction, although game development has not yet been a
dedicated area of development.

Cross-platform development is an important part of this vision. The aim is
to carry a common UI architecture across different presentation environments,
rather than develop an independent framework for each platform. Native desktop
applications have been the primary focus. Halcyon already runs in browsers
through WebAssembly, and early experiments have also explored its use in
mobile applications, including iOS. These platforms are at different stages
of development and validation.

Halcyon is designed with the future in mind. The goal is to take advantage
of advances in graphics hardware, rendering techniques, and development
tools, enabling new kinds of applications and visual experiences without
compromising the framework's architectural foundations.

Different UI frameworks make different choices about these problems. Halcyon
represents a particular set of engineering priorities, not a judgment that
other approaches are inherently wrong.

See [HAL-000, Design Principles](../specs/SPEC-HAL-000-Glossary-Naming-Principles.md#6-design-principles),
[HAL-001](../specs/SPEC-HAL-001-Core-Architecture.md), and
the [development history](../../README.md#development-history).

## 2. Why retained mode?

A control is more than what is visible at a particular instant. It can have
interaction state, focus, an active edit, animation state, cached measurements,
and relationships to other elements.

Halcyon therefore models the interface as a persistent WidgetTree. Changes
to application data do not inherently require replacing widget instances.
Rebuilds are permitted, but they must respect the identity and lifetime of
the objects being reconciled.

Consider a data-backed list in which a value changes while the user is editing
another control. The change in data is not, by itself, a reason to discard
the editor, reset focus, or reconstruct unrelated layout.

Retaining structure allows the engine to distinguish changes in values,
geometry, and membership. It also creates obligations: lifetime management,
identity reconciliation, invalidation, and cache correctness must be handled
explicitly. Retained mode is not automatically faster or simpler merely
because objects persist.

Halcyon's chosen response is to make those obligations part of the architecture
rather than leave them to individual application or widget implementations.

See [HAL-002, Pillars A and C](../specs/SPEC-HAL-002-Constitution-and-Architectural-Invariants.md)
and [HAL-354](../specs/SPEC-HAL-354-Retained-Value-Updates-and-Scene-Fragments.md).

## 3. Why GPU-first?

GPU rendering is part of Halcyon's architectural model, rather than a later
optimization applied to a fundamentally different drawing system.

The intended separation is between the semantic and geometric description
of an interface and the GPU work needed to present it. Widgets describe
visual content through the framework's scene mechanisms. The renderer
consumes an authorized, stable description and manages its own execution
and resources.

This gives ordinary UI drawing and more specialized graphics a common
composition boundary. A complex chart, for example, can use an admitted
custom GPU pipeline while remaining part of the interface's ordinary
geometry, clipping, and draw order.

GPU-first does not mean moving application logic, input interpretation, or
widget ownership onto the GPU. Nor does it mean that every application
developer must write shaders.

The approach has costs: GPU resource lifetimes, synchronization, backend
differences, shader correctness, text rasterization, and presentation all
require careful handling. Accelerating drawing alone cannot resolve
incorrect ownership or excessive work elsewhere in the frame.

See [HAL-002, Pillar E](../specs/SPEC-HAL-002-Constitution-and-Architectural-Invariants.md),
[HAL-295](../specs/SPEC-HAL-295-Custom-Render-Pass.md),
its [authority reconciliation](../specs/SPEC-HAL-295A-Custom-Render-Pass-Authority-Reconciliation.md),
and [Rendering and Compositor](rendering-and-compositor.md).

## 4. Why does Halcyon have a Constitution?

In a complex framework, a change can be locally correct and globally wrong.

A widget might appear to work after directly changing another widget's state.
A callback might successfully replace a screen while an event is still being
routed. A rendering shortcut might produce the expected pixels while reading
geometry from a different stage of the frame. Each local result can conceal
a violation of the assumptions on which other subsystems depend.

The Constitution, SPEC-HAL-002, defines the common architectural invariants
that all subsystems must respect. It places constraints on ownership,
identity, mutation, phase separation, rendering snapshots, and semantic
authority.

Its purpose is not to describe every widget or predict every future feature.
It establishes the boundaries within which features may be designed and
implemented. In particular, a subsystem-specific specification cannot
silently override a constitutional invariant.

This gives the framework a stable basis for reviewing changes across
subsystem boundaries. A proposal can be evaluated not only by asking whether
it works in isolation, but also by asking whether it preserves the
assumptions that make the rest of the system coherent.

The Constitution is an architectural correctness criterion. It is not proof
that an implementation conforms to it, and it is not a guarantee against
bugs, deadlocks, performance problems, or unsafe application behavior.

Its value depends on whether the invariants are reflected in implementation,
review, and executable evidence.

Read the [Constitution](../specs/SPEC-HAL-002-Constitution-and-Architectural-Invariants.md)
and its [amendment guidelines](../specs/SPEC-HAL-002A-Constitution-Amendment-Guidelines.md).

## 5. Why so many specifications?

Source code describes what a particular implementation does. It does not
necessarily explain which behavior is intentional, which assumptions other
components may rely on, or which seemingly harmless changes would invalidate
those assumptions.

Halcyon's specifications make important contracts explicit. They give
subsystems defined responsibilities, vocabulary, invariants, and boundaries
against which an implementation can be assessed.

The point is not to specify every implementation detail or require a new
document for every change. It is to preserve the reasoning needed to evolve
a system whose components interact in ways that are difficult to recover
from code alone.

The development approach also looks ahead to increasingly capable AI models.
Explicit contracts, well-defined boundaries, and structured documentation
provide reusable engineering knowledge that both human developers and AI
agents can work with. As those tools improve, the intention is to make
complex software easier to develop, extend, and verify without sacrificing
architectural coherence.

This is a direction for the engineering workflow and the developer
experience, not a dependency on any particular AI model or a requirement to
use AI when building or running an application. The documentation system
explicitly accounts for AI-assisted development, while the maintainer
remains responsible for architectural decisions and acceptance.

A specification and an implementation are separate facts. A Draft records
intent; an adopted contract establishes requirements within its scope.
Source code shows what has been implemented. Tests are central to
verification: they check behavior against the specified contracts and help
prevent regressions. Direct observation of the running system provides
further evidence. Neither the existence of a SPEC nor its level of detail
establishes that the corresponding subsystem is complete.

When documentation and implementation disagree, the discrepancy must remain
visible until it is resolved. The implementation does not silently change
the governing contract, and the contract does not make a missing implementation
appear complete.

The [documentation index](index.md) explains this hierarchy. The
[contradiction register](audits/contradictions.md) records identified
discrepancies, and [HAL-309](../specs/SPEC-HAL-309-Documentation-System.md)
defines the documentation and evidence boundaries.

## 6. Why strict ownership and identity rules?

The WidgetTree is the sole owner of widget instances. Other components
identify UI objects through the framework's identifiers rather than keeping
persistent references to those objects.

This addresses a structural problem: a widget may be moved, replaced, or
removed while other parts of the interface still need to refer to its
logical role. A persistent cross-widget reference can turn a local structural
change into a lifetime problem elsewhere in the application.

IDs separate identification from access and ownership. A reference may be
used within its permitted scope, but it does not become an independent
authority over the object's lifetime.

Stable identity also matters independently of Rust's borrowing rules.
For a virtualized list, selection must refer to the logical item, not to the
currently visible row that happens to display it. Scrolling an item out of
view does not mean that the item has ceased to exist.

Rust provides valuable tools for memory and lifetime safety. The architectural
rules address additional questions that the language cannot answer by itself:
who owns a UI object, what its identity means, and which component is allowed
to change its state.

See [HAL-002, Pillar C and sections 3–4](../specs/SPEC-HAL-002-Constitution-and-Architectural-Invariants.md),
[HAL-010](../specs/SPEC-HAL-010-Widget-System.md) (Draft), and
[PM-001](../specs/SPEC-PM-001-Virtualization-Identity-and-Invisible-Selection-Failure.md).

## 7. Why explicit frame phases?

Events, structural changes, layout, scene construction, and rendering have
different responsibilities. They also have different requirements for
reading and changing state.

Halcyon's constitutional frame model orders event intake, deferred mutation,
rebuild, layout, scene construction, and GPU rendering. Later contracts
refine the work within that model.

The reason is to ensure that downstream stages operate on an appropriate,
coherent input. For example, if an event requests navigation, applying that
request at the designated mutation boundary avoids changing the tree while
the current routing operation still depends on it.

Similarly, rendering must consume geometry from the corresponding layout
result, rather than recomputing or altering shared semantic state as a side
effect of drawing.

An ordered model does not require every frame to repeat all possible work.
HAL-354 distinguishes changes that affect paint from changes that affect
layout or structure, and defines when retained scene values may be reused.

The optimization question is therefore not how to bypass a phase, but how
to establish that the phase has no relevant work to perform.

See [HAL-002, Pillar B and section 5](../specs/SPEC-HAL-002-Constitution-and-Architectural-Invariants.md),
[HAL-354](../specs/SPEC-HAL-354-Retained-Value-Updates-and-Scene-Fragments.md), and
[Runtime and Frame Lifecycle](runtime-and-frame-lifecycle.md).

## 8. Why explicit rather than implicit?

Halcyon favors explicit ownership, data flow, and actions over hidden global
state or automatic propagation whose effects are difficult to trace.

An important distinction is the difference between an indication of intent
and an authorized state change. A highlighted suggestion, a focused item,
or a transient pointer movement is not necessarily a decision to modify
application data. A framework should not infer an irreversible operation
from an intermediate interaction state.

Explicitness does not mean that every application must manually implement
the engine's internal machinery. Reusable abstractions, composite widgets,
convenient public APIs, and future scaffolding are useful precisely because
they can encapsulate repeated work while preserving its ownership and meaning.

The aim is to make abstractions easier to reason about, not to prohibit
abstraction.

See [HAL-000](../specs/SPEC-HAL-000-Glossary-Naming-Principles.md) and
[HAL-002, Pillar D and section 4.11](../specs/SPEC-HAL-002-Constitution-and-Architectural-Invariants.md).

## 9. Where does application state belong?

The application owns its domain data and business decisions. Halcyon owns
the UI mechanisms that receive input, interpret widget interaction, and
project authorized state into the interface.

This distinction is particularly important for asynchronous data. A network
worker can produce a new result without acquiring authority to mutate a
mounted widget, reorganize a screen, or schedule immediate layout and
rendering.

HAL-354 specifies synchronized data ingress, engine-owned update cadence,
and semantic invalidation. A value update that leaves geometry unchanged
does not inherently require layout. Repeated changes may be coalesced within
the existing update and invalidation model.

The purpose is to preserve the application's freedom to organize its data
and concurrent work without exposing the retained UI structure to uncontrolled
mutation or coupling the frame rate directly to the data arrival rate.

A practical application should therefore distinguish domain state, UI
projection, and interaction actions. Halcyon does not prescribe the
application's business model or require domain logic to live in widgets.

See [HAL-002, Pillar D](../specs/SPEC-HAL-002-Constitution-and-Architectural-Invariants.md),
[HAL-354](../specs/SPEC-HAL-354-Retained-Value-Updates-and-Scene-Fragments.md),
and [Data and Update Patterns](data-and-update-patterns.md).

## 10. Why engine sovereignty?

Halcyon distinguishes platform transport from the meaning of an interaction.

Operating systems and browsers provide input delivery, windows or other
surfaces, presentation, and integration services. Those services are essential,
but they do not replace the framework's own authority over widget focus,
capture, modality, routing, layout, composition, draw order, or frame
execution.

For example, a platform-level focus notification is an input to the
framework's focus management. It is not, by itself, permission to select an
arbitrary widget or bypass an active interaction domain.

The reason for this separation is to avoid multiple independent owners
making conflicting decisions about the same interface. It also gives
platform-specific integration a common semantic model to work with.

Engine sovereignty does not mean ignoring the operating system, refusing
native services, or insisting that every platform look and behave identically.
Platform capabilities and constraints remain real. Clipboard integration,
input methods, accessibility bridges, and other native services can be
necessary parts of an application; their interaction with Halcyon must have
a defined authority boundary.

Consistent semantic ownership is an architectural objective, not a claim
that platform parity has already been achieved.

See [HAL-002, Pillar F](../specs/SPEC-HAL-002-Constitution-and-Architectural-Invariants.md),
[HAL-001, section 2.5](../specs/SPEC-HAL-001-Core-Architecture.md), and
[Input, Events, Focus, and Actions](input-events-focus-and-actions.md).

## 11. How extensible is Halcyon?

An extension should be able to add capabilities without having to become
a second owner of the whole framework.

The first extension path is ordinary composition: combine existing widgets,
application data, and explicit actions into a project-specific control.
A more general reusable control can then expose its own clear interface
without requiring knowledge of the original application's domain model.

Custom widgets may use the established lifecycle, layout, interaction,
and painting mechanisms. Where a distinct interaction policy is needed,
its semantics belong to the appropriate widget or existing shared
interaction contract rather than to the visual style.

Specialized rendering has a separate extension boundary. HAL-295,
HAL-295A, and HAL-266A define an explicitly admitted offscreen producer
that uses its own resources, observes the engine's target preparation
boundary, and publishes a sampled image to ordinary composition.

This provides a way to develop GPU-rendered content using custom WGSL
shaders without granting an extension ownership of renderer-private
attachments, presentation, or unrelated UI state. It is a bounded extension
mechanism, not unrestricted access to the renderer.

The design intentionally places constraints on extensibility. Allowing
arbitrary access to internal state might make one extension easier to write,
but would weaken the assumptions on which unrelated components depend.

Practical guides, examples, recommended project structures, styling,
theming, and scaffolding are further development priorities, as recorded
in the [Roadmap](../../ROADMAP.md). Their inclusion there does not establish
that every proposed guide or tool already exists.

See [HAL-002, section 9](../specs/SPEC-HAL-002-Constitution-and-Architectural-Invariants.md),
[HAL-295](../specs/SPEC-HAL-295-Custom-Render-Pass.md),
[HAL-295A](../specs/SPEC-HAL-295A-Custom-Render-Pass-Authority-Reconciliation.md), and
[HAL-266A](../specs/SPEC-HAL-266A-Per-Target-Custom-Render-Preparation.md).

## 12. Can the architecture evolve?

Yes. It is important to distinguish the framework's foundations, its
subsystem contracts, and its implementation.

The permanent constitutional invariants define the framework's architectural
identity. HAL-002A identifies principles that cannot be removed or weakened,
including sole WidgetTree ownership, stable identity, deferred mutation,
phase separation, and snapshot-based rendering.

Other constitutional provisions may be clarified or extended, and structural
or emergency amendments may be considered within the defined process.
Dependent specifications must be reviewed and reconciled, and permanent
invariants must remain intact.

Subsystem contracts can evolve within those boundaries. The implementation
can change to correct defects, improve performance, add features, or better
satisfy existing contracts. Public APIs are not frozen merely because the
architecture has explicit foundations.

This distinction allows Halcyon to have a stable architectural identity
without treating its present design details or beta implementation as final.

See [HAL-002A](../specs/SPEC-HAL-002A-Constitution-Amendment-Guidelines.md),
[HAL-002, section 10](../specs/SPEC-HAL-002-Constitution-and-Architectural-Invariants.md),
and the [Roadmap](../../ROADMAP.md).

## 13. What is the PM specification series?

The Pathology and Mistake Prevention (PM) series records recurring classes of
engineering failure. It is separate from feature design. Its purpose is to
preserve knowledge of why a failure occurred and what must not be repeated.

Two examples illustrate the distinction:

- [PM-001](../specs/SPEC-PM-001-Virtualization-Identity-and-Invisible-Selection-Failure.md)
  describes selection corruption caused by treating visible rows as the
  universe of logical items. The underlying mistake is conflating visibility
  with existence. Fixing the symptoms of one list would not address the
  same mistake in another virtualized collection.
- [PM-027](../specs/SPEC-PM-027-Silent-Required-Text-Refusal.md)
  describes a presentation failure in which a UI update was accepted but
  required text could not be realized, leaving an earlier frame visible.
  The relevant lesson is to distinguish state mutation, frame compilation,
  successful output, and actual presentation. One cannot establish the
  others by inference.

A PM document is not a declaration that every instance of the failure has
been eliminated. It gives future work a concrete failure class and prevention
rules against which behavior can be checked.

The broader goal is to retain engineering knowledge as the framework grows,
instead of repeatedly paying for the same lessons in different subsystems.

See [PM-000](../specs/SPEC-PM-000-Pathology-and-Mistake-Prevention-Series.md)
for the purpose, scope, and binding role of the series.

## 14. What are the trade-offs?

Halcyon's architecture is a choice, not a free advantage.

| Choice | Intended benefit | Cost or obligation |
| --- | --- | --- |
| Retained widgets and stable IDs | Preserve identity and state; reuse unaffected structure | Explicit lifetime, reconciliation, and invalidation management |
| Phase-separated execution | Keep input, mutations, geometry, and rendering coherent | Changes must occur at the appropriate boundary |
| GPU-first rendering | Compose UI and specialized graphical content through a common visual model | Resource management, synchronization, backend qualification, and rendering complexity |
| Engine-owned semantics | Avoid competing authorities for interaction | Platform integration and behavioral testing require substantial work |
| Explicit contracts and PM rules | Preserve reasoning across subsystems and over time | Specifications, implementation, and evidence must be kept aligned |
| Bounded extension mechanisms | Allow specialized components without sacrificing architectural ownership | Extensions must respect the framework's lifecycle and resource boundaries |

These costs are particularly relevant while Halcyon is in beta. Some
application-facing APIs, examples, integration workflows, styling facilities,
and platform support still require further development.

For ordinary applications, the goal is to provide abstractions that make
the architecture practical to use. A small application should not require
its author to understand every internal subsystem. Developers extending the
framework at lower levels need a more detailed understanding of the
boundaries they are using.

The value of the architecture must ultimately be demonstrated by usable
applications and reliable observed behavior. A well-written specification
does not substitute for either.

## Reading further

- [Getting started](getting-started.md): build and run an application.
- [Architecture overview](overview.md): understand the structure of the engine
  at the guide's recorded revision.
- [FAQ](faq.md): practical questions, project policies, and present limitations.
- [Roadmap](../../ROADMAP.md): further development directions.

The [documentation index](index.md) remains the canonical guide registry.

## Sources and scope

This guide explains design choices from the governing documents and the
maintainer's account of the project's origins. Its source basis is the
revision named in the metadata, with the inspected public-documentation
wording prepared on 2026-11-24. The prepared 0.270.3-beta source candidate
also contains a subsequent narrow font-loading correction; this rationale
does not requalify that implementation or advance other guide pins.

The sections describe requirements, motivations, and trade-offs, not a new
implementation audit. Under HAL-309, authority and implementation status
remain separate:

| Subject | Authority and source | Implementation scope |
| --- | --- | --- |
| Design priorities and application origins | HAL-000, HAL-001; maintainer's development history and stated original goals, 2026-11-24 | Design intent; not-applicable to historical rationale |
| Visual expressiveness, long-running applications, cross-platform direction, and future development | Maintainer's stated design goals, 2026-11-24; HAL-000 and Roadmap | Goals and observed development direction; no claim of complete platform parity, game-engine capability, or production qualification |
| AI-assisted development and future model capabilities | Maintainer's stated development direction, 2026-11-24; HAL-309 §§1 and 9; FAQ | Engineering approach and aspiration; not a runtime AI requirement |
| Retention, identity, phases, data ownership, GPU-first model, and sovereignty | Binding constitutional invariants, HAL-002; HAL-001 Foundational; HAL-354 Binding | Requirements explained; current full conformance not established here |
| Widget composition | HAL-010 Draft, subject to HAL-002 | Intent explained, not adoption of Draft clauses |
| Custom-render extension | HAL-295 Implemented / Closed; HAL-295A and HAL-266A Adopted / Binding, subject to their stated scope | Existing qualified extension contract; no new implementation or platform-parity claim |
| Architectural evolution | Binding constitutional amendment process, HAL-002A | not-applicable to process explanation |
| Recurring failure classes | PM-000 Active / Foundational; PM-001 and PM-027 Active | Historical findings; no claim of exhaustive remediation |
| Documentation and planned work | HAL-309 Adopted; non-binding Roadmap | Requirements, verification, and future intent remain separate |

The owning SPECs, including their amendments and exact declared statuses,
remain authoritative. In particular, proposed scoped amendments are not
adopted by this explanation. For implementation evidence and its limitations,
consult the relevant source, tests, and revision-qualified guides.
