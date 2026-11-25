use crate::core::appearance::ButtonMaterialPolicy;
use crate::core::paint::Color;
use crate::core::style::StyleSpec;

// ============================================================================
// Canonical Theme System
// Single source of truth for ALL spacing, radii, typography, colors, density.
// ============================================================================
//
// GEOMETRY-AUDIT-STATUS: CLOSED
// All framework geometry values are either:
//   - theme-authoritative (defined in Theme structs)
//   - runtime-synced from theme (cached in layout(), consumed elsewhere)
//   - explicitly invariant (documented with INV-* comments)
// Demo/test widgets are exempt (see widgets/list/demo.rs).
// Future geometry changes require SPEC process.
//
// ============================================================================

/// Density mode drives all geometry calculations.
///
/// # EnterpriseGrid Anchor
///
/// `Compact` is the **canonical EnterpriseGrid mode** — the visual calibration
/// anchor for Halcyon. Other modes are defined relative to it, not independently.
///
/// EnterpriseGrid is optimized for:
/// - Dense grids and data tables
/// - Trading terminals
/// - IDE / professional desktop software
///
/// It is explicitly **not** a touch mode and **not** web-comfort spacing.
///
/// # Mode Invariants
///
/// Each mode locks a closed set of parameters:
/// - control_height
/// - font_size
/// - line_height
/// - padding_x / padding_y
/// - radius
/// - baseline_offset
///
/// **Changing any parameter without switching modes is architecturally invalid.**
/// Widgets MUST NOT override individual geometry values — they consume the
/// resolved mode as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DensityMode {
    /// EnterpriseGrid anchor: 24px controls, 13px text, 2px radius.
    /// Maximum density for power users and data-heavy layouts.
    Compact,
    /// Balanced desktop: 28px controls, 13px text, 2px radius.
    /// General-purpose desktop use.
    #[default]
    Regular,
    /// Relaxed spacing: 32px controls.
    /// Accessibility-friendly, touch-compatible.
    Comfortable,
    /// JC-authorized 30px trial (SPEC-HAL-091A).
    Intermediate30,
}

// ============================================================================
// GeometrySpec: Per-widget geometry values driven by density
// ============================================================================

/// TextField geometry values
#[derive(Debug, Clone, Copy)]
pub struct TextFieldGeometry {
    pub height: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub radius: f32,
    pub icon_size: f32,
    pub min_hit_size: f32,
}

/// Button geometry values
#[derive(Debug, Clone, Copy)]
pub struct ButtonGeometry {
    pub height: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub radius: f32,
}

/// Checkbox geometry values
#[derive(Debug, Clone, Copy)]
pub struct CheckboxGeometry {
    pub size: f32,
    pub radius: f32,
    pub border_thickness: f32,
    pub focus_ring_thickness: f32,
    pub focus_ring_padding: f32,
    pub checkmark_inset: f32,
    pub checkmark_thickness: f32,
    pub checkmark_leg_ratio: f32,
    /// SPEC-HAL-208: Horizontal gap between indicator and label text.
    /// Used by Checkbox and RadioButton when labels are present.
    pub label_gap: f32,
}

/// Icon geometry values
#[derive(Debug, Clone, Copy)]
pub struct IconGeometry {
    pub size_small: f32,
    pub size_regular: f32,
    pub size_large: f32,
}

/// Palette (command palette, action palette) geometry values
#[derive(Debug, Clone, Copy)]
pub struct PaletteGeometry {
    pub width: f32,
    pub height: f32,
}

/// TextArea geometry values
#[derive(Debug, Clone, Copy)]
pub struct TextAreaGeometry {
    pub min_height: f32,
}

/// ComboBox geometry values
#[derive(Debug, Clone, Copy)]
pub struct ComboBoxGeometry {
    pub affordance_width: f32,
    pub radius: f32,
}

/// Switch / Toggle geometry values (SPEC-HAL-209)
#[derive(Debug, Clone, Copy)]
pub struct SwitchGeometry {
    pub track_width: f32,
    pub track_height: f32,
    pub thumb_diameter: f32,
    pub thumb_inset: f32,
    pub border_thickness: f32,
    /// SPEC-HAL-208: Horizontal gap between track and label text.
    pub label_gap: f32,
}

/// Slider geometry values (SPEC-HAL-210)
#[derive(Debug, Clone, Copy)]
pub struct SliderGeometry {
    pub track_length: f32,
    pub track_thickness: f32,
    pub thumb_diameter: f32,
    pub border_thickness: f32,
    /// Horizontal gap between slider and value label text.
    pub label_gap: f32,
}

/// ProgressBar geometry values (SPEC-HAL-211)
#[derive(Debug, Clone, Copy)]
pub struct ProgressBarGeometry {
    pub track_length: f32,
    pub track_thickness: f32,
    pub border_thickness: f32,
    pub radius: f32,
    /// Horizontal gap between track and label text.
    pub label_gap: f32,
}

/// BusyIndicator geometry values (SPEC-HAL-211)
#[derive(Debug, Clone, Copy)]
pub struct BusyIndicatorGeometry {
    pub track_length: f32,
    pub track_thickness: f32,
    pub border_thickness: f32,
    pub radius: f32,
    /// Fraction of track_length occupied by the sliding indicator (0.0..1.0).
    pub indicator_ratio: f32,
    /// Horizontal gap between track and label text.
    pub label_gap: f32,
}

/// SpinBox geometry values (SPEC-HAL-212)
#[derive(Debug, Clone, Copy)]
pub struct SpinBoxGeometry {
    pub height: f32,
    pub value_width: f32,
    pub button_width: f32,
    pub border_thickness: f32,
    pub radius: f32,
    /// Horizontal gap between control and label text.
    pub label_gap: f32,
}

/// Divider geometry values (SPEC-HAL-213)
#[derive(Debug, Clone, Copy)]
pub struct DividerGeometry {
    pub thickness: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct ColorPickerGeometry {
    /// Width of the SV (saturation/value) square.
    pub sv_size: f32,
    /// Height of the hue strip.
    pub hue_strip_height: f32,
    /// Gap between SV square and hue strip.
    pub strip_gap: f32,
    /// Width of the hex input field inside the picker.
    pub hex_field_width: f32,
    /// Height of the trigger swatch rect (in ColorPickerField).
    pub swatch_height: f32,
    /// Width of the trigger swatch rect.
    pub swatch_width: f32,
    /// Corner radius for the SV square and hue strip.
    pub radius: f32,
    /// Corner radius for the trigger swatch.
    pub swatch_radius: f32,
    /// Padding inside the popup around picker contents.
    pub popup_padding: f32,
}

/// SegmentedControl geometry values (SPEC-HAL-274)
#[derive(Debug, Clone, Copy)]
pub struct SegmentedControlGeometry {
    /// Height of the control.
    pub height: f32,
    /// Minimum segment width (content may expand it).
    pub min_segment_width: f32,
    /// Horizontal padding inside each segment.
    pub padding_x: f32,
    /// Corner radius for the outer bar.
    pub radius: f32,
    /// Width of separator lines between segments.
    pub separator_width: f32,
}

/// TimePicker geometry values (SPEC-HAL-273)
#[derive(Debug, Clone, Copy)]
pub struct TimePickerGeometry {
    /// Width of each time column (hour, minute).
    pub column_width: f32,
    /// Height of each column (visible row count * row height).
    pub column_height: f32,
    /// Width of AM/PM toggle column.
    pub ampm_width: f32,
    /// Width of colon separator.
    pub separator_width: f32,
    /// Corner radius for the picker background.
    pub radius: f32,
    /// Trigger field height.
    pub field_height: f32,
    /// Horizontal padding inside trigger field.
    pub field_padding_x: f32,
    /// Padding inside popup around the picker.
    pub popup_padding: f32,
}

/// SPEC-HAL-278: Bar-style track widget geometry.
#[derive(Debug, Clone, Copy)]
pub struct TrackBarGeometry {
    /// Height of the bar (vertical extent).
    pub bar_height: f32,
    /// Width of the edge handle hit zone (logical pixels).
    pub handle_hit_width: f32,
    /// Visual handle indicator width (the visible grab line).
    pub handle_visual_width: f32,
    /// Corner radius for the bar.
    pub radius: f32,
}

/// SPEC-HAL-276: TagInput widget geometry.
#[derive(Debug, Clone, Copy)]
pub struct TagInputGeometry {
    pub chip_height: f32,
    pub chip_padding_x: f32,
    pub chip_gap: f32,
    pub chip_radius: f32,
    pub chip_close_size: f32,
    pub checkbox_size: f32,
    pub checkbox_gap: f32,
    pub popup_max_visible_rows: usize,
    pub radius: f32,
}

/// Master geometry specification holding all widget geometries
#[derive(Debug, Clone, Copy)]
pub struct GeometrySpec {
    pub text_field: TextFieldGeometry,
    pub button: ButtonGeometry,
    pub checkbox: CheckboxGeometry,
    pub icon: IconGeometry,
    pub palette: PaletteGeometry,
    pub text_area: TextAreaGeometry,
    pub combo_box: ComboBoxGeometry,
    pub switch: SwitchGeometry,
    pub slider: SliderGeometry,
    pub progress_bar: ProgressBarGeometry,
    pub busy_indicator: BusyIndicatorGeometry,
    pub spin_box: SpinBoxGeometry,
    pub divider: DividerGeometry,
    pub color_picker: ColorPickerGeometry,
    pub segmented_control: SegmentedControlGeometry,
    pub time_picker: TimePickerGeometry,
    pub tag_input: TagInputGeometry,
    pub track_bar: TrackBarGeometry,
    // =========================================================================
    // INV-ROOT-INSET-CONTRACT: Root Content Inset Ownership Rules
    // =========================================================================
    //
    // `root_inset` is a CONTENT PADDING token, NOT a layout offset.
    //
    // OWNERSHIP:
    //   - SOLE CONSUMER: ScrollArea (consumes in layout to inset content)
    //   - FORBIDDEN: ScreenHost, widgets, demos, any container/layout widget
    //
    // SEMANTICS:
    //   - Content is positioned at (root_inset, root_inset) inside ScrollArea
    //   - Scrollbars use container rect (full allocation), unaffected by inset
    //   - Window edges, container allocation, layout constraints NEVER see inset
    //
    // NON-SCROLLAREA ROOT CONTENT:
    //   - If root widget is NOT a ScrollArea (e.g., Flex, Label), NO inset is applied
    //   - This is intentional: root_inset only affects scrollable content areas
    //   - Screens with non-scrollable root content render edge-to-edge
    //   - Do NOT add fallback padding or special-case handling
    //
    // ANTI-PATTERNS (FORBIDDEN):
    //   - ScreenHost applying inset to constraints or child positioning
    //   - Widgets reading root_inset to self-inset
    //   - Multiple consumers applying inset (double-inset bug)
    //   - Using root_inset as container margin
    //   - Adding fallback inset for non-ScrollArea roots
    //
    // GUARD: ScreenHost has a debug_assert preventing root_inset access.
    // Future attempts to consume root_inset outside ScrollArea must be reviewed.
    //
    // See: INV-ROOT-INSET-CONTENT, INV-SCREENHOST-NO-INSET
    // =========================================================================
    /// Root content inset — CONTENT PADDING consumed only by ScrollArea.
    /// Density-scoped: tighter in Compact, roomier in Comfortable.
    /// See INV-ROOT-INSET-CONTRACT above for ownership rules.
    pub root_inset: f32,
}

impl GeometrySpec {
    /// Factory method: create geometry spec for a given density mode and typography.
    ///
    /// # Control Height Contract (SPEC-HAL-092)
    ///
    /// Control heights are DERIVED from typography, not hardcoded:
    /// ```text
    /// control_height = ceil(line_height + 2 * padding_y)
    /// ```
    ///
    /// This guarantees the invariant:
    /// ```text
    /// control_height >= ascent + descent + 2 * padding_y
    /// ```
    ///
    /// Without this contract, vertical centering is mathematically impossible.
    pub fn for_density(d: DensityMode, typography: &TypographySpec) -> Self {
        // Control height = line_height + vertical padding
        // line_height already contains ascent + descent + line_gap
        let line_height = typography.line_height;

        match d {
            // =================================================================
            // ENTERPRISEGRID MODE (Compact)
            // =================================================================
            // Canonical dense desktop density. All values are LOCKED as a set.
            // Do NOT tune individual parameters — switch modes instead.
            //
            // Derived: control_height = ceil(line_height + 2×padding_y)
            //        = ceil(20 + 4) = 24px
            //
            // This mode is optimized for:
            //   - Dense grids and data tables
            //   - Trading terminals
            //   - IDE / professional desktop software
            // This is NOT a touch mode. This is NOT web-comfort spacing.
            // =================================================================
            DensityMode::Compact => {
                // SPEC-HAL-093 v3: Was 4.0. Produces 24px control height
                // (professional desktop density — VS Properties / classic
                // desktop).
                let padding_y = 2.0;
                let control_height = (line_height + 2.0 * padding_y).ceil();
                Self {
                    text_field: TextFieldGeometry {
                        height: control_height,
                        padding_x: 3.0, // Was 4.0
                        padding_y,
                        radius: 2.0, // Restrained desktop input corners
                        icon_size: 14.0,
                        min_hit_size: control_height,
                    },
                    button: ButtonGeometry {
                        height: control_height, // 28px — LOCKED (do not override)
                        padding_x: 6.0,         // SPEC-HAL-094: P-S
                        padding_y,
                        radius: 0.0, // SPEC-HAL-093 v3: R-0 for all controls
                    },
                    checkbox: CheckboxGeometry {
                        size: 16.0,
                        radius: 0.0,
                        border_thickness: 1.0,
                        focus_ring_thickness: 2.0,
                        focus_ring_padding: 2.0,
                        checkmark_inset: 3.5,
                        checkmark_thickness: 2.0,
                        checkmark_leg_ratio: 0.5,
                        label_gap: 6.0,
                    },
                    icon: IconGeometry {
                        size_small: 12.0,
                        size_regular: 14.0,
                        size_large: 18.0,
                    },
                    palette: PaletteGeometry {
                        width: 600.0,
                        height: 400.0,
                    },
                    text_area: TextAreaGeometry { min_height: 100.0 },
                    combo_box: ComboBoxGeometry {
                        affordance_width: 20.0,
                        radius: 2.0, // Same default corner as editable fields
                    },
                    switch: SwitchGeometry {
                        track_width: 32.0,    // Was 28.0
                        track_height: 18.0,   // Was 14.0 — matches checkbox visual mass
                        thumb_diameter: 14.0, // Was 10.0 — proportional to larger track
                        thumb_inset: 2.0,
                        border_thickness: 1.0,
                        label_gap: 6.0,
                    },
                    slider: SliderGeometry {
                        track_length: 120.0,
                        track_thickness: 4.0,
                        thumb_diameter: 12.0,
                        border_thickness: 1.0,
                        label_gap: 6.0,
                    },
                    progress_bar: ProgressBarGeometry {
                        track_length: 120.0,
                        track_thickness: 10.0,
                        border_thickness: 1.0,
                        radius: 2.0,
                        label_gap: 6.0,
                    },
                    busy_indicator: BusyIndicatorGeometry {
                        track_length: 120.0,
                        track_thickness: 4.0,
                        border_thickness: 1.0,
                        radius: 2.0,
                        indicator_ratio: 0.3,
                        label_gap: 6.0,
                    },
                    spin_box: SpinBoxGeometry {
                        height: control_height,
                        value_width: 40.0,
                        button_width: 20.0,
                        border_thickness: 1.0,
                        radius: 0.0, // SPEC-HAL-093 v3: R-0 for all controls
                        label_gap: 6.0,
                    },
                    divider: DividerGeometry { thickness: 1.0 },
                    color_picker: ColorPickerGeometry {
                        sv_size: 180.0,
                        hue_strip_height: 12.0,
                        strip_gap: 6.0,
                        hex_field_width: 80.0,
                        swatch_height: control_height,
                        swatch_width: 32.0,
                        radius: 2.0,
                        swatch_radius: 2.0,
                        popup_padding: 8.0,
                    },
                    segmented_control: SegmentedControlGeometry {
                        height: 24.0,
                        min_segment_width: 48.0,
                        padding_x: 8.0,
                        radius: 2.0,
                        separator_width: 1.0,
                    },
                    time_picker: TimePickerGeometry {
                        column_width: 24.0,
                        column_height: 24.0,
                        ampm_width: 24.0,
                        separator_width: 6.0,
                        radius: 2.0,
                        field_height: 24.0,
                        field_padding_x: 6.0,
                        popup_padding: 6.0,
                    },
                    tag_input: TagInputGeometry {
                        chip_height: 18.0,
                        chip_padding_x: 4.0,
                        chip_gap: 3.0,
                        chip_radius: 3.0,
                        chip_close_size: 10.0,
                        checkbox_size: 12.0,
                        checkbox_gap: 6.0,
                        popup_max_visible_rows: 8,
                        radius: 3.0,
                    },
                    track_bar: TrackBarGeometry {
                        bar_height: 16.0,
                        handle_hit_width: 8.0,
                        handle_visual_width: 3.0,
                        radius: 2.0,
                    },
                    root_inset: 8.0, // SPEC-HAL-090 edge padding via ScrollArea
                }
            }
            DensityMode::Regular => {
                let padding_y = 4.0;
                let control_height = (line_height + 2.0 * padding_y).ceil();
                Self {
                    text_field: TextFieldGeometry {
                        height: control_height,
                        padding_x: 6.0,
                        padding_y,
                        radius: 2.0, // SPEC-HAL-094: R-XS
                        icon_size: 16.0,
                        min_hit_size: control_height,
                    },
                    button: ButtonGeometry {
                        height: control_height,
                        padding_x: 6.0, // SPEC-HAL-094: P-S
                        padding_y,
                        radius: 0.0, // SPEC-HAL-093 v3: R-0 for all controls
                    },
                    checkbox: CheckboxGeometry {
                        size: 16.0,
                        radius: 0.0,
                        border_thickness: 1.0,
                        focus_ring_thickness: 2.0,
                        focus_ring_padding: 2.0,
                        checkmark_inset: 4.0,
                        checkmark_thickness: 2.0,
                        checkmark_leg_ratio: 0.5,
                        label_gap: 8.0,
                    },
                    icon: IconGeometry {
                        size_small: 14.0,
                        size_regular: 16.0,
                        size_large: 20.0,
                    },
                    palette: PaletteGeometry {
                        width: 600.0,
                        height: 400.0,
                    },
                    text_area: TextAreaGeometry { min_height: 100.0 },
                    combo_box: ComboBoxGeometry {
                        affordance_width: 24.0,
                        radius: 2.0,
                    },
                    switch: SwitchGeometry {
                        track_width: 38.0,    // Was 34.0
                        track_height: 18.0,   // Was 16.0 — matches checkbox visual mass
                        thumb_diameter: 14.0, // Was 12.0 — proportional to larger track
                        thumb_inset: 2.0,
                        border_thickness: 1.0,
                        label_gap: 8.0,
                    },
                    slider: SliderGeometry {
                        track_length: 150.0,
                        track_thickness: 4.0,
                        thumb_diameter: 14.0,
                        border_thickness: 1.0,
                        label_gap: 8.0,
                    },
                    progress_bar: ProgressBarGeometry {
                        track_length: 150.0,
                        track_thickness: 12.0,
                        border_thickness: 1.0,
                        radius: 2.0,
                        label_gap: 8.0,
                    },
                    busy_indicator: BusyIndicatorGeometry {
                        track_length: 150.0,
                        track_thickness: 4.0,
                        border_thickness: 1.0,
                        radius: 2.0,
                        indicator_ratio: 0.3,
                        label_gap: 8.0,
                    },
                    spin_box: SpinBoxGeometry {
                        height: control_height,
                        value_width: 48.0,
                        button_width: 24.0,
                        border_thickness: 1.0,
                        radius: 2.0,
                        label_gap: 8.0,
                    },
                    divider: DividerGeometry { thickness: 1.0 },
                    color_picker: ColorPickerGeometry {
                        sv_size: 200.0,
                        hue_strip_height: 14.0,
                        strip_gap: 8.0,
                        hex_field_width: 88.0,
                        swatch_height: 24.0,
                        swatch_width: 36.0,
                        radius: 2.0,
                        swatch_radius: 2.0,
                        popup_padding: 10.0,
                    },
                    segmented_control: SegmentedControlGeometry {
                        height: 28.0,
                        min_segment_width: 56.0,
                        padding_x: 10.0,
                        radius: 2.0,
                        separator_width: 1.0,
                    },
                    time_picker: TimePickerGeometry {
                        column_width: 28.0,
                        column_height: 28.0,
                        ampm_width: 28.0,
                        separator_width: 8.0,
                        radius: 2.0,
                        field_height: 28.0,
                        field_padding_x: 8.0,
                        popup_padding: 8.0,
                    },
                    tag_input: TagInputGeometry {
                        chip_height: 20.0,
                        chip_padding_x: 5.0,
                        chip_gap: 4.0,
                        chip_radius: 4.0,
                        chip_close_size: 12.0,
                        checkbox_size: 14.0,
                        checkbox_gap: 6.0,
                        popup_max_visible_rows: 8,
                        radius: 4.0,
                    },
                    track_bar: TrackBarGeometry {
                        bar_height: 20.0,
                        handle_hit_width: 10.0,
                        handle_visual_width: 3.0,
                        radius: 2.0,
                    },
                    root_inset: 10.0, // SPEC-HAL-090 edge padding via ScrollArea
                }
            }
            DensityMode::Comfortable => {
                let padding_y = 6.0;
                let control_height = (line_height + 2.0 * padding_y).ceil();
                Self {
                    text_field: TextFieldGeometry {
                        height: control_height,
                        padding_x: 8.0,
                        padding_y,
                        radius: 2.0, // Tightened: barely perceptible rounding
                        icon_size: 18.0,
                        min_hit_size: control_height,
                    },
                    button: ButtonGeometry {
                        height: control_height,
                        padding_x: 16.0,
                        padding_y,
                        radius: 0.0, // SPEC-HAL-093 v3: R-0 for all controls
                    },
                    checkbox: CheckboxGeometry {
                        size: 18.0,
                        radius: 2.0,
                        border_thickness: 1.0,
                        focus_ring_thickness: 2.0,
                        focus_ring_padding: 2.0,
                        checkmark_inset: 4.5,
                        checkmark_thickness: 3.0,
                        checkmark_leg_ratio: 0.5,
                        label_gap: 8.0,
                    },
                    icon: IconGeometry {
                        size_small: 16.0,
                        size_regular: 18.0,
                        size_large: 24.0,
                    },
                    palette: PaletteGeometry {
                        width: 600.0,
                        height: 400.0,
                    },
                    text_area: TextAreaGeometry { min_height: 100.0 },
                    combo_box: ComboBoxGeometry {
                        affordance_width: 28.0,
                        radius: 2.0,
                    },
                    switch: SwitchGeometry {
                        track_width: 44.0,    // Was 40.0
                        track_height: 22.0,   // Was 20.0 — matches checkbox visual mass
                        thumb_diameter: 18.0, // Was 16.0 — proportional to larger track
                        thumb_inset: 2.0,
                        border_thickness: 1.0,
                        label_gap: 8.0,
                    },
                    slider: SliderGeometry {
                        track_length: 180.0,
                        track_thickness: 6.0,
                        thumb_diameter: 18.0,
                        border_thickness: 1.0,
                        label_gap: 8.0,
                    },
                    progress_bar: ProgressBarGeometry {
                        track_length: 180.0,
                        track_thickness: 16.0,
                        border_thickness: 1.0,
                        radius: 3.0,
                        label_gap: 8.0,
                    },
                    busy_indicator: BusyIndicatorGeometry {
                        track_length: 180.0,
                        track_thickness: 6.0,
                        border_thickness: 1.0,
                        radius: 3.0,
                        indicator_ratio: 0.3,
                        label_gap: 8.0,
                    },
                    spin_box: SpinBoxGeometry {
                        height: control_height,
                        value_width: 56.0,
                        button_width: 28.0,
                        border_thickness: 1.0,
                        radius: 2.0,
                        label_gap: 8.0,
                    },
                    divider: DividerGeometry { thickness: 1.0 },
                    color_picker: ColorPickerGeometry {
                        sv_size: 220.0,
                        hue_strip_height: 16.0,
                        strip_gap: 10.0,
                        hex_field_width: 96.0,
                        swatch_height: 28.0,
                        swatch_width: 40.0,
                        radius: 3.0,
                        swatch_radius: 3.0,
                        popup_padding: 12.0,
                    },
                    segmented_control: SegmentedControlGeometry {
                        height: 32.0,
                        min_segment_width: 64.0,
                        padding_x: 12.0,
                        radius: 3.0,
                        separator_width: 1.0,
                    },
                    time_picker: TimePickerGeometry {
                        column_width: 32.0,
                        column_height: 32.0,
                        ampm_width: 32.0,
                        separator_width: 10.0,
                        radius: 3.0,
                        field_height: 32.0,
                        field_padding_x: 10.0,
                        popup_padding: 8.0,
                    },
                    tag_input: TagInputGeometry {
                        chip_height: 22.0,
                        chip_padding_x: 7.0,
                        chip_gap: 4.0,
                        chip_radius: 4.0,
                        chip_close_size: 14.0,
                        checkbox_size: 16.0,
                        checkbox_gap: 8.0,
                        popup_max_visible_rows: 10,
                        radius: 4.0,
                    },
                    track_bar: TrackBarGeometry {
                        bar_height: 24.0,
                        handle_hit_width: 12.0,
                        handle_visual_width: 4.0,
                        radius: 3.0,
                    },
                    root_inset: 12.0, // SPEC-HAL-090 edge padding via ScrollArea
                }
            }
            DensityMode::Intermediate30 => {
                let padding_y = 5.0;
                let control_height = (line_height + 2.0 * padding_y).ceil();
                Self {
                    text_field: TextFieldGeometry {
                        height: control_height,
                        padding_x: 7.0,
                        padding_y,
                        radius: 2.0,
                        icon_size: 17.0,
                        min_hit_size: control_height,
                    },
                    button: ButtonGeometry {
                        height: control_height,
                        padding_x: 11.0,
                        padding_y,
                        radius: 0.0,
                    },
                    checkbox: CheckboxGeometry {
                        size: 17.0,
                        radius: 1.0,
                        border_thickness: 1.0,
                        focus_ring_thickness: 2.0,
                        focus_ring_padding: 2.0,
                        checkmark_inset: 4.25,
                        checkmark_thickness: 2.5,
                        checkmark_leg_ratio: 0.5,
                        label_gap: 8.0,
                    },
                    icon: IconGeometry {
                        size_small: 15.0,
                        size_regular: 17.0,
                        size_large: 22.0,
                    },
                    palette: PaletteGeometry {
                        width: 600.0,
                        height: 400.0,
                    },
                    text_area: TextAreaGeometry { min_height: 100.0 },
                    combo_box: ComboBoxGeometry {
                        affordance_width: 26.0,
                        radius: 2.0,
                    },
                    switch: SwitchGeometry {
                        track_width: 41.0,
                        track_height: 20.0,
                        thumb_diameter: 16.0,
                        thumb_inset: 2.0,
                        border_thickness: 1.0,
                        label_gap: 8.0,
                    },
                    slider: SliderGeometry {
                        track_length: 165.0,
                        track_thickness: 5.0,
                        thumb_diameter: 16.0,
                        border_thickness: 1.0,
                        label_gap: 8.0,
                    },
                    progress_bar: ProgressBarGeometry {
                        track_length: 165.0,
                        track_thickness: 14.0,
                        border_thickness: 1.0,
                        radius: 2.5,
                        label_gap: 8.0,
                    },
                    busy_indicator: BusyIndicatorGeometry {
                        track_length: 165.0,
                        track_thickness: 5.0,
                        border_thickness: 1.0,
                        radius: 2.5,
                        indicator_ratio: 0.3,
                        label_gap: 8.0,
                    },
                    spin_box: SpinBoxGeometry {
                        height: control_height,
                        value_width: 52.0,
                        button_width: 26.0,
                        border_thickness: 1.0,
                        radius: 2.0,
                        label_gap: 8.0,
                    },
                    divider: DividerGeometry { thickness: 1.0 },
                    color_picker: ColorPickerGeometry {
                        sv_size: 210.0,
                        hue_strip_height: 15.0,
                        strip_gap: 9.0,
                        hex_field_width: 92.0,
                        swatch_height: 26.0,
                        swatch_width: 38.0,
                        radius: 2.5,
                        swatch_radius: 2.5,
                        popup_padding: 11.0,
                    },
                    segmented_control: SegmentedControlGeometry {
                        height: 30.0,
                        min_segment_width: 60.0,
                        padding_x: 11.0,
                        radius: 2.5,
                        separator_width: 1.0,
                    },
                    time_picker: TimePickerGeometry {
                        column_width: 30.0,
                        column_height: 30.0,
                        ampm_width: 30.0,
                        separator_width: 9.0,
                        radius: 2.5,
                        field_height: 30.0,
                        field_padding_x: 9.0,
                        popup_padding: 8.0,
                    },
                    tag_input: TagInputGeometry {
                        chip_height: 21.0,
                        chip_padding_x: 6.0,
                        chip_gap: 4.0,
                        chip_radius: 4.0,
                        chip_close_size: 13.0,
                        checkbox_size: 15.0,
                        checkbox_gap: 7.0,
                        popup_max_visible_rows: 9,
                        radius: 4.0,
                    },
                    track_bar: TrackBarGeometry {
                        bar_height: 22.0,
                        handle_hit_width: 11.0,
                        handle_visual_width: 3.5,
                        radius: 2.5,
                    },
                    root_inset: 11.0,
                }
            }
        }
    }
}

// ============================================================================
// TypographySpec: Font sizes, line heights, baseline offsets
// ============================================================================

/// Typography specification
#[derive(Debug, Clone, Copy)]
pub struct TypographySpec {
    pub font_size: f32,
    /// Editable values; captions and surrounding labels keep `font_size`.
    pub input_font_size: f32,
    pub line_height: f32,
    pub baseline_offset: f32,
    pub placeholder_opacity: f32,
    pub caret_thickness: f32,
    /// Margin around caret for scroll visibility (keeps caret from edge)
    pub caret_margin: f32,
    /// Selection rectangle Y offset from text baseline (visual alignment tweak)
    pub selection_y_offset: f32,
    /// Width of selection indicator for empty lines (visual minimum)
    pub empty_line_selection_width: f32,
    /// Selection highlight alpha (text field/area selection overlay)
    pub selection_alpha: f32,
}

impl Default for TypographySpec {
    fn default() -> Self {
        // =====================================================================
        // ENTERPRISEGRID TYPOGRAPHY BASELINE (SPEC-HAL-092 + SPEC-HAL-092A)
        // =====================================================================
        // This is the canonical typography configuration for dense desktop UI.
        // These values are LOCKED as a set — do not tune individually.
        //
        // Target: Trading terminals, IDE inspectors, data-heavy grids
        // Anti-target: Touch UI, web comfort spacing
        // =====================================================================
        Self {
            font_size: 13.0, // T-M token (SPEC-HAL-092 §4)
            input_font_size: 14.0,
            line_height: 20.0, // 5×4U grid (SPEC-HAL-092 §3)
            // SPEC-HAL-092A: Optical baseline correction.
            // Negative = move text up slightly to counteract visual sinking.
            // Applied at paint time via compute_control_text_baseline_y().
            baseline_offset: -1.0,
            placeholder_opacity: 0.45,
            caret_thickness: 1.5,
            caret_margin: 4.0,
            selection_y_offset: 1.5,
            empty_line_selection_width: 4.0,
            selection_alpha: 0.35,
        }
    }
}

// ============================================================================
// ColorSpec: SPEC-HAL-096 Hierarchical Semantic Color Tokens
// ============================================================================

/// Surface colors — background layers per SPEC-HAL-096 Section 2
#[derive(Debug, Clone, Copy)]
pub struct SurfaceColors {
    /// Window frame, title bar, toolbar backgrounds
    pub chrome: Color,
    /// Main canvas background (root)
    pub base0: Color,
    /// Text fields, lists, table cells, editable regions
    pub content: Color,
    /// Sidebars, panels, inspectors
    pub level1: Color,
    /// Hover states, selected rows, elevated surfaces
    pub level2: Color,
    /// Dropdowns, popovers, tooltips, modal overlays
    pub level3: Color,
    /// Elevated card / container surface — between base0 and content.
    /// Subtle lift: perceptible against canvas, not white.
    pub card: Color,
}

/// Text colors — typography per SPEC-HAL-096 Section 3.1
#[derive(Debug, Clone, Copy)]
pub struct TextColors {
    /// Body text, data, labels
    pub primary: Color,
    /// Hints, metadata, descriptions
    pub secondary: Color,
    /// Disabled text, placeholders
    pub muted: Color,
    /// Text on accent backgrounds
    pub inverse: Color,
}

/// Icon colors — per SPEC-HAL-096 Section 3.2
#[derive(Debug, Clone, Copy)]
pub struct IconColors {
    /// Default icon color
    pub primary: Color,
    /// De-emphasized icons
    pub secondary: Color,
    /// Disabled icons
    pub muted: Color,
    /// Active/selected state icons
    pub accent: Color,
    /// Folder icon base color (warm amber/orange)
    pub folder: Color,
    /// File icon base color (cool steel blue)
    pub file: Color,
}

/// Border colors — per SPEC-HAL-096 Section 3.3
#[derive(Debug, Clone, Copy)]
pub struct BorderColors {
    /// Panel dividers, inactive separators
    pub subtle: Color,
    /// Input fields, buttons, containers
    pub default: Color,
    /// High-contrast borders, active containers
    pub strong: Color,
    /// Keyboard focus indicator
    pub focus: Color,
    /// SPEC-HAL-093 v3: Bright side of sculpted edge (lighter than surface)
    pub highlight: Color,
    /// SPEC-HAL-093 v3: Dark side of sculpted edge (darker than surface)
    pub shadow: Color,
}

/// Accent colors — per SPEC-HAL-096 Section 3.4
#[derive(Debug, Clone, Copy)]
pub struct AccentColors {
    /// Primary actions, active selections
    pub primary: Color,
    /// Hover state on primary actions
    pub hover: Color,
    /// Subtle selection backgrounds
    pub muted: Color,
    /// Tinted surface for selected regions
    pub background: Color,
}

/// Semantic state colors — per SPEC-HAL-096 Section 3.5
#[derive(Debug, Clone, Copy)]
pub struct SemanticColors {
    /// Confirmations, valid states
    pub success: Color,
    /// Thresholds, caution
    pub warning: Color,
    /// Failed states, invalid input
    pub error: Color,
    /// Neutral information
    pub info: Color,
}

/// Scrollbar colors — per SPEC-HAL-096 Section 8
#[derive(Debug, Clone, Copy)]
pub struct ScrollbarColors {
    /// Scrollbar background
    pub track: Color,
    /// Draggable thumb
    pub thumb: Color,
    /// Thumb on hover
    pub thumb_hover: Color,
}

/// Control colors — dedicated tokens for interactive controls (buttons, etc.)
/// These provide proper contrast against surfaces without using accent colors.
#[derive(Debug, Clone, Copy)]
pub struct ControlColors {
    /// Default control background (clearly visible against base0/level1)
    pub background: Color,
    /// Hover state background (subtle lift)
    pub background_hover: Color,
    /// Pressed state background (subtle press)
    pub background_pressed: Color,
    /// Control border (visible but quiet)
    pub border: Color,
    /// Control border on hover
    pub border_hover: Color,
    /// Control text (high-contrast neutral)
    pub text: Color,
}

/// List widget colors — neutral grayscale for information-dense displays.
///
/// Lists are structural data views, not decorative UI. Selection is expressed
/// by luminance/value contrast only — no accent hues, no brand colors.
#[derive(Debug, Clone, Copy)]
pub struct ListColors {
    /// Header and footer background (distinct from content rows). Used for
    /// the top-level Section row archetype in PropertyEditor v2 per
    /// SPEC-HAL-260A §14.3 pass 2.
    pub header_background: Color,
    /// Sub-header background (subdued band, distinct from header). Used
    /// for the nested SubSection row archetype in PropertyEditor v2 per
    /// SPEC-HAL-260A §5.5.1 + §14.3 pass 2. Visually subordinate to
    /// `header_background` but still clearly a band — sits between
    /// Section prominence and row content.
    pub subheader_background: Color,
    /// Translucent overlay painted on top of `header_background` /
    /// `subheader_background` when the pointer hovers a band row.
    /// Monochrome modulation (lighten in dark themes, darken in light),
    /// distinct from the data-row `row_hover` which uses cell-level
    /// solid replacement. Shared between Section and SubSection
    /// archetypes (the difference is the base color underneath).
    pub band_hover_overlay: Color,
    /// Translucent overlay painted on top of `header_background` /
    /// `subheader_background` when the band row holds PE focus
    /// (`PeFocus::Band` targets this row AND PE Outer has focus). More
    /// pronounced than `band_hover_overlay`. Distinct from data-row
    /// `row_selected` (blue) — bands use monochrome modulation so their
    /// active state reads as "cursor on band" without conflating with
    /// row selection (bands are non-selectable per §10.2).
    pub band_focused_overlay: Color,
    /// Translucent overlay painted on top of `row_background`,
    /// `row_background_alt`, or `row_selected[_inactive]` when a Data
    /// row holds PE focus (`PeFocus::Cell` targets this row AND PE
    /// Outer has focus). Monochrome modulation matching the band
    /// pattern — gentler than `band_focused_overlay` because Data
    /// rows already carry the per-cell focus border at LAYER_FOCUS as
    /// the precise focus signal; this overlay is the supplementary
    /// row-axis affordance for keyboard-navigation orientation.
    pub row_focused_overlay: Color,
    /// Default row background (alternates with row_background_alt for zebra)
    pub row_background: Color,
    /// Alternate row background for zebra striping (odd rows)
    pub row_background_alt: Color,
    /// Hovered row background (weaker than selection)
    pub row_hover: Color,
    /// Selected row background (value contrast, no hue)
    pub row_selected: Color,
    /// Selected row when list is unfocused (subdued but visible)
    pub row_selected_inactive: Color,
    /// Default row text color
    pub text: Color,
    /// Text color on selected rows (may invert for contrast)
    pub text_selected: Color,
    /// Left-edge indicator stripe for active selection (neutral grayscale)
    pub selection_indicator: Color,
    /// Pinned column background tint (transparent overlay for visual distinction)
    pub pinned_tint: Color,
}

/// ComboBox input background colors — ambient state signaling.
///
/// Subtle background tints that communicate interaction state
/// without text messages, icons, or intrusive indicators.
#[derive(Debug, Clone, Copy)]
pub struct ComboBoxInputColors {
    /// Suggestions active or list item selected
    pub browsing: Color,
    /// Custom (non-matching) value is valid
    pub custom: Color,
    /// No matches exist and custom values not allowed
    pub no_match: Color,
    /// Async search pending — subtle neutral tint indicating "working"
    pub loading: Color,
}

#[derive(Debug, Clone, Copy)]
pub struct GaugeColors {
    /// Track background (unfilled bar / arc).
    pub track: Color,
    /// Default fill color when consumer doesn't specify.
    pub fill: Color,
    /// Needle / pointer indicator color.
    pub needle: Color,
    /// Tick mark color.
    pub tick: Color,
    /// Tick label text color.
    pub tick_label: Color,
}

/// Plot-specific semantic colors.
///
/// These tokens cover Plot-owned chrome and the default appearance of built-in
/// series. Consumer-provided Plot schemes, series styles, and per-point colors
/// remain authoritative.
#[derive(Debug, Clone, Copy)]
pub struct PlotColors {
    pub background: Color,
    pub grid: Color,
    pub crosshair: Color,
    pub synced_crosshair: Color,
    pub label_text: Color,
    pub label_background: Color,
    pub info_title: Color,
    pub info_data: Color,
    pub info_label: Color,
    pub crosshair_label_text: Color,
    pub crosshair_label_background: Color,
    pub selection_fill: Color,
    pub selection_border: Color,
    pub box_selection_border: Color,
    pub tool: Color,
    pub ghost_line: Color,
    pub tool_overlay: Color,
    pub tool_guide: Color,
    pub tool_horizontal_line: Color,
    pub tool_preview_fill: Color,
    pub tool_preview_border: Color,
    pub candle_up: Color,
    pub candle_down: Color,
    pub bar: Color,
    pub line: Color,
    pub marker_fill: Color,
    pub marker_border: Color,
    pub area_fill: Color,
    pub area_line: Color,
    pub histogram_positive: Color,
    pub histogram_negative: Color,
    pub depth_bid: Color,
    pub depth_ask: Color,
    pub volume_bearish: Color,
    pub volume_neutral_bearish: Color,
    pub volume_neutral_bullish: Color,
    pub volume_bullish: Color,
    pub volume_scrim: Color,
    pub annotation_neutral: Color,
    pub annotation_line: Color,
    pub annotation_fill: Color,
    pub annotation_text: Color,
    pub info_up: Color,
    pub info_down: Color,
}

/// SPEC-HAL-096 compliant hierarchical color specification.
///
/// Colors are organized by semantic role using plural-named groups:
/// - **surfaces**: Background layers (chrome, base0, content, level1-3)
/// - **typography**: Text colors (primary, secondary, muted, inverse)
/// - **icons**: Iconography (primary, secondary, muted, accent)
/// - **borders**: Outlines (subtle, default, strong, focus)
/// - **accents**: Interactive highlights (primary, hover, muted, background)
/// - **controls**: Interactive control surfaces (background, hover, pressed, border, text)
/// - **semantic**: States (success, warning, error, info)
/// - **scrollbars**: Scrollbar theming (track, thumb, thumb_hover)
#[derive(Debug, Clone, Copy)]
pub struct ColorSpec {
    pub surfaces: SurfaceColors,
    pub typography: TextColors,
    pub icons: IconColors,
    pub borders: BorderColors,
    pub accents: AccentColors,
    pub controls: ControlColors,
    pub list: ListColors,
    pub semantic: SemanticColors,
    pub scrollbars: ScrollbarColors,
    pub combo_box_input: ComboBoxInputColors,
    pub gauge: GaugeColors,
    pub plot: PlotColors,
}

impl ColorSpec {
    /// Dark theme color spec — Halcyon Dark (SPEC-HAL-096 Section 9)
    pub fn dark() -> Self {
        // Achromatic desktop surfaces; accent and semantic roles remain distinct.
        let surfaces = SurfaceColors {
            chrome: Color::from_hex(0x171717),  // Near-black window frame
            base0: Color::from_hex(0x202020),   // Main canvas — true dark
            content: Color::from_hex(0x292929), // Text fields, table cells
            level1: Color::from_hex(0x242424),  // Sidebars, panels
            level2: Color::from_hex(0x303030),  // Hover states, selected
            level3: Color::from_hex(0x343434),  // Dropdowns, popovers
            card: Color::from_hex(0x2D2D2D),    // Elevated surface above the canvas
        };
        let typography = TextColors {
            primary: Color::from_hex(0xDCDCDE),   // Body text — warm white
            secondary: Color::from_hex(0x8E8E94), // Hints, metadata
            muted: Color::from_hex(0x52525A),     // Disabled, placeholders
            inverse: Color::from_hex(0x111113),   // Text on accent backgrounds
        };
        let icons = IconColors {
            primary: Color::from_hex(0xDCDCDE),
            secondary: Color::from_hex(0x8E8E94),
            muted: Color::from_hex(0x52525A),
            accent: Color::from_hex(0x91A2A9),
            folder: Color::from_srgb(0.92, 0.65, 0.22, 1.0), // Deep warm amber
            file: Color::from_srgb(0.82, 0.81, 0.78, 1.0),   // Muted paper
        };
        let borders = BorderColors {
            subtle: Color::from_hex(0x414141),    // Quiet section dividers
            default: Color::from_hex(0x535353),   // Standard borders
            strong: Color::from_hex(0x717171),    // Emphasized edges
            focus: Color::from_hex(0x91A2A9),     // Muted slate focus ring
            highlight: Color::from_hex(0x5B5B5B), // SPEC-093 v3: bright side (dark theme)
            shadow: Color::from_hex(0x1C1C1C),    // Was 0x111316. Slightly lighter for subtlety
        };
        let accents = AccentColors {
            primary: Color::from_hex(0x91A2A9),    // Muted slate
            hover: Color::from_hex(0xA5B4BA),      // Lift on hover
            muted: Color::from_hex(0x91A2A9),      // Selection tint
            background: Color::from_hex(0x2B3337), // Subdued accent wash
        };
        let semantic = SemanticColors {
            success: Color::from_hex(0x3DAA5C), // Clear but not neon green
            warning: Color::from_hex(0xD9A530), // Warm amber — readable, not glaring
            error: Color::from_hex(0xD04848),   // Firm red — unmistakable, not screaming
            info: Color::from_hex(0x4888C8),    // Informational blue
        };
        let scrollbars = ScrollbarColors {
            track: Color::from_hex(0x1B1B1B), // Darker than base0
            thumb: Color::from_hex(0x555555), // Quiet resting thumb
            thumb_hover: Color::from_hex(0x6B6B6B),
        };
        let controls = ControlColors {
            background: Color::from_hex(0x383838),       // Raised, visible
            background_hover: Color::from_hex(0x424242), // Clear hover lift
            background_pressed: Color::from_hex(0x303030), // Sink on press
            border: Color::from_hex(0x666666),           // Defined edge
            border_hover: Color::from_hex(0x7A7A7A),
            text: Color::from_hex(0xDCDCDE), // Same as typography.primary
        };
        // List colors. SPEC-HAL-332 Principles 2-4 + SPEC-HAL-332A §4:
        // refined dark-theme palette addressing five defects observed
        // across framework demos (PE v2, List, TreeView, ListView):
        // (1) `row_selected` is accent-blended over `row_background`
        //     (~25% accent), eliminating the navy-slab read of the
        //     prior 0x1C3454. Cleaner blue chroma (B>>R, balanced G),
        //     not the Win2000 teal-navy.
        // (2) Band backgrounds carry a cool cast (B > G > R) to
        //     eliminate the warm-brown read of equal-channel grays.
        //     Pro dark themes (Resolve, Houdini, IDA) lean cool.
        // (3) Zebra at 9-level differential — perceptible without
        //     dominating; previous 6-level was borderline.
        // (4) Subheader band 10 levels above rows, 6 below header —
        //     SubSection reads as a distinct gentler band.
        // (5) Section band 16 levels above rows with cool cast —
        //     clearly distinct band, no longer reads warm.
        // Direction preserved: bands BRIGHTER than rows; header more
        // prominent than subheader; selection cleanly accent-tinted.
        let list = ListColors {
            header_background: Color::from_hex(0x282A30), // PE Section band (~42 lum, cool cast). 16 levels above rows.
            subheader_background: Color::from_hex(0x222428), // PE SubSection band (~36 lum, cool cast). 10 levels above rows, 6 below header.
            band_hover_overlay: Color::from_hex(0xFFFFFF).with_alpha(0.06), // Lighten on hover (dark theme — overlay over dark band base)
            band_focused_overlay: Color::from_hex(0xFFFFFF).with_alpha(0.12), // More pronounced lighten when band holds focus
            row_focused_overlay: Color::from_hex(0xFFFFFF).with_alpha(0.08), // Data-row focus overlay — gentler than band; supplements per-cell focus border
            row_background: Color::from_hex(0x19191C), // Data row (~26 lum). Same as surfaces.content.
            row_background_alt: Color::from_hex(0x111114), // Zebra stripe (~17 lum). 9 levels below row — clearly perceptible.
            row_hover: Color::from_hex(0x232328),
            row_selected: Color::from_hex(0x253547), // Selection (~52 lum). ~25% accent over row_background — cleaner blue, less teal-navy slab. Refined pro-tool selection.
            row_selected_inactive: Color::from_hex(0x182840), // Unfocused selection — subdued, established semantic preserved.
            text: Color::from_hex(0xDCDCDE),
            text_selected: Color::from_hex(0xFFFFFF),
            selection_indicator: Color::from_hex(0x4888C8),
            pinned_tint: Color::from_hex(0x404858).with_alpha(0.16),
        };

        let combo_box_input = ComboBoxInputColors {
            browsing: Color::from_hex(0x151A26),
            custom: Color::from_hex(0x151E16),
            no_match: Color::from_hex(0x221816),
            loading: Color::from_hex(0x1A1A22),
        };

        let gauge = GaugeColors {
            track: Color::from_hex(0x28282E),
            fill: Color::from_hex(0x4080B8),
            needle: Color::from_hex(0xE8E8EC),
            tick: Color::from_hex(0x58585E),
            tick_label: Color::from_hex(0x8A8A90),
        };
        let plot = PlotColors {
            background: Color::BLACK,
            grid: Color::from_srgb(0.25, 0.25, 0.25, 0.08),
            crosshair: Color::from_srgb(0.8, 0.8, 0.8, 0.7),
            synced_crosshair: Color::from_srgb(0.5, 0.5, 0.5, 0.5),
            label_text: Color::from_srgb(0.8, 0.8, 0.8, 0.9),
            label_background: Color::from_srgb(0.1, 0.1, 0.1, 0.7),
            info_title: Color::from_srgb(0.9, 0.9, 0.9, 0.9),
            info_data: Color::from_srgb(0.85, 0.85, 0.85, 0.85),
            info_label: Color::from_srgb(0.5, 0.5, 0.5, 0.5),
            crosshair_label_text: Color::from_srgb(1.0, 1.0, 1.0, 0.95),
            crosshair_label_background: Color::from_srgb(0.15, 0.15, 0.2, 0.9),
            selection_fill: Color::from_srgb(0.4, 0.8, 1.0, 0.12),
            selection_border: Color::from_srgb(0.4, 0.85, 1.0, 0.5),
            box_selection_border: Color::from_srgb(0.2, 0.5, 1.0, 0.5),
            tool: Color::from_srgb(1.0, 1.0, 1.0, 0.9),
            ghost_line: Color::from_srgb(0.5, 0.5, 0.5, 0.3),
            tool_overlay: Color::from_linear(1.0, 1.0, 1.0, 0.8),
            tool_guide: Color::from_linear(0.5, 0.5, 0.5, 0.4),
            tool_horizontal_line: Color::from_linear(0.8, 0.8, 0.8, 0.6),
            tool_preview_fill: Color::from_linear(0.3, 0.5, 1.0, 0.1),
            tool_preview_border: Color::from_linear(0.3, 0.5, 1.0, 0.6),
            candle_up: Color::from_linear(0.0, 1.0, 0.0, 1.0),
            candle_down: Color::from_linear(1.0, 0.0, 0.0, 1.0),
            bar: Color::from_srgb(0.3, 0.5, 1.0, 0.7),
            line: Color::WHITE,
            marker_fill: Color::WHITE,
            marker_border: Color::TRANSPARENT,
            area_fill: Color::from_srgb(0.3, 0.5, 1.0, 0.15),
            area_line: Color::from_srgb(0.3, 0.5, 1.0, 0.4),
            histogram_positive: Color::from_hex(0x2ECC71).with_alpha(0.7),
            histogram_negative: Color::from_hex(0xF05454).with_alpha(0.7),
            depth_bid: Color::from_hex(0x2ECC71).with_alpha(0.5),
            depth_ask: Color::from_hex(0xF05454).with_alpha(0.5),
            volume_bearish: Color::from_linear(0.7, 0.1, 0.0, 0.6),
            volume_neutral_bearish: Color::from_linear(0.4, 0.3, 0.03, 0.6),
            volume_neutral_bullish: Color::from_linear(0.4, 0.3, 0.12, 0.6),
            volume_bullish: Color::from_linear(0.0, 0.8, 0.0, 0.6),
            volume_scrim: Color::from_linear(0.0, 0.0, 0.0, 0.65),
            annotation_neutral: Color::from_srgb(0.5, 0.5, 0.5, 0.8),
            annotation_line: Color::from_srgb(1.0, 1.0, 1.0, 0.8),
            annotation_fill: Color::from_srgb(0.5, 0.5, 0.5, 0.1),
            annotation_text: Color::WHITE,
            info_up: Color::from_srgb(0.18, 0.80, 0.44, 0.95),
            info_down: Color::from_srgb(0.94, 0.33, 0.33, 0.95),
        };

        Self {
            surfaces,
            typography,
            icons,
            borders,
            accents,
            controls,
            list,
            semantic,
            scrollbars,
            combo_box_input,
            gauge,
            plot,
        }
    }

    /// Light theme color spec — Halcyon Light (SPEC-HAL-096 Section 10)
    pub fn light() -> Self {
        // Achromatic desktop surfaces keep hierarchy in luminance,
        // with a separate restrained accent and unchanged semantic roles.
        let surfaces = SurfaceColors {
            chrome: Color::from_hex(0xE7E7E7), // Title bar, toolstrip, status chrome
            base0: Color::from_hex(0xF7F7F7),  // Main canvas
            content: Color::from_hex(0xFDFDFD), // Text fields, table cells, data surfaces.
            level1: Color::from_hex(0xF2F2F2), // Sidebars, panels
            level2: Color::from_hex(0xE9E9E9), // Hover states, selected rows
            level3: Color::from_hex(0xFAFAFA), // Dropdowns, popovers, tooltips
            card: Color::from_hex(0xFFFFFF),   // Elevated card surface
        };
        let typography = TextColors {
            primary: Color::from_hex(0x1A1A1A),   // Body text, data, labels
            secondary: Color::from_hex(0x555555), // Hints, metadata, descriptions
            muted: Color::from_hex(0x999999),     // Disabled text, placeholders
            inverse: Color::from_hex(0xFFFFFF),   // Text on accent backgrounds
        };
        let icons = IconColors {
            primary: Color::from_hex(0x1A1A1A),
            secondary: Color::from_hex(0x555555),
            muted: Color::from_hex(0x999999),
            accent: Color::from_hex(0x647781),
            folder: Color::from_srgb(0.95, 0.68, 0.25, 1.0), // Bright warm orange
            file: Color::from_srgb(0.88, 0.87, 0.84, 1.0),   // Warm off-white/cream paper
        };
        let borders = BorderColors {
            subtle: Color::from_hex(0xD5D5D5),
            default: Color::from_hex(0xB9B9B9),
            strong: Color::from_hex(0x969696),
            focus: Color::from_hex(0x647781),
            highlight: Color::from_hex(0xFFFFFF), // SPEC-093 v3: bright side of sculpted edge
            shadow: Color::from_hex(0xC6C6C6),    // Subtle chassis shadow edge
        };
        let accents = AccentColors {
            primary: Color::from_hex(0x647781),
            hover: Color::from_hex(0x536670),
            muted: Color::from_hex(0x647781),
            background: Color::from_hex(0xE5EAEC),
        };
        let semantic = SemanticColors {
            success: Color::from_hex(0x228B22),
            warning: Color::from_hex(0xD97706),
            error: Color::from_hex(0xC00000),
            info: Color::from_hex(0x0066CC),
        };
        let scrollbars = ScrollbarColors {
            track: Color::from_hex(0xF0F0F0),
            // SPEC-HAL-093 v3: scrollbar thumb is subtle, not dominant.
            // Lighter resting color, previous shade promoted to hover.
            thumb: Color::from_hex(0xB9B9B9),
            thumb_hover: Color::from_hex(0x999999),
        };
        // Control surfaces: clearly visible against base0/level1, neutral (no accent)
        let controls = ControlColors {
            background: Color::from_hex(0xEAEAEA), // Raised control surface
            background_hover: Color::from_hex(0xF1F1F1), // Hover state
            background_pressed: Color::from_hex(0xDCDCDC), // Pressed: visible sink
            border: Color::from_hex(0xB0B0B0),     // Control edge
            border_hover: Color::from_hex(0x909090), // Hover edge emphasis
            text: Color::from_hex(0x1A1A1A),       // High-contrast dark
        };
        // List colors. SPEC-HAL-332 + SPEC-HAL-332A: band-to-row
        // contrast must satisfy charter Principle 4 (hierarchy readable
        // without squinting). Light-theme convention: bands DARKER than
        // rows (weighted/sunken pattern — Bloomberg / spreadsheet
        // tradition). Band-to-row differential: 35 luminance levels for
        // Section (header), 24 for SubSection (subheader). Previous
        // values gave only 19 levels for Section, below readability.
        let list = ListColors {
            header_background: Color::from_hex(0xDFDFDF), // PE Section band.
            subheader_background: Color::from_hex(0xEAEAEA), // PE SubSection band.
            band_hover_overlay: Color::from_hex(0x000000).with_alpha(0.06), // Darken on hover (light theme — overlay over light band base)
            band_focused_overlay: Color::from_hex(0x000000).with_alpha(0.12), // More pronounced darken when band holds focus
            row_focused_overlay: Color::from_hex(0x000000).with_alpha(0.08), // Data-row focus overlay — gentler than band; supplements per-cell focus border
            row_background: Color::from_hex(0xFFFFFF),                       // Data row.
            row_background_alt: Color::from_hex(0xF3F3F3),                   // Zebra stripe
            row_hover: Color::from_hex(0xE2EAF4),                            // Hover row
            row_selected: Color::from_hex(0xCCE4F7),                         // Light blue selection
            row_selected_inactive: Color::from_hex(0xDFEAF2), // Subdued blue when unfocused
            text: Color::from_hex(0x1A1A1A),                  // Same as typography.primary
            text_selected: Color::from_hex(0x1A1A1A),         // Dark text on light blue selection
            selection_indicator: Color::from_hex(0x0066CC),   // Blue indicator stripe
            pinned_tint: Color::from_hex(0x8090A8).with_alpha(0.26), // Cool steel-gray tint for pinned columns
        };

        let combo_box_input = ComboBoxInputColors {
            browsing: Color::from_hex(0xE2EAF4),
            custom: Color::from_hex(0xE8F3E8),
            no_match: Color::from_hex(0xF7E6D2),
            loading: Color::from_hex(0xEAEAEA),
        };

        let gauge = GaugeColors {
            track: Color::from_hex(0xCCCCCC),
            fill: Color::from_hex(0x0066CC),
            needle: Color::from_hex(0x1A1A1A),
            tick: Color::from_hex(0xB5B5B5),
            tick_label: Color::from_hex(0x555555),
        };
        let plot = PlotColors {
            background: Color::from_hex(0xFAFBFC),
            grid: Color::from_hex(0xAEB7C2).with_alpha(0.32),
            crosshair: Color::from_hex(0x3F4852).with_alpha(0.78),
            synced_crosshair: Color::from_hex(0x66717D).with_alpha(0.58),
            label_text: Color::from_hex(0x2B333B).with_alpha(0.96),
            label_background: Color::from_hex(0xEEF1F4).with_alpha(0.97),
            info_title: Color::from_hex(0x252C33).with_alpha(0.96),
            info_data: Color::from_hex(0x303840).with_alpha(0.92),
            info_label: Color::from_hex(0x56616C).with_alpha(0.82),
            crosshair_label_text: typography.inverse.with_alpha(0.98),
            crosshair_label_background: accents.primary.with_alpha(0.94),
            selection_fill: accents.primary.with_alpha(0.12),
            selection_border: accents.primary.with_alpha(0.55),
            box_selection_border: accents.primary.with_alpha(0.65),
            tool: accents.primary.with_alpha(0.9),
            ghost_line: Color::from_hex(0x56616C).with_alpha(0.38),
            tool_overlay: accents.primary.with_alpha(0.8),
            tool_guide: Color::from_hex(0x56616C).with_alpha(0.48),
            tool_horizontal_line: Color::from_hex(0x3F4852).with_alpha(0.68),
            tool_preview_fill: accents.primary.with_alpha(0.1),
            tool_preview_border: accents.primary.with_alpha(0.6),
            candle_up: semantic.success,
            candle_down: semantic.error,
            bar: accents.primary.with_alpha(0.58),
            line: accents.primary,
            marker_fill: accents.primary,
            marker_border: Color::TRANSPARENT,
            area_fill: accents.primary.with_alpha(0.14),
            area_line: accents.primary.with_alpha(0.5),
            histogram_positive: semantic.success.with_alpha(0.72),
            histogram_negative: semantic.error.with_alpha(0.72),
            depth_bid: semantic.success.with_alpha(0.5),
            depth_ask: semantic.error.with_alpha(0.5),
            volume_bearish: semantic.error.with_alpha(0.5),
            volume_neutral_bearish: typography.secondary.with_alpha(0.32),
            volume_neutral_bullish: typography.secondary.with_alpha(0.32),
            volume_bullish: semantic.success.with_alpha(0.5),
            volume_scrim: Color::TRANSPARENT,
            annotation_neutral: Color::from_hex(0x4F5B66).with_alpha(0.78),
            annotation_line: Color::from_hex(0x303840).with_alpha(0.82),
            annotation_fill: Color::from_hex(0x546271).with_alpha(0.06),
            annotation_text: Color::from_hex(0x20262D),
            info_up: semantic.success.with_alpha(0.95),
            info_down: semantic.error.with_alpha(0.95),
        };

        Self {
            surfaces,
            typography,
            icons,
            borders,
            accents,
            controls,
            list,
            semantic,
            scrollbars,
            combo_box_input,
            gauge,
            plot,
        }
    }
}

// ============================================================================
// SpacingSpec: SPEC-HAL-090 Spacing Tokens
// ============================================================================

/// SPEC-HAL-090 spacing tokens.
///
/// All values in logical pixels (1U = 1px @ 96 DPI).
/// Widgets MUST use these tokens instead of hardcoding pixel values.
///
/// SPEC-HAL-091 defines density scaling for XS through XL. XXS and 2XL have no
/// density rows in the spec, so they remain at their SPEC-HAL-090 base values.
#[derive(Debug, Clone, Copy)]
pub struct SpacingSpec {
    /// XXS = 2U — Micro gaps, gutter spacing, tight internal margins
    pub xxs: f32,
    /// XS = 4U — Minimal gaps, icon-to-text, tight groups
    pub xs: f32,
    /// S = 6U — Inner padding, label→input separation
    pub s: f32,
    /// M = 8U — Standard gaps, panel padding
    pub m: f32,
    /// L = 10U — Section separation
    pub l: f32,
    /// XL = 12U — Major section breaks
    pub xl: f32,
    /// 2XL = 16U — Extra-large section breaks
    pub x2l: f32,
}

impl SpacingSpec {
    pub fn for_density(d: DensityMode) -> Self {
        match d {
            DensityMode::Compact => Self {
                xxs: 2.0,
                xs: 4.0,
                s: 6.0,
                m: 7.0,
                l: 9.0,
                xl: 11.0,
                x2l: 16.0,
            },
            DensityMode::Regular => Self {
                xxs: 2.0,
                xs: 4.0,
                s: 6.0,
                m: 8.0,
                l: 10.0,
                xl: 12.0,
                x2l: 16.0,
            },
            DensityMode::Comfortable => Self {
                xxs: 2.0,
                xs: 5.0,
                s: 8.0,
                m: 10.0,
                l: 13.0,
                xl: 16.0,
                x2l: 16.0,
            },
            DensityMode::Intermediate30 => Self {
                xxs: 2.0,
                xs: 4.5,
                s: 7.0,
                m: 9.0,
                l: 11.5,
                xl: 14.0,
                x2l: 16.0,
            },
        }
    }

    // =========================================================================
    // SPEC-HAL-094 Spacing Aliases
    // =========================================================================
    // SPEC-HAL-094 uses S-XS/S-S/S-M/S-L naming. These are pure aliases to
    // SPEC-HAL-090 tokens with NO new numeric values.

    /// SPEC-HAL-094 S-XS = SPEC-HAL-090 XS (4U)
    #[inline]
    pub fn s_xs(&self) -> f32 {
        self.xs
    }

    /// SPEC-HAL-094 S-S = SPEC-HAL-090 S (6U)
    #[inline]
    pub fn s_s(&self) -> f32 {
        self.s
    }

    /// SPEC-HAL-094 S-M = SPEC-HAL-090 M (8U)
    #[inline]
    pub fn s_m(&self) -> f32 {
        self.m
    }

    /// SPEC-HAL-094 S-L = SPEC-HAL-090 XL (12U)
    /// Note: SPEC-HAL-094's S-L is 12U, which corresponds to SPEC-HAL-090's XL.
    #[inline]
    pub fn s_l(&self) -> f32 {
        self.xl
    }
}

impl Default for SpacingSpec {
    fn default() -> Self {
        Self::for_density(DensityMode::Regular)
    }
}

// ============================================================================
// PaddingSpec: SPEC-HAL-090 Padding Tokens
// ============================================================================

/// SPEC-HAL-090 padding tokens.
///
/// Padding is specialized spacing for internal element boundaries.
/// All values in logical pixels (1U = 1px @ 96 DPI).
#[derive(Debug, Clone, Copy)]
pub struct PaddingSpec {
    /// P-XS = 4U — Vertical padding in controls
    pub xs: f32,
    /// P-S = 6U — Horizontal padding in controls
    pub s: f32,
    /// P-M = 8U — Panel internal padding
    pub m: f32,
    /// P-L = 10U — Section padding, dialog padding
    pub l: f32,
}

impl Default for PaddingSpec {
    fn default() -> Self {
        // SPEC-HAL-090 Section 4: Padding tokens
        Self {
            xs: 4.0,
            s: 6.0,
            m: 8.0,
            l: 10.0,
        }
    }
}

// ============================================================================
// BorderSpec: SPEC-HAL-093 Border Thickness Tokens
// ============================================================================

/// SPEC-HAL-093 border thickness tokens.
///
/// Border thicknesses for controls and containers.
/// B-0/B-1/B-2 as defined in SPEC-HAL-093.
#[derive(Debug, Clone, Copy)]
pub struct BorderSpec {
    /// B-0 = 0px — No border
    pub b0: f32,
    /// B-1 = 1px — Standard control border
    pub b1: f32,
    /// B-2 = 2px — Heavy/emphasized border
    pub b2: f32,
}

impl BorderSpec {
    /// Deprecated alias for b1. Prefer `borders.b1` in new code.
    #[inline]
    pub fn standard(&self) -> f32 {
        self.b1
    }
}

impl Default for BorderSpec {
    fn default() -> Self {
        // SPEC-HAL-093 border thickness tokens
        Self {
            b0: 0.0,
            b1: 1.0,
            b2: 2.0,
        }
    }
}

// ============================================================================
// RadiusSpec: SPEC-HAL-093 Corner Radius Tokens
// ============================================================================

/// SPEC-HAL-093 corner radius tokens.
///
/// R-0 through R-L as defined in SPEC-HAL-093.
#[derive(Debug, Clone, Copy)]
pub struct RadiusSpec {
    /// R-0 = 0px — No rounding (sharp corners)
    pub r0: f32,
    /// R-XS = 2px — Inputs, toolbar buttons
    pub xs: f32,
    /// R-S = 3px — Buttons, dropdowns, tags
    pub s: f32,
    /// R-M = 4px — Containers, dialog bodies
    pub m: f32,
    /// R-L = 6px — Popovers, floating panels
    pub l: f32,
}

impl Default for RadiusSpec {
    fn default() -> Self {
        // SPEC-HAL-093 v3: sharp corners are the default
        // R-0 covers 90%+ of Halcyon (buttons, inputs, panels, tables)
        // Rounded corners reserved for floating/overlay elements only
        Self {
            r0: 0.0,
            xs: 2.0, // Tooltips, dropdown menus
            s: 3.0,  // Popovers, dialog bodies
            m: 4.0,  // Maximum: modal dialogs only
            l: 4.0,  // Capped at R-M (no large radii in Halcyon)
        }
    }
}

// ============================================================================
// ScrollbarGeometrySpec: SPEC-HAL-094 Scrollbar Geometry Tokens
// ============================================================================

/// Scrollbar geometry values.
///
/// Default values match existing literals exactly (visual output unchanged).
#[derive(Debug, Clone, Copy)]
pub struct ScrollbarGeometrySpec {
    /// Scrollbar track thickness (width for vertical, height for horizontal)
    pub thickness: f32,
    /// Minimum thumb size (never smaller than this)
    pub min_thumb_size: f32,
    /// Corner radius for rounded thumb ends
    pub corner_radius: f32,
    /// Hit area expansion for easier clicking (SPEC-HAL-096D)
    /// Extends hit-test region beyond visual bounds; does NOT affect layout or paint
    pub hit_expansion: f32,
}

impl Default for ScrollbarGeometrySpec {
    fn default() -> Self {
        Self {
            thickness: 10.0,      // Matches ScrollbarConfig::default().thickness
            min_thumb_size: 22.0, // Matches ScrollbarConfig::default().min_thumb_size
            corner_radius: 5.0,   // Matches ScrollbarConfig::default().corner_radius
            hit_expansion: 6.0,   // Matches ScrollbarConfig::default().hit_expansion
        }
    }
}

// ============================================================================
// ListGeometrySpec: SPEC-HAL-094 List Item Geometry Tokens
// ============================================================================

/// List item geometry values.
///
/// Density-aware geometry for native desktop list density.
/// Compact mode matches Win32/Qt/Cocoa native list spacing.
#[derive(Debug, Clone, Copy)]
pub struct ListGeometrySpec {
    /// Minimum row height for list items (fallback when no child content)
    pub row_height_min: f32,
    /// Canonical row height for fixed-height lists
    pub row_height: f32,
    /// Horizontal content padding (text inset from row edge)
    pub padding_x: f32,
}

impl ListGeometrySpec {
    /// Create list geometry for a given density mode.
    ///
    /// Values are tuned for native desktop density:
    /// - Compact: 24px rows, 6px padding (Win32/Qt-like)
    /// - Regular: 28px rows, 8px padding
    /// - Comfortable: 32px rows, 12px padding (touch-friendly)
    pub fn for_density(d: DensityMode) -> Self {
        match d {
            DensityMode::Compact => Self {
                row_height_min: 20.0,
                row_height: 24.0,
                padding_x: 6.0,
            },
            DensityMode::Regular => Self {
                row_height_min: 24.0,
                row_height: 28.0,
                padding_x: 8.0,
            },
            DensityMode::Comfortable => Self {
                row_height_min: 28.0,
                row_height: 32.0,
                padding_x: 12.0,
            },
            DensityMode::Intermediate30 => Self {
                row_height_min: 26.0,
                row_height: 30.0,
                padding_x: 10.0,
            },
        }
    }
}

impl Default for ListGeometrySpec {
    fn default() -> Self {
        // Lists ALWAYS default to compact density for information-dense display.
        // Lists are data views, not touch controls — density favors readability.
        // To use Regular/Comfortable, explicitly call for_density().
        Self::for_density(DensityMode::Compact)
    }
}

// ============================================================================
// InteractionSpec: Timing values for caret blink, clicks, hover, etc.
// ============================================================================

/// Interaction timing specification
#[derive(Debug, Clone, Copy)]
pub struct InteractionSpec {
    pub caret_blink_ms: u16,
    pub click_interval_ms: u16,
    pub hover_delay_ms: u16,
    pub press_slop_px: f32,
}

impl Default for InteractionSpec {
    fn default() -> Self {
        Self {
            caret_blink_ms: 530,
            click_interval_ms: 280,
            hover_delay_ms: 40,
            press_slop_px: 3.0,
        }
    }
}

// ============================================================================
// ShadowSpec + FocusRingSpec: Theme-owned shadow and focus ring constants
// ============================================================================

/// SPEC-R-013-v2: Shadow elevation parameters.
/// Each elevation level defines near (sharp) and far (soft) layer params.
#[derive(Debug, Clone, Copy)]
pub struct ElevationLevel {
    pub near_blur: f32,
    pub near_offset_y: f32,
    pub near_alpha: f32,
    pub far_blur: f32,
    pub far_offset_y: f32,
    pub far_alpha: f32,
}

/// SPEC-R-013-v2: Shadow configuration.
#[derive(Debug, Clone, Copy)]
pub struct ShadowSpec {
    /// Elevation levels 0-4 (0 = no shadow)
    pub elevation: [ElevationLevel; 5],
    /// Inner shadow for text input fields
    pub input_inner_blur: f32,
    pub input_inner_alpha: f32,
    /// Inner shadow for text areas
    pub textarea_inner_blur: f32,
    pub textarea_inner_alpha: f32,
    /// Popup shadow (menu, dropdown, context menu)
    pub popup_blur: f32,
    pub popup_offset_y: f32,
    pub popup_alpha: f32,
    /// Tooltip shadow
    pub tooltip_blur: f32,
    pub tooltip_offset_y: f32,
    pub tooltip_alpha: f32,
}

/// SPEC-HAL-226A: Tooltip timing and gap configuration.
#[derive(Debug, Clone, Copy)]
pub struct TooltipSpec {
    pub show_delay_ms: u32,
    pub hide_delay_ms: u32,
    pub reshow_window_ms: u32,
    pub gap: f32,
}

impl Default for TooltipSpec {
    fn default() -> Self {
        Self {
            show_delay_ms: 500,
            hide_delay_ms: 0,
            reshow_window_ms: 100,
            gap: 4.0,
        }
    }
}

impl Default for ShadowSpec {
    fn default() -> Self {
        Self {
            elevation: [
                ElevationLevel {
                    near_blur: 0.0,
                    near_offset_y: 0.0,
                    near_alpha: 0.0,
                    far_blur: 0.0,
                    far_offset_y: 0.0,
                    far_alpha: 0.0,
                },
                ElevationLevel {
                    near_blur: 0.5,
                    near_offset_y: 0.5,
                    near_alpha: 0.04,
                    far_blur: 1.5,
                    far_offset_y: 1.0,
                    far_alpha: 0.03,
                },
                ElevationLevel {
                    near_blur: 1.0,
                    near_offset_y: 0.5,
                    near_alpha: 0.06,
                    far_blur: 4.0,
                    far_offset_y: 1.5,
                    far_alpha: 0.04,
                },
                ElevationLevel {
                    near_blur: 1.5,
                    near_offset_y: 1.0,
                    near_alpha: 0.08,
                    far_blur: 8.0,
                    far_offset_y: 3.0,
                    far_alpha: 0.06,
                },
                ElevationLevel {
                    near_blur: 2.0,
                    near_offset_y: 1.5,
                    near_alpha: 0.10,
                    far_blur: 12.0,
                    far_offset_y: 4.0,
                    far_alpha: 0.08,
                },
            ],
            input_inner_blur: 1.5,
            input_inner_alpha: 0.04,
            textarea_inner_blur: 2.0,
            textarea_inner_alpha: 0.05,
            popup_blur: 4.0,
            popup_offset_y: 2.0,
            popup_alpha: 0.12,
            tooltip_blur: 2.0,
            tooltip_offset_y: 1.0,
            tooltip_alpha: 0.08,
        }
    }
}

/// SPEC-R-015-v2: Focus ring configuration.
#[derive(Debug, Clone, Copy)]
pub struct FocusRingSpec {
    pub spread: f32,
    pub softness: f32,
    pub alpha: f32,
    pub capture_color: Color,
    /// SPEC-HAL-328 §12.3: Indicator width when the widget is focused but
    /// not captured (Navigation mode). Logical pixels.
    pub focused_width: f32,
    /// SPEC-HAL-328 §12.3: Indicator width when the widget is focused and
    /// captured (Capture mode). Logical pixels.
    pub captured_width: f32,
}

impl Default for FocusRingSpec {
    fn default() -> Self {
        Self {
            spread: 1.0,
            softness: 0.8,
            alpha: 0.35,
            capture_color: Color::from_hex(0x4B9AFF),
            focused_width: 1.0,
            captured_width: 1.0,
        }
    }
}

impl FocusRingSpec {
    fn for_theme(is_dark: bool) -> Self {
        if is_dark {
            Self::default()
        } else {
            Self {
                capture_color: Color::from_hex(0x1D7DFC),
                ..Self::default()
            }
        }
    }
}

// ============================================================================
// ControlChrome: SPEC-HAL-332 shared control chrome contract (scaffolding)
// ============================================================================
//
// A single shared vocabulary for control fill / border / radius / focus,
// consumed by all interactive controls so chrome is defined once rather than
// re-derived per widget. This phase (Phase 1) adds the types and the selector
// methods only; no widget consumes them yet.

/// The chrome archetype a control belongs to. Selects fill family, border
/// idiom, radius cap, and the legal focus treatment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlChromeCategory {
    /// Buttons, spin-box stepper buttons, segmented selected segment, chips.
    RaisedClickable,
    /// Text fields, text areas, combo-box trigger, spin-box value field.
    RecessedEditable,
    /// Checkbox, radio, switch, segmented control.
    BinarySelector,
    /// Slider, range slider, progress/busy track, track-bar.
    TrackRange,
    /// GroupBox, Panel, Container, section bands.
    SectionContainer,
}

/// Interaction state, orthogonal to validation (see `ControlValidationState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlChromeState {
    Idle,
    Hovered,
    Pressed,
    Focused,
    Captured,
    Disabled,
    Selected,
}

/// Validation overlay applied on top of a `ControlChromeState`. Maps to
/// `SemanticColors`; `Neutral` leaves the state's normal border in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlValidationState {
    Neutral,
    Error,
    Warning,
    Success,
}

/// How focus is expressed. No glow, ever (SPEC-HAL-332 Principle 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlFocusStyle {
    /// No focus affordance painted by the contract.
    None,
    /// 1px hairline border in `borders.focus` following the control radius.
    HairlineRing,
    /// The control's resting border color-shifts to `borders.focus`.
    BorderShift,
    /// 1px `borders.focus` stroke around a round indicator/thumb.
    IndicatorStroke,
    /// Accent left-edge stripe (reserved for list rows).
    EdgeStripe,
}

/// The resolved style a widget paints from. Returned by
/// [`Theme::control_chrome`].
#[derive(Debug, Clone, Copy)]
pub struct ControlChromeSpec {
    pub fill: Option<Color>,
    pub border_color: Color,
    pub border_width: f32,
    pub radius: f32,
    pub content_color: Color,
    pub focus_style: ControlFocusStyle,
    pub focus_color: Color,
    pub focus_width: f32,
}

/// Solid TextField/ComboBox appearance under SPEC-HAL-012; no metric inputs.
#[derive(Clone, Copy)]
pub(crate) struct InputChrome {
    pub chrome: ControlChromeSpec,
    pub inner_edge: Color,
    pub affordance: Color,
    pub browsing: Color,
    pub selection: Color,
    pub caret: Color,
}

// ============================================================================
// Theme: The single authoritative theme struct
// ============================================================================

/// Canonical theme with density-driven geometry
///
/// This is the single source of truth for ALL spacing, radii, typography,
/// colors, and interaction timing. Widgets MUST NOT hardcode these values.
///
/// Immutable during layout/paint. Stored globally in App, exposed through
/// LayoutContext and PaintContext.
#[derive(Debug, Clone)]
pub struct Theme {
    pub density: DensityMode,
    pub button_material_policy: ButtonMaterialPolicy,
    pub geometry: GeometrySpec,
    pub typography: TypographySpec,
    pub colors: ColorSpec,
    pub interaction: InteractionSpec,
    /// SPEC-HAL-096A: Style primitives defaults
    pub styles: StyleSpec,
    /// SPEC-HAL-090: Spacing tokens (XS/S/M/L/XL/2XL)
    pub spacing: SpacingSpec,
    /// SPEC-HAL-090: Padding tokens (P-XS/P-S/P-M/P-L)
    pub padding: PaddingSpec,
    /// SPEC-HAL-093: Border thickness tokens (B-0/B-1/B-2)
    pub borders: BorderSpec,
    /// SPEC-HAL-093: Corner radius tokens (R-0/R-XS/R-S/R-M/R-L)
    pub radii: RadiusSpec,
    /// SPEC-HAL-094: Scrollbar geometry tokens
    pub scrollbar_geom: ScrollbarGeometrySpec,
    /// SPEC-HAL-094: List item geometry tokens
    pub list_geom: ListGeometrySpec,
    /// SPEC-R-013-v2: Shadow parameters
    pub shadows: ShadowSpec,
    /// SPEC-R-015-v2: Focus ring parameters
    pub focus_ring: FocusRingSpec,
    /// SPEC-HAL-226A: Tooltip timing and gap
    pub tooltip: TooltipSpec,
    /// Track whether this is a dark theme (for toggle logic)
    is_dark: bool,
}

impl Theme {
    /// The caller qualifies keyboard Capture for this exact paint owner/part.
    pub(crate) fn track_accent_colors(&self, keyboard_captured: bool) -> (Color, Color) {
        if keyboard_captured {
            let capture = self.focus_ring.capture_color;
            return (capture, capture.lighten(0.04));
        }
        let defaults = if self.is_dark {
            ColorSpec::dark()
        } else {
            ColorSpec::light()
        };
        let accents = self.colors.accents;
        // Explicit product accents remain controlling outside keyboard Capture.
        if accents.primary != defaults.accents.primary || accents.hover != defaults.accents.hover {
            return (accents.primary, accents.hover);
        }
        if self.is_dark {
            (Color::from_hex(0x929598), Color::from_hex(0xA1A5A8))
        } else {
            (Color::from_hex(0x727679), Color::from_hex(0x656A6F))
        }
    }

    pub(crate) fn text_selection_color(&self) -> Color {
        let default_muted = if self.is_dark() {
            ColorSpec::dark().accents.muted
        } else {
            ColorSpec::light().accents.muted
        };
        let mut selection = if self.colors.accents.muted != default_muted {
            self.colors.accents.muted
        } else {
            self.focus_ring.capture_color
        };
        selection.a = self.typography.selection_alpha;
        selection
    }

    pub(crate) fn input_chrome(&self, state: ControlChromeState) -> InputChrome {
        let (surface, edge, inner, affordance, focus) = if self.is_dark() {
            (0x272829, 0x141516, 0x464749, 0x343537, 0x9b9d9f)
        } else {
            (0xfafafa, 0x96989b, 0xe7e8e9, 0xe9eaeb, 0x55585b)
        };
        let surface = Color::from_hex(surface);
        let affordance = Color::from_hex(affordance);
        let mut chrome = self.control_chrome(
            ControlChromeCategory::RecessedEditable,
            state,
            ControlValidationState::Neutral,
        );
        chrome.fill = Some(surface);
        chrome.border_color = match state {
            ControlChromeState::Captured => self.focus_ring.capture_color,
            ControlChromeState::Focused => Color::from_hex(focus),
            ControlChromeState::Disabled => Color::from_hex(inner),
            _ => Color::from_hex(edge),
        };
        chrome.focus_color = chrome.border_color;
        InputChrome {
            chrome,
            inner_edge: Color::from_hex(inner),
            affordance,
            browsing: surface.lerp(affordance, 0.5),
            selection: self.text_selection_color(),
            caret: self.colors.typography.primary,
        }
    }

    pub(crate) fn time_picker_chrome(&self, state: ControlChromeState) -> InputChrome {
        let mut style = self.input_chrome(state);
        let defaults = if self.is_dark {
            ColorSpec::dark()
        } else {
            ColorSpec::light()
        };

        // Preserve the public theme inputs used by the existing TimePicker.
        if self.colors.surfaces.content != defaults.surfaces.content {
            style.chrome.fill = Some(self.colors.surfaces.content);
        }
        if self.colors.surfaces.level2 != defaults.surfaces.level2 {
            style.affordance = self.colors.surfaces.level2;
        }
        let border = match state {
            ControlChromeState::Focused | ControlChromeState::Captured => {
                (self.colors.borders.focus != defaults.borders.focus)
                    .then_some(self.colors.borders.focus)
            }
            _ => (self.colors.borders.default != defaults.borders.default)
                .then_some(self.colors.borders.default),
        };
        if let Some(border) = border {
            style.chrome.border_color = border;
        }
        if state == ControlChromeState::Captured
            && self.focus_ring.capture_color != FocusRingSpec::for_theme(self.is_dark).capture_color
        {
            style.chrome.focus_color = self.focus_ring.capture_color;
        }
        style
    }
    pub(crate) fn tag_input_chrome(
        &self,
        state: ControlChromeState,
        validation_error: bool,
    ) -> InputChrome {
        let mut style = self.input_chrome(state);
        let defaults = if self.is_dark {
            ColorSpec::dark()
        } else {
            ColorSpec::light()
        };
        if self.colors.surfaces.content != defaults.surfaces.content {
            style.chrome.fill = Some(self.colors.surfaces.content);
        }
        if self.colors.controls.background != defaults.controls.background {
            style.affordance = self.colors.controls.background;
        }
        let focused = matches!(
            state,
            ControlChromeState::Focused | ControlChromeState::Captured
        );
        let border = if focused {
            (self.colors.borders.focus != defaults.borders.focus)
                .then_some(self.colors.borders.focus)
        } else {
            (self.colors.borders.default != defaults.borders.default)
                .then_some(self.colors.borders.default)
        };
        if let Some(border) = border {
            style.chrome.border_color = border;
        }
        if state == ControlChromeState::Captured
            && self.focus_ring.capture_color != FocusRingSpec::for_theme(self.is_dark).capture_color
        {
            style.chrome.border_color = self.focus_ring.capture_color;
        }
        style.chrome.focus_color = style.chrome.border_color;
        style.chrome.border_width = self.control_border(focused, false, validation_error).1;
        if validation_error {
            if focused {
                style.inner_edge = style.chrome.border_color;
            }
            style.chrome.border_color = self.colors.semantic.error;
        }
        if self.colors.accents.primary != defaults.accents.primary {
            style.caret = self.colors.accents.primary;
        }
        style
    }

    pub(crate) fn text_area_chrome(&self, state: ControlChromeState) -> InputChrome {
        let mut style = self.input_chrome(state);
        let defaults = if self.is_dark {
            ColorSpec::dark()
        } else {
            ColorSpec::light()
        };
        if self.colors.surfaces.content != defaults.surfaces.content {
            style.chrome.fill = Some(self.colors.surfaces.content);
        }
        let border = match state {
            ControlChromeState::Captured => (self.focus_ring.capture_color
                != FocusRingSpec::for_theme(self.is_dark).capture_color)
                .then_some(self.focus_ring.capture_color),
            ControlChromeState::Focused => (self.colors.borders.focus != defaults.borders.focus)
                .then_some(self.colors.borders.focus),
            ControlChromeState::Disabled => (self.colors.borders.subtle != defaults.borders.subtle)
                .then_some(self.colors.borders.subtle),
            _ => (self.colors.borders.default != defaults.borders.default)
                .then_some(self.colors.borders.default),
        };
        if let Some(border) = border {
            style.chrome.border_color = border;
        }
        style.chrome.focus_color = style.chrome.border_color;
        if self.colors.accents.primary != defaults.accents.primary {
            style.caret = self.colors.accents.primary;
        }
        style
    }

    /// Default Button paint recipe; density and semantic owners stay unchanged.
    pub fn button_appearance_values(
        &self,
        contract: &crate::core::appearance::ButtonAppearanceContract,
    ) -> crate::core::appearance::ButtonAppearanceValues {
        use crate::core::appearance::{AppearanceStroke, AppearanceSurface};
        use crate::core::paint::{ColorStop, GradientDef, GradientKind, SurfaceGradient};

        let mut values = self.button_material_appearance_values(contract);
        let states = if self.is_dark {
            [
                (
                    [0x505050, 0x494949, 0x424242, 0x3C3C3C],
                    [0x626262, 0x555555, 0x3D3D3D, 0x292929],
                    0x202020,
                ),
                (
                    [0x595959, 0x525252, 0x4A4A4A, 0x434343],
                    [0x707070, 0x606060, 0x454545, 0x2E2E2E],
                    0x242424,
                ),
                (
                    [0x343434, 0x383838, 0x3E3E3E, 0x434343],
                    [0x262626, 0x323232, 0x4C4C4C, 0x626262],
                    0x202020,
                ),
                (
                    [0x3C3C3C, 0x393939, 0x363636, 0x343434],
                    [0x505050, 0x454545, 0x353535, 0x2D2D2D],
                    0x292929,
                ),
            ]
        } else {
            [
                (
                    [0xECECEC, 0xE5E5E5, 0xDDDDDD, 0xD6D6D6],
                    [0xFFFFFF, 0xF4F4F4, 0xCCCCCC, 0xBDBDBD],
                    0x868686,
                ),
                (
                    [0xF4F4F4, 0xEDEDED, 0xE5E5E5, 0xDEDEDE],
                    [0xFFFFFF, 0xFAFAFA, 0xD4D4D4, 0xC3C3C3],
                    0x7E7E7E,
                ),
                (
                    [0xC8C8C8, 0xCFCFCF, 0xD7D7D7, 0xDEDEDE],
                    [0xB0B0B0, 0xC1C1C1, 0xE2E2E2, 0xF3F3F3],
                    0x858585,
                ),
                (
                    [0xE6E6E6, 0xE3E3E3, 0xDFDFDF, 0xDCDCDC],
                    [0xEFEFEF, 0xE9E9E9, 0xD3D3D3, 0xC6C6C6],
                    0xB7B7B7,
                ),
            ]
        };
        let gradient = |colors: [u32; 4]| {
            AppearanceSurface::Gradient(
                SurfaceGradient::new(GradientDef {
                    kind: GradientKind::Linear,
                    stops: [0.0, 0.18, 0.74, 1.0]
                        .into_iter()
                        .zip(colors)
                        .map(|(offset, color)| ColorStop {
                            offset,
                            color: Color::from_hex(color),
                        })
                        .collect(),
                    p0: (0.5, 0.0),
                    p1: (0.5, 1.0),
                })
                .expect("default Button gradient is valid"),
            )
        };
        for (paint, (face, bevel, edge)) in [
            &mut values.resting,
            &mut values.hovered,
            &mut values.pressed,
            &mut values.disabled,
        ]
        .into_iter()
        .zip(states)
        {
            if let Some(paint) = paint {
                paint.underlay = Some(gradient(bevel));
                paint.surface = gradient(face);
                paint.radius = self.radii.s.min(3.0);
                paint.border = Some(AppearanceStroke {
                    color: Color::from_hex(edge),
                    width: self.borders.b1 * 0.65,
                });
            }
        }
        if let Some(paint) = &mut values.disabled {
            paint.content_color = Color::from_hex(if self.is_dark { 0x858587 } else { 0x949496 });
        }
        if let Some(focus) = &mut values.focus {
            focus.color = Color::from_hex(if self.is_dark { 0xA8AAAC } else { 0x6C6E70 });
        }
        values
    }
    // Explicit material requests retain their existing companion chrome.
    pub(crate) fn button_material_appearance_values(
        &self,
        contract: &crate::core::appearance::ButtonAppearanceContract,
    ) -> crate::core::appearance::ButtonAppearanceValues {
        use crate::core::appearance::{
            AppearanceStroke, AppearanceSurface, ButtonAppearanceMetrics, ButtonAppearanceValues,
            ButtonPaintValues,
        };
        use crate::core::layout::Padding;
        use crate::core::paint::{ColorStop, GradientDef, GradientKind, SurfaceGradient};

        let paint = |state| {
            let chrome = self.control_chrome(
                ControlChromeCategory::RaisedClickable,
                state,
                ControlValidationState::Neutral,
            );
            chrome.fill.map(|fill| ButtonPaintValues {
                surface: AppearanceSurface::Solid(fill),
                radius: chrome.radius,
                content_color: chrome.content_color,
                underlay: None,
                border: Some(AppearanceStroke {
                    color: chrome.border_color,
                    width: chrome.border_width,
                }),
                overlay: None,
            })
        };
        let resting = paint(ControlChromeState::Idle).map(|mut paint| {
            let colors = if self.is_dark {
                [0x454545, 0x404040, 0x393939, 0x323232]
            } else {
                [0xF1F1F1, 0xEAEAEA, 0xE4E4E4, 0xD5D5D5]
            };
            paint.surface = AppearanceSurface::Gradient(
                SurfaceGradient::new(GradientDef {
                    kind: GradientKind::Linear,
                    stops: [0.0, 0.18, 0.74, 1.0]
                        .into_iter()
                        .zip(colors)
                        .map(|(offset, color)| ColorStop {
                            offset,
                            color: Color::from_hex(color),
                        })
                        .collect(),
                    p0: (0.5, 0.0),
                    p1: (0.5, 1.0),
                })
                .expect("default Button satin gradient is valid"),
            );
            paint.border = None;
            paint
        });
        let focused = self.control_chrome(
            ControlChromeCategory::RaisedClickable,
            ControlChromeState::Focused,
            ControlValidationState::Neutral,
        );
        ButtonAppearanceValues {
            metrics: Some(ButtonAppearanceMetrics {
                padding: Padding::symmetric(
                    self.geometry.button.padding_x,
                    self.geometry.button.padding_y,
                ),
            }),
            resting,
            hovered: paint(ControlChromeState::Hovered),
            pressed: paint(ControlChromeState::Pressed),
            disabled: paint(ControlChromeState::Disabled),
            selected: contract
                .selection_owner()
                .and_then(|_| paint(ControlChromeState::Selected)),
            focus: Some(AppearanceStroke {
                color: focused.focus_color,
                width: focused.focus_width,
            }),
        }
    }

    /// Create a dark theme with the given density
    pub fn dark() -> Self {
        Self::dark_with_density(DensityMode::Regular)
    }

    /// Create a dark theme with specific density
    pub fn dark_with_density(density: DensityMode) -> Self {
        let typography = TypographySpec::default();
        Self {
            density,
            button_material_policy: ButtonMaterialPolicy::Standard,
            geometry: GeometrySpec::for_density(density, &typography),
            typography,
            colors: ColorSpec::dark(),
            interaction: InteractionSpec::default(),
            styles: StyleSpec::default(),
            spacing: SpacingSpec::for_density(density),
            padding: PaddingSpec::default(),
            borders: BorderSpec::default(),
            radii: RadiusSpec::default(),
            scrollbar_geom: ScrollbarGeometrySpec::default(),
            list_geom: ListGeometrySpec::for_density(density),
            shadows: ShadowSpec::default(),
            focus_ring: FocusRingSpec::for_theme(true),
            tooltip: TooltipSpec::default(),
            is_dark: true,
        }
    }

    /// Create a light theme with the given density
    pub fn light() -> Self {
        Self::light_with_density(DensityMode::Regular)
    }

    /// Create a light theme with specific density
    pub fn light_with_density(density: DensityMode) -> Self {
        let typography = TypographySpec::default();
        Self {
            density,
            button_material_policy: ButtonMaterialPolicy::Standard,
            geometry: GeometrySpec::for_density(density, &typography),
            typography,
            colors: ColorSpec::light(),
            interaction: InteractionSpec::default(),
            styles: StyleSpec::default(),
            spacing: SpacingSpec::for_density(density),
            padding: PaddingSpec::default(),
            borders: BorderSpec::default(),
            radii: RadiusSpec::default(),
            scrollbar_geom: ScrollbarGeometrySpec::default(),
            list_geom: ListGeometrySpec::for_density(density),
            shadows: ShadowSpec::default(),
            focus_ring: FocusRingSpec::for_theme(false),
            tooltip: TooltipSpec::default(),
            is_dark: false,
        }
    }

    /// Check if this is a dark theme
    pub fn is_dark(&self) -> bool {
        self.is_dark
    }

    /// Toggle between dark and light themes, preserving density and Button material policy.
    pub fn toggle(&mut self) {
        let density = self.density;
        let button_material_policy = self.button_material_policy;
        *self = if self.is_dark {
            Self::light_with_density(density)
        } else {
            Self::dark_with_density(density)
        };
        self.button_material_policy = button_material_policy;
    }

    /// Change density and recompute geometry
    pub fn set_density(&mut self, density: DensityMode) {
        self.density = density;
        self.geometry = GeometrySpec::for_density(density, &self.typography);
        self.spacing = SpacingSpec::for_density(density);
        self.list_geom = ListGeometrySpec::for_density(density);
    }

    /// Default bordered-input adapter under SPEC-HAL-328 §12.4/§12.8.
    /// Capture uses `capture_color`; Navigation uses `borders.focus`.
    /// Configured widths and error-border precedence are preserved. The
    /// consuming widget still owns its complete focus/capture projection.
    pub fn control_border(&self, focused: bool, captured: bool, error: bool) -> (Color, f32) {
        if error {
            (self.colors.semantic.error, 1.0)
        } else if captured {
            (
                self.focus_ring.capture_color,
                self.focus_ring.captured_width,
            )
        } else if focused {
            (self.colors.borders.focus, self.focus_ring.focused_width)
        } else {
            (self.colors.borders.default, 1.0)
        }
    }

    /// SPEC-HAL-332 Control Chrome (Phase 1 scaffolding — no callers yet).
    ///
    /// Clamp a requested corner radius to the charter cap for `category`.
    /// Round indicators/thumbs (`is_round == true`) are returned unchanged
    /// (a circle is a deliberate semantic, not a rounded rectangle). All
    /// functional categories — including `SectionContainer` — clamp to
    /// `requested.min(2.0)` per SPEC-HAL-332 Principle 6. Geometry tokens
    /// are not mutated here.
    pub fn chrome_radius(
        &self,
        category: ControlChromeCategory,
        requested: f32,
        is_round: bool,
    ) -> f32 {
        let _ = category;
        if is_round {
            requested
        } else {
            requested.min(2.0)
        }
    }

    /// SPEC-HAL-332 Control Chrome (Phase 1 scaffolding — no callers yet).
    ///
    /// Resolve the shared chrome style for a control archetype in a given
    /// interaction state, with a validation overlay. Uses existing tokens
    /// only; consumes no shadow/glow parameters (Principle 8). Focus is a
    /// hairline / stroke / border-shift — never a glow.
    pub fn control_chrome(
        &self,
        category: ControlChromeCategory,
        state: ControlChromeState,
        validation: ControlValidationState,
    ) -> ControlChromeSpec {
        let co = self.colors.controls;
        let su = self.colors.surfaces;
        let bo = self.colors.borders;
        let ac = self.colors.accents;
        let ty = self.colors.typography;
        let li = self.colors.list;

        // Scope the input capture color to editable chrome; BinarySelector
        // also uses Captured for an armed pointer press.
        let focus_color = match (category, state) {
            (ControlChromeCategory::RecessedEditable, ControlChromeState::Captured) => {
                self.focus_ring.capture_color
            }
            _ => bo.focus,
        };

        // Width follows focus state: captured/focused use the focus-ring
        // widths, every other state uses the standard 1px border token.
        let width = match state {
            ControlChromeState::Captured => self.focus_ring.captured_width,
            ControlChromeState::Focused => self.focus_ring.focused_width,
            _ => self.borders.b1,
        };

        let (fill, border_color, content_color, focus_style) = match category {
            ControlChromeCategory::RaisedClickable => {
                let fs = ControlFocusStyle::HairlineRing;
                match state {
                    ControlChromeState::Idle => (Some(co.background), co.border, co.text, fs),
                    ControlChromeState::Hovered => {
                        (Some(co.background_hover), co.border_hover, co.text, fs)
                    }
                    ControlChromeState::Pressed => {
                        (Some(co.background_pressed), co.border_hover, co.text, fs)
                    }
                    ControlChromeState::Focused | ControlChromeState::Captured => {
                        (Some(co.background), bo.focus, co.text, fs)
                    }
                    ControlChromeState::Disabled => (Some(co.background), bo.subtle, ty.muted, fs),
                    ControlChromeState::Selected => (Some(ac.primary), ac.primary, ty.inverse, fs),
                }
            }
            ControlChromeCategory::RecessedEditable => {
                let fs = ControlFocusStyle::BorderShift;
                match state {
                    ControlChromeState::Focused | ControlChromeState::Captured => {
                        (Some(su.content), focus_color, ty.primary, fs)
                    }
                    ControlChromeState::Disabled => (Some(su.content), bo.subtle, ty.muted, fs),
                    // Idle, Hovered, Pressed, Selected rest at the recessed
                    // editable surface (inputs do not hover-fill).
                    _ => (Some(su.content), bo.default, ty.primary, fs),
                }
            }
            ControlChromeCategory::BinarySelector => {
                let fs = ControlFocusStyle::HairlineRing;
                match state {
                    ControlChromeState::Idle => (Some(su.content), bo.default, ty.primary, fs),
                    ControlChromeState::Hovered => (Some(su.level2), bo.default, ty.primary, fs),
                    ControlChromeState::Pressed => (Some(su.level3), bo.default, ty.primary, fs),
                    ControlChromeState::Focused | ControlChromeState::Captured => {
                        (Some(su.content), bo.focus, ty.primary, fs)
                    }
                    ControlChromeState::Disabled => (Some(su.content), bo.subtle, ty.muted, fs),
                    ControlChromeState::Selected => (Some(ac.primary), ac.primary, ty.inverse, fs),
                }
            }
            ControlChromeCategory::TrackRange => {
                let fs = ControlFocusStyle::IndicatorStroke;
                match state {
                    ControlChromeState::Idle => (Some(su.content), bo.default, ty.primary, fs),
                    ControlChromeState::Hovered => (Some(ac.hover), bo.default, ty.primary, fs),
                    ControlChromeState::Pressed | ControlChromeState::Selected => {
                        (Some(ac.primary), bo.default, ty.primary, fs)
                    }
                    ControlChromeState::Focused | ControlChromeState::Captured => {
                        (Some(su.content), bo.focus, ty.primary, fs)
                    }
                    ControlChromeState::Disabled => (Some(su.content), bo.subtle, ty.muted, fs),
                }
            }
            ControlChromeCategory::SectionContainer => {
                let fs = ControlFocusStyle::None;
                match state {
                    ControlChromeState::Disabled => (Some(su.level1), bo.subtle, ty.muted, fs),
                    ControlChromeState::Selected => {
                        (Some(li.row_selected), bo.subtle, ty.primary, fs)
                    }
                    // Idle, Hovered, Pressed, Focused, Captured: static chrome.
                    _ => (Some(su.level1), bo.subtle, ty.primary, fs),
                }
            }
        };

        // Validation overlay (SPEC-HAL-332 §4 P3): overrides the border color
        // only; precedence Error > Warning > Success > Neutral.
        let border_color = match validation {
            ControlValidationState::Error => self.colors.semantic.error,
            ControlValidationState::Warning => self.colors.semantic.warning,
            ControlValidationState::Success => self.colors.semantic.success,
            ControlValidationState::Neutral => border_color,
        };

        ControlChromeSpec {
            fill,
            border_color,
            border_width: width,
            radius: self.radii.r0,
            content_color,
            focus_style,
            focus_color,
            focus_width: width,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        // The framework default theme is light.
        Self::light()
    }
}
