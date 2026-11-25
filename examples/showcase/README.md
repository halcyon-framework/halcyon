# Halcyon Showcase

Showcase demonstrates the composition of Halcyon's widgets and application
mechanics through an ordinary App and Screen.

From the Halcyon source root:

```sh
cargo run --locked -p halcyon-rs --example showcase
```

See the [Web Showcase](../web_showcase/README.md) for the browser variant and
its platform boundaries.

## Source map

The following source map was originally checked against Halcyon
`0.230.3-beta`, revision `2781438a361874ed52068ec859a7ef29c2e9d0e0`.
The map is explanatory, not a framework specification. Current behavior
follows the source and governing specifications.

| Showcase tab | Rust implementation |
| --- | --- |
| Inputs and Controls | [tab_inputs.rs](tab_inputs.rs) |
| Gauges | [tab_display.rs](tab_display.rs) |
| Plot | [tab_plot.rs](tab_plot.rs) |
| Plot x4 | [tab_plot_grids.rs](tab_plot_grids.rs) |
| Plot x20 | [tab_plot_grids.rs](tab_plot_grids.rs) |
| Order Book | [tab_order_book.rs](tab_order_book.rs) |
| Property Editor | [tab_property_editor.rs](tab_property_editor.rs) |
| Tree View | [tab_tree_view.rs](tab_tree_view.rs) |
| List View | [tab_list_view.rs](tab_list_view.rs) |
| Event Log | [tab_event_log.rs](tab_event_log.rs) |
| Console | [tab_console.rs](tab_console.rs) |
| Navigator | [tab_navigator.rs](tab_navigator.rs) |
| Overlays and Windows | [tab_overlays.rs](tab_overlays.rs) |
| HF Data | [tab_live_data.rs](tab_live_data.rs) |
| Workbench | [tab_workbench.rs](tab_workbench.rs) |
| Node Graph | [tab_node_graph.rs](tab_node_graph.rs) |
| Viewport 3D | [tab_viewport_3d.rs](tab_viewport_3d.rs) |

## Existing integration examples

- [main.rs](main.rs): application lifecycle, tab placement/switching and status
  bar; start here to follow how a tab is mounted and updated.
- [menu.rs](menu.rs): Showcase `MenuModel` and Action System binding.
  [tab_overlays.rs](tab_overlays.rs): modals, toasts, context menus and windows.
- [tab_workbench.rs](tab_workbench.rs): composition of `SplitPane`,
  `ListView`, `TreeView` and `PropertyEditor`.
- [tab_live_data.rs](tab_live_data.rs): `DataSource` and retained refresh;
  [tab_event_log.rs](tab_event_log.rs): expandable `EventLog`;
  [tab_console.rs](tab_console.rs): `Console` and generated streaming demo lines.
- [tip.rs](tip.rs): shared `TooltipHost` alias used by the Showcase.

Showcase metrics, market data, event entries and generated console logs are
demonstration data, not real telemetry or performance evidence. A demo or
screenshot does not establish a new framework contract or conformance.
