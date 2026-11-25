// SPEC-HAL-011 Tier-1 Widgets v2.0
// Canonical: FlexRow/FlexColumn, Container, Panel, Label, Button, Divider, ScrollArea
// Spacers: FixedSpacer, FlexSpacer
// Tier-2: FlowGrid
//
// =============================================================================
// GEOMETRY AUTHORITY ENFORCEMENT
// =============================================================================
// Framework widgets may NOT introduce new geometry literals.
// Any new geometry requires:
//   - A theme token (spacing, radii, borders, geometry.*), OR
//   - An explicit INV-* invariant comment justifying the literal
// Demo/test widgets (list/demo.rs) are exempt from this rule.
// See theme.rs GEOMETRY-AUDIT-STATUS for audit closure marker.
// =============================================================================

pub mod action_palette;
pub mod aligned_order_book;
pub mod auxiliary_text;
pub mod breadcrumbs;
pub mod busy_indicator;
pub mod button;
pub(crate) mod cell_editor_factory;
pub mod checkbox;
pub mod color_picker;
pub mod color_picker_field;
pub mod combo_box;
pub(crate) mod composite_edit_host;
pub mod console;
pub mod container;
pub mod control_alignment_group;
pub mod control_caption;
pub use control_alignment_group::ControlAlignmentGroup;
pub use control_caption::ControlCaption;
pub mod context_menu;
pub mod date_picker;
pub mod date_picker_field;
pub mod date_time_picker;
pub mod dialog;
pub mod disclosure_indicator;
pub mod disclosure_toggle;
pub mod divider;
pub mod dropdown_menu;
pub mod event_log;
pub mod exclusion_flow;
pub mod fixed_spacer;
pub mod flex;
pub mod flex_spacer;
pub mod flow_grid;
pub mod gauge;
pub mod groupbox;
pub mod keybind_editor;
pub mod label;
pub mod linear_gauge;
pub mod list;
pub mod list_view;
pub mod menu_bar;
pub mod menu_surface;
pub mod markdown;
pub mod navigator;
pub mod node_flow_surface;
pub mod notification_log;
pub mod order_book;
pub mod order_book_body;
pub mod panel;
pub mod popup_menu;
pub mod progress_bar;
pub mod property_editor;
pub mod radio_button;
pub mod radio_group;
pub mod radio_group_widget;
pub mod range_slider;
pub mod readout;
pub mod screen_host;
pub mod scroll_area;
pub mod scrollbar;
pub mod section_header;
pub mod section_surfaces;
pub mod segmented_control;
pub mod separator_group;
pub mod slider;
pub mod sparkline;
pub mod spin_box;
pub mod split_pane;
pub mod switch;
pub mod tab_bar;
pub mod tab_switcher;
pub mod tabular_view;
pub mod tag_input;
pub mod text_area;
pub mod text_block;
pub mod text_document;
pub mod text_edit;
pub mod text_field;
pub mod text_layout_cache;
pub mod text_widget_test_harness;
pub mod time_picker;
pub mod timeline_canvas;
pub mod timeline_header;
pub mod timeline_spatial;
pub mod toast;
pub mod tooltip_host;
pub mod tooltip_surface;
pub mod tree_view;
pub mod trigger_chrome;

// SPEC-HAL-020 Stage 6: Action Palette Widget
pub use action_palette::ActionPalette;
pub use aligned_order_book::{
    AlignedOrderBook, AlignedOrderBookRowLayout, AlignedOrderBookSelection,
};
pub use auxiliary_text::AuxiliaryText;

// Experimental layout exclusion flow container.
pub use exclusion_flow::{ExclusionFlow, ExclusionFlowDebug};

// SPEC-HAL-130/131: RadioGroup
pub use radio_group_widget::RadioGroupId;

// SPEC-HAL-201: ComboBox (Resolver-Driven, Zero Semantics)
pub use combo_box::{ComboBox, DisplaySelectionError, GroupedProjectionError, OptionGroupBoundary};

// Compact paint-only data readouts.
pub use readout::{
    KeyValueItem, KeyValueStrip, ReadoutCell, ReadoutColumn, ReadoutTable, ReadoutTextOverflow,
    StatusGrid, StatusGridCell, StatusGridMetric, StatusGridTone, StatusGridTonePresentation,
    StatusGridWideRow,
};
pub use section_surfaces::{
    DisclosureSectionHeader, SectionBandHeader, SectionBandLevel, SectionBandMarker,
    SectionMetaHeader,
};

// SPEC-HAL-187/188: ListView (TabularSubstrate consumer)
pub use list_view::ListView;

// SPEC-HAL-219: TabBar (Pure Navigation Selector)
pub use tab_bar::{TabBar, TabId};

// SPEC-HAL-333 Rev 4: MenuBar (in-window menu bar) + concrete InWindowMenuHost bridge.
// DismissPolicy stays module-scoped: core/overlay_root.rs owns a different
// type of the same name (SPEC-HAL-127).
pub use menu_bar::{
    BarTitle, InWindowMenuHost, MenuBar, MenuBarMailbox, MenuBarPolicy, OpenTrigger,
};

// SPEC-HAL-294: externally-owned node-flow editor Widget adapter.
pub use node_flow_surface::{NodeFlowSurface, NodeFlowSurfaceBinding};

// SPEC-HAL-321: TabSwitcher (content multiplexer for tab layouts)
pub use tab_switcher::TabSwitcher;

// SPEC-HAL-220: Breadcrumbs (Navigation Path Control)
pub use breadcrumbs::{Breadcrumbs, CrumbId};

// SPEC-HAL-226: TooltipSurface (Overlay-Only, Non-Action)
pub use tooltip_surface::TooltipSurface;
// SPEC-HAL-226A: Layout-transparent consumer tooltip binding.
pub use tooltip_host::TooltipHost;

// SPEC-HAL-238: DatePickerField (Trigger + Anchored Popup)
pub use date_picker_field::DatePickerField;
pub use date_time_picker::DateTimePicker;

// SPEC-HAL-329: KeybindEditor (Single-Stroke Capture, Category K)
pub use keybind_editor::{ConflictInfo, KeybindEditor};
