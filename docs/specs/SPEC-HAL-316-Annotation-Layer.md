# SPEC-HAL-316: **Annotation Layer**

### *Framework-Level Visual Annotations, Connectors, and Guided Overlays*

**Author:** JC
**Status:** Draft
**Depends on:**

* SPEC-HAL-001 (Core Architecture)
* SPEC-HAL-002 (Constitution)
* SPEC-HAL-081C (Overlay Surface Integration)
* SPEC-HAL-226 (Tooltip / Hint / Help Surfaces)
* SPEC-HAL-296 (Plot — coordinate-authority rendering)

---

# 1. Purpose

This SPEC defines a **framework-level annotation system** for Halcyon.

Annotations are visual overlays — labels, connectors, arrows,
highlights, bubbles — attached to widgets or coordinates, rendered
above all normal content, with automatic positioning and optional
obstacle-avoiding routing.

This system unifies several ad-hoc patterns that exist today:

| Current Pattern | Problem |
|----------------|---------|
| Tooltip (SPEC-HAL-226) | Hover-only, no connectors, no sequencing |
| Validation error (PropertyEditor) | Hand-positioned, z-order fights, widget-specific |
| Plot markers (PriceLevel, PlotTag, PlotCircle) | Plot-specific, no widget awareness |
| ComboBox popup overlay | Widget-specific paint_overlay |

Each of these reinvents "render something above normal content,
positioned relative to a widget." The annotation layer provides
a single, general mechanism.

---

# 2. Design Principles

**DP-316-1: Annotations are overlay projections, not tree members.**
Annotations do not participate in layout, do not receive focus, do not
affect widget tree structure. They are rendered in a dedicated overlay
pass after all widget content.

**DP-316-2: Anchoring by ID, not reference.**
Annotations reference widgets by `WidgetId`. The framework resolves
current bounds from the WidgetTree at render time. No persistent
references. Constitutional compliance (SPEC-HAL-002 Pillar C).

**DP-316-3: Consumer defines semantics, framework handles visuals.**
The consumer says "connect A to B with a label." The framework
computes positions, resolves routes, handles z-ordering, renders
primitives. The consumer never computes screen coordinates.

**DP-316-4: Layout-driven invalidation.**
Annotation positions recompute when the WidgetTree layout changes.
Not per-frame. Cached until layout invalidation.

**DP-316-5: Composable primitives.**
Complex annotations are composed from simple primitives: labels,
connectors, highlights, circles, arrows. No monolithic annotation
widget.

---

# 3. Annotation Primitives

## 3.1 Anchor

An anchor identifies where an annotation attaches.

```rust
/// Where an annotation attaches.
pub enum AnnotationAnchor {
    /// Attached to a widget by ID. Framework resolves current bounds.
    Widget(WidgetId),
    /// Fixed screen-space position (logical pixels from viewport origin).
    Screen(f32, f32),
    /// World coordinates (for Plot integration).
    /// Requires a PlotId to resolve the coordinate transform.
    World { plot_id: WidgetId, x: f64, y: f64 },
}
```

## 3.2 Label

A text bubble attached to an anchor.

```rust
pub struct AnnotationLabel {
    pub id: u64,
    pub anchor: AnnotationAnchor,
    pub text: String,
    pub placement: Placement,
    pub style: LabelStyle,
}

/// Where the label sits relative to its anchor.
pub enum Placement {
    Above,
    Below,
    Left,
    Right,
    /// Framework chooses best position to avoid occlusion.
    Auto,
}

pub struct LabelStyle {
    pub text_color: Color,
    pub background: Color,
    pub border_color: Option<Color>,
    pub border_width: f32,
    pub corner_radius: f32,
    pub padding: (f32, f32),
    pub font_size: Option<f32>,
    pub max_width: Option<f32>,
}
```

`Placement::Auto` uses the RouteResolver (§6) to find the least
occluded position.

## 3.3 Connector

A visual link between two anchors.

```rust
pub struct AnnotationConnector {
    pub id: u64,
    pub source: AnnotationAnchor,
    pub target: AnnotationAnchor,
    pub style: ConnectorStyle,
    pub routing: RouteStrategy,
    /// Optional label attached at the midpoint of the connector.
    pub label: Option<String>,
}

pub struct ConnectorStyle {
    pub color: Color,
    pub width: f32,
    pub line_style: LineStyle,
    pub arrow_start: bool,
    pub arrow_end: bool,
    pub arrow_size: f32,
}

pub enum LineStyle {
    Solid,
    Dashed { dash: f32, gap: f32 },
    Dotted { dot: f32, gap: f32 },
}

pub enum RouteStrategy {
    /// Straight line from source to target.
    Direct,
    /// Route around visible widgets between source and target.
    AvoidWidgets,
    /// Smooth Bezier curve. Control points auto-computed.
    Bezier,
    /// Bezier curve routed around obstacles.
    BezierAvoid,
    /// Orthogonal (right-angle) routing.
    Orthogonal,
}
```

## 3.4 Highlight

A visual emphasis on a widget — border glow, dimming of surroundings,
pulsing outline.

```rust
pub struct AnnotationHighlight {
    pub id: u64,
    pub target: WidgetId,
    pub style: HighlightStyle,
}

pub struct HighlightStyle {
    /// Border drawn around the widget's bounds.
    pub border_color: Color,
    pub border_width: f32,
    pub corner_radius: f32,
    /// Padding between widget bounds and highlight border.
    pub padding: f32,
    /// If true, dim all content outside the highlight.
    pub spotlight: bool,
    /// Spotlight dimming color (typically black at 50-70% alpha).
    pub spotlight_color: Color,
    /// Optional pulse animation.
    pub pulse: bool,
}
```

Spotlight mode dims the entire UI except the highlighted widget.
Used for tutorial focus steps.

## 3.5 Circle Marker

A circle at an anchor point. Reuses the same SDF rendering as
Plot's PlotCircle but anchored to widgets or screen coordinates.

```rust
pub struct AnnotationCircle {
    pub id: u64,
    pub anchor: AnnotationAnchor,
    pub radius: f32,
    pub fill_color: Color,
    pub border_color: Color,
    pub border_width: f32,
}
```

## 3.6 Composite Annotation

Bundles multiple primitives into a single logical annotation.
Used for tutorial steps: highlight + label + connector.

```rust
pub struct CompositeAnnotation {
    pub id: u64,
    pub labels: Vec<AnnotationLabel>,
    pub connectors: Vec<AnnotationConnector>,
    pub highlights: Vec<AnnotationHighlight>,
    pub circles: Vec<AnnotationCircle>,
}
```

---

# 4. Annotation Store

Annotations are stored on the `App` (not on individual widgets),
similar to how navigation state lives on the App.

```rust
pub struct AnnotationStore {
    labels: Vec<AnnotationLabel>,
    connectors: Vec<AnnotationConnector>,
    highlights: Vec<AnnotationHighlight>,
    circles: Vec<AnnotationCircle>,
    composites: Vec<CompositeAnnotation>,

    /// Resolved positions (recomputed on layout change).
    resolved: ResolvedAnnotations,
    /// Layout generation when resolved was last computed.
    resolved_generation: u64,
}
```

Consumer access via `AnnotationContext`:

```rust
impl AnnotationContext {
    pub fn add_label(&mut self, label: AnnotationLabel);
    pub fn remove(&mut self, id: u64);
    pub fn clear(&mut self);

    pub fn add_connector(&mut self, connector: AnnotationConnector);
    pub fn add_highlight(&mut self, highlight: AnnotationHighlight);
    pub fn add_circle(&mut self, circle: AnnotationCircle);
    pub fn add_composite(&mut self, composite: CompositeAnnotation);

    /// Activate a tutorial sequence.
    pub fn start_guide(&mut self, guide: AnnotationGuide);
    pub fn advance_guide(&mut self);
    pub fn dismiss_guide(&mut self);
}
```

---

# 5. Auto-Placement

When `Placement::Auto` is specified, the framework picks the position
that minimizes occlusion.

Algorithm:

1. Resolve anchor bounds from WidgetTree (widget ID → screen rect).
2. Compute candidate positions: Above, Below, Left, Right of anchor.
3. For each candidate, compute the label rect.
4. Score each candidate by overlap area with visible widget rects.
5. Pick the candidate with the lowest overlap score.
6. If all candidates overlap, pick the one with the least overlap
   and shift the label to minimize it.

This runs once per layout cycle. Cached.

---

# 6. Route Resolution

## 6.1 Obstacle Map

The obstacle map is extracted from the WidgetTree:

```rust
fn build_obstacle_map(tree: &WidgetTree, exclude: &[WidgetId]) -> Vec<RectF> {
    // Collect screen-space bounding rects of all visible widgets
    // excluding source/target widgets and their ancestors.
}
```

The `exclude` list prevents the source and target widgets from
acting as obstacles for their own connector.

## 6.2 Visibility Graph Router

For `RouteStrategy::AvoidWidgets`:

1. Build obstacle map (axis-aligned rects).
2. Compute visibility graph: vertices are obstacle corners (inflated
   by connector width / 2 for clearance), edges are line segments
   that don't intersect any obstacle.
3. Add source and target points as graph vertices.
4. Dijkstra shortest path from source to target.
5. Result: ordered list of waypoints.

Waypoints are rendered as a polyline (connected quads with
mitered or rounded joins).

Complexity: O(n² log n) for n obstacle corners. For typical UI
(< 100 visible widgets), this is sub-millisecond.

## 6.3 Bezier Routing

For `RouteStrategy::Bezier`:

1. Compute straight-line direction from source to target.
2. Generate control points offset perpendicular to the line at
   1/3 and 2/3 distance.
3. Default offset: 30% of line length, curved away from the
   denser side (more obstacles above → curve below).

For `RouteStrategy::BezierAvoid`:

1. Run visibility graph to get waypoints.
2. Fit a smooth cubic Bezier spline through the waypoints.
3. Catmull-Rom to Bezier conversion for C1 continuity.

## 6.4 Orthogonal Routing

For `RouteStrategy::Orthogonal`:

1. Compute L-shaped or Z-shaped path (1-2 right angles).
2. Prefer the path that avoids the most obstacles.
3. If both L-shapes are blocked, fall back to Z-shape with
   a middle segment offset to clear obstacles.

## 6.5 Cache Invalidation

Route results are cached and keyed on:
- Source widget bounds
- Target widget bounds
- Obstacle map hash (changes when any visible widget moves/resizes)

Recomputation happens only when the key changes. Layout invalidation
triggers cache clear.

---

# 7. Rendering

## 7.1 Render Phase

Annotations render in a dedicated phase AFTER all widget paint
and paint_overlay passes. This ensures annotations appear above
all widget content including popups and tooltips.

The annotation render uses the same PaintSink interface as widgets.
Available primitives:

- `push_rect` / `push_rounded_rect` — label backgrounds
- `push_border_rounded_rect` — label borders
- `push_text_with_backgrounds` — label text at overlay z
- Overlay vertex buffer — connector lines, arrows, highlights
- Circle shader — annotation circles (SDF, same as Plot)

Z-index range: `LAYER_ANNOTATION = 2000` (above `LAYER_OVERLAY = 1000`).

## 7.2 Connector Rendering

Straight and polyline connectors: rotated quads per segment.
For each segment (p1, p2):

1. Direction = normalize(p2 - p1)
2. Perpendicular = (-dir.y, dir.x)
3. Four corners = p1 ± perp * half_width, p2 ± perp * half_width
4. Two triangles (6 vertices)

Dashed/dotted: subdivide segment at dash intervals, same as
PriceLevel dashed lines.

Arrowheads: equilateral triangle at endpoint, aligned with
connector direction. 3 vertices.

Bezier connectors: evaluate curve at n intervals (n = length / 4px),
emit quad strip along the evaluated polyline.

## 7.3 Spotlight Rendering

A fullscreen rect at `LAYER_ANNOTATION - 1` with the spotlight
color (e.g., rgba(0, 0, 0, 0.6)). Then a cutout rect matching
the highlighted widget's padded bounds — rendered with the
background color (effectively punching a hole).

Implementation: two rects per spotlight region (above and below
the cutout), or a stencil-based approach.

Simpler approach: render the dimming rect fullscreen, then render
the highlighted widget's area again (re-paint from cache or just
clear the spotlight rect at that location). Given Halcyon's
overlay architecture, the simplest correct approach is:

1. Fullscreen dim rect at z=1999
2. Clear rect (background color) at highlighted widget bounds at z=1999
3. Highlight border at z=2000

---

# 8. Tutorial / Guide System

## 8.1 AnnotationGuide

A sequential series of steps, each a `CompositeAnnotation`.

```rust
pub struct AnnotationGuide {
    pub id: u64,
    pub steps: Vec<GuideStep>,
}

pub struct GuideStep {
    /// Composited annotation for this step.
    pub annotation: CompositeAnnotation,
    /// Advance trigger.
    pub advance: AdvanceTrigger,
}

pub enum AdvanceTrigger {
    /// Advance when the user clicks the highlighted widget.
    ClickTarget(WidgetId),
    /// Advance when the user clicks a "Next" button in the label.
    ClickNext,
    /// Advance after a duration.
    Timeout(std::time::Duration),
    /// Advance on any click.
    AnyClick,
    /// Manual advance only (consumer calls advance_guide).
    Manual,
}
```

## 8.2 Guide Lifecycle

1. `start_guide(guide)` — activates step 0.
2. Current step's annotations are rendered.
3. On advance trigger → deactivate current step, activate next.
4. After last step → guide complete, all annotations removed.
5. `dismiss_guide()` — cancel at any point.

## 8.3 Interaction During Guide

When a guide step has `spotlight: true`, input outside the
highlighted region is blocked. Pointer events on the dimmed
area are consumed (return true) but not forwarded.

When `spotlight: false`, all normal interaction continues.
Annotations are passive overlays.

---

# 9. Plot Integration

Plot markers (PriceLevel, PlotTag, PlotRegion, PlotCircle) remain
Plot-internal for performance reasons — they render in the offscreen
GPU texture, not the widget overlay pass.

The annotation layer integrates with Plot via `AnnotationAnchor::World`:

```rust
AnnotationAnchor::World {
    plot_id: my_plot_widget_id,
    x: 1735689600.0,
    y: 101.5,
}
```

The annotation layer queries the Plot widget for its current
`Transform` (world → screen mapping) and resolves the anchor to
screen coordinates. This enables connectors between Plot data points
and non-Plot widgets (e.g., an arrow from a chart signal to a
dashboard indicator).

---

# 10. Invariants

**INV-316-1:** Annotations never participate in layout. They are
rendered after layout is complete. They do not affect widget sizes
or positions.

**INV-316-2:** Annotations reference widgets by ID only. No
persistent references to widget instances. Bounds are resolved
from the WidgetTree at render time.

**INV-316-3:** Route resolution runs at most once per layout cycle.
Results are cached and reused across frames until layout invalidates.

**INV-316-4:** Annotations render at z >= LAYER_ANNOTATION (2000).
They appear above all widget content, overlays, tooltips, and popups.

**INV-316-5:** Spotlight mode blocks input outside the highlighted
region. Non-spotlight annotations are passive (do not affect input
routing).

**INV-316-6:** The annotation store is owned by the App. Annotations
do not create tree nodes, do not have WidgetIds, and do not
participate in the widget lifecycle.

**INV-316-7:** Guide steps are sequential. Only one step is active
at a time. Advancing deactivates the current step before activating
the next.

---

# 11. Forbidden Behaviors

**FORBID-316-1:** Annotations MUST NOT mutate the WidgetTree.
They are projection-only. No adding children, no reparenting,
no modifying widget state.

**FORBID-316-2:** Annotations MUST NOT participate in focus or
capture. They are not focusable widgets. If an annotation label
has interactive content (e.g., a "Next" button in a guide step),
it is handled via a dedicated annotation event path, not the
widget focus system.

**FORBID-316-3:** The RouteResolver MUST NOT run per-frame.
It runs on layout invalidation only. If a consumer forces
per-frame route resolution, performance degrades — this is a
consumer bug, not a framework responsibility.

**FORBID-316-4:** Annotations MUST NOT hold references to
widget instances, layout products, or text shaping results.
Constitutional compliance (SPEC-HAL-002 §3.9).

---

# 12. Relationship to Existing Systems

| System | Relationship |
|--------|-------------|
| Tooltip (SPEC-HAL-226) | Tooltip becomes a specialized AnnotationLabel with hover trigger. Existing tooltip API remains as convenience. |
| Validation errors | Become AnnotationLabels with error style, anchored below editing widget. |
| Plot markers | Remain Plot-internal for GPU performance. Annotation layer bridges via World anchors. |
| ComboBox popup | Remains widget-level paint_overlay. Not an annotation — it's a structural part of the widget. |
| Modal overlays | Remain compositor-level. Annotations are visual only, modals are structural. |

---

# 13. Implementation Phases

**Phase 1:** Core primitives — AnnotationLabel, AnnotationHighlight.
Direct placement (no routing). Render in overlay pass. App-level store.

**Phase 2:** Connectors — AnnotationConnector with Direct and Bezier
routing. Arrow rendering. Polyline quad-strip renderer.

**Phase 3:** Route resolution — Visibility graph router for
AvoidWidgets and BezierAvoid. Obstacle map from WidgetTree.
Cache with layout-driven invalidation.

**Phase 4:** Guide system — AnnotationGuide, sequential steps,
advance triggers, spotlight mode with input blocking.

**Phase 5:** Plot integration — World anchor resolution, cross-widget
connectors (chart ↔ dashboard).

---

# 14. Acceptance Criteria

1. AnnotationLabel renders above all widget content at correct anchor position
2. Auto-placement picks least-occluded position among four candidates
3. AnnotationConnector renders straight line between two widget anchors
4. Connector with `arrow_end: true` renders arrowhead at target
5. Dashed/dotted connectors render correct patterns
6. Bezier connector renders smooth curve between anchors
7. AvoidWidgets routing produces obstacle-free path
8. AnnotationHighlight draws border around target widget bounds
9. Spotlight dims all content except highlighted widget
10. Guide system advances through steps on trigger
11. Spotlight blocks input outside highlighted region
12. Route cache invalidates on layout change
13. Annotations do not affect layout (INV-316-1)
14. Widget deletion removes annotations anchored to it
15. Connectors update when source/target widgets move (layout change)
16. World anchors resolve correctly through Plot's Transform
17. 50 simultaneous annotations render without frame drop
