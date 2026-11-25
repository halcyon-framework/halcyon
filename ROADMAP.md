# Roadmap

Halcyon's current priority is to make the framework easier to adopt, extend,
and use reliably in real applications. The following are development directions,
not a release schedule or a commitment to deliver particular features.

## Near-term priorities

### Application development

- Provide recommended project structures for both small single-crate
  applications and larger Cargo workspaces.
- Explain the organization of application state, screens, reusable widgets,
  domain logic, resources, themes, and platform-specific entry points.
- Provide a small but complete reference application beyond the minimal
  [Getting Started example](docs/guide/getting-started.md), demonstrating
  navigation, application-owned state, asynchronous data and ordinary
  retained-widget updates.
- Develop native and WebAssembly starter templates, followed by project
  scaffolding based on the documented structures.
- Simplify dependency integration, resource handling, application builds,
  and native and Web deployment. Qualify a functional crates.io distribution
  once the corrected dependency sources can be resolved reliably.

### Extending Halcyon

- Explain how to compose existing widgets into application-specific controls
  and how to implement new widgets using the existing widget lifecycle,
  layout, input, painting, theming, and update contracts.
- Demonstrate how to turn a project-specific widget into a reusable component,
  including explicit inputs, state ownership, actions, and packaging in a
  separate Rust crate.
- Provide a practical guide to custom WGSL shaders and GPU-rendered content
  through Halcyon's existing custom-render extension point. Cover pipeline
  setup, resource ownership, per-target preparation, offscreen rendering,
  texture publication, and composition with ordinary UI.
- Add small, independently runnable examples for these extension paths.
  Explain when ordinary scene primitives are sufficient and when custom GPU
  rendering is appropriate. Application extensions must not depend on
  renderer-private resources or bypass framework ownership boundaries.

  The governing custom-render contracts are
  [HAL-295](docs/specs/SPEC-HAL-295-Custom-Render-Pass.md),
  [HAL-295A](docs/specs/SPEC-HAL-295A-Custom-Render-Pass-Authority-Reconciliation.md),
  and [HAL-266A](docs/specs/SPEC-HAL-266A-Per-Target-Custom-Render-Preparation.md).

### Widget styling and theming

- Provide practical guidance on styling individual widgets, creating and
  customizing application themes, and designing reusable themed components.
- Review the styling capabilities of every widget for which visual
  customization is meaningful. Complete missing style controls and improve
  consistency across widgets, including colors, typography, geometry,
  borders, materials, and interaction-state appearance where applicable.
- Clarify the boundaries between theme defaults, widget-specific styling,
  application overrides, and structural or interaction-owned geometry.
- Add focused examples of custom widget styles, complete themes, and runtime
  theme switching, with consistent behavior across supported platforms.

### Widget interaction policies

- Review every widget's interaction contract and its actual implementation.
  Identify missing, incomplete, inconsistent, or incorrectly owned policies.
- Ensure that each widget requiring a distinct interaction policy has an
  explicit, widget-appropriate policy, including relevant pointer, keyboard,
  touch, focus, selection, activation, capture, and disabled-state behavior.
- Reuse shared interaction contracts where sufficient. Do not introduce
  widget-specific policies for passive widgets or duplicate common behavior
  merely for uniformity.
- Document the applicable policies and qualify their implementation with
  focused behavioral and regression tests. Keep interaction semantics
  independent of visual styling.

### Examples and demonstrations

- Refine the existing examples in `examples/`, improving visual quality,
  usability, clarity, and consistency with current framework APIs and
  recommended application patterns.
- Expand the collection with focused, independently runnable examples that
  demonstrate capabilities not yet adequately represented.
- Develop more practical examples of combined framework capabilities, while
  keeping introductory examples small and easy to understand.

### Reliability and platform coverage

- Stabilize existing behavior and address reproducible defects.
- Expand native Windows and especially Linux testing, while maintaining
  macOS coverage.
- Improve Web and mobile verification, including input, clipboard, text
  editing, scaling, and platform-specific interaction.
- Establish repeatable cross-platform build and regression checks, and
  clearly distinguish compilation from verified runtime behavior.
- Improve API documentation, troubleshooting guidance, and the practical
  examples alongside the existing architectural documentation.

## Further development

- Improve the completeness and consistency of existing widgets, layouts,
  text editing, theming, and data-backed views.
- Continue developing and qualifying the existing custom-rendering, plotting,
  and 3D subsystems within their respective contracts.
- Expand internationalization and accessibility work. Localization
  ([HAL-023](docs/specs/SPEC-HAL-023-Accessibility-i18n.md)) remains a draft;
  accessibility development must respect the existing
  [HAL-125](docs/specs/SPEC-HAL-125-Accessibility-State-Projection-Layer.md)
  design and its scope.
- Improve diagnostics, profiling, performance benchmarks, and regression
  coverage for complex applications.
- Make application integration and upgrades more predictable as the public
  API matures.
- Define a structured YAML front matter schema for specifications, distinct
  from the explanatory guide metadata defined by HAL-309. Represent SPEC
  identity, title, declared status, normative authority, authorship, dates,
  dependencies, amendment and supersession relationships, and implementation
  status where explicitly established.
- Review the existing specification corpus and migrate its metadata to the
  agreed schema without changing architectural meaning, declared status,
  authority, historical provenance, or effective amendment scope. Record
  missing or ambiguous metadata for review rather than inventing values.
- Use the structured metadata to support machine-readable dependency graphs,
  cross-reference validation, amendment and supersession tracking, and
  detection of inconsistent or potentially stale documentation. Keep
  specification authority distinct from implementation and verification
  evidence.
- Extend the existing native read-only Markdown viewer
  ([HAL-390](docs/specs/SPEC-HAL-390-Native-Markdown-Viewer.md)), improving
  formatting coverage, reading and navigation ergonomics, code-block
  presentation, and cross-platform support.
- Develop a reusable read-only source code viewer with language-aware syntax
  highlighting, line numbers, text selection and copying, and efficient
  scrolling through large source files.

## Long-term direction

Work toward a stable, well-documented framework suitable for demanding
production applications, with predictable behavior, practical extension
mechanisms, reliable distribution, and clearly established platform support.

Progress toward a stable release depends on implementation maturity and
verification rather than a predetermined date or version milestone.

## Scope and status

This roadmap is non-binding. It does not adopt a proposed specification,
change an architectural contract, or establish that a planned capability
is implemented. The [documentation index](docs/guide/index.md),
[specifications](docs/specs/), and [changelog](CHANGELOG.md) retain their
respective roles.

Priorities may change as development progresses, existing defects are
identified, and actual application requirements become clearer.
