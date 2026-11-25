---
doc-id: halcyon-guide-widget-catalog
class: first-class
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-24
governing-specs:
  - SPEC-HAL-011
  - SPEC-HAL-309
---

# Widget Catalog

Complete public widget-module inventory from `src/widgets/mod.rs:17-97` —
the authoritative inventory (INV-309-0) — organized into coherent
families. **Provenance:** base commit
`b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` with four locally modified,
uncommitted files present and used on owner order
(`examples/showcase/tab_plot.rs`, `src/tests/market_detail.rs`,
`src/widgets/aligned_order_book.rs`, `src/widgets/mod.rs`); re-verified
2026-07-24 at this base (81-module inventory at `src/widgets/mod.rs:17-97`
and the geometry house law at `:6-15` re-checked on disk); final re-pin
to a clean revision when that unit commits. Two-axis tags per
SPEC-HAL-309 §7.

Catalog scope: identity, family, and evidenced role. Per-widget
construction/method detail is Rustdoc territory (SPEC-HAL-309 §2.5);
tabular internals live in `tabular-and-hierarchical-views.md`, plot
composition in `plot-and-market-detail.md`. Purposes cited from mod.rs
comments, prelude exports (`src/lib.rs:40-57`), or reads made in this
programme; entries without a citation are inventory-only
(purpose axis `unknown`).

House law at the top of the inventory file: **Geometry Authority
Enforcement** — framework widgets may not introduce new geometry
literals; new geometry requires a theme token or an explicit `INV-*`
justification (`src/widgets/mod.rs:6-15`).

## 1. Tier-1 Core (SPEC-HAL-011 v2.0, `src/widgets/mod.rs:1-4`)

`flex` (FlexRow/FlexColumn), `container`, `panel`, `label`, `button`,
`divider`, `scroll_area`, spacers `fixed_spacer` + `flex_spacer`; Tier-2:
`flow_grid`. All prelude-exported (`src/lib.rs:41-57`) with
`auxiliary_text`, `groupbox`, `notification_log`, `section_header`,
`text_block`, `text_edit`, `text_field`.

## 2. Layout and Structure

`split_pane`, `exclusion_flow` (experimental layout exclusion flow,
`:106-107`), `separator_group`, `section_surfaces`
(DisclosureSectionHeader, SectionBandHeader/Level/Marker, SectionMetaHeader,
`:121-124`), `screen_host` (the tree root mounted by `App::run`,
`src/core/app.rs:9143-9149`), `scrollbar`.

## 3. Input Controls

`checkbox` (SPEC-HAL-111/208 — `examples/checkbox.rs:1-2`), `switch`,
`radio_button`, `radio_group`, `radio_group_widget` (SPEC-HAL-130/131,
`:109-110`), `slider`, `range_slider`, `spin_box`, `segmented_control`,
`combo_box` (SPEC-HAL-201 "Resolver-Driven, Zero Semantics", `:112-113`),
`tag_input`, `keybind_editor` (SPEC-HAL-329 single-stroke capture,
`:153-154`), `color_picker`, `color_picker_field`, `date_picker`,
`date_picker_field` (SPEC-HAL-238 trigger + anchored popup, `:150-151`),
`time_picker`.

## 4. Text and Documents

`text_field`, `text_area`, `text_block`, `text_edit`, `text_document`,
`text_layout_cache`, `text_widget_test_harness` (test harness, in-tree),
`console`, `event_log`.

## 5. Display and Readouts

`label`, `auxiliary_text`, `gauge`, `linear_gauge`, `sparkline`,
`progress_bar`, `busy_indicator`, `readout` — "compact paint-only data
readouts": KeyValueStrip/Item, ReadoutTable/Column/Cell, StatusGrid
family (`:115-120`; StatusGrid's invalidation contract is binding —
SPEC-HAL-354:300-301).

## 6. Navigation and Chrome

`tab_bar` (SPEC-HAL-219 pure navigation selector, `:129-130`),
`tab_switcher` (SPEC-HAL-321 content multiplexer, `:139-140`),
`breadcrumbs` (SPEC-HAL-220, `:142-143`), `navigator`, `menu_bar`
(SPEC-HAL-333 Rev 4 in-window bar + InWindowMenuHost bridge; its
DismissPolicy is module-scoped, distinct from the SPEC-HAL-127 type —
`:132-137`), `menu_surface`, `trigger_chrome`.

## 7. Overlays and Transients

`dialog`, `toast`, `popup_menu`, `context_menu`, `dropdown_menu`,
`tooltip_surface` (SPEC-HAL-226 overlay-only non-action, `:145-146`),
`tooltip_host` (SPEC-HAL-226A layout-transparent binding, `:147-148`),
`action_palette` (SPEC-HAL-020 Stage 6, `:99-100`).

## 8. Tabular, Hierarchical, and Cell-Bearing

`tabular_view` (TabularSubstrate + DataSource), `list_view` (ListView,
SPEC-HAL-187/188 consumer, `:126-127`), `tree_view`, `property_editor`,
`list`, `disclosure_indicator`, `disclosure_toggle`,
`cell_editor_factory` (`pub(crate)` — internal, not public surface,
`:23`). Full architecture: `tabular-and-hierarchical-views.md`.

## 9. Market Data

`order_book`, `order_book_body` (row flash ownership is binding —
SPEC-HAL-354:306-307), `aligned_order_book` (SPEC-HAL-296 §15.2 /
SPEC-HAL-303 price-aligned ladder; AlignedOrderBook,
AlignedOrderBookRowLayout, AlignedOrderBookSelection — `:101-103`,
`src/widgets/aligned_order_book.rs:1`). Composition with Plot:
`plot-and-market-detail.md`.

## 10. Temporal

`timeline_canvas`, `timeline_header`, `timeline_spatial`
(SPEC-HAL-334-336 temporal substrate lineage; engine side is
`engines::temporal`, `src/engines/mod.rs:16`).

## 11. Coverage Statement (INV-309-0)

All 81 modules declared at `src/widgets/mod.rs:17-97` are listed above
exactly once across §1-§10; the plot-composed widgets (§9) and tabular
internals (§8) are elaborated in their subsystem guides. `Plot` itself is
not a `widgets/` module — it is the peer module `src/plot/`
(`src/plot/mod.rs:8`) and appears in `plot-and-market-detail.md`.
Per-widget interaction policies remain per-SPEC declarations; nothing in
this catalog generalizes them (PRE-FLIGHT.md §B2).
