# SPEC-HAL-283: TreeMap - Rectangular Hierarchical Area Visualization

### Rectangular v1 widget for weighted hierarchical data

**Author:** JC
**Status:** Draft (Binding upon adoption)
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-226A (Tooltip Wrapper / Trigger Registration)
* SPEC-HAL-328 (Keyboard Authority, Focus, and Capture)
* SPEC-PM-006 (Rendering Pre-Flight Checklist)

---

# 1. Purpose

TreeMap renders hierarchical weighted data as nested rectangles. Each sibling
rectangle receives area proportional to its positive weight. The widget is for
"what occupies the most space" views: portfolio composition, disk usage, budget
allocation, code complexity, market maps, resource consumption.

TreeMap is a projection widget. The consumer owns the data, assigns stable node
identity, chooses weight and color metrics, filters data before construction,
and interprets emitted callbacks. TreeMap computes rectangular layout and
renders it.

V1 is rectangular only. Radial treemaps, sunburst charts, icicle charts,
Voronoi treemaps, cushion shading, and ProportionBar are not part of this SPEC.

---

# 2. Data Model

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TreeMapNodeId(pub u64);

#[derive(Debug, Clone)]
pub struct TreeMapNode {
    /// Stable identity. Must be globally unique within this TreeMap root.
    pub id: TreeMapNodeId,
    /// Primary visible text.
    pub label: String,
    /// Secondary visible text and tooltip detail.
    pub detail: Option<String>,
    /// Area weight among this node's siblings. Must be positive to participate.
    pub weight: f64,
    /// Explicit fill color. Used by Direct mapping and as fallback in metric modes.
    pub color: Option<Color>,
    /// Independent color metric. Does not affect area.
    pub color_value: Option<f64>,
    /// Hierarchical children.
    pub children: Vec<TreeMapNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeMapDataError {
    DuplicateId(TreeMapNodeId),
}
```

## 2.1 Invariants

**INV-283-D1: Stable Global Node Identity**

Every `TreeMapNode.id` value MUST be globally unique within the complete root
tree. IDs are not scoped by parent. `TreeMap::try_new` and `TreeMap::set_root`
must reject duplicate IDs with `TreeMapDataError::DuplicateId(id)`.

**INV-283-D2: Identity Drives State**

Hover, active cell, selection, drill path, and context callback targets are all
stored by `TreeMapNodeId`. Index-based identity is forbidden.

**INV-283-D3: Positive Weight Only**

A node with `weight <= 0.0`, NaN, or infinity is excluded from layout and paint.
Its descendants are not promoted. Exclusion is deterministic and does not mutate
consumer data.

**INV-283-D4: Branch Weight Is Consumer-Owned**

A branch node's `weight` is its area weight among siblings. TreeMap does not
derive branch weight from children. If a consumer wants branch area to equal the
sum of descendants, the consumer supplies that value.

**INV-283-D5: Child Layout Is Local**

Within a visible branch content rect, child areas are normalized from that
branch's valid positive children only.

**INV-283-D6: Size and Color Are Independent**

`weight` controls area. `color_value` controls metric color. These fields may
represent different metrics.

**INV-283-D7: Pure Input**

TreeMap never merges, filters, sorts, aggregates, or rewrites consumer nodes.
Layout algorithms may reorder rectangles internally for geometry, but consumer
data order and identity remain unchanged.

**INV-283-D8: Invalid Drill Path Recovery**

If `set_root` or `set_data` removes a node present in `drill_path`, the widget
truncates the path to the deepest still-valid prefix. If no prefix is valid, the
path becomes empty.

---

# 3. Policy

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TreeMapPolicy {
    /// Paint-only projection. Tooltip registration is allowed.
    /// No hover highlight, selection, drill, context callback, or keyboard handling.
    Static,

    /// Hover, selection, drill-down, context callback, and keyboard capture.
    #[default]
    Interactive,
}
```

Default policy: `Interactive`.

## 3.1 Static Policy

`TreeMapPolicy::Static`:

* `focusable() == false`
* `capture_eligible() == false`
* `defines_focus_scope() == false`
* pointer events return `false`
* key events return `false`
* no selection, drill, hover highlight, or context callback fires
* tooltips MAY be registered during paint when `config.show_tooltips == true`

Tooltip hover is informational only and is owned by the framework tooltip
manager.

## 3.2 Interactive Policy

`TreeMapPolicy::Interactive`:

* `focusable() == true`
* `capture_eligible() == true`
* `defines_focus_scope() == false`
* `accepts_keyboard_activation() == false`

Navigation mode belongs to the framework. The widget does not mutate internal
selection, drill path, or active cell from keyboard input unless
`cx.keyboard_mode == KeyboardAuthorityMode::Capture`.

Pointer click on the widget enters Capture through the framework per
SPEC-HAL-328. The widget does not store a local `in_capture`, `captured`,
`editing`, or equivalent state field.

---

# 4. Public Configuration

```rust
#[derive(Debug, Clone)]
pub struct TreeMapConfig {
    /// Show branch header bands when there is enough space.
    pub show_headers: bool,
    /// Requested branch header height in logical pixels. Default: 20.0.
    pub header_height: f32,
    /// Gap/inset between cells in logical pixels. Default: 1.0.
    pub cell_padding: f32,
    /// Resting cell border width in logical pixels. Default: 1.0.
    pub border_width: f32,
    /// Cells with computed area below this value skip text and tooltip.
    /// Their geometry remains reserved by layout. Default: 4.0.
    pub min_interactive_area: f32,
    /// Maximum descendant depth shown relative to the current view root.
    /// Default: 3. Minimum: 1.
    pub visible_depth: usize,
    /// Register per-cell tooltips through PaintContext.
    pub show_tooltips: bool,
    /// Optional text color override. None = contrast from fill luminance.
    pub label_color_override: Option<Color>,
}
```

```rust
impl Default for TreeMapConfig {
    fn default() -> Self {
        Self {
            show_headers: true,
            header_height: 20.0,
            cell_padding: 1.0,
            border_width: 1.0,
            min_interactive_area: 4.0,
            visible_depth: 3,
            show_tooltips: true,
            label_color_override: None,
        }
    }
}
```

---

# 5. Layout

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TreeMapLayout {
    /// Bruls/Huizing/van Wijk squarified rectangles. Default.
    #[default]
    Squarified,
    /// Alternating horizontal/vertical splits preserving input order.
    SliceAndDice,
}
```

## 5.1 Layout Product

```rust
#[derive(Debug, Clone)]
pub struct TreeMapCellRect {
    pub id: TreeMapNodeId,
    pub path: Vec<TreeMapNodeId>,
    pub rect: RectF,
    pub header_rect: Option<RectF>,
    pub content_rect: RectF,
    pub depth: usize,
    pub is_branch: bool,
    pub text_level: TreeMapTextLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeMapTextLevel {
    Full,
    Label,
    Abbreviated,
    None,
}
```

Layout is a pure function of:

```rust
(root, drill_path, bounds, layout_algorithm, config.visible_depth, header rules)
    -> Vec<TreeMapCellRect>
```

No AppState, WidgetTree, callback, focus state, capture state, or theme mutation
is allowed during layout.

## 5.2 Current View Root

The current view root is:

* the root node when `drill_path` is empty
* the node reached by `drill_path` when every path segment is valid

The current view root itself does not receive a visible parent cell. Its valid
positive children fill the widget bounds.

If the current view root has no valid positive children and its own weight is
valid, the current view root renders as a single cell filling the widget bounds.
If the current view root also has invalid weight, TreeMap renders no cells.

## 5.3 Visible Depth

`config.visible_depth` is measured relative to the current view root.

* `visible_depth == 1`: only current-root children are rendered
* `visible_depth == 2`: children and grandchildren are rendered
* deeper descendants are represented by their ancestor cell at the cutoff depth

A branch at the cutoff depth renders as an aggregate cell using its own
`weight`, `label`, `detail`, and color resolution. Its children are not laid out
until the user drills into it or `visible_depth` increases.

## 5.4 Squarified Layout

Squarified layout:

* filters invalid non-positive siblings before layout
* sorts valid siblings by descending `weight`
* breaks ties by original sibling order
* produces deterministic rectangles for identical input
* does not preserve visible ordering

The algorithm emits axis-aligned rectangles only. It does not emit paths,
rounded rects, gradients, or custom render primitives.

## 5.5 Slice-and-Dice Layout

Slice-and-dice layout:

* preserves original sibling order after invalid nodes are excluded
* splits along X at even relative depths
* splits along Y at odd relative depths
* assigns each sibling a span proportional to `weight / total_valid_weight`

## 5.6 Branch Header Bands

Branch headers are rectangular bands, not separate widgets.

Header visibility for a branch cell:

```rust
show =
    config.show_headers
    && branch_has_visible_children
    && depth < config.visible_depth
    && rect.width >= 48.0
    && rect.height >= config.header_height + 16.0
```

If `show == true`:

```rust
let inner = rect.inset(config.cell_padding);
header_rect = RectF {
    x: inner.x,
    y: inner.y,
    width: inner.width,
    height: config.header_height.min(inner.height * 0.35),
};
content_rect = RectF {
    x: inner.x,
    y: header_rect.y + header_rect.height,
    width: inner.width,
    height: inner.height - header_rect.height,
};
```

If `show == false`:

```rust
header_rect = None;
content_rect = rect.inset(config.cell_padding);
```

If `content_rect.width <= 0.0` or `content_rect.height <= 0.0`, no children are
laid out.

---

# 6. Color Mapping

```rust
#[derive(Debug, Clone)]
pub enum TreeMapColorMapping {
    /// Use node.color, falling back to fallback_color.
    Direct {
        fallback_color: Color,
    },

    /// Map node.color_value through a deterministic gradient.
    MetricGradient(TreeMapGradient),

    /// Map node.color_value through sorted upper-bound thresholds.
    MetricDiscrete {
        thresholds: Vec<TreeMapColorThreshold>,
        fallback_color: Color,
    },

    /// Assign colors by top-level category; descendants inherit with depth tint.
    Category {
        palette: Vec<Color>,
        fallback_color: Color,
        depth_tint: f32,
    },
}

#[derive(Debug, Clone)]
pub enum TreeMapGradient {
    Sequential {
        min_value: f64,
        max_value: f64,
        min_color: Color,
        max_color: Color,
        fallback_color: Color,
    },
    Diverging {
        min_value: f64,
        mid_value: f64,
        max_value: f64,
        min_color: Color,
        mid_color: Color,
        max_color: Color,
        fallback_color: Color,
    },
}

#[derive(Debug, Clone)]
pub struct TreeMapColorThreshold {
    pub upper_bound: f64,
    pub color: Color,
}
```

There is no `Custom(Box<dyn Fn(...)>)` color mapping in v1. The color mapping
enum must be able to derive `Debug` and `Clone`.

## 6.1 Direct

`Direct` uses `node.color` when present. If absent, it uses `fallback_color`.

## 6.2 Metric Gradient

Metric gradients use `node.color_value`. If `color_value` is absent, NaN, or
infinite, the resolved color is:

1. `node.color`, when present
2. the gradient's `fallback_color`

Sequential gradients require `min_value < max_value`.

Diverging gradients require:

```rust
min_value < mid_value && mid_value < max_value
```

The diverging midpoint is always `mid_value`. There is no implicit zero or
implicit midpoint rule.

Values below `min_value` clamp to `min_color`. Values above `max_value` clamp to
`max_color`.

## 6.3 Metric Discrete

`thresholds` MUST be sorted by strictly ascending `upper_bound`.

Resolution:

```rust
for threshold in thresholds {
    if value < threshold.upper_bound {
        return threshold.color;
    }
}
return fallback_color;
```

Therefore values greater than or equal to the last threshold use
`fallback_color`. Empty thresholds also use `fallback_color`.

If `color_value` is absent, NaN, or infinite, the resolved color is:

1. `node.color`, when present
2. `fallback_color`

## 6.4 Category

Category mode is separate from metric heat coloring.

* top-level visible children receive colors from `palette` by sibling order
* if `palette` is empty, `fallback_color` is used
* descendants inherit the nearest top-level category color
* descendant color is tinted by:

```rust
let t = (depth as f32 * depth_tint).clamp(0.0, 0.35);
let color = category_color.lerp(Color::WHITE, t);
```

`color_value` is ignored in Category mode.

## 6.5 Branch Color vs Leaf Color

Metric modes color every rendered cell from that cell's own `color_value`.
Branch headers use the branch cell's resolved fill color.

Category mode colors structure. A branch and its descendants share the same
category lineage, with deterministic depth tinting.

The widget must not combine category color and metric heat color in v1.

## 6.6 Label Contrast

If `config.label_color_override` is `Some(color)`, all cell and header labels
use that color.

If no override is provided, text color is derived from the resolved fill color's
relative luminance:

```rust
let y = 0.2126 * r_linear + 0.7152 * g_linear + 0.0722 * b_linear;
let text = if y < 0.45 { Color::WHITE } else { Color::BLACK };
```

The luminance computation uses linearized sRGB channels.

---

# 7. Rendering

## 7.1 Pipeline Rule

TreeMap v1 uses the Rect pipeline for all non-text primitives:

* `push_rect`
* `push_border_rect`

Text uses the normal text path.

TreeMap v1 MUST NOT use:

* `push_rounded_rect`
* `push_border_rounded_rect`
* `push_gradient_rect`
* `paint_linear_gradient`
* `paint_vertical_gradient`
* `paint_radial_gradient`
* `DecorationStack`
* custom GPU pipelines
* offscreen textures

Cushion shading is out of scope for v1 because it requires gradient rendering
inside the same visual unit and conflicts with the Rect-only pipeline rule.

## 7.2 Paint Order

For each visible cell, paint in this order:

1. cell fill via `push_rect`
2. cell border via `push_border_rect`
3. branch header fill via `push_rect`, when `header_rect` exists
4. branch header text
5. child cells, recursively by depth
6. cell label/detail text

All non-text primitives remain in the Rect pipeline. Z-order is deterministic:
parent cells paint before child cells; siblings paint in the layout product
order.

Every painted cell emits a fill and border primitive in resting state. Hover,
selection, focus, and capture modulate existing fill/border attributes only.

## 7.3 Text Degradation

Text level is resolved from the cell content rect:

```rust
Full        if width >= 96.0 && height >= 52.0
Label       if width >= 56.0 && height >= 24.0
Abbreviated if width >= 36.0 && height >= 18.0
None        otherwise
```

Rules:

* `Full`: render `label` and `detail` when `detail` exists
* `Label`: render `label` only
* `Abbreviated`: render one line, ellipsized to fit
* `None`: render no text

Text is never scaled by viewport width. If measured text does not fit its target
rect, it is ellipsized. If ellipsized text still does not fit, text is omitted.

Branch header labels are single-line ellipsized labels. If the header text does
not fit, it is omitted; the header band may still render.

## 7.4 Tooltip Registration

When `config.show_tooltips == true`, every painted cell whose area is greater
than or equal to `config.min_interactive_area` registers a tooltip during paint:

```rust
cx.register_tooltip(cell.rect, TooltipPayload::Text(tooltip_text));
```

`tooltip_text` contains, in this order:

1. full path labels joined by `" / "`
2. node `detail`, when present
3. `weight`
4. `color_value`, when present
5. child count for branch nodes

Tooltip lifecycle, timing, placement, and singleton behavior are owned by the
framework tooltip manager. TreeMap does not own tooltip timers or visibility.

---

# 8. Interaction

Interaction exists only when `policy == TreeMapPolicy::Interactive`.

## 8.1 Hit Testing

Hit testing uses the current frame's `TreeMapCellRect` list.

Given a pointer point:

1. find all cell rects containing the point
2. choose the deepest cell
3. break depth ties by latest paint order

If no cell contains the point, the hit target is `None`.

## 8.2 Widget State

```rust
pub struct TreeMap {
    id: WidgetInternalId,
    root: TreeMapNode,
    policy: TreeMapPolicy,
    layout: TreeMapLayout,
    color_mapping: TreeMapColorMapping,
    config: TreeMapConfig,

    /// Current drill-down path from root to current view root.
    drill_path: Vec<TreeMapNodeId>,
    /// Pointer hover target. None when no visible cell is hovered.
    hovered_cell: Option<TreeMapNodeId>,
    /// Keyboard-active internal cell used only while the widget owns capture.
    active_cell: Option<TreeMapNodeId>,
    /// Consumer-visible selected node.
    selected_cell: Option<TreeMapNodeId>,

    cached_cells: Vec<TreeMapCellRect>,
    cached_bounds: Option<RectF>,

    on_select: Option<Box<dyn FnMut(TreeMapNodeId, &TreeMapNode)>>,
    on_drill: Option<Box<dyn FnMut(&[TreeMapNodeId])>>,
    on_context: Option<Box<dyn FnMut(TreeMapNodeId, &TreeMapNode)>>,
}
```

The widget does not store focus or capture mirror fields. Paint queries
`cx.is_focused()` and `cx.has_capture()`. Event handling checks
`cx.keyboard_mode`.

`TreeMap` itself does not derive `Debug` or `Clone` because callback fields are
not cloneable/debuggable.

## 8.3 Pointer

Pointer move:

* updates `hovered_cell`
* requests repaint when hover target changes
* does not fire selection or drill callbacks

Left click:

* leaf cell: sets `selected_cell`, sets `active_cell`, fires `on_select`
* branch header: drills into that branch, updates `drill_path`, fires `on_drill`
* branch body: sets `selected_cell`, sets `active_cell`, fires `on_select`

Double left click:

* branch cell: drills into that branch, updates `drill_path`, fires `on_drill`
* leaf cell: same as single left click

Right click:

* visible cell: sets `active_cell`, fires `on_context`
* empty space: returns `false`

The widget does not open a menu. The consumer owns context menu composition.

## 8.4 Keyboard in Capture

Keyboard handling is legal only when:

```rust
cx.keyboard_mode == KeyboardAuthorityMode::Capture
```

Navigation mode key events return `false`.

Capture-mode keys:

* ArrowLeft / ArrowRight / ArrowUp / ArrowDown:
  move `active_cell` to the nearest visible cell in that direction, using
  SPEC-HAL-328 edge-based spatial adjacency over visible cell rects
* Enter:
  branch active cell drills in; leaf active cell selects and fires `on_select`
* Space:
  selects active cell and fires `on_select`
* Backspace:
  pops one `drill_path` level when not at root; fires `on_drill`
* Tab / Shift+Tab:
  return `false`; framework handles capture release and traversal
* Escape:
  return `false`; framework releases capture unconditionally

Escape never drills up, clears selection, cancels state, or defocuses.

## 8.5 Drill-Down

Drilling into a branch appends that branch ID to `drill_path`.

The drilled branch becomes the current view root. Its valid positive children
fill the full widget bounds. `visible_depth` is evaluated relative to the new
view root.

Backspace pops one level. External breadcrumbs are composition; TreeMap does not
render breadcrumb UI.

---

# 9. Focus and Capture Visuals

TreeMap is a composite interactive visualization. Framework focus is on the
TreeMap widget. The active internal cell is widget-owned state.

Focus/capture indication is intrinsic:

* every painted cell has a resting border
* hovered cell modulates fill toward white by `0.08`
* selected cell modulates its existing border to `theme.colors.accents.primary`
* active cell, when the widget is focused, modulates its existing border to
  `theme.colors.borders.focus`
* active cell border width is `1.0` in Navigation focus and `2.0` in Capture

The widget must not paint at `LAYER_FOCUS`, add a new focus ring, add a new
capture ring, or store local capture state.

---

# 10. Performance

**INV-283-P1: Layout Cache**

Layout is recomputed only when one of these changes:

* root data
* widget bounds
* `drill_path`
* `visible_depth`
* layout algorithm
* header-affecting config

**INV-283-P2: Layout Includes Tiny Positive Nodes**

All valid positive nodes within visible depth participate in layout. Tiny nodes
are not redistributed away. Paint/text/tooltip may be culled, but geometry
proportion remains correct.

**INV-283-P3: Text Culling**

Text shaping is skipped for `TreeMapTextLevel::None`.

**INV-283-P4: Target**

The widget must handle 500 visible cells at 60 fps with cached layout. Larger
trees are supported through `visible_depth` and drill-down.

---

# 11. Consumer API

```rust
let root = TreeMapNode {
    id: TreeMapNodeId(1),
    label: "Market".to_string(),
    detail: Some("S&P sample".to_string()),
    weight: 1.0,
    color: None,
    color_value: None,
    children: vec![
        TreeMapNode {
            id: TreeMapNodeId(2),
            label: "Technology".to_string(),
            detail: Some("+1.2%".to_string()),
            weight: 42.0,
            color: None,
            color_value: Some(1.2),
            children: Vec::new(),
        },
        TreeMapNode {
            id: TreeMapNodeId(3),
            label: "Health Care".to_string(),
            detail: Some("-0.4%".to_string()),
            weight: 18.0,
            color: None,
            color_value: Some(-0.4),
            children: Vec::new(),
        },
    ],
};

let tree_map = TreeMap::try_new(root).expect("TreeMap sample data has unique IDs")
    .with_policy(TreeMapPolicy::Interactive)
    .with_layout(TreeMapLayout::Squarified)
    .with_color_mapping(TreeMapColorMapping::MetricGradient(
        TreeMapGradient::Diverging {
            min_value: -5.0,
            mid_value: 0.0,
            max_value: 5.0,
            min_color: Color::from_hex(0xC0392B),
            mid_color: Color::from_hex(0xF2F2F2),
            max_color: Color::from_hex(0x27AE60),
            fallback_color: Color::from_hex(0x3A3A3A),
        },
    ))
    .with_visible_depth(3)
    .on_select(|id, node| {
        tracing::debug!("selected treemap node {:?}: {}", id, node.label);
    })
    .on_context(|id, node| {
        tracing::debug!("context requested for {:?}: {}", id, node.label);
    });
```

---

# 12. Forbidden Behaviors

**FORBID-283-0:** No radial treemap, sunburst, icicle, Voronoi, or organic
layout in v1.

**FORBID-283-1:** No cushion shading, rendering-gradient primitives, rounded
rectangles, DecorationStack, offscreen texture, or custom GPU pipeline in v1.

**FORBID-283-2:** No local capture mirror field: `in_capture`, `captured`,
`editing`, or equivalent.

**FORBID-283-3:** No widget-specific Escape behavior. Escape is framework-owned.

**FORBID-283-4:** No duplicate node IDs.

**FORBID-283-5:** No index-based persistent hover, selection, active cell, or
drill state.

**FORBID-283-6:** No custom callback color mapping in v1.

**FORBID-283-7:** No built-in breadcrumb, legend, filter, aggregation, or context
menu UI.

**FORBID-283-8:** No ProportionBar in this SPEC.

---

# 13. Acceptance Criteria

1. Renders rectangular nested cells from `TreeMapNode`.
2. Rejects duplicate node IDs with `TreeMapDataError::DuplicateId`.
3. Excludes non-positive, NaN, and infinite weights from layout.
4. Squarified layout is deterministic and produces readable aspect ratios.
5. Slice-and-dice layout preserves input order.
6. `visible_depth` limits rendered nesting and drill-down resets depth relative
   to the drilled branch.
7. Branch headers render only when header rules allow them.
8. Branch content rect excludes visible header height.
9. Direct color mapping uses `node.color` with fallback.
10. Metric gradient mapping uses explicit `mid_value` for diverging gradients.
11. Discrete mapping uses `fallback_color` for values greater than or equal to
    the last threshold.
12. Category mapping is separate from metric heat mapping.
13. Label color is derived from fill luminance unless explicitly overridden.
14. Text degrades from label+detail to label to ellipsis to no text.
15. Tooltip trigger registration uses `PaintContext::register_tooltip`.
16. Rect pipeline only: `push_rect` and `push_border_rect` for all non-text
    primitives.
17. Static policy accepts no input and fires no callbacks.
18. Interactive policy supports hover, selection, drill-down, context callback,
    and capture-mode keyboard navigation.
19. Escape releases capture through the framework and does not drill up.
20. Right-click on a visible cell fires `on_context`.
21. No Rust implementation, example, catalog entry, or test is implied by this
    SPEC until a task explicitly orders it.

---

# 14. Non-Goals

* radial treemap
* sunburst chart
* icicle chart
* Voronoi treemap
* cushion shading
* rounded cells
* animated drill transitions
* real-time streaming data ownership
* comparison mode
* built-in legend
* built-in breadcrumbs
* built-in filtering
* built-in context menu
* ProportionBar
* custom color callback mapping

---

# 15. Consistency Matrix

| Pillar | Status |
|--------|--------|
| A - Retained-Mode Ownership | Preserved. TreeMap is a widget in the tree. |
| B - Phase Separation | Preserved. Layout and paint are pure; callbacks fire from event handling. |
| C - ID-based Identity | Preserved. Node identity is `TreeMapNodeId`; persistent index identity is forbidden. |
| D - Pure Data | Preserved. Consumer owns data and meaning. |
| E - GPU-First | Preserved. Scene graph Rect/Text primitives only; no custom pipeline. |
| F - Sovereignty | Preserved. Focus, capture, tooltip lifecycle, and Escape semantics remain framework-owned. |
