---
doc-id: halcyon-guide-faq
class: first-class
authority: explanatory-non-binding
halcyon-revision: 0a633294d43dbc8ccc9df9c7bda3baf1dc600357
verified-at: 2026-11-22
governing-specs:
  - SPEC-HAL-002
  - SPEC-HAL-002A
  - SPEC-HAL-309
---

# Halcyon FAQ

## Getting started and troubleshooting

### Where should I start?

Follow [Getting started](getting-started.md). It contains the tested
source-tree workflow and a complete application with its own Cargo root.
The [documentation index](index.md) provides the reading paths for the
rest of the framework.

### Can I install Halcyon from crates.io?

Use `cargo add halcyon-rs@0.271.4-beta` to install the published beta, then
import `halcyon` in Rust. This source checkout is the local `0.272.0-beta`
release candidate.
See [Getting started](getting-started.md).

### Which Winit types should a standalone application use?

Use `halcyon::winit` for types passed into or returned by Halcyon. On native
targets it re-exports registry `winit` 0.30.12, including the same types a
direct dependency on `winit = "=0.30.12"` supplies. On WebAssembly, the
published beta and this source candidate re-export `halcyon-winit` 0.30.13.
Upstream `winit` has a separate type identity on WebAssembly.

## Architecture and design

For the reasons behind Halcyon's architectural choices, read
[Design Philosophy and Architectural Rationale](design-rationale.md).

### Is Halcyon intended only for trading applications?

No. Demanding data-intensive applications, including trading interfaces,
shaped its original requirements. Stable identity, retained state, explicit
ownership, and specialized rendering are useful in other application domains
as well. Halcyon does not prescribe an application's business model.

### Do I need to understand the Constitution to build an application?

No. Start with the public APIs, [Getting started](getting-started.md), and
the examples. The Constitution governs the framework's architecture; it is
not a requirement to study every internal subsystem before using ordinary
widgets.

Understanding the relevant ownership and lifecycle contracts becomes more
important when implementing custom widgets or extending lower-level
mechanisms. The [design rationale](design-rationale.md) explains why those
boundaries exist.

### Can I use Halcyon without writing custom GPU code?

Yes. Ordinary application development uses the public widget and scene
mechanisms. The native and Web starter demonstrates this without an
application-authored GPU pipeline.

Custom rendering is an extension path for specialized graphical content.
It is not a prerequisite for building an application.

### Can I extend Halcyon without modifying the framework itself?

Yes. Application-specific composition and custom widgets are intended
extension routes. The admitted custom-render mechanism also permits
specialized offscreen GPU work within its defined resource and composition
boundaries.

The public APIs and governing contracts determine what an extension may do;
not every internal mechanism is a supported extension point. Practical
extension guides and examples are among the priorities in the
[Roadmap](../../ROADMAP.md).

### Why does Halcyon have a Constitution?

The [Constitution](../specs/SPEC-HAL-002-Constitution-and-Architectural-Invariants.md)
defines the framework's foundational rules, including retained ownership,
stable identities, deferred mutation, and separation of frame phases.
It gives changes a common set of constraints across interconnected systems.

Its [amendment guidelines](../specs/SPEC-HAL-002A-Constitution-Amendment-Guidelines.md)
define how those foundations may be changed. Ordinary implementation work
does not silently revise them.

### Does a specification mean that a capability is implemented?

No. Each specification carries its own declared status. A Draft describes
intent; it does not establish working behavior.

Source, tests, and observed execution establish implementation at a
particular revision. The guides explain those sources and record the
revision they were checked against. The
[documentation authority hierarchy](index.md#authority-hierarchy) explains
these distinctions.

### What happens when the implementation contradicts a specification?

A discrepancy must be identified rather than treated as an implicit change
to the governing contract. The [contradiction register](audits/contradictions.md)
records such findings. Resolution requires a correction to the implementation
or an explicit, permitted change to the applicable specification.

The Constitution and its permanent invariants retain precedence. A successful
build or test in isolation does not resolve an architectural contradiction.

### Can architectural decisions be questioned?

Yes. The Constitution and architectural specifications are published so that the project’s decisions and constraints can be examined.

Questions, counterexamples, and evidence of inconsistencies are welcome. Final decisions about the official implementation remain with the maintainer.

## Development history

### When did development begin, and why is the project only now public?

Development began in November 2025. Halcyon was originally intended for
my own use, and its development priorities followed the needs of my own
projects. The decision to release it as open source came much later.

### Why does the public repository not contain the full original development history?

The public repository represents the transition to open source. The original
private development history is retained separately because it includes internal
engineering material and infrastructure not intended for public distribution.
[HISTORY.md](../../HISTORY.md) records the individual historical changes.

## Contributing and governance

### Who develops and maintains Halcyon?

Halcyon is developed and maintained by its founder,
[Andrey Gubarev](https://github.com/agubarev), with extensive AI assistance.

I have led the project from its inception through continuous daily development. I define its direction and architectural contracts, direct implementation work, review changes, and take responsibility for the official implementation.

The project currently has one human maintainer. Its development pace and maintenance capacity reflect that.

### How is AI used?

AI tools are a substantial part of Halcyon’s engineering workflow, including implementation, investigation, testing, and review.

Architectural decisions, scope, acceptance criteria, and responsibility remain with me. I decide which changes become part of the official implementation and which claims the available evidence supports.

### How does sponsorship work?

[GitHub Sponsors](https://github.com/sponsors/agubarev) provides a way to
voluntarily support Halcyon's ongoing development and maintenance. The founder
remains responsible for the project's direction, architecture, releases, and
participation policies.

Sponsorship does not confer decision-making authority, approval rights,
exclusive access to code, priority support, or a commitment to implement a
particular feature. Sponsors are welcome to submit questions and proposals
through the same channels as other users.

Paid development or support services are agreed separately in writing, with
explicit scope, deliverables, and terms. Such agreements do not transfer
governance of the project.

### Why are external pull requests not accepted?

Halcyon is developed through an established workflow that keeps architectural decisions, implementation, and verification under one engineering lead.

Accepting an external implementation requires reviewing its assumptions, checking its interactions with existing contracts, resolving design differences, and taking responsibility for its continued maintenance. That work remains necessary even when a change applies cleanly and passes its tests.

With one maintainer, I have chosen to concentrate that capacity on developing and maintaining the official implementation. External pull requests are therefore not accepted.

There is no scheduled date for changing this policy. Any change to the participation model will be documented explicitly.

### Will the current participation and project management practices change?

The current arrangements reflect the project's present stage of development
and maintenance capacity. Participation policies, development workflows, and
project management practices may evolve as the project matures.

A public project board may also be introduced when it becomes useful for
communicating development priorities and tracking progress.

Any such changes will be documented explicitly.

### How can I help?

Bug reports, reproducible examples, technical questions, criticism, and proposals are welcome through Issues.

For a bug report, describe the observed behavior, the expected behavior, the environment, and the steps needed to reproduce it. For a proposal, explain the use case, the desired behavior, and the relevant constraints.

Reports and proposals inform development. They do not establish a delivery commitment or a response deadline.

### What is the scope of project discussions?

Halcyon discussions focus on the framework's development and use. Technical
criticism and questions about architectural decisions are welcome. Political
debates unrelated to Halcyon and abusive conduct are outside the scope of the
project's discussion channels.

The [participation rules](../../CONTRIBUTING.md#discussion-scope) and the
[Code of Conduct](../../CODE_OF_CONDUCT.md) apply to everyone, including sponsors.

### How are technical questions addressed?

Technical questions are welcome. Where a question requires deeper
investigation, answers are developed through examination of the source code,
specifications, and tests rather than immediate assumptions.

Recurring questions and useful technical explanations will gradually become
part of the FAQ and project documentation, benefiting all users rather than
remaining confined to individual discussions.

Accuracy and lasting usefulness take precedence over response speed.

### Do I need to understand all the specifications before reporting a bug?

No. A clear description and a reproducible example are enough to report a
problem. References to specifications are useful when you have them.

### Can I include code in an Issue?

Yes. Code can make a report reproducible or explain a proposed approach.

A full implementation attached to an Issue is subject to the same intake
policy as a pull request. Opening an Issue does not create an obligation
to review or integrate that implementation.

### What about small fixes or documentation corrections?

Report them through Issues. The same policy applies to external pull
requests of every size, including documentation changes.

## Licensing and forks

### How is Halcyon licensed?

Halcyon's own code is licensed under the [MIT License](../../LICENSE).
Bundled third-party code and assets retain their applicable license terms
and notices. See [Third-party notices](../THIRD-PARTY-NOTICES.md).

### Does the participation policy restrict forks or independent development?

No. This policy describes how the official Halcyon project is maintained.
It adds no restrictions to the rights granted by the license accompanying
the source.

The maintainer's authority concerns the official project. Independent
development does not create an obligation for the maintainer to review,
integrate, or maintain a fork's changes.

## Project status and roadmap

### Is Halcyon a stable release?

Halcyon is an experimental beta. Public APIs and integration requirements
can change between releases, as stated in the [README](../../README.md).

### Can I use Halcyon in production?

The MIT license permits commercial and closed-source use of Halcyon's own
code. Suitability for a particular application depends on that application's
requirements and verification. The beta status does not imply API stability,
production certification, guaranteed support, or a maintenance schedule.

### Which platforms have received the most testing?

Development and testing have primarily taken place on macOS, followed by
Windows 10. Linux has received comparatively little testing. This uneven
coverage is a known limitation of the current beta; broader cross-platform
validation remains part of further development.

### Which setup has been verified?

[Getting started](getting-started.md) records its original tested Apple Silicon
macOS and browser WebAssembly route, the separately qualified source-based
0.270.2-beta build route including Windows cross-target compilation, and the
subsequent scoped 0.270.3-beta native and browser observations. A cross-target
check is not native Windows runtime acceptance.

Compilation, automated tests, rendered output, and physical device input
establish different facts. None of them alone establishes every platform
or interaction scenario. Physical iPhone host-picker and interactive
UIKit acceptance remain open at the original verification date. Browser
3D also has separate, incomplete qualification.

### What are the current limitations?

The current beta should not be treated as having a complete, uniform
capability set across all targets.

- The full framework is currently distributed as source. The initial
  `halcyon-rs` 0.0.0 registry package is not a usable implementation.
- Linux has received comparatively little testing. Current Web and mobile
  interaction qualification is also incomplete; see the verification
  boundaries above and in [Getting started](getting-started.md).
- The application localization contract in
  [HAL-023](../specs/SPEC-HAL-023-Accessibility-i18n.md) is a Draft and
  explicitly records that it is not implemented. This does not mean that
  applications cannot display text in different languages.
- Full platform accessibility integration is not established. The
  [HAL-125](../specs/SPEC-HAL-125-Accessibility-State-Projection-Layer.md)
  design addresses semantic state projection, not a complete platform
  screen-reader implementation.
- Application scaffolding, recommended project structures, practical
  custom-widget and shader guides, and more complete styling and theming
  coverage remain [further development priorities](../../ROADMAP.md).

This is not an exhaustive list of unsupported features. Consult the
relevant specification and its status, the implementation, and actual
verification evidence for a particular requirement.

### How should I report a security vulnerability?

Follow [SECURITY.md](../../SECURITY.md). Use private vulnerability reporting
where available, and do not include sensitive vulnerability details in
public Issues.

### Where can I follow progress?

The [Roadmap](../../ROADMAP.md) describes development directions, and the
[Changelog](../../CHANGELOG.md) records changes. The documentation index
records guide freshness, and each specification retains its own status.
Proposals and draft specifications do not establish a delivery date or an
implemented capability.

## Sources and verification scope

This FAQ explains the maintainer's decisions and the inspected sources.
It does not amend architectural contracts or license terms. Authority
and implementation are classified separately under
[HAL-309](../specs/SPEC-HAL-309-Documentation-System.md#7-claims-classification-and-citation).

| Claims | Authority | Implementation | Source |
|---|---|---|---|
| Founder, AI use, participation, and decision ownership | binding maintainer decision | not-applicable | Maintainer's explicit statements and public-documentation instructions, 2026-11-22 |
| Evolution of participation and project management practices | maintainer statement | not-applicable | Maintainer's public-documentation instructions, 2026-11-24 |
| Sponsorship and project governance | binding maintainer decision | not-applicable | Maintainer's sponsorship-policy instructions, 2026-11-23 and 2026-11-24; these additions do not advance the source verification pin |
| Discussion scope and moderation | binding maintainer decision | not-applicable | [Participation rules](../../CONTRIBUTING.md#discussion-scope); maintainer's public-discussion instructions, 2026-11-24 |
| Technical communication and documentation | maintainer policy | not-applicable | Maintainer's explicit communication-policy instructions, 2026-11-24 |
| Constitutional rules and amendment procedure | binding | not-applicable | HAL-002 and HAL-002A linked above; this FAQ makes no blanket implementation-conformance claim |
| Application use, extensibility, and design rationale | explanatory; underlying contracts retain their declared authority | ordinary native/Web starter and qualified custom-render contract; no exhaustive extension-conformance claim | [Design rationale](design-rationale.md); Getting Started; HAL-295/295A/266A |
| Current limitations and further development | explanatory; Draft/Design and non-binding roadmap retain their status | partial; specific absent or unqualified capabilities identified above | HAL-023, HAL-125, Getting Started, and [Roadmap](../../ROADMAP.md) |
| Documentation hierarchy and claim status | binding | not-applicable | HAL-309 and the documentation index linked above |
| Starter setup and application-root patches | none; explanatory source report | implemented at the original Getting Started pin; newer source-distribution build and runtime scopes separately qualified | Getting Started, including its dated 0.270.2-beta build and 0.270.3-beta release qualification; original runtime verification pin unchanged |
| MIT license | binding maintainer choice | not-applicable | [Project license](../../LICENSE); [third-party notices](../THIRD-PARTY-NOTICES.md); license documentation does not advance the source verification pin |
| Beta and platform verification boundaries | none; explanatory source report | partial | README, Getting Started, the dated framework verification record, and maintainer's platform-testing disclosure on 2026-11-24 |
| Development origin and public-history boundary | maintainer statement | not-applicable | README, HISTORY.md, maintainer's publication decisions on 2026-11-23 and 2026-11-24 |
| Initial registry notice and source distribution | maintainer distribution decision | implemented for the separately qualified source build route; functional registry package not yet distributed | README and Getting Started |
| Security reporting guidance | project policy | not-applicable | [SECURITY.md](../../SECURITY.md); private reporting is subject to repository configuration |
