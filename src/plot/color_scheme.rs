use crate::core::paint::Color;
use crate::core::theme::Theme;

/// Resolved colors for Plot rendering.
///
/// Plot defaults are copied from the active [`Theme`]. Supplying a complete
/// scheme through `Plot::with_color_scheme` opts that Plot out of theme-derived
/// colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlotColorScheme {
    pub background: Color,
    pub grid: Color,
    pub crosshair: Color,
    pub synced_crosshair: Color,
    pub label_text: Color,
    pub label_background: Color,
    pub info_title: Color,
    pub info_data: Color,
    /// Dim color for OHLCV labels ("O:", "H:", "L:", "C:", "V:").
    pub info_label: Color,
    pub crosshair_label_text: Color,
    pub crosshair_label_background: Color,
    pub selection_fill: Color,
    pub selection_border: Color,
    pub box_selection_border: Color,
    pub tool_color: Color,
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

impl PlotColorScheme {
    pub fn from_theme(theme: &Theme) -> Self {
        let colors = theme.colors.plot;
        Self {
            background: colors.background,
            grid: colors.grid,
            crosshair: colors.crosshair,
            synced_crosshair: colors.synced_crosshair,
            label_text: colors.label_text,
            label_background: colors.label_background,
            info_title: colors.info_title,
            info_data: colors.info_data,
            info_label: colors.info_label,
            crosshair_label_text: colors.crosshair_label_text,
            crosshair_label_background: colors.crosshair_label_background,
            selection_fill: colors.selection_fill,
            selection_border: colors.selection_border,
            box_selection_border: colors.box_selection_border,
            tool_color: colors.tool,
            ghost_line: colors.ghost_line,
            tool_overlay: colors.tool_overlay,
            tool_guide: colors.tool_guide,
            tool_horizontal_line: colors.tool_horizontal_line,
            tool_preview_fill: colors.tool_preview_fill,
            tool_preview_border: colors.tool_preview_border,
            candle_up: colors.candle_up,
            candle_down: colors.candle_down,
            bar: colors.bar,
            line: colors.line,
            marker_fill: colors.marker_fill,
            marker_border: colors.marker_border,
            area_fill: colors.area_fill,
            area_line: colors.area_line,
            histogram_positive: colors.histogram_positive,
            histogram_negative: colors.histogram_negative,
            depth_bid: colors.depth_bid,
            depth_ask: colors.depth_ask,
            volume_bearish: colors.volume_bearish,
            volume_neutral_bearish: colors.volume_neutral_bearish,
            volume_neutral_bullish: colors.volume_neutral_bullish,
            volume_bullish: colors.volume_bullish,
            volume_scrim: colors.volume_scrim,
            annotation_neutral: colors.annotation_neutral,
            annotation_line: colors.annotation_line,
            annotation_fill: colors.annotation_fill,
            annotation_text: colors.annotation_text,
            info_up: colors.info_up,
            info_down: colors.info_down,
        }
    }

    pub fn dark() -> Self {
        Self::from_theme(&Theme::dark())
    }

    pub fn light() -> Self {
        Self::from_theme(&Theme::light())
    }
}

impl Default for PlotColorScheme {
    fn default() -> Self {
        Self::dark()
    }
}
