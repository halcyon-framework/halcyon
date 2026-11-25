# Local control alignment

**Status:** Explanatory, non-binding guidance for local control alignment.

The Constitution (SPEC-HAL-002/002A) and the applicable geometry, invalidation,
theming, widget-ownership, input and retained-rendering contracts
(SPEC-HAL-024/026/012/141/328/354) govern the mechanisms described here.
SPEC-HAL-008/275/226A/369 retain their Draft status; SPEC-HAL-363/364 retain
their Proposed status. SPEC-HAL-332 is rejected and retired. This guide does
not adopt any of them or establish runtime or visual conformance.

## Opt in

Create a normal tree-owned ControlAlignmentGroup::column(), optionally with_spacing. Add existing controls as direct rows, then call WidgetTree::set_control_alignment_member(row, true). For a TooltipHost row, mark the host; it explicitly forwards the parts protocol to its single actual tree child. Unmarked rows and independent/nested groups do not contribute parts. There is no window scan, type/text heuristic, hidden label node or global registry.

Direct participants are horizontal Slider Track/Bar/Pill and linear RangeSlider. Captioned TextField/ComboBox rows participate through ControlCaption: mark the host row, not the editor child. The host supplies the caption and arranges one real tree-owned body-only editor child; the editor remains the only input/focus/popup owner. Radial and other families are not advertised as participants. An explicitly marked unsupported row fails the layout contract rather than silently misaligning. Other owners can implement the same value-only protocol without changing the group's semantics.

## Explicit composition

ControlAlignmentGroup::with_trailing_order(ControlTrailingOrder::ValueThenLabel) opts this group into body → numeric value → right caption. The default LabelThenValue keeps body → caption → value. Named columns carry their full actual extents and assignments through the value-only parts protocol. Participants consume the supplied slots instead of reconstructing the default order. Equal final columns reuse the existing cache; a changed assignment reaches current Layout.

Slider::with_label_side(SliderLabelSide::Left) places its own caption before a horizontal Track/Bar/Pill. Right is the unchanged default; numeric with_value_field_side remains independent. A labeled Radial plus Left is not supported and fails explicitly in Layout. The horizontal Left path also resolves its local parts outside a group; normal ancestor clipping remains responsible for overflow. No new widget-owned self clipping is claimed. RangeSlider and ControlCaption do not expose this Slider left-caption API.

The Calibration panel in the [Showcase inputs example](../examples/showcase/tab_inputs.rs) demonstrates two independent local groups: A uses ValueThenLabel with right Slider captions; B uses left Slider captions and right numeric fields. Both keep values 42/75, domains, natural parts and existing editor ownership. Only track allocation absorbs extra width; composition/resize does not edit values or emit callbacks.

## Captioned editor rows

Add ControlCaption::new("Target node") as a row and add TextField::new() as its one actual tree child; mark the caption row (or its TooltipHost) as a member. The same composition accepts ComboBox. The host owns only its caption and child placement; it does not paint, rebase events for, or embed the editor. WidgetTree's normal child rect anchors caret, cursor, popup, overlay hit and joined chrome to the actual field body. Caption and unused reserves are not hit targets. set_caption uses UpdateContext layout invalidation; changing the caption does not change the child identity or editing state.

Scoped TextField measurement reports a flexible minimum from its existing min_hit_size and padding/caret metrics. Scoped ComboBox measurement uses its existing empty-input minimum, padding and affordance. Both are finite and independent of current text/filter results; final Arrange gives the real child the shared body allocation before any input. These scoped probes do not consume TextField start-captured initialization. Unwrapped ordinary TextField fill sizing and ordinary ComboBox preferred width remain unchanged. The host is a row for this explicit local group, whose existing traversal clip encloses the entire row when logical content overflows; it emits no widget-owned clip. It accepts body-only participants, rather than discarding another control's owned labels/values. It is not an implicit arbitrary-widget form adapter.

## Ownership and geometry

During scoped Measure the current control reports ControlPartsRequirements through LayoutContext: natural body width/row height and optional leading/trailing label/value widths with existing gaps. None differs from an empty owned label. TextEngine measurement stays with the control. Ordinary descendant layout clears the request; only forward_control_parts explicitly passes it through an identity single-child wrapper.

The local group recomputes maxima from current rows. It assigns one visible logical body interval, plus per-side label/value slots, in scoped Arrange. Right labels start next to the body; left labels end next to it. Glyphs and numeric editors keep their natural widths and existing order. Missing parts leave empty reserves, not input/editor targets. The group owns no value, callback, focus, Capture, tooltip timing or editing state.

Participants publish their final body, owned-label and numeric-field geometry in Layout. Paint reads it; input can use it before the next Paint. The visible body interval is shared directly rather than shortened again by individual decorative gutters. Existing paint projection owns physical snapping; no scale/absolute coordinate enters Layout and no arbitrary rounded screen pixel is promised to invert to an exact float. Slider decoration remains centered on existing value geometry under admitted ancestor clips.

## Constraints, overflow and input

Natural measure is finite under unbounded constraints and independent of shared allocation. Tight allocation has normal priority. Outside opt-in, existing natural/Start/explicit-width paths remain unchanged. Two groups have no shared maxima.

At insufficient width, preserve natural side parts in sequence and reduce body width to zero before allowing overlap. No shrink/ellipsis/wrapping policy is added. size respects Constraints; content_size retains the full logical extent of actual parts and rows, even when wider/taller. A conditional rectangular child clip uses existing traversal to restrict visibility and initial hit; it never rewrites content_size. The group has no scrolling, viewport-constraint request or scroll handlers.

No new body drag or mapping is admitted without available travel (Track body width must exceed the resting thumb). Existing Up/cancel cleanup and keyboard/editor paths remain. Restore/resize/alignment never changes values or emits callbacks. The existing widget rectangle may remain the router's owner target, but unused reserves cannot activate track/editor/cursor subparts.

## Cache and lifecycle

Only rows using this local capability receive optional per-node natural/assignment bookkeeping. Natural requirements and final assignment are distinct dependencies. A real natural probe forces final Arrange; equal outer Constraints alone do not permit reuse. Ordinary layout after leaving the group clears aligned geometry. Equal effective assignment and clean metrics reuse settled output.

Membership, metric, theme/font and structural changes use ordinary Layout invalidation. Removing the widest row recomputes current maxima; replacement invalidates the old owner's measurement. Temporary take/restore is not mount/removal. No borrowed widget, text/shaping/context references are stored in protocol values. Existing retained rendering clears its cache on a topology-generation change. The `slider_alignment_local_dependencies_preserve_retained_peers` regression in [slider_value_field_side.rs](../src/tests/slider_value_field_side.rs) checks independent fragment reuse across nonstructural shared-metric and membership updates. It checks fresh Layout and current reserves after removal or insertion separately; it does not assert independent cache retention across a topology-generation change. Test source identifies the intended checks, not a current executed result.

The Envelope and Calibration panels in the [Showcase inputs example](../examples/showcase/tab_inputs.rs) demonstrate separate opt-in groups, allocated through Stretch Flex hosting. The local group itself keeps natural/loose sizing; a parent supplies explicit width through normal tight/stretch constraints. Showcase owns no text measurement or fixed label widths. Envelope keeps its three sliders and adds independent captioned TextField/ComboBox rows in the same group; Calibration compares both explicit compositions in independent groups with numeric fields on the right. Framework left-side field support is unchanged. These examples illustrate composition; native visual quality requires review of the rendered result.

## Implementation references

- [ControlAlignmentGroup](../src/widgets/control_alignment_group.rs) owns the local row layout.
- [ControlCaption](../src/widgets/control_caption.rs) places one real editor child and its caption.
- [Control-parts values](../src/core/layout.rs) describe requirements and assigned columns.
- [LayoutContext](../src/core/widget/mod.rs) dispatches scoped measurement, arrangement and forwarding.
