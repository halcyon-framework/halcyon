# Local Halcyon browser probe

This example uses the ordinary Halcyon retained widget tree, text, input and
WebGPU renderer. The HTML hosts the canvas and reports startup errors; it
does not implement application controls or own application state.

## Build and serve locally

Use Rust and Cargo, native build tools, `cmake` on `PATH`, and Python 3.9 or
later. From the Halcyon source root, install the WASM target and the CLI
version matching the pinned `wasm-bindgen` library:

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked --version 0.2.127 --root target/halcyon-tools/wasm-bindgen-0.2.127 wasm-bindgen-cli
./target/halcyon-tools/wasm-bindgen-0.2.127/bin/wasm-bindgen --version
```

```sh
CMAKE=cmake cargo build --locked --target wasm32-unknown-unknown --target-dir target -p halcyon-rs --example web_local_probe
mkdir -p target/web-local-probe
./target/halcyon-tools/wasm-bindgen-0.2.127/bin/wasm-bindgen --target web --out-dir target/web-local-probe --out-name halcyon_probe target/wasm32-unknown-unknown/debug/examples/web_local_probe.wasm
cp examples/web_local_probe/index.html target/web-local-probe/index.html
python3 -m http.server 8787 --bind 127.0.0.1 --directory target/web-local-probe
```

Open [the local probe](http://127.0.0.1:8787/) in a WebGPU-capable browser
on the same machine. Try clicking the Button, entering text in the TextField,
and resizing the browser. Stop the local server with Ctrl+C. Rebuild the WASM
and rerun bindgen before reloading after source changes.

The browser probe uses the normal non-atomic `wasm32-unknown-unknown` target.
Its original development runs used a local build workflow. The standalone
recipe above follows the source-root setup documented in
[Getting started](../../docs/guide/getting-started.md); this exact recipe
requires release qualification. Compilation alone does not establish browser
rendering or input conformance.
