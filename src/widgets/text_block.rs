// SPEC-HAL-048: Multi-line Text Layout - TextBlock Widget
// Migrated from legacy LineBreaker to ParagraphLayoutEngine

use crate::core::event::{Event, EventPhase, KeyCode, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::Color;
use crate::core::paragraph::{
    LayoutParams, LineWithSource, Paragraph, ParagraphLayoutEngine, Run, TextAlign, TextStyle,
};
use crate::core::text_engine::{GlyphRun, TextBinding, TextEngine, TextInputError};
use crate::core::text_selection::{
    ReadOnlyTextSelection, TextSelectionPolicy, hit_test_run, selection_spans,
};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::platform::font_manager::FontFaceCatalog;
use crate::utils::id::WidgetInternalId;

/// Multi-line text widget with wrapping and alignment.
///
/// Uses SPEC-048 ParagraphLayoutEngine for line breaking and layout,
/// while rendering through TextEngine for renderer compatibility.
pub struct TextBlock {
    id: WidgetInternalId,
    text: String,
    font_size: f32,
    /// Text color. None = use theme.colors.typography.primary (default).
    color: Option<Color>,
    align: TextAlign,
    wrap: bool,
    lines: Vec<LineWithSource>,
    cached_width: f32,
    cached_size: Size,
    cached_catalog: Option<FontFaceCatalog>,
    cached_primary: Option<TextBinding>,
    cached_metrics: [u32; 4],
    cached_layout_scale: f32,
    /// SPEC-HAL-315: Optional read-only text selection.
    text_selection: Option<ReadOnlyTextSelection>,
    /// Cached layout size for clip_bounds. Set during layout.
    layout_size: Option<Size>,
}

impl TextBlock {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            text: text.into(),
            font_size: 18.0,
            color: None, // Default: use theme.colors.typography.primary
            align: TextAlign::Left,
            wrap: true,
            lines: Vec::new(),
            cached_width: -1.0,
            cached_size: Size::zero(),
            cached_catalog: None,
            cached_primary: None,
            cached_metrics: [0; 4],
            cached_layout_scale: 1.0,
            text_selection: None,
            layout_size: None,
        }
    }

    pub fn with_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self.invalidate_cache();
        self
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn with_align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    pub fn with_wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self.invalidate_cache();
        self
    }

    pub(crate) fn set_font_size(&mut self, size: f32) {
        if (self.font_size - size).abs() > f32::EPSILON {
            self.font_size = size;
            self.invalidate_cache();
        }
    }

    pub(crate) fn set_color(&mut self, color: Option<Color>) {
        self.color = color;
    }

    /// Set the text content (runtime mutation)
    ///
    /// SPEC-HAL-158 INV-DIRTY-1: Automatic invalidation on state mutation.
    /// Caller must provide the NodeId so tree invalidation is enforced atomically.
    /// This makes it impossible to mutate layout-affecting state without propagation.
    pub fn set_text(
        &mut self,
        text: impl Into<String>,
        cx: &mut LayoutContext,
        node_id: crate::core::widget_tree::NodeId,
    ) {
        self.text = text.into();
        self.invalidate_cache();
        // SPEC-HAL-158 INV-DIRTY-1: Automatic tree invalidation (enforced, not caller-responsible)
        cx.tree.mark_layout_dirty(node_id);
        // INV-315-4: A replacement retires the old source selection.
        if let Some(ref mut sel) = self.text_selection {
            sel.set_text(self.text.clone());
        }
    }

    /// Get the text content
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Internal cache invalidation (private)
    ///
    /// SPEC-HAL-158 INV-DIRTY-1: This is internal-only. Public mutation paths
    /// (set_text) handle tree invalidation atomically. Builder-time methods
    /// (with_size, with_wrap, etc.) call this directly since widget is not yet in tree.
    fn invalidate_cache(&mut self) {
        self.cached_width = -1.0;
        self.lines.clear();
        self.cached_size = Size::zero();
        self.layout_size = None;
        self.cached_catalog = None;
        self.cached_primary = None;
    }

    /// SPEC-HAL-315: Enable text selection with the given policy.
    pub fn with_selection_policy(mut self, policy: TextSelectionPolicy) -> Self {
        if policy != TextSelectionPolicy::Disabled {
            self.text_selection = Some(ReadOnlyTextSelection::new(policy, self.text.clone()));
        }
        self
    }

    pub fn set_selection_policy(&mut self, policy: TextSelectionPolicy) {
        if policy == TextSelectionPolicy::Disabled {
            self.text_selection = None;
        } else {
            match &mut self.text_selection {
                Some(sel) => sel.set_policy(policy),
                None => {
                    self.text_selection =
                        Some(ReadOnlyTextSelection::new(policy, self.text.clone()));
                }
            }
        }
    }

    fn is_selectable(&self) -> bool {
        self.text_selection.is_some()
    }

    #[cfg(all(test, not(target_arch = "wasm32"), not(target_os = "ios")))]
    pub(crate) fn test_selection_offsets(&self) -> Option<(usize, usize)> {
        let selection = self.text_selection.as_ref()?.selection();
        Some((selection.anchor, selection.head))
    }

    fn shape_laid_out_run(
        &self,
        run: &Run,
        engine: &mut dyn TextEngine,
    ) -> Result<GlyphRun, TextInputError> {
        ParagraphLayoutEngine::shape_uniform_run(&self.text, run, self.cached_catalog.as_ref(), engine)
    }

    /// Hit-test: map (local_x, local_y) to a byte offset in self.text.
    ///
    /// Walks the same cached logical line heights as layout and paint
    /// to ensure hit positions match rendered positions.
    fn hit_test_position(
        &self,
        local_x: f32,
        local_y: f32,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
    ) -> usize {
        let mut cumulative_y = 0.0_f32;

        for (index, layout) in self.lines.iter().enumerate() {
            let line = &layout.line;
            cumulative_y += line.height;

            if local_y < cumulative_y || index + 1 == self.lines.len() {
                // This is the target line (or last line if Y is past all lines).
                let mut x = self.align.offset(
                    line.width,
                    self.layout_size.map_or(self.cached_size.width, |s| s.width),
                );
                for (i, face_run) in line.runs.iter().enumerate() {
                    if local_x < x + face_run.width || i + 1 == line.runs.len() {
                        let run = self
                            .shape_laid_out_run(face_run, text_engine)
                            .expect("UI-TYPOGRAPHY-INPUT: TextBlock hit input failed");
                        return face_run.range.start + hit_test_run(&run, local_x - x);
                    }
                    x += face_run.width;
                }
                return layout.source_start;
            }
        }
        self.text.len()
    }
}

impl WidgetDowncast for TextBlock {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TextBlock {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let max_width = constraints.max_width;

        let catalog = cx.text_engine.font_face_catalog();
        let primary = if catalog.is_some() {
            Some(
                cx.text_engine
                    .resolve_text_style(&TextStyle::default().with_size(self.font_size))
                    .expect("UI-TYPOGRAPHY-INPUT: TextBlock primary face unavailable")
                    .binding,
            )
        } else {
            None
        };
        let canonical_metrics = cx.text_engine.metrics("", self.font_size);
        let metrics_key = [
            canonical_metrics.ascent.to_bits(),
            canonical_metrics.descent.to_bits(),
            canonical_metrics.line_gap.to_bits(),
            canonical_metrics.line_height.to_bits(),
        ];
        let same_catalog = match (&catalog, &self.cached_catalog) {
            (Some(a), Some(b)) => a.same_generation(b),
            (None, None) => true,
            _ => false,
        };
        // Paragraph coordinates stay logical (scale 1), as before. Current
        // native-scale metrics and the actual catalog/default binding also
        // participate in eligibility; Paint/Compile own physical resampling.
        let layout_scale = 1.0;

        // Detect cache invalidation: width changed OR cache empty
        if self.cached_width != max_width
            || self.lines.is_empty()
            || !same_catalog
            || self.cached_primary != primary
            || self.cached_metrics != metrics_key
            || self.cached_layout_scale != layout_scale
        {
            // Guard: zero or negative width produces degenerate layout.
            // Non-visible widgets may receive max_width=0.0 from parent.
            if max_width <= 0.0 {
                self.lines.clear();
                self.cached_width = max_width;
                self.cached_size = Size::zero();
                self.layout_size = Some(Size::zero());
                return LayoutResult::new(Size::zero(), Size::zero());
            }
            // Determine effective width for line breaking
            let effective_width = if self.wrap {
                if max_width.is_finite() {
                    max_width
                } else {
                    // "wrap with infinite max" is nonsense: treat as no-wrap
                    f32::INFINITY
                }
            } else {
                f32::INFINITY
            };

            // L2.INV1: Get canonical metrics from text engine for ALL lines

            // Build layout parameters
            let params = LayoutParams {
                max_width: effective_width,
                scale: layout_scale,
                canonical_metrics,
            };

            // Build paragraph with uniform styling
            let color = self.color.unwrap_or(cx.theme.colors.typography.primary);
            let paragraph = Paragraph::from_plain_text(self.text.clone(), self.font_size, color);

            // Use ParagraphLayoutEngine with the TextEngine's shaper
            self.lines = if let Some(catalog) = &catalog {
                ParagraphLayoutEngine::try_layout_catalog(
                    &paragraph,
                    catalog,
                    Some(primary.as_ref().unwrap().face().font_id()),
                    &params,
                )
                .expect("UI-TYPOGRAPHY-INPUT: TextBlock registered layout failed")
            } else if let Some(shaper) = cx.text_engine.shaper() {
                ParagraphLayoutEngine::try_layout_with_line_sources(&paragraph, shaper, &params)
                    .expect("UI-TYPOGRAPHY-INPUT: paragraph layout failed")
            } else {
                // TODO: TextEngine without shaper - fallback to empty line
                // This should not happen with FontManagerTextEngine
                vec![LineWithSource::empty(canonical_metrics, 0)]
            };

            // Compute natural extent from paragraph line metrics. A finite
            // parent width is a fill-width allocation; an unbounded measurement
            // must report shaped text width, never the infinity sentinel.
            let natural_width = self.lines.iter().map(|entry| entry.line.width).fold(0.0, f32::max);
            let resolved_width = if max_width.is_finite() {
                max_width
            } else {
                natural_width
            };
            let total_height: f32 = self.lines.iter().map(|entry| entry.line.height).sum();

            self.cached_width = max_width;
            self.cached_size = Size::new(resolved_width, total_height);
            self.cached_catalog = catalog;
            self.cached_primary = primary;
            self.cached_metrics = metrics_key;
            self.cached_layout_scale = layout_scale;
        }

        // Cache constrained size for clip_bounds.
        let constrained_h = self.cached_size.height.min(constraints.max_height);
        self.layout_size = Some(Size::new(
            self.cached_size.width.min(constraints.max_width),
            constrained_h,
        ));

        // SPEC-HAL-086 Stage 2: Return LayoutResult (no overflow)
        LayoutResult::fit(self.cached_size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let container_width = rect.width;
        let color = self.color.unwrap_or(cx.theme.colors.typography.primary);
        let mut cumulative_y = 0.0_f32;

        // Selection range (if any).
        let sel_range = self.text_selection.as_ref().and_then(|sel| {
            if sel.has_selection() {
                Some(sel.range())
            } else {
                None
            }
        });
        let highlight_color = cx.theme.text_selection_color();

        let available_h = rect.height;

        for entry in &self.lines {
            let line = &entry.line;
            // Truncate: if this line's bottom exceeds available height, stop.
            if cumulative_y + line.height > available_h + 0.5 {
                break;
            }

            let x_offset = self.align.offset(line.width, container_width);
            let line_x = rect.x + x_offset;
            let line_y = rect.y + cumulative_y;

            let mut run_x = line_x;
            for face_run in &line.runs {
                let run = self
                    .shape_laid_out_run(face_run, cx.text_engine)
                    .expect("UI-TYPOGRAPHY-INPUT: TextBlock paint binding failed");
                // Line boxes stay canonical; actual face ascent aligns to their baseline.
                let run_y = line_y + line.baseline - run.ascent;
                let backgrounds = sel_range
                    .filter(|(start, end)| {
                        *end > face_run.range.start && *start < face_run.range.end
                    })
                    .map(|(start, end)| {
                        selection_spans(
                            &run,
                            start.max(face_run.range.start) - face_run.range.start,
                            end.min(face_run.range.end) - face_run.range.start,
                            line.height,
                            run.ascent - line.baseline,
                            highlight_color,
                        )
                    })
                    .unwrap_or_default();
                if backgrounds.is_empty() {
                    cx.text_engine.draw(
                        &run,
                        run_x,
                        run_y,
                        color,
                        crate::core::paint::LAYER_CONTENT,
                        cx.scene,
                    );
                } else {
                    cx.text_engine.draw_with_backgrounds(
                        &run,
                        run_x,
                        run_y,
                        color,
                        backgrounds,
                        crate::core::paint::LAYER_CONTENT,
                        cx.scene,
                    );
                }
                run_x += face_run.width;
            }

            // Preserve layout's fractional line-height sum; Compile owns pixel snapping.
            cumulative_y += line.height;
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if self.text_selection.is_none() {
            return false;
        }

        match event {
            // ── PointerDown: start selection ──
            Event::Pointer(pe)
                if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                    && pe.kind == PointerEventKind::Down
                    && pe.button == Some(PointerButton::Primary) =>
            {
                let click_count = cx.click_count();
                let in_capture = cx.has_pointer_capture();
                let shift = pe.modifiers.shift;
                let offset =
                    self.hit_test_position(pe.local_pos.0, pe.local_pos.1, cx.text_engine());
                let sel = self.text_selection.as_mut().unwrap();
                if sel.pointer_down(offset, click_count, shift, in_capture, shift) {
                    cx.capture_pointer(PointerButton::Primary);
                    return true;
                }
                false
            }

            // ── DoubleClick: word selection ──
            Event::Pointer(pe)
                if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                    && pe.kind == PointerEventKind::DoubleClick
                    && pe.button == Some(PointerButton::Primary) =>
            {
                let in_capture = cx.has_pointer_capture();
                let offset =
                    self.hit_test_position(pe.local_pos.0, pe.local_pos.1, cx.text_engine());
                let sel = self.text_selection.as_mut().unwrap();
                if sel.pointer_down(offset, 2, false, in_capture, false) {
                    return true;
                }
                false
            }

            // ── TripleClick: select all ──
            Event::Pointer(pe)
                if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                    && pe.kind == PointerEventKind::TripleClick
                    && pe.button == Some(PointerButton::Primary) =>
            {
                let in_capture = cx.has_pointer_capture();
                let offset =
                    self.hit_test_position(pe.local_pos.0, pe.local_pos.1, cx.text_engine());
                let sel = self.text_selection.as_mut().unwrap();
                if sel.pointer_down(offset, 3, false, in_capture, false) {
                    return true;
                }
                false
            }

            // ── PointerMove: extend drag selection ──
            Event::Pointer(pe)
                if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                    && pe.kind == PointerEventKind::Move =>
            {
                let is_dragging = self
                    .text_selection
                    .as_ref()
                    .map_or(false, |s| s.is_dragging());
                if !is_dragging {
                    return false;
                }
                let offset =
                    self.hit_test_position(pe.local_pos.0, pe.local_pos.1, cx.text_engine());
                let sel = self.text_selection.as_mut().unwrap();
                sel.pointer_move(offset);
                true
            }

            // ── PointerUp: end drag ──
            Event::Pointer(pe)
                if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                    && pe.kind == PointerEventKind::Up
                    && pe.button == Some(PointerButton::Primary) =>
            {
                let is_dragging = self
                    .text_selection
                    .as_ref()
                    .map_or(false, |s| s.is_dragging());
                if is_dragging {
                    let sel = self.text_selection.as_mut().unwrap();
                    sel.pointer_up();
                    cx.release_pointer();
                    return true;
                }
                false
            }

            // ── Escape: clear selection ──
            Event::Key(ke)
                if phase == EventPhase::Target && ke.is_down && ke.key_code == KeyCode::Escape =>
            {
                let sel = self.text_selection.as_mut().unwrap();
                if sel.has_selection() {
                    sel.clear();
                    return true;
                }
                false
            }

            // ── Text: Ctrl/Cmd+C copy, Ctrl/Cmd+A select all ──
            Event::Text(te) if phase == EventPhase::Target => {
                #[cfg(target_os = "macos")]
                let shortcut_mod = te.modifiers.meta;
                #[cfg(not(target_os = "macos"))]
                let shortcut_mod = te.modifiers.ctrl;

                if !shortcut_mod {
                    return false;
                }

                let sel = self.text_selection.as_mut().unwrap();
                match te.text.as_str() {
                    "c" | "C" => {
                        if let Some(text) = sel.copy_to_clipboard() {
                            cx.clipboard_set_text(&text);
                            return true;
                        }
                        false
                    }
                    "a" | "A" => {
                        sel.select_all();
                        true
                    }
                    _ => false,
                }
            }

            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        self.is_selectable()
    }

    fn focusable(&self) -> bool {
        self.is_selectable()
    }

    fn capture_eligible(&self) -> bool {
        self.is_selectable()
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.clear_read_only_text_selection();
    }

    fn on_capture_leave(&mut self, _cx: &mut EventContext) {
        if let Some(ref mut sel) = self.text_selection {
            if sel.policy() == TextSelectionPolicy::OnCapture {
                sel.clear();
            }
        }
    }

    fn clear_read_only_text_selection(&mut self) -> bool {
        let Some(sel) = self.text_selection.as_mut() else {
            return false;
        };
        if !sel.has_selection() && !sel.is_dragging() {
            return false;
        }
        sel.clear();
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "TextBlock"
    }
}

#[cfg(test)]
mod line_sequence_tests {
    use super::*;
    use crate::core::app::RetainedSceneTestHarness;
    use crate::core::icon::IconRegistry;
    use crate::core::scene::{SceneNode, TextNode};
    use crate::core::theme::Theme;
    use crate::core::widget_tree::WidgetTree;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;

    const TEXT: &str =
        "The left half of the image is visible. Its right half is covered by an orange rectangle.";

    #[cfg(target_os = "macos")]
    #[test]
    fn registered_textblock_clusters_wrap_hit_copy_and_catalog_identity() {
        use crate::platform::font_manager::FontManager;
        let mut engine = FontManagerTextEngine::new();
        let primary = engine.shape("Latin", 18.0).binding.unwrap();
        let mut fonts = FontManager::new();
        let primary_id = fonts
            .load_font_memory(
                primary.face().data().clone(),
                Some(primary.face().family_name().to_owned()),
            )
            .unwrap();
        let fallback = fonts
            .load_font_file("/System/Library/Fonts/Supplemental/Arial Unicode.ttf")
            .unwrap();
        let catalog = fonts.face_catalog();
        assert!(catalog.same_generation(&catalog.clone()));
        assert!(!catalog.same_generation(&fonts.face_catalog()));
        let face = fonts.get_face(fallback).unwrap();
        for ch in ['\u{770b}', '龍', '한'] {
            let actual = ttf_parser::Face::parse(&face.data, 0)
                .unwrap()
                .glyph_index(ch)
                .is_some_and(|id| id.0 != 0);
            assert_eq!(face.coverage_map.covers(ch), actual);
        }
        let mut engine =
            FontManagerTextEngine::with_face_catalog(catalog.clone(), primary_id).unwrap();
        let mut crossed_fallback_boundary = false;
        for text in [
            "Latin Привет 看 龍 한글\nSecond ffi e\u{301}",
            "אבג 看龍 דהו",
            "a\n\n看\n",
        ] {
            let mut block = TextBlock::new(text)
                .with_size(18.0)
                .with_selection_policy(TextSelectionPolicy::Enabled);
            let paragraph = Paragraph::new(text, TextStyle::default().with_size(18.0));
            block.lines = ParagraphLayoutEngine::try_layout_catalog(
                &paragraph,
                &catalog,
                Some(primary_id),
                &LayoutParams {
                    max_width: 110.0,
                    ..Default::default()
                },
            )
            .unwrap();
            block.cached_catalog = Some(catalog.clone());
            block.cached_size = Size::new(110.0, 500.0);
            let mut ranges = Vec::new();
            let mut line_y = 0.0f32;
            for entry in &block.lines {
                let line = &entry.line;
                let mut x = 0.0;
                for (index, run) in line.runs.iter().enumerate() {
                    if index > 0 && text.starts_with("Latin") {
                        let previous = &line.runs[index - 1];
                        if previous.font_id != run.font_id {
                            assert_eq!(previous.range.end, run.range.start);
                            for side in [-0.01, 0.01] {
                                assert_eq!(
                                    block.hit_test_position(
                                        x + side,
                                        line_y + line.height * 0.5,
                                        &mut engine
                                    ),
                                    run.range.start
                                );
                            }
                            crossed_fallback_boundary = true;
                        }
                    }
                    ranges.push(run.range.clone());
                    let shaped = block.shape_laid_out_run(run, &mut engine).unwrap();
                    assert_eq!(shaped.text, text[run.range.clone()]);
                    assert_eq!(
                        shaped.binding.as_ref().unwrap().face().font_id(),
                        run.font_id
                    );
                    assert!((shaped.width - run.width).abs() < 0.001);
                    assert!(shaped.glyphs.iter().all(|glyph| glyph.id != 0));
                    if text.starts_with("אבג") {
                        assert!(
                            shaped.clusters.iter().all(|cluster| cluster.direction
                                == crate::core::text_shaper::TextDirection::RightToLeft),
                            "fallback must preserve the primary styled span direction"
                        );
                    }
                    for cluster in &shaped.clusters {
                        let offset = run.range.start
                            + hit_test_run(&shaped, (cluster.x_start + cluster.x_end) * 0.5);
                        assert!(text.is_char_boundary(offset));
                        assert!(
                            offset == run.range.start + cluster.source.start
                                || offset == run.range.start + cluster.source.end
                        );
                        assert_eq!(
                            block.hit_test_position(
                                x + (cluster.x_start + cluster.x_end) * 0.5,
                                line_y + line.height * 0.5,
                                &mut engine
                            ),
                            offset
                        );
                    }
                    if text[run.range.clone()]
                        .chars()
                        .all(|ch| ch.is_ascii() && !ch.is_control())
                    {
                        assert_eq!(run.font_id, primary_id);
                    }
                    x += run.width;
                }
                assert!((x - line.width).abs() < 0.001);
                line_y += line.height;
            }
            ranges.sort_by_key(|range| range.start);
            let covered: String = ranges.iter().map(|range| &text[range.clone()]).collect();
            assert_eq!(covered, text.replace('\n', ""));
            let selection = block.text_selection.as_mut().unwrap();
            selection.select_all();
            assert_eq!(selection.copy_to_clipboard().as_deref(), Some(text));
        }
        assert!(crossed_fallback_boundary);
        let unsupported = Paragraph::new("a\u{10ffff}b", TextStyle::default());
        let collapsed = ParagraphLayoutEngine::try_layout_with_catalog(
            &unsupported,
            &catalog,
            primary_id,
            &LayoutParams {
                max_width: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            collapsed
                .iter()
                .flat_map(|line| &line.runs)
                .map(|run| &unsupported.text[run.range.clone()])
                .collect::<String>(),
            unsupported.text
        );
        let mut invalid = Paragraph::new("看", TextStyle::default());
        invalid.spans[0].range.start = 1;
        assert!(matches!(
            ParagraphLayoutEngine::try_layout_with_catalog(
                &invalid,
                &catalog,
                primary_id,
                &Default::default()
            ),
            Err(TextInputError::PositionOverflow)
        ));
        let lines = ParagraphLayoutEngine::try_layout_with_catalog(
            &unsupported,
            &catalog,
            primary_id,
            &Default::default(),
        )
        .unwrap();
        assert_eq!(
            lines
                .iter()
                .flat_map(|line| &line.runs)
                .map(|run| &unsupported.text[run.range.clone()])
                .collect::<String>(),
            unsupported.text
        );
        assert!(
            lines
                .iter()
                .flat_map(|line| &line.runs)
                .flat_map(|run| &run.glyphs)
                .any(|glyph| glyph.glyph_id == 0)
        );
        assert!(matches!(
            ParagraphLayoutEngine::try_layout_with_catalog(
                &unsupported,
                &catalog,
                u64::MAX,
                &Default::default()
            ),
            Err(TextInputError::FontUnavailable)
        ));
    }

    fn collect_text<'a>(nodes: &'a [SceneNode], texts: &mut Vec<&'a TextNode>) {
        for node in nodes {
            match node {
                SceneNode::Text(text) => texts.push(text),
                SceneNode::RetainedFragment(fragment) => collect_text(&fragment.children, texts),
                SceneNode::Clip(clip) => collect_text(&clip.children, texts),
                SceneNode::Layer(layer) => collect_text(&layer.children, texts),
                SceneNode::Scroll(scroll) => collect_text(&scroll.children, texts),
                _ => {}
            }
        }
    }

    fn measure_block(
        block: &mut TextBlock,
        engine: &mut FontManagerTextEngine,
        width: f32,
    ) -> Size {
        let mut tree = WidgetTree::new();
        let theme = Theme::dark();
        let _layout = crate::core::rect_authority::LayoutAuthorityGuard::enter();
        block
            .layout(
                &mut LayoutContext::new(&mut tree, engine, &theme),
                Constraints::new(0.0, width, 0.0, f32::INFINITY),
                &[],
            )
            .size
    }

    #[test]
    fn textblock_hardbreak_selection_maps_actual_source() {
        let mut engine = FontManagerTextEngine::new();
        let text = "alpha\n\nβeta\nγ";
        let mut block = TextBlock::new(text)
            .with_size(13.0)
            .with_selection_policy(TextSelectionPolicy::Enabled);

        for width in [120.0, 24.0] {
            measure_block(&mut block, &mut engine, width);
            assert_eq!(block.lines.len() > 4, width == 24.0);
            let blank = block
                .lines
                .iter()
                .position(|line| line.line.runs.is_empty())
                .unwrap();
            let blank_y = block.lines[..blank]
                .iter()
                .map(|line| line.line.height)
                .sum::<f32>()
                + block.lines[blank].line.height * 0.5;
            let insertion = block.hit_test_position(7.0, blank_y, &mut engine);
            assert_eq!(insertion, "alpha\n".len());
            let end = block.hit_test_position(100.0, f32::INFINITY, &mut engine);
            let selection = block.text_selection.as_mut().unwrap();
            assert!(selection.pointer_down(insertion, 1, false, false, false));
            selection.pointer_move(end);
            selection.pointer_up();
            assert_eq!(selection.selected_text(), "\nβeta\nγ");
        }

        let mut edge_lines = TextBlock::new("\nβ\n\nγ\n");
        measure_block(&mut edge_lines, &mut engine, 120.0);
        let mut y = 0.0;
        let mut insertions = Vec::new();
        for entry in &edge_lines.lines {
            if entry.line.runs.is_empty() {
                insertions.push(edge_lines.hit_test_position(
                    15.0,
                    y + entry.line.height * 0.5,
                    &mut engine,
                ));
            }
            y += entry.line.height;
        }
        assert_eq!(insertions, [0, "\nβ\n".len(), edge_lines.text().len()]);
    }

    fn paint_block(
        block: &TextBlock,
        engine: &mut FontManagerTextEngine,
        rect: RectF,
        scale: f32,
    ) -> Vec<TextNode> {
        use crate::core::animation::Timeline;
        use crate::core::event::KeyboardAuthorityMode;
        use crate::core::layout_transition::InterpolatedVisualRects;
        use crate::core::rect_authority::PaintAuthorityGuard;
        use crate::core::scene::SceneBuilder;

        let tree = WidgetTree::new();
        let theme = Theme::dark();
        let icons = IconRegistry::new(&[]);
        let timeline = Timeline::default();
        let transitions = InterpolatedVisualRects::default();
        let mut scene = SceneBuilder::with_scale_factor(scale);
        let mut overlays = Vec::new();
        let mut tooltips = Vec::new();
        {
            let _paint = PaintAuthorityGuard::enter();
            block.paint(
                &mut PaintContext::new(
                    &tree,
                    &mut scene,
                    &theme,
                    engine,
                    &icons,
                    0,
                    &timeline,
                    &transitions,
                    &mut overlays,
                    &mut tooltips,
                    scale,
                    None,
                    KeyboardAuthorityMode::Navigation,
                ),
                rect,
                &[],
            );
        }
        scene
            .build()
            .root
            .into_iter()
            .filter_map(|node| match node {
                SceneNode::Text(text) => Some(text),
                _ => None,
            })
            .collect()
    }

    const FRACTIONAL_TEXT: &str = "Дробные метрики сохраняют каждую строку текста при переносе.";

    #[test]
    fn text_selection_palette_text_block_paints_selected_characters() {
        use crate::tests::_support::{assert_painted_text_backgrounds, text_selection_palette_cases};

        for (theme, expected) in text_selection_palette_cases() {
            let mut block = TextBlock::new("selected block")
                .with_selection_policy(TextSelectionPolicy::Enabled);
            block.text_selection.as_mut().unwrap().select_all();
            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(block), None, 0);
            let frame = RetainedSceneTestHarness::new().frame(
                &mut tree,
                root,
                Constraints::loose(Size::new(300.0, 100.0)),
                &theme,
                &mut FontManagerTextEngine::new(),
                &IconRegistry::new(&[]),
            );
            assert_painted_text_backgrounds(&frame.scene_root, "selected block", &[expected]);
        }
    }

    #[test]
    fn fractional_wrapped_lines_paint_at_exact_measured_height() {
        for scale in [1.0, 1.5, 2.0] {
            let mut engine = FontManagerTextEngine::new();
            engine.set_scale(scale);
            let mut block = TextBlock::new(FRACTIONAL_TEXT).with_size(22.0);
            let measured = measure_block(&mut block, &mut engine, 180.0);
            assert!(block.lines.len() >= 3);
            assert!(block.lines.iter().all(|entry| entry.line.height.fract() > 0.5));
            assert_eq!(
                measured.height,
                block.lines.iter().map(|entry| entry.line.height).sum::<f32>()
            );
            let rect = RectF::new(7.25, 3.5, measured.width, measured.height);
            let texts = paint_block(&block, &mut engine, rect, scale);
            assert_eq!(
                texts
                    .iter()
                    .map(|node| node.text.as_str())
                    .collect::<String>(),
                FRACTIONAL_TEXT,
                "every measured line must reach Scene at scale {scale}"
            );
            let mut nodes = texts.iter();
            let mut line_top = 0.0;
            for entry in &block.lines {
                let line = &entry.line;
                let mut run_x = rect.x;
                for face_run in &line.runs {
                    let node = nodes.next().expect("measured run must be painted");
                    let shaped = block.shape_laid_out_run(face_run, &mut engine).unwrap();
                    assert!(shaped.glyphs.iter().all(|glyph| glyph.id != 0));
                    assert_eq!(node.text, FRACTIONAL_TEXT[face_run.range.clone()]);
                    assert_eq!(node.binding, shaped.binding);
                    assert_eq!(node.position.x, run_x);
                    assert_eq!(
                        node.position.y,
                        rect.y + line_top + line.baseline - shaped.ascent
                    );
                    assert!(node.backgrounds.is_empty());
                    run_x += face_run.width;
                }
                line_top += line.height;
            }
            assert!(nodes.next().is_none());
        }
    }

    #[test]
    fn fractional_wrapped_lines_preserve_constrained_height_truncation() {
        let mut engine = FontManagerTextEngine::new();
        let mut block = TextBlock::new(FRACTIONAL_TEXT).with_size(22.0);
        let measured = measure_block(&mut block, &mut engine, 180.0);
        assert!(block.lines.len() >= 3);
        let first_two: f32 = block.lines[..2].iter().map(|entry| entry.line.height).sum();
        for (height, count) in [
            (0.0, 0),
            (block.lines[0].line.height - 1.0, 0),
            (first_two, 2),
            (measured.height - 1.0, block.lines.len() - 1),
        ] {
            let expected = block.lines[..count]
                .iter()
                .flat_map(|entry| &entry.line.runs)
                .map(|run| &FRACTIONAL_TEXT[run.range.clone()])
                .collect::<String>();
            let texts = paint_block(
                &block,
                &mut engine,
                RectF::new(0.0, 0.0, measured.width, height),
                1.0,
            );
            assert_eq!(
                texts
                    .iter()
                    .map(|node| node.text.as_str())
                    .collect::<String>(),
                expected,
                "only complete lines may paint at height {height}"
            );
        }
    }

    #[test]
    fn fractional_line_hits_and_selection_backgrounds_use_measured_boundaries() {
        let mut engine = FontManagerTextEngine::new();
        let mut block = TextBlock::new(FRACTIONAL_TEXT)
            .with_size(22.0)
            .with_align(TextAlign::Center)
            .with_selection_policy(TextSelectionPolicy::Enabled);
        let measured = measure_block(&mut block, &mut engine, 180.0);
        block.text_selection.as_mut().unwrap().select_all();
        let mut boundary = 0.0;
        for pair in block.lines.windows(2) {
            boundary += pair[0].line.height;
            let next_line_x = block.align.offset(pair[1].line.width, measured.width);
            assert_eq!(
                block.hit_test_position(next_line_x, boundary, &mut engine),
                pair[1].line.runs[0].range.start,
                "hit at a measured line boundary must enter the next line"
            );
        }
        let rect = RectF::new(7.25, 3.5, measured.width, measured.height);
        let texts = paint_block(&block, &mut engine, rect, 1.0);
        let mut nodes = texts.iter();
        let mut line_top = 0.0;
        for entry in &block.lines {
            let line = &entry.line;
            let mut run_x = rect.x + block.align.offset(line.width, rect.width);
            for run in &line.runs {
                let node = nodes.next().expect("selected line must be painted");
                assert_eq!(node.text, FRACTIONAL_TEXT[run.range.clone()]);
                assert_eq!(node.position.x, run_x);
                assert!(!node.backgrounds.is_empty());
                for background in &node.backgrounds {
                    assert_eq!(node.position.y + background.y_offset, rect.y + line_top);
                    assert_eq!(background.height, line.height);
                    assert_eq!(background.color.a, Theme::dark().typography.selection_alpha);
                }
                run_x += run.width;
            }
            line_top += line.height;
        }
        assert!(nodes.next().is_none());
        assert_eq!(
            block
                .text_selection
                .as_ref()
                .unwrap()
                .copy_to_clipboard()
                .as_deref(),
            Some(FRACTIONAL_TEXT)
        );
    }

    #[test]
    fn fractional_text_keeps_natural_width_and_finite_wrap_allocation() {
        let mut engine = FontManagerTextEngine::new();
        let mut block = TextBlock::new(FRACTIONAL_TEXT).with_size(22.0);
        let natural = measure_block(&mut block, &mut engine, f32::INFINITY);
        assert_eq!(block.lines.len(), 1);
        assert_eq!(natural.width, block.lines[0].line.width);
        assert_eq!(natural.height, block.lines[0].line.height);
        let wrapped = measure_block(&mut block, &mut engine, 180.0);
        assert_eq!(wrapped.width, 180.0);
        assert!(wrapped.height > natural.height);
        block = block.with_wrap(false);
        let unwrapped = measure_block(&mut block, &mut engine, 180.0);
        assert_eq!(unwrapped.width, 180.0);
        assert_eq!(unwrapped.height, natural.height);
        assert_eq!(block.lines.len(), 1);
        let texts = paint_block(
            &block,
            &mut engine,
            RectF::new(0.0, 0.0, unwrapped.width, unwrapped.height),
            1.0,
        );
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0].text, FRACTIONAL_TEXT);
    }

    #[test]
    fn paragraph_line_order_reaches_text_block_paint_in_source_order() {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(TextBlock::new(TEXT).with_size(23.0)), None, 0);
        let mut text_engine = FontManagerTextEngine::new();
        let mut harness = RetainedSceneTestHarness::new();
        let frame = harness.frame(
            &mut tree,
            root,
            Constraints {
                min_width: 371.0,
                max_width: 371.0,
                min_height: 0.0,
                max_height: 200.0,
            },
            &Theme::dark(),
            &mut text_engine,
            &IconRegistry::new(&[]),
        );

        let mut texts = Vec::new();
        collect_text(&frame.scene_root, &mut texts);
        let painted_lines: Vec<_> = texts.iter().map(|text| text.text.as_str()).collect();

        assert!(painted_lines.len() >= 3);
        assert_eq!(painted_lines.concat(), TEXT);
        assert!(
            texts
                .windows(2)
                .all(|pair| { pair[0].position.y < pair[1].position.y })
        );
    }
}
