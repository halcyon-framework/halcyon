# Halcyon Web Showcase

Web Showcase runs the same Rust Showcase screen as the native desktop
application, compiled to WebAssembly.

## Try the native Showcase first

For a first look at Halcyon, run the [native Showcase](../showcase/README.md)
from the Halcyon source root:

```sh
cargo run --locked -p halcyon-rs --example showcase
```

This opens a native desktop application. No browser, WebAssembly build or
local web server is required.

## Browser variant

The ordinary Halcyon browser App runs the original
[`ShowcaseScreen`](../showcase/main.rs) and draws its interface on the canvas.
The HTML loads WASM, sizes the canvas and displays startup errors. The App
can also use browser-native text input and a host choice picker for admitted
touch interactions.

The default is the native Showcase's light theme, Regular density and UiScale 1.0.
Showcase selects SdfV3 for its Button widgets through its Theme at every
density. The current target still determines material admission.
Native Showcase starts at **780 × 900 logical pixels**. The WASM canvas fills the
browser content viewport and resizes with its window; the browser supplies DPR
and backing size. Halcyon's ScrollArea handles excess content without outer
document scrolling. The fixed, scrollable error panel shows a short message
for GPU startup refusal, with the underlying diagnosis in the console.
Unexpected errors retain their detailed console and panel diagnostics.
The page title reports passively observed DPR, CSS canvas size, backing pixels,
browser viewport and configured canvas/view formats. It does not read or drive
Halcyon widget state or schedule frames.

## Build and serve the Web Showcase locally

Use Rust and Cargo, native build tools, `cmake` on `PATH`, and Python 3.9 or
later. The recipe was tested with Rust and Cargo 1.100.0 on Apple Silicon
macOS. It uses the Halcyon source root and installs the CLI version that
matches the pinned `wasm-bindgen` library.

From the Halcyon source root:

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked --version 0.2.127 --root target/halcyon-tools/wasm-bindgen-0.2.127 wasm-bindgen-cli
./target/halcyon-tools/wasm-bindgen-0.2.127/bin/wasm-bindgen --version
```

```sh
CMAKE=cmake cargo build --locked --target wasm32-unknown-unknown --target-dir target -p halcyon-rs --example web_showcase
mkdir -p target/web-showcase
./target/halcyon-tools/wasm-bindgen-0.2.127/bin/wasm-bindgen --target web --out-dir target/web-showcase --out-name halcyon_showcase target/wasm32-unknown-unknown/debug/examples/web_showcase.wasm
cp examples/web_showcase/index.html target/web-showcase/index.html
python3 examples/web_showcase/serve.py
```

Open [the local Showcase](http://127.0.0.1:8788/) in a WebGPU-capable desktop
browser on the same machine. The foreground server binds only to `127.0.0.1`,
refuses an occupied port, and sends `Cache-Control: no-store`. Stop it with
Ctrl+C. Rebuild the WASM and rerun bindgen before reloading after Rust changes.

## Platform boundaries

- Native macOS uses SystemUI/CoreText; WASM uses embedded Inter and Glyphon/Swash.
  The shared shaping engine does not make these fonts or rasterizers identical.
- All 17 tab entries remain. Plot, Plot x4 and Plot x20 explicitly report their
  existing native `PlotRenderPool` worker requirement instead of starting
  unsupported browser threads. No substitute plots or renderer are supplied.
- Order Book shows its original initial book and explicitly reports that its
  native mock feed is unavailable. Browser process CPU/memory metrics are also
  reported unavailable. Native workers and metrics retain their original path.
- Other sections use their original implementations. Compilation and tab
  availability alone are not a claim that every browser service, 3D path, child
  window or clipboard operation has been verified.
- A Safari Viewport 3D observation reached an uncaptured WebGPU error and a Rust
  abort. Browser 3D is not qualified by this example; reload to return to Inputs.
  This example does not suppress the device error or change the 3D renderer.
- Existing browser keyboard/Capture and clipboard limitations belong to the
  framework adapter; this example does not override them.

For a native comparison, run the original zero-argument `showcase` example with
the same theme/density and match its logical client area to the browser viewport.
Compare at equal physical scale without resampling. Account for the different
fonts when comparing Button material and input-surface chrome.
