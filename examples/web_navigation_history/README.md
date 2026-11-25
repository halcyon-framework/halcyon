# Browser navigation in an ordinary App

[The example](../web_navigation_history.rs) declares Home at `/` and Confirm at
`/confirm`. The Screen draws the committed route and owns its draft in memory.
The same App stays mounted when its route changes.

Declare the root and a finite set of static public paths with `WebNavigation::new`
and pass the result through `AppConfig::with_web_navigation`. A Screen reads
`AppContext::web_navigation_projection()`, looks up declared paths with
`web_navigation_path()`, and obtains a scoped `web_navigation_intents()` sink
for its callbacks. `request` admits a typed
`NavigationIntent` to the App loop. The committed projection determines the
current view and control availability. Use that projection for the base Screen
instead of the legacy screen-stack commands.

The platform adapter keeps URL processing outside NavigationHost (HAL-228 §6).
It uses trusted `popstate` in the actual canvas window and confers no authority on
the browser to select a route. Browser Back and Forward ask the existing host
to move to an exact committed history slot, under the ordinary guard and
prepared frame binding. A refused move between owned entries requests one
correction to the previous slot. Structural Up remains an application
navigation command. Native desktop and Web apps without a route declaration
keep their existing navigation paths.

The active top-level document needs the browser's Navigation API entry identity
for source qualification. The adapter uses History API writes and traversal.
It matches each opaque per-binding token against the browser-issued entry
identity, so a copied state value cannot claim another slot. A foreign entry
or a retired mount closes the binding. History contains only opaque tokens and
the declared public paths. Store private data in the application. A fresh
document admits a recognized deep link once and starts its own in-memory state.

Build from the Halcyon source root with the matching wasm-bindgen CLI (0.2.127).

```sh
CMAKE=cmake cargo build --locked --target wasm32-unknown-unknown -p halcyon --example web_navigation_history
mkdir -p target/web-navigation-history
wasm-bindgen --target web --out-dir target/web-navigation-history --out-name halcyon_history target/wasm32-unknown-unknown/debug/examples/web_navigation_history.wasm
cp examples/web_navigation_history/index.html target/web-navigation-history/index.html
```

Serve the generated assets and the same `index.html` at both declared paths on
one origin. Open Home, enter a draft, and select Continue. Browser Back and
Forward restore the route while keeping application memory. Toggle Back
Guard on Confirm and try Back again to observe a refused traversal.
