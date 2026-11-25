---
doc-id: halcyon-guide-getting-started
class: first-class
authority: explanatory-non-binding
halcyon-revision: eec0b3045b653195d3b9d43c761d2b3170fe8dc9
verified-at: 2026-11-22
governing-specs:
  - SPEC-HAL-002
  - SPEC-HAL-011
  - SPEC-HAL-018
  - SPEC-HAL-309
  - SPEC-HAL-398
  - SPEC-HAL-400
---

# Getting started

Start with the complete `halcyon` source checkout while the functional
registry beta is pending. After publication, add the beta from your
application's Cargo root.

```sh
cargo add halcyon-rs@0.271.3-beta
```

Use `halcyon` in Rust imports. A bare `cargo add halcyon-rs` selects the
0.0.0 notice while it is the only stable release. Use `halcyon::winit` when
interoperating with Halcyon's windowing types on either target.

The original native and browser App walkthrough was verified against
Halcyon 0.268.2-beta. Its source-based build route was separately qualified
with Halcyon 0.270.2-beta on 2026-11-24, as recorded below. The original
tested setup uses Apple Silicon macOS, Rust and Cargo 1.100.0, native build
tools, `cmake` on `PATH`, and Python 3.9 or later for local Web serving.

## Run a source-tree example

From the Halcyon source root:

```sh
cargo run --locked -p halcyon-rs --example showcase
cargo run --locked -p halcyon-rs --example checkbox
```

Each command opens its own application. Close the first before running the
second.
See the [Web Showcase](../../examples/web_showcase/README.md) and
[Web Touch Input](../../examples/web_touch_input/README.md) for the same
source-root workflow in a WebGPU-capable desktop browser.

For the smallest native `App`, start with [`hello_world`](../../examples/hello_world.rs). It mounts a greeting, Button and live click counter through the ordinary Screen lifecycle:

```sh
cargo run --locked -p halcyon-rs --example hello_world
```

For a small data-backed view instead of the larger stress demonstrations:

```sh
cargo run --locked -p halcyon-rs --example listview_basic
cargo run --locked -p halcyon-rs --example treeview_basic
```

The three examples compiled and linked under Halcyon `0.270.2-beta` on Apple Silicon. This does not establish GUI interaction or support on other platforms. The existing [advanced ListView](../../examples/listview.rs) and [TreeView](../../examples/treeview.rs) remain separate stress demonstrations.

## Create a sibling application

Place `halcyon-starter` beside the complete `halcyon` source checkout.
Create the four files below in `halcyon-starter`. This application has its
own Cargo workspace and lockfile. Cargo selects the windowing dependency from
Halcyon's manifest. Native targets use registry `winit` 0.30.12. WebAssembly
uses the `halcyon-winit` helper, whose types are distinct from those of
upstream Winit. After the registry beta is published, replace the starter's
path dependency with the registry dependency from the command above.

### `Cargo.toml`

```toml
[package]
name = "halcyon-starter"
version = "0.1.0"
edition = "2024"
publish = false

[workspace]

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
halcyon-rs = { path = "../halcyon" }
tracing = "0.1.41"
tracing-subscriber = "0.3.19"

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "=0.2.127"
web-sys = { version = "=0.3.104", features = ["console", "Window", "Document", "Element", "Node"] }
```

### `src/lib.rs`

```rust
use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::{EdgeInsets, FlexParams};
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::prelude::{Button, Flex, Label, TextField};
use halcyon::widgets::combo_box::ComboBox;
use std::cell::Cell;
use std::rc::Rc;

pub fn run() {
    App::new(AppConfig::new("Halcyon Starter").with_size(440.0, 320.0)).run(StarterScreen {
        placeholder: Label::new(""),
        rebuild: true,
        tap_count: Rc::new(Cell::new(0)),
        tap_status: None,
        displayed_taps: 0,
    });
}

struct StarterScreen {
    placeholder: Label,
    rebuild: bool,
    tap_count: Rc<Cell<u64>>,
    tap_status: Option<NodeId>,
    displayed_taps: u64,
}

impl Screen for StarterScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let column = cx.mount_child(Box::new(
            Flex::column()
                .with_padding(EdgeInsets::all(20.0))
                .with_spacing(12.0),
        ));
        let mut options = ComboBox::simple(vec![
            "Option 1".into(),
            "Option 2".into(),
            "Option 3".into(),
        ]);
        options.set_text_input_enabled(false);

        let count = Rc::clone(&self.tap_count);
        let tap = Button::with_text("Test tap").on_click(move || {
            count.set(count.get().checked_add(1).expect("tap count overflow"));
        });
        let controls: [(Box<dyn Widget>, f32); 4] = [
            (Box::new(Label::new("Halcyon Starter")), 24.0),
            (Box::new(TextField::new().with_placeholder("Type here")), 38.0),
            (Box::new(options), 38.0),
            (Box::new(tap), 42.0),
        ];
        for (widget, height) in controls {
            let child = cx.tree.add_child(column, widget, None, 0);
            cx.tree.set_flex(child, FlexParams::fixed(height));
        }

        self.displayed_taps = self.tap_count.get();
        let status = cx.tree.add_child(
            column,
            Box::new(Label::new(format!("Test tap count: {}", self.displayed_taps))),
            None,
            0,
        );
        cx.tree.set_flex(status, FlexParams::fixed(23.0));
        self.tap_status = Some(status);
        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn update(&mut self, tree: &mut WidgetTree) {
        let count = self.tap_count.get();
        if count == self.displayed_taps {
            return;
        }
        let Some(status) = self.tap_status else {
            return;
        };
        tree.node_mut(status)
            .expect("current tap status mount")
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<Label>()
            .expect("tap status Label")
            .set_text(format!("Test tap count: {count}"));
        tree.mark_layout_dirty(status);
        self.displayed_taps = count;
    }

    fn mark_rebuild(&mut self) {
        self.rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.rebuild)
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn browser_start() {
    if let Err(error) = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .with_max_level(tracing::Level::ERROR)
        .with_writer(|| BrowserErrors(String::new()))
        .try_init()
    {
        report_error(&error.to_string());
    }
    std::panic::set_hook(Box::new(|info| report_error(&info.to_string())));
    run();
}

#[cfg(target_arch = "wasm32")]
struct BrowserErrors(String);

#[cfg(target_arch = "wasm32")]
impl std::io::Write for BrowserErrors {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.push_str(&String::from_utf8_lossy(bytes));
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for BrowserErrors {
    fn drop(&mut self) {
        if !self.0.is_empty() {
            report_error(&self.0);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn report_error(message: &str) {
    web_sys::console::error_1(&message.into());
    if let Some(output) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("boot-error"))
    {
        if output.text_content().is_none_or(|text| text.is_empty()) {
            output.set_text_content(Some(message));
        }
        if let Err(error) = output.remove_attribute("hidden") {
            web_sys::console::error_1(&error);
        }
    }
}
```

### `src/main.rs`

```rust
fn main() {
    halcyon_starter::run();
}
```

### `index.html`

```html
<!doctype html>
<html lang="en">
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Halcyon Starter</title>
<style>
html, body { margin: 0; width: 100%; height: 100%; overflow: hidden; }
canvas { display: block; width: 100vw !important; height: 100vh !important; }
#boot-error {
    position: fixed; bottom: 0; left: 0; right: 0; max-height: 30vh;
    z-index: 1; margin: 0; overflow: auto; white-space: pre-wrap;
    padding: 12px; background: #fff; color: #900;
}
</style>
<body>
<pre id="boot-error" hidden></pre>
<script type="module">
try {
    if (!navigator.gpu) throw new Error('WebGPU is unavailable in this browser.');
    const { default: init } = await import('./halcyon_starter.js');
    await init();
} catch (error) {
    const output = document.getElementById('boot-error');
    output.textContent ||= String(error?.stack ?? error);
    output.hidden = false;
    console.error(error);
}
</script>
</body>
</html>
```

## Build and run the standalone application

From `halcyon-starter`, seed the lockfile once and let Cargo resolve the
application’s own root entry. The first check can fetch dependencies.

```sh
cp ../halcyon/Cargo.lock Cargo.lock
cargo check
cargo build --locked --bin halcyon-starter
cargo run --locked --bin halcyon-starter
```

An existing Windows application lockfile can retain an incompatible
`gpu-allocator` dependency. To refresh that package and its dependencies,
run `cargo update -p gpu-allocator --recursive` in the application root.

The native window opens at 440 × 320 logical pixels using the default Theme.
Type in the TextField, open the ComboBox, and click **Test tap** to increase the
visible count. The callback records the count, and the Screen’s `update`
changes the mounted Label and marks its layout dirty. The explicit rebuild flag
lets the Screen keep its mounted TextField, ComboBox and Button through ordinary
updates. `handle_input` leaves event handling to the framework.

### Run it in a browser

From the Halcyon source root, install the WASM target and the CLI version
matching the pinned `wasm-bindgen` library:

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked --version 0.2.127 --root target/halcyon-tools/wasm-bindgen-0.2.127 wasm-bindgen-cli
./target/halcyon-tools/wasm-bindgen-0.2.127/bin/wasm-bindgen --version
```

From `halcyon-starter`:

```sh
CMAKE=cmake cargo build --locked --target wasm32-unknown-unknown --target-dir target --lib
mkdir -p target/web
../halcyon/target/halcyon-tools/wasm-bindgen-0.2.127/bin/wasm-bindgen --target web --out-dir target/web --out-name halcyon_starter target/wasm32-unknown-unknown/debug/halcyon_starter.wasm
cp index.html target/web/index.html
python3 -m http.server 8795 --bind 127.0.0.1 --directory target/web
```

Open [the local starter](http://127.0.0.1:8795/) in a WebGPU-capable browser on
the same machine. The foreground server refuses an occupied port. Stop it
with Ctrl+C. Rebuild, rerun bindgen, and reload the page after changing the
Rust code. The canvas fills
the browser viewport. Both entry points call the same synchronous `run()`,
and `App::run` handles platform scheduling. Browser startup errors appear in
the page and console.

## Source and authority

The source links below refer to Halcyon 0.268.2-beta at the revision in this
guide’s metadata. The [documentation index](index.md) records the scope and
provenance of other guides.

| Behavior | Authority | Implementation at the verified revision |
| --- | --- | --- |
| App runs a Screen with a platform-specific event loop. | `draft-intent` for the general App description in [HAL-018](../specs/SPEC-HAL-018-App-Framework.md), whose declared status is Draft. Native first-frame ordering is `binding` under §16. | `implemented` by [`App::run`](../../src/core/app/run.rs#L14) and the [`Screen` trait](../../src/core/app.rs#L8830). |
| The Screen builds a widget tree and projects the Button count during `update`. | `binding` phase ownership under [HAL-002](../specs/SPEC-HAL-002-Constitution-and-Architectural-Invariants.md). [HAL-011](../specs/SPEC-HAL-011-Tier1-Widgets.md) declares Implemented for Tier-1 widgets. | `implemented` with [`ScreenFrameContext::mount_child`](../../src/core/app.rs#L8760), the public widget APIs, and the same callback and Update pattern as [Touch Input](../../examples/touch_input.rs). |
| Browser text editing and host-mediated touch choices use the ordinary App. | `binding` in [HAL-398](../specs/SPEC-HAL-398-Web-Text-Input-and-Virtual-Keyboard-Bridge.md) and [HAL-400](../specs/SPEC-HAL-400-Host-Native-Choice-Picker-For-Touch.md). | `implemented` in the [Web text bridge](../../src/platform/web/text_input.rs) and [`ComboBox`](../../src/widgets/combo_box.rs). The starter uses the ordinary mouse popup for the desktop choice scenario. |

## What was verified

The following original observations retain the guide's historical
0.268.2-beta verification pin.

- **Build:** Cargo resolved the starter as a separate workspace with the
  corrected `winit` 0.30.12 and `wgpu-hal` 30.0.2 paths and one wgpu/Naga
  30.0.2 family. The locked native and WASM builds and bindgen completed.
- **Native runtime:** On Apple Silicon macOS, the starter showed its first
  window and a successor frame. One AppKit-routed Button click displayed
  `Test tap count: 1`.
- **Browser runtime:** Headless Brave at DPR 1 displayed the initial Web
  Showcase and Touch Input screens. The starter displayed `hello λ` after a
  trusted browser text event, **Option 2** after mouse selection in the
  Halcyon popup, and `Test tap count: 1` after one click.

These observations cover the tested desktop setup. Physical mobile input,
the system picker, and browser 3D have separate verification paths.

### Separate source-distribution build qualification (2026-11-24)

The four literal starter file blocks were built from an isolated copy of
the prepared Halcyon 0.270.2-beta public source set, based on original
revision `51beb5c3e5a1c86b3cc7b75e1c94cb4b4c9f6c71` and the selected source
candidate bytes. The copy excluded private repository metadata, configuration,
and internal development materials. The standalone application's manifest
owned both dependency patches, and Cargo resolved its own lockfile.

- Native Apple Silicon macOS starter check and executable linking: passed.
- Locked WASM starter library build and matching `wasm-bindgen` 0.2.127
  Web export: passed.
- Locked `x86_64-pc-windows-msvc` starter check: passed. This establishes
  cross-target compilation, not Windows linking or native execution.
- Dependency resolution: corrected `winit` 0.30.12 and `wgpu-hal` 30.0.2
  selected from the isolated source tree, with one Winit, one wgpu-hal,
  and one wgpu/Naga 30.0.2 family. The Windows DX12 dependencies of
  wgpu-hal and gpu-allocator 0.28.0 select the same `windows` 0.62.0.

This is a separate build qualification, not an advancement of the original
runtime verification pin. No new native or browser interaction observation
is implied by these builds.

### Release qualification (2026-11-24)

The prepared Halcyon 0.270.3-beta source distribution was qualified using
the same standalone application and corrected dependency sources.
The native executable, WebAssembly library, and matching Web export built
successfully. Source-tree native examples and Web examples also built.

The Apple Silicon macOS run produced its first window and a successor
frame, and an AppKit-routed Button event incremented the visible counter.
Native text input was not exercised in that run. The Brave run produced
its first visible frame, accepted trusted input of `hello λ`, selected
**Option 2**, and incremented the Button counter to 1. No page or GPU errors
were observed in that run. These results do not establish physical device
input or runtime acceptance on other platforms.

The earlier Windows cross-target result is retained at its 0.270.2-beta
basis; the release qualification did not repeat native Windows validation.
