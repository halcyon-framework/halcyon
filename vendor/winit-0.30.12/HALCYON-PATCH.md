# halcyon-winit 0.30.14

`halcyon-winit` is a temporary Halcyon-specific Web derivative of upstream
[winit](https://github.com/rust-windowing/winit). Its Rust library is named
`winit`. Halcyon selects this derivative on WebAssembly and uses upstream
winit 0.30.12 on native targets.

The base is the published `winit` 0.30.12 crate at upstream commit
`f6893a4390dfe6118ce4b33458d458fd3efd3025`, with archive SHA-256
`c66d4b9ed69c4009f6321f762d6e61ad8a2389cd431b97cb1e146812e9e6c732`.
The source and derivative use the [Apache License 2.0](LICENSE). The package
includes the [upstream readme](README.md), [original manifest](Cargo.upstream.toml),
and [image attribution](docs/res/ATTRIBUTION.md).

The five changed Web files supply modifier ordering, browser clipboard
shortcut transport, and a caller-owned sibling input overlay.

- [`src/platform_impl/web/event_loop/window_target.rs`](src/platform_impl/web/event_loop/window_target.rs)
  emits any changed modifier snapshot before the original keyboard press or
  release. One key event and zero or one modifier events are delivered per
  callback.
- [`src/platform_impl/web/web_sys/canvas.rs`](src/platform_impl/web/web_sys/canvas.rs)
  exempts C, X and V (case-insensitive) with Meta or Ctrl and neither Alt nor
  Shift from keydown default prevention. The keyboard event still reaches
  winit, and the browser can issue its trusted Copy, Cut or Paste event for
  Halcyon's active-editor handling. Shared input listeners route keyboard,
  wheel, focus-loss and pointer events from the canvas and sibling overlay
  through the same handlers. Overlay listeners accept only trusted events.
  Moving focus within the combined input surface preserves window focus.
- [`src/platform/web.rs`](src/platform/web.rs) adds the public
  `WindowAttributesExtWebSys::with_input_overlay(Option<HtmlElement>)` method.
- [`src/platform_impl/web/window.rs`](src/platform_impl/web/window.rs) holds the
  optional caller-provided element with the other Web window attributes.
- [`src/platform_impl/web/web_sys/pointer.rs`](src/platform_impl/web/web_sys/pointer.rs)
  maps overlay and canvas events into the same canvas backing coordinates,
  treats moves between those targets as movement within one input surface,
  and keeps ordinary mouse focus and pointer capture on the canvas. Touch
  over the overlay retains the browser's activation behavior for the host
  picker. Halcyon owns selection and commit.

Version 0.30.13 adds `WindowAttributesExtWebSys::with_context_menu_enabled`.
The existing canvas listener uses this override for `contextmenu` and follows
the window's `prevent_default` value when the override is unset.

Version 0.30.14 adds `WindowExtWebSys::set_context_menu_enabled`. The existing
canvas listener reads a canvas-local shared optional override. Updating it through
the existing main-thread window dispatcher is synchronous on that thread and
changes only context-menu cancellation; other default-event policies are unchanged.
This local derivative has not been published.

Applications using Halcyon's windowing types import them from
`halcyon::winit`. Upstream winit types have a separate identity on WebAssembly.
See the [standalone application guide](https://github.com/halcyon-framework/halcyon/blob/master/docs/guide/getting-started.md).
