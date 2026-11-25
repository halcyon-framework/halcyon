# Halcyon Web input patches for winit 0.30.12

Base: the published `winit` 0.30.12 crate, SHA-256
`c66d4b9ed69c4009f6321f762d6e61ad8a2389cd431b97cb1e146812e9e6c732`.
The package version is 0.30.12 and the upstream [license](LICENSE) applies
to the vendored crate. The upstream [image attribution](docs/res/ATTRIBUTION.md)
is retained.

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

An application that depends on Halcyon selects this patched source in its
own root Cargo manifest. See the [standalone application guide](../../docs/guide/getting-started.md).
