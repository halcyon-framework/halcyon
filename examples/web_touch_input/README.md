# Touch Input in a local browser

The Web example runs the same [Halcyon screen](../touch_input.rs) as
`cargo run --locked -p halcyon --example touch_input`. The App renders its
interface on the canvas and handles browser text input and supported host
choice pickers. The HTML hosts the canvas and reports startup errors.

## Build and serve locally

Use Rust and Cargo, native build tools, `cmake` on `PATH`, and Python 3.9 or
later. The recipe was tested with Rust and Cargo 1.100.0 on Apple Silicon
macOS. From the Halcyon source root, install the WASM target and the CLI
matching `Cargo.lock`:

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked --version 0.2.127 --root target/halcyon-tools/wasm-bindgen-0.2.127 wasm-bindgen-cli
./target/halcyon-tools/wasm-bindgen-0.2.127/bin/wasm-bindgen --version
```

```sh
CMAKE=cmake cargo build --locked --target wasm32-unknown-unknown --target-dir target -p halcyon --example web_touch_input
mkdir -p target/web-touch-input
./target/halcyon-tools/wasm-bindgen-0.2.127/bin/wasm-bindgen --target web --out-dir target/web-touch-input --out-name halcyon_touch_input target/wasm32-unknown-unknown/debug/examples/web_touch_input.wasm
cp examples/web_touch_input/index.html target/web-touch-input/index.html
python3 -m http.server 8794 --bind 127.0.0.1 --directory target/web-touch-input
```

Open [the local example](http://127.0.0.1:8794/) in a WebGPU-capable desktop
browser on the same machine. Keep the static server in the foreground and
stop it with Ctrl+C. It refuses an occupied port. Rebuild the WASM, rerun
bindgen and reload the browser manually after changes to the Rust code.

## Manual checks

- On a qualified physical touch device, tap the Email field once and observe
  whether the software keyboard opens. Enter and delete text, try IME
  composition, then edit Notes with a newline and an emoji.
- Tap Profile with a finger on a supported host. Choose an option in the
  system picker, then reopen it and dismiss it. Start a pan on Profile and
  check that the page scrolls. Use the mouse or physical keyboard to open
  and navigate the Halcyon popup.
- Tap **Send me updates** and **Include notifications** and check that each
  toggles once. Try the Volume slider and Quantity stepper.
- At **Test tap**, note the count, tap once, and expect an increase of one.
  Two taps add two. Start a pan on the Button and expect the count to stay the same.
- Pan the page and check that it follows your finger. Flick and release to
  observe continued scrolling, then put a finger down again to stop it.
- On desktop, exercise mouse, wheel, and physical keyboard navigation and
  editing, including Tab and Escape.

Record the observed behavior. The loopback session serves the browser on the
host machine. Physical phone checks require a device-accessible setup of
their own.
