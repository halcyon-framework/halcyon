# Halcyon

Halcyon is a GPU-first Rust UI framework for building native and browser
applications with a retained widget tree. The starter runs one Screen on
macOS and WebAssembly.

This is an experimental beta. Public APIs and integration requirements can
change between releases. The getting-started route was tested with Rust and
Cargo 1.100.0 on Apple Silicon macOS and with the WASM build in local Brave
at DPR 1. Use native build tools, `cmake` on `PATH`, and Python 3.9 or later
to serve the Web examples.

From the provided Halcyon source root, run Showcase:

```sh
cargo run --locked -p halcyon --example showcase
```

This beta is distributed as a complete source tree. The initial
[`halcyon-rs` 0.0.0 package](https://crates.io/crates/halcyon-rs) is a notice
of the official package identity, not an installable framework release.
A separate Cargo application must select Halcyon's corrected `winit` source
in its own root manifest. The standalone example in the
[Getting started guide](docs/guide/getting-started.md#create-a-sibling-application)
includes the exact pin and patch.

- [Getting started](docs/guide/getting-started.md)
- [Documentation index](docs/guide/index.md)
- [Design philosophy and architectural rationale](docs/guide/design-rationale.md)
- [Roadmap](ROADMAP.md)
- [Web Showcase](examples/web_showcase/README.md)
- [Web Touch Input](examples/web_touch_input/README.md)

## Platform support

Halcyon has been developed and tested primarily on macOS, with Windows 10
being the second most extensively tested platform.

Linux has received comparatively little testing so far. This is a known
limitation of the current beta, and broader cross-platform testing and
validation remain important areas for further development.

## Development history

Halcyon has been in active development since November 2025. It was
originally developed for my own use, with no plans for a public release.
Its development priorities were driven by the needs of my own projects.
The decision to make it open source came much later.

This repository marks the transition from private development to open
source, rather than the beginning of the project. The original development
history is preserved privately because it includes internal engineering
material and development infrastructure not intended for public distribution.

See [HISTORY.md](HISTORY.md) for the chronological record of development
before the public release.

## Participation

Halcyon is developed and maintained by its founder,
[Andrey Gubarev](https://github.com/agubarev), with extensive AI assistance.
It has one human maintainer. Architectural decisions and responsibility for
the official implementation remain with the founder.

Bug reports, reproducible examples, technical questions, criticism, and
proposals are welcome through Issues. External pull requests are not
accepted, and there is no scheduled date for changing this policy.

Read the [FAQ](docs/guide/faq.md) for the development model, participation
rules, licensing status, and verified project boundaries.

See [CONTRIBUTING.md](CONTRIBUTING.md) for participation rules and
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for community conduct.
For vulnerability reporting, see [SECURITY.md](SECURITY.md).

## License

Halcyon is licensed under the [MIT License](LICENSE). Bundled third-party
code and assets retain their own licenses; see [Third-party notices](docs/THIRD-PARTY-NOTICES.md).

## Support Halcyon

If you would like to support the ongoing development and maintenance of
Halcyon, you can do so through [GitHub Sponsors](https://github.com/sponsors/agubarev).

Sponsorship is voluntary and does not confer influence over project decisions,
priority support, or a commitment to deliver particular features.
