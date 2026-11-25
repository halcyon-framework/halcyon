//! SPEC-HAL-276: TagInput — Tag Selection Widget
//! SPEC-HAL-276C: Unified Input Model — one inline input, no popup filter field.
//!
//! Toggle-based selection of multiple items from a filterable popup list.
//! Closed state displays chips (or count badge / summary). Popup stays open
//! while toggling items — commit on close (Enter / click-outside / Tab).

use std::cell::Cell;
use std::collections::HashSet;

use crate::core::composite_text_edit::{
    CompositeEditOperation, CompositeEditOutcome, CompositeEditReply, CompositeEditorId,
};
use crate::core::event::{Event, EventPhase, KeyCode, PointerButton, PointerEventKind};
use crate::core::icon::IconId;
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{Point, RectF, Size};
use crate::core::paint::{
    ColorStop, GradientDef, GradientKind, LAYER_CONTENT, LAYER_OVERLAY, RoundedCorners,
    TextBackgroundSpan,
};
use crate::core::paragraph::TextAlign;
use crate::core::theme::{ControlChromeCategory, ControlChromeState};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::{NodeId, OverlayHitRegion};
use crate::renderer::path::{Path, StrokeParams};
use crate::utils::id::TreeWidgetId;
use crate::utils::id::WidgetInternalId;
use crate::widgets::combo_box::PopupGeometry;
use crate::widgets::composite_edit_host::{
    CompositeEditorFilter, CompositeEditorSeed, CompositeEditorState, CompositeHostContinuation,
    CompositeHostStep,
};
use crate::widgets::scrollbar::ScrollbarV;

// Popup geometry constants.
const ROW_HEIGHT: f32 = 24.0;
const POPUP_TEXT_LAYER: u32 = LAYER_OVERLAY + 2;

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TagItem {
    pub id: u64,
    pub label: String,
    pub group: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagInputPolicy {
    #[default]
    Full,
    ToggleOnly,
    Locked,
    ReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagDisplayMode {
    #[default]
    Chips,
    CountBadge,
    Summary,
}

/// SPEC-HAL-276A §14: Controls whether users can create new tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagCreationMode {
    /// Selection from predefined items only. Default. Current behavior.
    #[default]
    SelectOnly,
    /// Free-form text input only. No popup, no predefined list.
    FreeFormOnly,
    /// Inline text for custom tag creation + popup for predefined item selection.
    Hybrid,
}

/// SPEC-HAL-276A §15: A selected tag — predefined item or user-created.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TagEntry {
    /// Selected from the predefined item list.
    Item(u64),
    /// User-created free-form tag.
    Custom(String),
}

/// SPEC-HAL-276A §17: Which keys trigger tag creation.
#[derive(Debug, Clone, Copy)]
pub struct TagTriggerKeys {
    pub enter: bool,
    pub comma: bool,
    pub tab: bool,
}

impl Default for TagTriggerKeys {
    fn default() -> Self {
        Self {
            enter: true,
            comma: false,
            tab: false,
        }
    }
}

/// Cached chip rect for hit-testing × buttons.
#[derive(Debug, Clone)]
struct ChipRect {
    entry: TagEntry,
    close_rect: RectF,
}

/// Which affordance icon is being interacted with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AffordanceIcon {
    SelectAll,
    ClearAll,
    Dropdown,
}

/// A row in the popup display list (item or group header).
#[derive(Debug, Clone)]
enum PopupRow {
    GroupHeader(String),
    Item(u64),
}

// ─────────────────────────────────────────────────────────────────────────────
// Widget
// ─────────────────────────────────────────────────────────────────────────────

pub struct TagInput {
    id: WidgetInternalId,
    items: Vec<TagItem>,
    // Committed selection (the "real" value)
    committed_set: HashSet<TagEntry>,
    committed_order: Vec<TagEntry>,
    // Working selection (diverges during popup open)
    working_set: HashSet<TagEntry>,
    working_order: Vec<TagEntry>,
    // Filtered item IDs (recomputed from inline_input text)
    filtered_ids: Vec<u64>,
    // Popup state
    popup_open: bool,
    highlighted_index: Option<usize>, // index into filtered_ids
    popup_scroll_row: usize,
    // Config
    display_mode: TagDisplayMode,
    policy: TagInputPolicy,
    max_selections: Option<usize>,
    placeholder: String,
    // Widget state
    focused: bool,
    in_capture: bool,
    // SPEC-HAL-276A
    tag_creation_mode: TagCreationMode,
    trigger_keys: TagTriggerKeys,
    tag_validator: Option<fn(&str) -> bool>,
    allow_duplicate_tags: bool,
    // Callbacks
    on_change: Option<Box<dyn FnMut(&[TagEntry])>>,
    // Cached layout
    control_height: f32,
    last_layout_width: f32,
    affordance_width: f32,
    // Scrollbar
    scrollbar_v: ScrollbarV,
    // Chip rects for × hit-testing (rebuilt each paint)
    chip_rects: Cell<Vec<ChipRect>>,
    /// Inline text field — used for all modes when captured.
    inline_input: CompositeEditorState,
    inline_span: Option<(f32, f32)>,
    inline_cursor_rect: Option<RectF>,
    visible_chip_count: usize,
    overflow_badge_span: Option<(f32, f32)>,
    inline_layout_dirty: bool,
    /// Ghost completion suffix from highlighted item (visual only).
    ghost_completion: Option<String>,
    ghost_source: Option<(Option<u64>, Option<u64>, String)>,
    /// Validation error state (border turns red).
    validation_error: bool,
    /// Chip flash animation: (entry, start_ms).
    chip_flash: Option<(TagEntry, u64)>,
    /// Last frame time for chip flash timing.
    last_frame_ms: u64,
    /// Number of items above which Select All / Clear All appear.
    select_all_threshold: usize,
    /// Cached popup display rows (items + group headers).
    popup_rows: Vec<PopupRow>,
    /// Affordance icon hover state.
    hovered_icon: Option<AffordanceIcon>,
    /// Affordance icon press state.
    pressed_icon: Option<AffordanceIcon>,
    /// Which chip's × close button is hovered (by entry).
    hovered_close: Option<TagEntry>,
}

pub(crate) enum CompositeTagContinuation {
    Entry,
    Retire {
        handled: bool,
    },
    Key {
        code: KeyCode,
    },
    Input {
        refilter: bool,
        clear_validation: bool,
        force_handled: bool,
    },
    Clear {
        refilter: bool,
        emitted: bool,
    },
    Create {
        refilter: bool,
    },
    Split {
        text: String,
    },
}

impl TagInput {
    /// Select shared text-entry presentation before mounting this control.
    pub fn with_smooth_caret_prototype(mut self, enabled: bool) -> Self {
        self.inline_input
            .set_initial_motion(enabled)
            .expect("composite presentation is a construction property");
        self
    }
    pub(crate) fn composite_seed(&self) -> CompositeEditorSeed {
        let mut seed = self.inline_input.construction_seed();
        seed.palette = crate::widgets::composite_edit_host::CompositeEditorPalette::InputChrome;
        seed
    }
    pub(crate) fn composite_id(&self) -> Option<&CompositeEditorId> {
        self.inline_input.id.as_ref()
    }
    pub(crate) fn composite_bind(&mut self, id: CompositeEditorId, child: NodeId) -> bool {
        self.inline_input.bind(id, child)
    }
    pub(crate) fn composite_active(&self) -> bool {
        self.in_capture && self.can_interact() && self.inline_span.is_some()
    }
    pub(crate) fn composite_visible(&self) -> bool {
        self.inline_span.is_some()
    }
    pub(crate) fn composite_policy(&self) -> crate::core::text_edit_policy::TextEditPolicy {
        crate::core::text_edit_policy::TextEditPolicy {
            enabled: self.can_interact(),
            read_only: false,
        }
    }
    pub(crate) fn composite_observe(&mut self, reply: &CompositeEditReply) -> bool {
        let applied = self.inline_input.absorb(reply);
        if applied && self.ghost_completion.is_some() {
            self.set_ghost_completion(self.ghost_completion.clone());
        }
        applied
    }
    pub(crate) fn composite_presentation_revision(&self) -> u64 {
        self.inline_input.presentation_revision()
    }
    fn set_ghost_completion(&mut self, ghost: Option<String>) {
        let source = ghost.as_ref().map(|_| {
            let id = self
                .highlighted_index
                .and_then(|index| self.filtered_ids.get(index))
                .copied();
            let label = id
                .and_then(|id| self.items.iter().find(|item| item.id == id))
                .map(|item| item.label.clone())
                .unwrap_or_default();
            (
                self.inline_input
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.revision),
                id,
                label,
            )
        });
        if self.ghost_completion != ghost || self.ghost_source != source {
            self.inline_input.change_presentation_source();
            self.ghost_completion = ghost;
            self.ghost_source = source;
        }
    }
    pub(crate) fn composite_set_items(&mut self, items: Vec<TagItem>) {
        self.items = items;
        self.refilter_from_input();
    }
    pub(crate) fn composite_set_policy(&mut self, policy: TagInputPolicy) {
        self.policy = policy;
        self.popup_open = false;
        self.refilter_from_input();
    }
    fn request_inline(
        &self,
        cx: &mut EventContext,
        operation: CompositeEditOperation,
        continuation: CompositeTagContinuation,
    ) -> bool {
        cx.request_composite_edit(operation, CompositeHostContinuation::Tag(continuation))
            .is_ok()
    }
    fn final_clear(refilter: bool, emitted: bool) -> CompositeHostStep {
        CompositeHostStep::Final {
            operation: CompositeEditOperation::ClearAfterTagCommand,
            continuation: CompositeHostContinuation::Tag(CompositeTagContinuation::Clear {
                refilter,
                emitted,
            }),
        }
    }
    fn final_input(refilter: bool, force_handled: bool) -> CompositeHostStep {
        CompositeHostStep::Final {
            operation: CompositeEditOperation::ForwardCurrentInput,
            continuation: CompositeHostContinuation::Tag(CompositeTagContinuation::Input {
                refilter,
                clear_validation: false,
                force_handled,
            }),
        }
    }
    pub(crate) fn resume_composite(
        &mut self,
        cx: &mut EventContext,
        continuation: CompositeTagContinuation,
        reply: CompositeEditReply,
    ) -> CompositeHostStep {
        if let CompositeTagContinuation::Retire { handled } = continuation {
            return CompositeHostStep::Done { handled };
        }
        if !self.composite_observe(&reply) {
            return CompositeHostStep::Done {
                handled: reply.handled,
            };
        }
        match continuation {
            CompositeTagContinuation::Entry => {
                if self.tag_creation_mode != TagCreationMode::FreeFormOnly {
                    self.open_popup();
                }
                CompositeHostStep::Done { handled: true }
            }
            CompositeTagContinuation::Key { code } => self.resume_inline_key(cx, code),
            CompositeTagContinuation::Input {
                refilter,
                clear_validation,
                force_handled,
            } => {
                if refilter {
                    self.refilter_from_input();
                }
                if clear_validation {
                    self.validation_error = false;
                }
                CompositeHostStep::Done {
                    handled: force_handled || reply.handled,
                }
            }
            CompositeTagContinuation::Clear { refilter, .. } => {
                if refilter {
                    self.refilter_from_input();
                }
                CompositeHostStep::Done { handled: true }
            }
            CompositeTagContinuation::Create { refilter } => {
                if self.try_create_tag() {
                    Self::final_clear(refilter, true)
                } else {
                    CompositeHostStep::Done { handled: true }
                }
            }
            CompositeTagContinuation::Split { text } => {
                let delimiters = [
                    if self.trigger_keys.comma { ',' } else { '\0' },
                    if self.trigger_keys.tab { '\t' } else { '\0' },
                ];
                for segment in text.split(|c| delimiters.contains(&c)) {
                    let trimmed = segment.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if let Some(item) = self
                        .items
                        .iter()
                        .find(|item| item.label.eq_ignore_ascii_case(trimmed) && item.enabled)
                    {
                        let entry = TagEntry::Item(item.id);
                        if !self.working_set.contains(&entry) {
                            self.working_set.insert(entry.clone());
                            self.working_order.push(entry);
                        }
                    } else if matches!(
                        self.tag_creation_mode,
                        TagCreationMode::Hybrid | TagCreationMode::FreeFormOnly
                    ) {
                        let entry = TagEntry::Custom(trimmed.to_owned());
                        if self.allow_duplicate_tags || !self.working_set.contains(&entry) {
                            self.working_set.insert(entry.clone());
                            self.working_order.push(entry);
                        }
                    }
                }
                self.committed_set = self.working_set.clone();
                self.committed_order = self.working_order.clone();
                self.fire_on_change();
                Self::final_clear(true, true)
            }
            CompositeTagContinuation::Retire { .. } => unreachable!(),
        }
    }

    fn resume_inline_key(&mut self, cx: &mut EventContext, code: KeyCode) -> CompositeHostStep {
        match code {
            KeyCode::Enter => self.handle_enter(cx),
            KeyCode::Tab => {
                if self.popup_open {
                    self.close_popup_commit();
                }
                CompositeHostStep::Final {
                    operation: CompositeEditOperation::RetireDelegation,
                    continuation: CompositeHostContinuation::Tag(
                        CompositeTagContinuation::Retire { handled: false },
                    ),
                }
            }
            KeyCode::Down => {
                if self.popup_open {
                    self.highlight_next_enabled();
                    self.ensure_highlight_visible();
                    self.recompute_ghost();
                } else if self.tag_creation_mode != TagCreationMode::FreeFormOnly {
                    self.open_popup();
                }
                CompositeHostStep::Done { handled: true }
            }
            KeyCode::Up => {
                if self.popup_open {
                    self.highlight_prev_enabled();
                    self.ensure_highlight_visible();
                    self.recompute_ghost();
                } else if self.tag_creation_mode != TagCreationMode::FreeFormOnly {
                    self.open_popup();
                    if let Some(index) = self.filtered_ids.iter().rposition(|id| {
                        self.items.iter().any(|item| item.id == *id && item.enabled)
                    }) {
                        self.highlighted_index = Some(index);
                        self.ensure_highlight_visible();
                    }
                }
                CompositeHostStep::Done { handled: true }
            }
            KeyCode::Backspace if self.inline_input.text().is_empty() => {
                CompositeHostStep::Done { handled: true }
            }
            KeyCode::Backspace | KeyCode::Delete => Self::final_input(true, true),
            KeyCode::Home | KeyCode::End | KeyCode::Left | KeyCode::Right => {
                Self::final_input(false, true)
            }
            KeyCode::PageDown if self.popup_open => {
                self.highlighted_index = Some(
                    (self.highlighted_index.unwrap_or(0) + self.visible_rows())
                        .min(self.filtered_ids.len().saturating_sub(1)),
                );
                self.ensure_highlight_visible();
                self.recompute_ghost();
                CompositeHostStep::Done { handled: true }
            }
            KeyCode::PageUp if self.popup_open => {
                self.highlighted_index = Some(
                    self.highlighted_index
                        .unwrap_or(0)
                        .saturating_sub(self.visible_rows()),
                );
                self.ensure_highlight_visible();
                self.recompute_ghost();
                CompositeHostStep::Done { handled: true }
            }
            _ => Self::final_input(false, false),
        }
    }
    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            items: Vec::new(),
            committed_set: HashSet::new(),
            committed_order: Vec::new(),
            working_set: HashSet::new(),
            working_order: Vec::new(),
            filtered_ids: Vec::new(),
            popup_open: false,
            highlighted_index: None,
            popup_scroll_row: 0,
            display_mode: TagDisplayMode::default(),
            policy: TagInputPolicy::default(),
            max_selections: None,
            placeholder: String::new(),
            focused: false,
            in_capture: false,
            tag_creation_mode: TagCreationMode::SelectOnly,
            trigger_keys: TagTriggerKeys::default(),
            tag_validator: None,
            allow_duplicate_tags: false,
            on_change: None,
            control_height: 0.0,
            last_layout_width: 0.0,
            affordance_width: 20.0,
            scrollbar_v: ScrollbarV::new(),
            chip_rects: Cell::new(Vec::new()),
            inline_input: Self::make_inline_input(),
            inline_span: None,
            inline_cursor_rect: None,
            visible_chip_count: 0,
            overflow_badge_span: None,
            inline_layout_dirty: true,
            ghost_completion: None,
            ghost_source: None,
            validation_error: false,
            chip_flash: None,
            last_frame_ms: 0,
            select_all_threshold: 10,
            popup_rows: Vec::new(),
            hovered_icon: None,
            pressed_icon: None,
            hovered_close: None,
        }
    }

    // ── Builders ──

    pub fn with_items(mut self, items: Vec<TagItem>) -> Self {
        self.items = items;
        self.refilter_from_input();
        self
    }

    pub fn with_selection(mut self, ids: &[u64]) -> Self {
        self.committed_set.clear();
        self.committed_order.clear();
        for &id in ids {
            let entry = TagEntry::Item(id);
            if self.items.iter().any(|it| it.id == id) && self.committed_set.insert(entry.clone()) {
                self.committed_order.push(entry);
            }
        }
        self.working_set = self.committed_set.clone();
        self.working_order = self.committed_order.clone();
        self
    }

    pub fn with_policy(mut self, policy: TagInputPolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn with_display_mode(mut self, mode: TagDisplayMode) -> Self {
        self.display_mode = mode;
        self
    }

    pub fn with_max_selections(mut self, max: usize) -> Self {
        self.max_selections = Some(max);
        self
    }

    pub fn with_placeholder(mut self, text: &str) -> Self {
        self.placeholder = text.to_string();
        self
    }

    pub fn on_change(mut self, cb: impl FnMut(&[TagEntry]) + 'static) -> Self {
        self.on_change = Some(Box::new(cb));
        self
    }

    pub fn with_tag_creation(mut self, mode: TagCreationMode) -> Self {
        self.tag_creation_mode = mode;
        self
    }

    pub fn with_trigger_keys(mut self, keys: TagTriggerKeys) -> Self {
        self.trigger_keys = keys;
        self
    }

    pub fn with_tag_validator(mut self, f: fn(&str) -> bool) -> Self {
        self.tag_validator = Some(f);
        self
    }

    pub fn with_allow_duplicate_tags(mut self, allow: bool) -> Self {
        self.allow_duplicate_tags = allow;
        self
    }

    // ── Setters ──

    pub fn set_items(&mut self, items: Vec<TagItem>) -> Result<(), CompositeEditOutcome> {
        if self.inline_input.id.is_some() {
            return Err(CompositeEditOutcome::OwnerContextRequired);
        }
        self.composite_set_items(items);
        Ok(())
    }

    pub fn set_policy(&mut self, policy: TagInputPolicy) -> Result<(), CompositeEditOutcome> {
        if self.inline_input.id.is_some() {
            return Err(CompositeEditOutcome::OwnerContextRequired);
        }
        self.composite_set_policy(policy);
        Ok(())
    }

    // ── Selection API ──

    pub fn committed_ids(&self) -> Vec<u64> {
        self.committed_order
            .iter()
            .filter_map(|e| {
                if let TagEntry::Item(id) = e {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn committed_entries(&self) -> &[TagEntry] {
        &self.committed_order
    }

    // ── Internal ──

    fn make_inline_input() -> CompositeEditorState {
        CompositeEditorState::new(TextAlign::Left, CompositeEditorFilter::Any)
    }

    /// Refilter popup items based on inline input text.
    fn refilter_from_input(&mut self) {
        self.inline_layout_dirty = true;
        let query = self.inline_input.text().to_lowercase();
        self.filtered_ids = if query.is_empty() {
            self.items.iter().map(|i| i.id).collect()
        } else {
            self.items
                .iter()
                .filter(|i| i.label.to_lowercase().contains(&query))
                .map(|i| i.id)
                .collect()
        };
        // Sort by group
        let items_ref = &self.items;
        self.filtered_ids.sort_by(|a, b| {
            let ga = items_ref
                .iter()
                .find(|it| it.id == *a)
                .and_then(|it| it.group.as_deref());
            let gb = items_ref
                .iter()
                .find(|it| it.id == *b)
                .and_then(|it| it.group.as_deref());
            ga.cmp(&gb)
        });
        // Rebuild popup display rows
        self.popup_rows = self.build_popup_rows();
        // Reset highlight to first enabled match
        self.highlighted_index = None;
        for (i, &id) in self.filtered_ids.iter().enumerate() {
            if let Some(item) = self.items.iter().find(|it| it.id == id) {
                if item.enabled {
                    self.highlighted_index = Some(i);
                    break;
                }
            }
        }
        self.recompute_ghost();
        // Clamp scroll
        let max = self.max_scroll_row();
        if self.popup_scroll_row > max {
            self.popup_scroll_row = max;
        }
    }

    fn recompute_ghost(&mut self) {
        let query = self.inline_input.text();
        if query.is_empty() {
            self.set_ghost_completion(None);
            return;
        }
        if let Some(idx) = self.highlighted_index {
            if idx < self.filtered_ids.len() {
                let id = self.filtered_ids[idx];
                if let Some(item) = self.items.iter().find(|it| it.id == id) {
                    if item.label.to_lowercase().starts_with(&query.to_lowercase()) {
                        let suffix: String =
                            item.label.chars().skip(query.chars().count()).collect();
                        self.set_ghost_completion(if suffix.is_empty() {
                            None
                        } else {
                            Some(suffix)
                        });
                        return;
                    }
                }
            }
        }
        self.set_ghost_completion(None);
    }

    fn toggle_item(&mut self, id: u64) -> bool {
        let entry = TagEntry::Item(id);
        if self.working_set.contains(&entry) {
            self.working_set.remove(&entry);
            self.working_order.retain(|x| *x != entry);
        } else {
            if let Some(max) = self.max_selections {
                if self.working_set.len() >= max {
                    return false;
                }
            }
            if let Some(item) = self.items.iter().find(|it| it.id == id) {
                if !item.enabled {
                    return false;
                }
            }
            self.working_set.insert(entry.clone());
            self.working_order.push(entry);
        }
        // Immediate commit — each toggle is an explicit user action
        self.committed_set = self.working_set.clone();
        self.committed_order = self.working_order.clone();
        self.fire_on_change();
        true
    }

    fn remove_chip(&mut self, entry: &TagEntry) {
        self.committed_set.remove(entry);
        self.committed_order.retain(|x| x != entry);
        self.working_set = self.committed_set.clone();
        self.working_order = self.committed_order.clone();
        self.fire_on_change();
    }

    fn fire_on_change(&mut self) {
        self.inline_layout_dirty = true;
        if let Some(ref mut cb) = self.on_change {
            cb(&self.committed_order);
        }
    }

    fn open_popup(&mut self) {
        if self.tag_creation_mode == TagCreationMode::FreeFormOnly {
            return;
        }
        if matches!(
            self.policy,
            TagInputPolicy::Locked | TagInputPolicy::ReadOnly
        ) {
            return;
        }
        self.working_set = self.committed_set.clone();
        self.working_order = self.committed_order.clone();
        self.refilter_from_input();
        self.popup_open = true;
        self.popup_scroll_row = 0;
    }

    fn close_popup_commit(&mut self) {
        self.inline_layout_dirty = true;
        // Toggle/create already committed immediately — nothing extra to commit
        self.popup_open = false;
        self.set_ghost_completion(None);
    }

    fn visible_rows(&self) -> usize {
        let max_rows = 8;
        self.popup_rows.len().min(max_rows)
    }

    fn popup_geometry(&self, rect: RectF) -> Option<PopupGeometry> {
        if !self.popup_open {
            return None;
        }
        let rows = self.visible_rows();
        if rows == 0 && self.inline_input.text().is_empty() {
            return None;
        }
        Some(PopupGeometry::from_rects(
            rect,
            RectF::new(
                rect.x,
                rect.bottom(),
                rect.width,
                rows.max(1) as f32 * ROW_HEIGHT,
            ),
        ))
    }

    fn max_scroll_row(&self) -> usize {
        self.filtered_ids.len().saturating_sub(self.visible_rows())
    }

    fn ensure_highlight_visible(&mut self) {
        let Some(idx) = self.highlighted_index else {
            return;
        };
        let visible = self.visible_rows();
        if idx < self.popup_scroll_row {
            self.popup_scroll_row = idx;
        } else if idx >= self.popup_scroll_row + visible {
            self.popup_scroll_row = idx + 1 - visible;
        }
    }

    fn highlight_next_enabled(&mut self) {
        if self.filtered_ids.is_empty() {
            return;
        }
        let start = self.highlighted_index.map(|i| i + 1).unwrap_or(0);
        for i in start..self.filtered_ids.len() {
            let id = self.filtered_ids[i];
            if let Some(item) = self.items.iter().find(|it| it.id == id) {
                if item.enabled {
                    self.highlighted_index = Some(i);
                    return;
                }
            }
        }
    }

    fn highlight_prev_enabled(&mut self) {
        if self.filtered_ids.is_empty() {
            return;
        }
        let start = match self.highlighted_index {
            Some(0) | None => return,
            Some(i) => i - 1,
        };
        for i in (0..=start).rev() {
            let id = self.filtered_ids[i];
            if let Some(item) = self.items.iter().find(|it| it.id == id) {
                if item.enabled {
                    self.highlighted_index = Some(i);
                    return;
                }
            }
        }
    }

    fn popup_row_hit(&self, local_x: f32, local_y: f32) -> Option<usize> {
        let popup_top = self.control_height;
        let popup_bottom = popup_top + self.visible_rows() as f32 * ROW_HEIGHT;
        if !(local_x >= 0.0
            && local_x < self.last_layout_width
            && local_y >= popup_top
            && local_y < popup_bottom)
        {
            return None;
        }
        let local_row = ((local_y - popup_top) / ROW_HEIGHT) as usize;
        let display_idx = self.popup_scroll_row + local_row;
        if display_idx >= self.popup_rows.len() {
            return None;
        }
        match &self.popup_rows[display_idx] {
            PopupRow::Item(id) => self.filtered_ids.iter().position(|fid| fid == id),
            PopupRow::GroupHeader(_) => None,
        }
    }

    fn needs_scrollbar(&self) -> bool {
        self.filtered_ids.len() > self.visible_rows()
    }

    fn label_for_entry(&self, entry: &TagEntry) -> String {
        match entry {
            TagEntry::Item(id) => self
                .items
                .iter()
                .find(|it| it.id == *id)
                .map(|it| it.label.clone())
                .unwrap_or_else(|| "?".to_string()),
            TagEntry::Custom(s) => s.clone(),
        }
    }

    /// Attempt to create a Custom tag from the inline input text.
    fn try_create_tag(&mut self) -> bool {
        let text = self.inline_input.text().trim().to_string();
        if text.is_empty() {
            return false;
        }
        if let Some(validator) = self.tag_validator {
            if !validator(&text) {
                self.validation_error = true;
                return false;
            }
        }
        let entry = TagEntry::Custom(text);
        if !self.allow_duplicate_tags {
            let set = &self.working_set;
            if set.contains(&entry) {
                self.chip_flash = Some((entry, self.last_frame_ms));
                return false;
            }
        }
        if let Some(max) = self.max_selections {
            if self.working_set.len() >= max {
                return false;
            }
        }
        // Add to both sets atomically
        self.working_set.insert(entry.clone());
        self.working_order.push(entry);
        self.committed_set = self.working_set.clone();
        self.committed_order = self.working_order.clone();
        self.fire_on_change();
        self.validation_error = false;
        true
    }

    /// Handle Enter key — unified for all modes.
    fn handle_enter(&mut self, cx: &mut EventContext) -> CompositeHostStep {
        let input_text = self.inline_input.text().trim().to_string();

        if let Some(idx) = self.highlighted_index {
            // Highlighted item → toggle it
            let emitted = if idx < self.filtered_ids.len() {
                let id = self.filtered_ids[idx];
                self.toggle_item(id)
            } else {
                false
            };
            return Self::final_clear(true, emitted);
        } else if !input_text.is_empty() {
            // No highlight — check case-insensitive exact match against items
            let exact = self
                .items
                .iter()
                .find(|item| item.label.eq_ignore_ascii_case(&input_text) && item.enabled)
                .map(|item| item.id);

            if let Some(id) = exact {
                let emitted = self.toggle_item(id);
                return Self::final_clear(true, emitted);
            } else if matches!(
                self.tag_creation_mode,
                TagCreationMode::Hybrid | TagCreationMode::FreeFormOnly
            ) {
                if self.try_create_tag() {
                    return Self::final_clear(true, true);
                }
                self.refilter_from_input();
            }
        } else {
            // Empty input → close and commit
            if self.popup_open {
                self.close_popup_commit();
            }
            cx.release_keyboard_capture();
        }
        CompositeHostStep::Done { handled: true }
    }

    fn build_popup_rows(&self) -> Vec<PopupRow> {
        let mut rows = Vec::new();
        let mut last_group: Option<&str> = None;
        for &id in &self.filtered_ids {
            if let Some(item) = self.items.iter().find(|it| it.id == id) {
                let current_group = item.group.as_deref();
                if current_group != last_group && current_group.is_some() {
                    rows.push(PopupRow::GroupHeader(current_group.unwrap().to_string()));
                    last_group = current_group;
                }
                rows.push(PopupRow::Item(id));
            }
        }
        rows
    }

    fn show_action_icons(&self) -> bool {
        self.items.len() > self.select_all_threshold
            && matches!(self.policy, TagInputPolicy::Full)
            && self.tag_creation_mode != TagCreationMode::FreeFormOnly
    }

    fn select_all_enabled(&mut self) {
        for item in &self.items {
            if item.enabled {
                let entry = TagEntry::Item(item.id);
                if !self.working_set.contains(&entry) {
                    self.working_set.insert(entry.clone());
                    self.working_order.push(entry);
                }
            }
        }
        self.committed_set = self.working_set.clone();
        self.committed_order = self.working_order.clone();
        self.fire_on_change();
    }

    fn clear_all(&mut self) {
        // Remove only Item entries (preserve Custom tags)
        self.working_order
            .retain(|e| matches!(e, TagEntry::Custom(_)));
        self.working_set = self.working_order.iter().cloned().collect();
        self.committed_set = self.working_set.clone();
        self.committed_order = self.working_order.clone();
        self.fire_on_change();
    }

    fn hit_test_affordance(&self, local_x: f32, local_y: f32) -> Option<AffordanceIcon> {
        if local_y < 0.0 || local_y >= self.control_height {
            return None;
        }
        if self.tag_creation_mode == TagCreationMode::FreeFormOnly {
            return None;
        }
        let aff_right = self.last_layout_width;
        let icon_slot = 22.0;

        // Dropdown region (rightmost, always present)
        let combo_aw = if self.show_action_icons() {
            self.affordance_width - icon_slot * 2.0
        } else {
            self.affordance_width
        };
        let dd_start = aff_right - combo_aw;
        if local_x >= dd_start && local_x < aff_right {
            return Some(AffordanceIcon::Dropdown);
        }

        // Select All / Clear All (only when icons shown)
        if self.show_action_icons() {
            let clr_start = aff_right - combo_aw - icon_slot;
            if local_x >= clr_start && local_x < clr_start + icon_slot {
                return Some(AffordanceIcon::ClearAll);
            }
            let sel_start = aff_right - combo_aw - icon_slot * 2.0;
            if local_x >= sel_start && local_x < sel_start + icon_slot {
                return Some(AffordanceIcon::SelectAll);
            }
        }
        None
    }

    fn can_interact(&self) -> bool {
        matches!(
            self.policy,
            TagInputPolicy::Full | TagInputPolicy::ToggleOnly
        )
    }
}

impl WidgetDowncast for TagInput {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TagInput {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let tf = &cx.theme.geometry.text_field;
        let size = constraints.constrain(Size::new(constraints.max_width, tf.height));
        self.control_height = size.height;
        self.last_layout_width = size.width;

        let icon_slot = 22.0;
        if self.tag_creation_mode == TagCreationMode::FreeFormOnly {
            self.affordance_width = 0.0;
        } else if self.show_action_icons() {
            self.affordance_width = icon_slot * 2.0 + cx.theme.geometry.combo_box.affordance_width;
        } else {
            self.affordance_width = cx.theme.geometry.combo_box.affordance_width;
        }

        let selection = if self.popup_open {
            &self.working_order
        } else {
            &self.committed_order
        };
        let placeholder = selection.is_empty()
            && !self.in_capture
            && self.tag_creation_mode == TagCreationMode::SelectOnly;
        let show_inline = self.in_capture
            || self.tag_creation_mode != TagCreationMode::SelectOnly
            || !self.inline_input.text().is_empty();
        self.inline_span = None;
        self.inline_cursor_rect = None;
        self.visible_chip_count = 0;
        self.overflow_badge_span = None;
        if self.display_mode == TagDisplayMode::Chips && !placeholder {
            let ms = &cx.theme.geometry.tag_input;
            let padding = cx.theme.geometry.text_field.padding_x;
            let right = (self.last_layout_width - self.affordance_width).max(0.0);
            let available = (right - 2.0 * padding).max(0.0);
            // INV-276-ARCH3: the existing 60px typing allowance is bounded by
            // actual allocation; neither chips nor the child may overrun it.
            let typing_width = if show_inline {
                60.0_f32.min(available)
            } else {
                0.0
            };
            let max_x = (right - padding - typing_width).max(padding);
            let badge_width = if selection.is_empty() {
                0.0
            } else {
                cx.text_engine
                    .shape(
                        &format!("+{}", selection.len()),
                        cx.theme.typography.font_size,
                    )
                    .width
            };
            let mut chip_x = padding;
            let mut chip_end = 0.0;
            let mut count = 0;
            for entry in selection {
                let label = self.label_for_entry(entry);
                let width = cx
                    .text_engine
                    .shape(&label, cx.theme.typography.font_size)
                    .width
                    + ms.chip_padding_x * 2.0
                    + ms.chip_close_size;
                let remaining = selection.len() - count - 1;
                let reserve = if remaining > 0 {
                    ms.chip_gap + badge_width
                } else {
                    0.0
                };
                if chip_x + width + reserve > max_x {
                    break;
                }
                chip_end = chip_x + width;
                chip_x = chip_end + ms.chip_gap;
                count += 1;
            }
            self.visible_chip_count = count;
            if count < selection.len() {
                let width = cx
                    .text_engine
                    .shape(
                        &format!("+{}", selection.len() - count),
                        cx.theme.typography.font_size,
                    )
                    .width;
                let width = width.min((max_x - chip_x).max(0.0));
                self.overflow_badge_span = Some((chip_x, width));
                chip_end = chip_x + width;
                chip_x = chip_end + ms.chip_gap;
            }
            if show_inline {
                let start = if selection.is_empty() {
                    0.0
                } else {
                    (chip_x - padding).max(chip_end).min(right)
                };
                self.inline_span = Some((start, (right - start).max(0.0)));
                self.inline_cursor_rect = Some(RectF::new(
                    start,
                    0.0,
                    (right - start).max(0.0),
                    self.control_height,
                ));
            }
        }
        self.inline_layout_dirty = false;

        // Always layout inline_input
        let (start, width) = self.inline_span.unwrap_or((0.0, size.width));
        self.inline_input.layout(
            cx,
            Constraints::tight(Size::new(width, self.control_height)),
            start,
            0.0,
        );

        // Sync scrollbar config
        self.scrollbar_v.config.thickness = cx.theme.scrollbar_geom.thickness;
        self.scrollbar_v.config.min_thumb_size = cx.theme.scrollbar_geom.min_thumb_size;
        self.scrollbar_v.config.corner_radius = cx.theme.scrollbar_geom.corner_radius;
        self.scrollbar_v.config.hit_expansion = cx.theme.scrollbar_geom.hit_expansion;

        LayoutResult::fit(size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let t = cx.theme;
        let ms = &t.geometry.tag_input;
        let aw = self.affordance_width;
        let font_size = t.typography.font_size;
        let text_padding = t.geometry.text_field.padding_x;

        let state = if cx.has_capture() {
            ControlChromeState::Captured
        } else if cx.has_focus_cue() {
            ControlChromeState::Focused
        } else {
            ControlChromeState::Idle
        };
        let style = t.tag_input_chrome(state, self.validation_error);
        let text_layer = LAYER_CONTENT + 5;
        let chip_layer = LAYER_CONTENT + 2;
        let detail_layer = LAYER_CONTENT + 3;

        let no_match_creating = self.in_capture
            && self.tag_creation_mode == TagCreationMode::Hybrid
            && !self.inline_input.text().is_empty()
            && self.filtered_ids.is_empty();
        let bg_color = if no_match_creating {
            t.colors.accents.background
        } else {
            style.chrome.fill.unwrap_or(t.colors.surfaces.content)
        };
        let radius = ms
            .radius
            .min(rect.width.max(0.0) * 0.5)
            .min(rect.height.max(0.0) * 0.5);
        let edge = style
            .chrome
            .border_width
            .min(rect.width.max(0.0) * 0.5)
            .min(rect.height.max(0.0) * 0.5);
        let inset = (edge + cx.one_px())
            .min(rect.width.max(0.0) * 0.5)
            .min(rect.height.max(0.0) * 0.5);
        let body = RectF::new(
            rect.x + inset,
            rect.y + inset,
            (rect.width - inset * 2.0).max(0.0),
            (rect.height - inset * 2.0).max(0.0),
        );
        let joined = self
            .popup_geometry(rect)
            .map(|geometry| geometry.snapped(cx.scale_factor));
        if let Some(geometry) = joined {
            let radius = t.chrome_radius(ControlChromeCategory::RecessedEditable, ms.radius, false);
            let edge = if cx.has_capture() || cx.has_focus_cue() {
                (style.chrome.border_width * cx.scale_factor).round().max(1.0) / cx.scale_factor
            } else {
                cx.one_px()
            };
            cx.scene.begin_clip(geometry.control, None);
            cx.scene.push_path(
                geometry.fill_outline(radius, edge),
                bg_color,
                LAYER_CONTENT - 2,
                None,
            );
            cx.scene.push_stroked_path(
                geometry.outline(radius, edge),
                StrokeParams {
                    width: edge.min(geometry.popup.height),
                    ..Default::default()
                },
                style.chrome.border_color,
                LAYER_CONTENT - 1,
                None,
            );
            let inner_edge = if self.validation_error { cx.one_px() } else { 0.0 };
            if inner_edge > 0.0 {
                cx.scene.push_stroked_path(
                    geometry.outline(radius, edge * 2.0 + inner_edge),
                    StrokeParams {
                        width: inner_edge,
                        ..Default::default()
                    },
                    style.inner_edge,
                    LAYER_CONTENT,
                    None,
                );
            }
            if let Some(divider) = geometry.input_divider(edge + inner_edge, cx.one_px()) {
                let mut path = Path::new();
                path.move_to(divider.x, divider.y);
                path.line_to(divider.right(), divider.y);
                path.line_to(divider.right(), divider.bottom());
                path.line_to(divider.x, divider.bottom());
                path.close();
                cx.scene.push_path(
                    path,
                    t.input_chrome(ControlChromeState::Idle).chrome.border_color,
                    LAYER_CONTENT + 1,
                    None,
                );
            }
            cx.scene.end_clip();
        } else {
            cx.scene.push_rounded_rect(
                rect,
                style.chrome.border_color,
                RoundedCorners::uniform(radius),
                LAYER_CONTENT - 2,
                None,
            );
            cx.scene.push_rounded_rect(
                RectF::new(
                    rect.x + edge,
                    rect.y + edge,
                    (rect.width - edge * 2.0).max(0.0),
                    (rect.height - edge * 2.0).max(0.0),
                ),
                style.inner_edge,
                RoundedCorners::uniform((radius - edge).max(0.0)),
                LAYER_CONTENT - 1,
                None,
            );
            cx.scene.push_rounded_rect(
                body,
                bg_color,
                RoundedCorners::uniform((radius - inset).max(0.0)),
                LAYER_CONTENT,
                None,
            );
        }
        if aw > 0.0 {
            let left = (rect.right() - aw).max(body.x);
            let affordance = RectF::new(left, body.y, (body.right() - left).max(0.0), body.height);
            if affordance.width > 0.0 && affordance.height > 0.0 {
                let inner_radius = (radius - inset).max(0.0);
                cx.scene.begin_clip(affordance, None);
                cx.scene.push_rounded_rect(
                    RectF::new(
                        left - inner_radius,
                        body.y,
                        affordance.width + inner_radius,
                        body.height,
                    ),
                    bg_color.lerp(style.affordance, 0.25),
                    RoundedCorners::uniform(inner_radius),
                    LAYER_CONTENT + 1,
                    None,
                );
                cx.scene.end_clip();
            }
        }

        // ── Content area (left of affordance) ──
        let content_width = (rect.width - aw).max(0.0);
        let mut chip_rects_out: Vec<ChipRect> = Vec::new();
        cx.scene.begin_clip(
            RectF::new(
                rect.x + text_padding,
                body.y,
                (content_width - 2.0 * text_padding).max(0.0),
                body.height,
            ),
            None,
        );

        let selection_ref = if self.popup_open {
            &self.working_order
        } else {
            &self.committed_order
        };

        let show_placeholder = selection_ref.is_empty()
            && !self.in_capture
            && self.tag_creation_mode == TagCreationMode::SelectOnly;

        if show_placeholder {
            if !self.placeholder.is_empty() {
                let run = cx.text_engine.shape(&self.placeholder, font_size);
                let baseline_y = rect.y
                    + crate::core::text_engine::compute_control_text_baseline_y(
                        rect.height,
                        run.ascent,
                        run.descent,
                        t.typography.baseline_offset,
                    );
                let text_y = baseline_y - run.ascent;
                cx.scene.push_text_with_backgrounds(
                    &run,
                    rect.x + text_padding,
                    text_y,
                    t.colors.typography.muted,
                    vec![],
                    text_layer,
                    None,
                );
            }
        } else {
            match self.display_mode {
                TagDisplayMode::Chips => {
                    let chip_h = ms.chip_height.min(body.height);
                    let chip_y = rect.y + (rect.height - chip_h) / 2.0;
                    let mut chip_x = rect.x + text_padding;
                    let overflow_count =
                        selection_ref.len().saturating_sub(self.visible_chip_count);

                    for entry in selection_ref.iter().take(self.visible_chip_count) {
                        let label = self.label_for_entry(entry);
                        let label_run = cx.text_engine.shape(&label, font_size);
                        let close_w = ms.chip_close_size;
                        let chip_w =
                            ms.chip_padding_x + label_run.width + ms.chip_padding_x + close_w;

                        let chip_rect = RectF::new(chip_x, chip_y, chip_w, chip_h);
                        let flash_active = self.chip_flash.as_ref().map_or(false, |(fe, start)| {
                            *fe == *entry && cx.frame_time_ms() - start < 200
                        });
                        let chip_radius = ms.chip_radius.max(0.0).min(chip_h * 0.5);
                        let chip_bg = if flash_active {
                            t.colors.semantic.error
                        } else {
                            bg_color.lerp(style.affordance, 0.45)
                        };
                        cx.scene.push_gradient_rect(
                            chip_rect,
                            chip_radius,
                            &GradientDef {
                                kind: GradientKind::Linear,
                                stops: vec![
                                    ColorStop {
                                        offset: 0.0,
                                        color: chip_bg.lighten(0.04),
                                    },
                                    ColorStop {
                                        offset: 1.0,
                                        color: chip_bg.darken(0.04),
                                    },
                                ],
                                p0: (0.0, 0.0),
                                p1: (0.0, 1.0),
                            },
                            chip_layer,
                            None,
                        );
                        cx.scene.push_border_rounded_rect(
                            chip_rect,
                            t.colors.borders.subtle,
                            cx.one_px(),
                            RoundedCorners::uniform(chip_radius),
                            detail_layer,
                            None,
                        );

                        let baseline_y = chip_y
                            + crate::core::text_engine::compute_control_text_baseline_y(
                                chip_h,
                                label_run.ascent,
                                label_run.descent,
                                t.typography.baseline_offset,
                            );
                        let label_y = baseline_y - label_run.ascent;
                        cx.scene.push_text_with_backgrounds(
                            &label_run,
                            chip_x + ms.chip_padding_x,
                            label_y,
                            t.colors.typography.primary,
                            vec![],
                            text_layer,
                            None,
                        );

                        // × close button
                        let close_x = chip_x + chip_w - close_w;
                        let close_rect = RectF::new(close_x, chip_y, close_w, chip_h);

                        // Hover feedback: circular background behind × when hovered
                        let close_hovered = self.hovered_close.as_ref() == Some(entry);
                        if close_hovered {
                            let bg_size = close_w.min(chip_h) - 2.0;
                            let bg_x = close_x + (close_w - bg_size) / 2.0;
                            let bg_y = chip_y + (chip_h - bg_size) / 2.0;
                            cx.scene.push_rounded_rect(
                                RectF::new(bg_x, bg_y, bg_size, bg_size),
                                t.colors.controls.background_hover,
                                RoundedCorners::uniform(bg_size / 2.0),
                                detail_layer + 1,
                                None,
                            );
                        }

                        let x_color = if close_hovered {
                            t.colors.typography.primary
                        } else {
                            t.colors.typography.muted
                        };

                        let x_run = cx
                            .text_engine
                            .shape("\u{00D7}", font_size.min(close_w) * 0.8);
                        let baseline_y = chip_y
                            + crate::core::text_engine::compute_control_text_baseline_y(
                                chip_h,
                                x_run.ascent,
                                x_run.descent,
                                t.typography.baseline_offset,
                            );
                        let x_text_y = baseline_y - x_run.ascent;
                        let x_text_x = close_x + (close_w - x_run.width) / 2.0;
                        cx.scene.push_text_with_backgrounds(
                            &x_run,
                            x_text_x,
                            x_text_y,
                            x_color,
                            vec![],
                            text_layer,
                            None,
                        );

                        let local_close_rect = RectF::new(
                            close_rect.x - rect.x,
                            close_rect.y - rect.y,
                            close_rect.width,
                            close_rect.height,
                        );
                        chip_rects_out.push(ChipRect {
                            entry: entry.clone(),
                            close_rect: local_close_rect,
                        });
                        chip_x += chip_w + ms.chip_gap;
                    }

                    // Overflow badge
                    if overflow_count > 0 {
                        let (badge_x, badge_width) = self
                            .overflow_badge_span
                            .expect("overflow badge requires Layout");
                        cx.scene.begin_clip(
                            RectF::new(rect.x + badge_x, body.y, badge_width, body.height),
                            None,
                        );
                        let badge_text = format!("+{}", overflow_count);
                        let badge_run = cx.text_engine.shape(&badge_text, font_size);
                        let baseline_y = chip_y
                            + crate::core::text_engine::compute_control_text_baseline_y(
                                chip_h,
                                badge_run.ascent,
                                badge_run.descent,
                                t.typography.baseline_offset,
                            );
                        let badge_y = baseline_y - badge_run.ascent;
                        cx.scene.push_text_with_backgrounds(
                            &badge_run,
                            rect.x + badge_x,
                            badge_y,
                            t.colors.typography.muted,
                            vec![],
                            text_layer,
                            None,
                        );
                        cx.scene.end_clip();
                    }

                    // ── Inline input — delegate to TextField (caret, selection, text) ──
                    let show_inline =
                        self.in_capture || self.tag_creation_mode != TagCreationMode::SelectOnly;
                    let input_text = self.inline_input.text();

                    if show_inline || !input_text.is_empty() {
                        // TextField adds its own padding_x internally.
                        // chip_x already includes text_padding, so subtract it
                        // to avoid double-padding.
                        let (start, width) = self
                            .inline_span
                            .expect("inline editor geometry requires Layout");
                        let tf_start = rect.x + start;
                        let _ = width; // The ordinary child traversal paints this exact Layout-owned region.

                        // Ghost completion (muted suffix after typed text)
                        if let Some(ref ghost) = self.ghost_completion {
                            let input_width = if input_text.is_empty() {
                                0.0
                            } else {
                                cx.text_engine.shape(input_text, font_size).width
                            };
                            let ghost_run = cx.text_engine.shape(ghost, font_size);
                            let ghost_x = tf_start + text_padding + input_width;
                            // Baseline-anchored to match the embedded TextField's text
                            // (text_field.rs paint): same control-text baseline math over
                            // the same rect, so the ghost suffix sits on the typed text's
                            // baseline instead of being naively block-centered.
                            let ghost_baseline_y = rect.y
                                + crate::core::text_engine::compute_control_text_baseline_y(
                                    rect.height,
                                    ghost_run.ascent,
                                    ghost_run.descent,
                                    t.typography.baseline_offset,
                                );
                            let ghost_y = ghost_baseline_y - ghost_run.ascent;
                            cx.scene.push_text_with_backgrounds(
                                &ghost_run,
                                ghost_x,
                                ghost_y,
                                t.colors.typography.muted,
                                vec![],
                                text_layer,
                                None,
                            );
                        }
                    }
                }
                TagDisplayMode::CountBadge => {
                    let text = format!("{} selected", selection_ref.len());
                    let run = cx.text_engine.shape(&text, font_size);
                    let baseline_y = rect.y
                        + crate::core::text_engine::compute_control_text_baseline_y(
                            rect.height,
                            run.ascent,
                            run.descent,
                            t.typography.baseline_offset,
                        );
                    let text_y = baseline_y - run.ascent;
                    cx.scene.push_text_with_backgrounds(
                        &run,
                        rect.x + text_padding,
                        text_y,
                        t.colors.typography.primary,
                        vec![],
                        text_layer,
                        None,
                    );
                }
                TagDisplayMode::Summary => {
                    let max_show = 2;
                    let labels: Vec<String> = selection_ref
                        .iter()
                        .take(max_show)
                        .map(|entry| self.label_for_entry(entry))
                        .collect();
                    let remaining = selection_ref.len().saturating_sub(max_show);
                    let text = if remaining > 0 {
                        format!("{} + {} more", labels.join(", "), remaining)
                    } else {
                        labels.join(", ")
                    };
                    let run = cx.text_engine.shape(&text, font_size);
                    let baseline_y = rect.y
                        + crate::core::text_engine::compute_control_text_baseline_y(
                            rect.height,
                            run.ascent,
                            run.descent,
                            t.typography.baseline_offset,
                        );
                    let text_y = baseline_y - run.ascent;
                    cx.scene.push_text_with_backgrounds(
                        &run,
                        rect.x + text_padding,
                        text_y,
                        t.colors.typography.primary,
                        vec![],
                        text_layer,
                        None,
                    );
                }
            }
        }

        cx.scene.end_clip();
        self.chip_rects.set(chip_rects_out);
        // The joined shell already owns the open input/popup perimeter.
        // Only the closed field needs this foreground border above its content.
        if joined.is_none() && edge > 0.0 {
            cx.scene.push_border_rounded_rect(
                rect,
                style.chrome.border_color,
                edge,
                RoundedCorners::uniform(radius),
                text_layer + 1,
                None,
            );
        }

        // ── Affordance area ── (hidden in FreeFormOnly)
        if self.tag_creation_mode == TagCreationMode::FreeFormOnly {
            return;
        }

        let icon_size = 14.0;
        let icon_slot = 22.0;
        let aff_right = rect.x + rect.width;
        let indicator_color = if matches!(
            self.policy,
            TagInputPolicy::Locked | TagInputPolicy::ReadOnly
        ) {
            t.colors.typography.muted
        } else {
            t.colors.typography.primary
        };

        // Hover/press background highlights for affordance regions
        let combo_aw = t.geometry.combo_box.affordance_width;

        if self.show_action_icons() {
            let sel_rect = RectF::new(
                aff_right - combo_aw - icon_slot * 2.0,
                rect.y + 2.0,
                icon_slot,
                rect.height - 4.0,
            );
            if self.pressed_icon == Some(AffordanceIcon::SelectAll) {
                cx.scene.push_rounded_rect(
                    sel_rect,
                    t.colors.controls.background_pressed,
                    RoundedCorners::uniform(2.0),
                    LAYER_CONTENT + 1,
                    None,
                );
            } else if self.hovered_icon == Some(AffordanceIcon::SelectAll) {
                cx.scene.push_rounded_rect(
                    sel_rect,
                    t.colors.controls.background_hover,
                    RoundedCorners::uniform(2.0),
                    LAYER_CONTENT + 1,
                    None,
                );
            }

            let clr_rect = RectF::new(
                aff_right - combo_aw - icon_slot,
                rect.y + 2.0,
                icon_slot,
                rect.height - 4.0,
            );
            if self.pressed_icon == Some(AffordanceIcon::ClearAll) {
                cx.scene.push_rounded_rect(
                    clr_rect,
                    t.colors.controls.background_pressed,
                    RoundedCorners::uniform(2.0),
                    LAYER_CONTENT + 1,
                    None,
                );
            } else if self.hovered_icon == Some(AffordanceIcon::ClearAll) {
                cx.scene.push_rounded_rect(
                    clr_rect,
                    t.colors.controls.background_hover,
                    RoundedCorners::uniform(2.0),
                    LAYER_CONTENT + 1,
                    None,
                );
            }
        }

        let dd_rect = RectF::new(
            aff_right - combo_aw,
            rect.y + 2.0,
            combo_aw,
            rect.height - 4.0,
        );
        if self.pressed_icon == Some(AffordanceIcon::Dropdown) {
            cx.scene.push_rounded_rect(
                dd_rect,
                t.colors.controls.background_pressed,
                RoundedCorners::uniform(2.0),
                LAYER_CONTENT + 1,
                None,
            );
        } else if self.hovered_icon == Some(AffordanceIcon::Dropdown) {
            cx.scene.push_rounded_rect(
                dd_rect,
                t.colors.controls.background_hover,
                RoundedCorners::uniform(2.0),
                LAYER_CONTENT + 1,
                None,
            );
        }

        // Dropdown affordance (rightmost, original bar indicator)
        let aff_x = aff_right - combo_aw;
        let aff_rect = RectF {
            x: aff_x,
            y: rect.y,
            width: combo_aw,
            height: rect.height,
        };
        let dd_color = match (self.pressed_icon, self.hovered_icon) {
            (Some(AffordanceIcon::Dropdown), _) => indicator_color.darken(0.3),
            (_, Some(AffordanceIcon::Dropdown)) => indicator_color.lighten(0.3),
            _ => indicator_color,
        };
        let bar_width = (aff_rect.width * 0.3).round().max(cx.one_px());
        let bar_height = cx.one_px();
        let bar_x = aff_rect.x + (aff_rect.width - bar_width) / 2.0;
        let bar_radius = RoundedCorners::uniform(0.5);
        if self.popup_open {
            let bar_y = aff_rect.y + (aff_rect.height - bar_height) / 2.0;
            cx.scene.push_rounded_rect(
                RectF::new(bar_x, bar_y, bar_width, bar_height),
                dd_color,
                bar_radius,
                text_layer,
                None,
            );
        } else {
            let gap = 3.0;
            let total_h = bar_height * 2.0 + gap;
            let start_y = aff_rect.y + (aff_rect.height - total_h) / 2.0;
            cx.scene.push_rounded_rect(
                RectF::new(bar_x, start_y, bar_width, bar_height),
                dd_color,
                bar_radius,
                text_layer,
                None,
            );
            cx.scene.push_rounded_rect(
                RectF::new(bar_x, start_y + bar_height + gap, bar_width, bar_height),
                dd_color,
                bar_radius,
                text_layer,
                None,
            );
        }

        // Select All + Clear All icons (only when threshold met)
        if self.show_action_icons() {
            let sel_base = t.colors.accents.primary;
            let sel_color = match (self.pressed_icon, self.hovered_icon) {
                (Some(AffordanceIcon::SelectAll), _) => sel_base.darken(0.3),
                (_, Some(AffordanceIcon::SelectAll)) => sel_base.lighten(0.3),
                _ => sel_base,
            };

            let clr_base = t.colors.semantic.error;
            let clr_color = match (self.pressed_icon, self.hovered_icon) {
                (Some(AffordanceIcon::ClearAll), _) => clr_base.darken(0.3),
                (_, Some(AffordanceIcon::ClearAll)) => clr_base.lighten(0.3),
                _ => clr_base,
            };

            let clr_x = aff_right - combo_aw - icon_slot + (icon_slot - icon_size) / 2.0;
            let clr_y = rect.y + (rect.height - icon_size) / 2.0;
            if let Some(def) = cx.icon_registry.get(IconId::CLEAR_ALL) {
                cx.scene
                    .push_icon(def, clr_x, clr_y, icon_size, clr_color, text_layer);
            }

            let sel_x = aff_right - combo_aw - icon_slot * 2.0 + (icon_slot - icon_size) / 2.0;
            let sel_y = rect.y + (rect.height - icon_size) / 2.0;
            if let Some(def) = cx.icon_registry.get(IconId::SELECT_ALL) {
                cx.scene
                    .push_icon(def, sel_x, sel_y, icon_size, sel_color, text_layer);
            }

            let sep_x = aff_right - combo_aw - icon_slot * 2.0 - 1.0;
            cx.scene.push_rect(
                RectF::new(sep_x, rect.y + 4.0, 1.0, rect.height - 8.0),
                t.colors.borders.subtle,
                text_layer,
                None,
            );
        }
    }

    fn paint_overlay(&self, cx: &mut PaintContext, rect: RectF, tree_widget_id: TreeWidgetId) {
        let Some(hit_geometry) = self.popup_geometry(rect) else {
            return;
        };
        let visible_rows = self.visible_rows().max(1);

        let t = cx.theme;
        let font_size = t.typography.font_size;
        let text_padding = t.geometry.text_field.padding_x;
        let ms = &t.geometry.tag_input;
        let list_colors = &t.colors.list;

        let popup_body_height = visible_rows as f32 * ROW_HEIGHT;

        let popup_rect = hit_geometry.popup;

        // Backdrop
        cx.register_overlay_hit_region(OverlayHitRegion {
            abs_rect: RectF::new(0.0, 0.0, 100_000.0, 100_000.0),
            owner_abs_rect: rect,
            widget_id: tree_widget_id,
        });

        // Popup region
        cx.register_overlay_hit_region(OverlayHitRegion {
            abs_rect: popup_rect,
            owner_abs_rect: rect,
            widget_id: tree_widget_id,
        });

        let state = if cx.has_capture() {
            ControlChromeState::Captured
        } else if cx.has_focus_cue() {
            ControlChromeState::Focused
        } else {
            ControlChromeState::Idle
        };
        let style = t.tag_input_chrome(state, self.validation_error);
        let radius = t.chrome_radius(ControlChromeCategory::RecessedEditable, ms.radius, false);
        let edge = if cx.has_capture() || cx.has_focus_cue() {
            (style.chrome.border_width * cx.scale_factor).round().max(1.0) / cx.scale_factor
        } else {
            cx.one_px()
        };
        let geometry = hit_geometry.snapped(cx.scale_factor);
        let corners = geometry.corners(geometry.popup, radius, edge);
        let popup_background = t.colors.surfaces.content;
        cx.scene.begin_clip(geometry.popup, Some(corners));
        cx.scene.push_path(
            geometry.fill_outline(radius, edge),
            popup_background,
            LAYER_OVERLAY,
            None,
        );

        // Item rows
        let row_base_y = popup_rect.y;
        if self.popup_rows.is_empty() {
            let label = "No matches";
            let run = cx.text_engine.shape(label, font_size);
            let text_x = popup_rect.x + text_padding;
            let text_y = row_base_y
                + crate::core::text_engine::compute_control_text_baseline_y(
                    ROW_HEIGHT,
                    run.ascent,
                    run.descent,
                    t.typography.baseline_offset,
                )
                - run.ascent;
            let bgs = vec![TextBackgroundSpan {
                x_offset: -text_padding,
                y_offset: -(text_y - row_base_y),
                width: popup_rect.width,
                height: ROW_HEIGHT,
                color: popup_background,
            }];
            cx.scene.push_text_with_backgrounds(
                &run,
                text_x,
                text_y,
                t.colors.typography.muted,
                bgs,
                POPUP_TEXT_LAYER,
                None,
            );
        } else {
            for i in 0..visible_rows {
                let display_idx = self.popup_scroll_row + i;
                if display_idx >= self.popup_rows.len() {
                    break;
                }
                let row_y = row_base_y + i as f32 * ROW_HEIGHT;

                match &self.popup_rows[display_idx] {
                    PopupRow::GroupHeader(group_name) => {
                        let run = cx.text_engine.shape(group_name, font_size);
                        let text_x = popup_rect.x + text_padding;
                        let text_y = row_y
                            + crate::core::text_engine::compute_control_text_baseline_y(
                                ROW_HEIGHT,
                                run.ascent,
                                run.descent,
                                t.typography.baseline_offset,
                            )
                            - run.ascent;
                        let bgs = vec![TextBackgroundSpan {
                            x_offset: -text_padding,
                            y_offset: -(text_y - row_y),
                            width: popup_rect.width,
                            height: ROW_HEIGHT,
                            color: t.colors.surfaces.level1,
                        }];
                        cx.scene.push_text_with_backgrounds(
                            &run,
                            text_x,
                            text_y,
                            t.colors.typography.muted,
                            bgs,
                            POPUP_TEXT_LAYER,
                            None,
                        );
                    }
                    PopupRow::Item(id) => {
                        let item = self.items.iter().find(|it| it.id == *id);
                        let label = item.map(|it| it.label.as_str()).unwrap_or("?");
                        let enabled = item.map(|it| it.enabled).unwrap_or(false);
                        let checked = self.working_set.contains(&TagEntry::Item(*id));
                        let item_index = self.filtered_ids.iter().position(|fid| fid == id);
                        let highlighted =
                            item_index.is_some() && self.highlighted_index == item_index;

                        let bg_color = if highlighted && enabled {
                            style.affordance
                        } else {
                            popup_background
                        };

                        let at_limit = self
                            .max_selections
                            .map(|m| self.working_set.len() >= m)
                            .unwrap_or(false);

                        let label_run = cx.text_engine.shape(label, font_size);
                        let text_x = popup_rect.x + text_padding;
                        let text_y = row_y
                            + crate::core::text_engine::compute_control_text_baseline_y(
                                ROW_HEIGHT,
                                label_run.ascent,
                                label_run.descent,
                                t.typography.baseline_offset,
                            )
                            - label_run.ascent;
                        let check_size = ms
                            .checkbox_size
                            .max(0.0)
                            .min(ROW_HEIGHT - 2.0 * cx.one_px());
                        let check_rect = RectF::new(
                            text_x,
                            row_y + (ROW_HEIGHT - check_size) / 2.0,
                            check_size,
                            check_size,
                        );
                        let label_x = check_rect.right() + ms.checkbox_gap;

                        let check_color = if !enabled || (at_limit && !checked) {
                            t.colors.typography.muted
                        } else if checked {
                            t.colors.accents.primary
                        } else {
                            t.colors.borders.default
                        };

                        let bgs = vec![TextBackgroundSpan {
                            x_offset: popup_rect.x - label_x,
                            y_offset: -(text_y - row_y),
                            width: popup_rect.width,
                            height: ROW_HEIGHT,
                            color: bg_color,
                        }];

                        let label_color = if !enabled || (at_limit && !checked) {
                            t.colors.typography.muted
                        } else {
                            list_colors.text
                        };

                        cx.scene.push_text_with_backgrounds(
                            &label_run,
                            label_x,
                            text_y,
                            label_color,
                            bgs,
                            POPUP_TEXT_LAYER,
                            None,
                        );

                        // Keep checkbox state independent of the label font's symbol coverage.
                        let border_width = cx.one_px();
                        let check_radius = t.geometry.checkbox.radius.min(check_size * 0.5);
                        if checked {
                            cx.scene.push_rounded_rect(
                                check_rect,
                                check_color,
                                RoundedCorners::uniform(check_radius),
                                POPUP_TEXT_LAYER + 1,
                                None,
                            );
                        }
                        cx.scene.push_border_rounded_rect(
                            check_rect,
                            check_color,
                            border_width,
                            RoundedCorners::uniform(check_radius),
                            POPUP_TEXT_LAYER + 1,
                            None,
                        );
                        if checked {
                            if let Some(icon) = cx.icon_registry.get(IconId::CHECK) {
                                cx.scene.push_icon(
                                    icon,
                                    check_rect.x,
                                    check_rect.y,
                                    check_size,
                                    t.colors.typography.inverse,
                                    POPUP_TEXT_LAYER + 2,
                                );
                            }
                        }
                    }
                }
            }
        }

        // Scrollbar
        if self.needs_scrollbar() {
            let content = self.popup_rows.len() as f32 * ROW_HEIGHT;
            let viewport = visible_rows as f32 * ROW_HEIGHT;
            let scroll_px = self.popup_scroll_row as f32 * ROW_HEIGHT;
            let border = 1.0;
            let inset_w = popup_rect.width - border;
            let inset_h = popup_body_height - 2.0 * border;
            let local_rect = RectF::new(0.0, 0.0, inset_w, inset_h);
            let sb_layout = self
                .scrollbar_v
                .layout(local_rect, inset_h, content, viewport, scroll_px);

            let sb_offset_y = popup_rect.y + border;
            cx.scene.push_rect(
                RectF::new(
                    popup_rect.x + sb_layout.track.x,
                    sb_offset_y + sb_layout.track.y,
                    sb_layout.track.width,
                    sb_layout.track.height,
                ),
                t.colors.scrollbars.track,
                LAYER_OVERLAY + 3,
                None,
            );
            cx.scene.push_rect(
                RectF::new(
                    popup_rect.x + sb_layout.thumb.x,
                    sb_offset_y + sb_layout.thumb.y,
                    sb_layout.thumb.width,
                    sb_layout.thumb.height,
                ),
                self.scrollbar_v.thumb_color(&t.colors),
                LAYER_OVERLAY + 4,
                None,
            );
        }
        cx.scene.end_clip();
        cx.scene.begin_clip(geometry.popup, None);
        cx.scene.push_stroked_path(
            geometry.outline(radius, edge),
            StrokeParams {
                width: edge.min(geometry.popup.height),
                ..Default::default()
            },
            style.chrome.border_color,
            LAYER_OVERLAY + 5,
            None,
        );
        if self.validation_error {
            let inner_edge = cx.one_px();
            cx.scene.push_stroked_path(
                geometry.outline(radius, edge * 2.0 + inner_edge),
                StrokeParams {
                    width: inner_edge,
                    ..Default::default()
                },
                style.inner_edge,
                LAYER_OVERLAY + 6,
                None,
            );
        }
        cx.scene.end_clip();
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        // Scrollbar drag continuation
        if self.scrollbar_v.dragging {
            if let Event::Pointer(pe) = event {
                match pe.kind {
                    PointerEventKind::Move => {
                        let (_, global_y) = pe.global_pos;
                        let scroll_px = self.scrollbar_v.update_drag(global_y);
                        let max_scroll_rows =
                            self.filtered_ids.len().saturating_sub(self.visible_rows());
                        self.popup_scroll_row = (scroll_px / ROW_HEIGHT)
                            .round()
                            .max(0.0)
                            .min(max_scroll_rows as f32)
                            as usize;
                        return true;
                    }
                    PointerEventKind::Up => {
                        self.scrollbar_v.end_drag();
                        return true;
                    }
                    _ => {}
                }
            }
        }

        // Update frame time for chip flash
        self.last_frame_ms = cx.frame_time_ms();
        if let Some((_, start_ms)) = &self.chip_flash {
            if self.last_frame_ms - start_ms >= 200 {
                self.chip_flash = None;
            }
        }

        if phase != EventPhase::Target {
            return false;
        }

        if self.policy == TagInputPolicy::Full {
            if let Event::Pointer(pe) = event {
                if self.inline_cursor_rect.is_some_and(|rect| {
                    pe.local_pos.0 >= rect.x
                        && pe.local_pos.0 < rect.right()
                        && pe.local_pos.1 >= rect.y
                        && pe.local_pos.1 < rect.bottom()
                }) {
                    cx.request_cursor(crate::core::event::CursorIcon::Text);
                }
            }
        }

        // ── Captured: all keyboard input routed here ──
        if self.in_capture {
            match event {
                Event::Key(ke) if ke.is_down => {
                    // Activity and the exact current draft settle before host commands.
                    return self.request_inline(
                        cx,
                        CompositeEditOperation::ObserveAndActivity,
                        CompositeTagContinuation::Key { code: ke.key_code },
                    );
                }
                Event::Text(te) => {
                    let has_delimiter = (self.trigger_keys.comma && te.text.contains(','))
                        || (self.trigger_keys.tab && te.text.contains('\t'));
                    if has_delimiter && te.text.len() > 1 {
                        return self.request_inline(
                            cx,
                            CompositeEditOperation::ObserveAndActivity,
                            CompositeTagContinuation::Split {
                                text: te.text.clone(),
                            },
                        );
                    }
                    if self.trigger_keys.comma && te.text == "," {
                        return self.request_inline(
                            cx,
                            CompositeEditOperation::ObserveAndActivity,
                            CompositeTagContinuation::Create { refilter: false },
                        );
                    }
                    return self.request_inline(
                        cx,
                        CompositeEditOperation::ForwardCurrentInput,
                        CompositeTagContinuation::Input {
                            refilter: true,
                            clear_validation: true,
                            force_handled: true,
                        },
                    );
                }
                _ => {}
            }
        }

        // ── Pointer events (captured and uncaptured) ──
        match event {
            Event::Pointer(pe) => match pe.kind {
                PointerEventKind::Down => {
                    if pe.button != Some(PointerButton::Primary) {
                        return false;
                    }
                    let (local_x, local_y) = pe.local_pos;

                    // Dismiss popup on outside click
                    if self.popup_open {
                        let popup_height = self.visible_rows().max(1) as f32 * ROW_HEIGHT;
                        let in_bounds = local_x >= 0.0
                            && local_x < self.last_layout_width
                            && local_y >= 0.0
                            && local_y < self.control_height + popup_height;
                        if !in_bounds {
                            self.close_popup_commit();
                            if self.tag_creation_mode == TagCreationMode::SelectOnly {
                                cx.release_keyboard_capture();
                            }
                            return true;
                        }
                    }

                    if local_y < self.control_height {
                        // Affordance icon hit test
                        if let Some(icon) = self.hit_test_affordance(local_x, local_y) {
                            self.pressed_icon = Some(icon);
                            match icon {
                                AffordanceIcon::SelectAll => self.select_all_enabled(),
                                AffordanceIcon::ClearAll => self.clear_all(),
                                AffordanceIcon::Dropdown => {
                                    if !self.in_capture {
                                        cx.request_keyboard_capture();
                                    }
                                    if self.popup_open {
                                        self.close_popup_commit();
                                    } else {
                                        self.open_popup();
                                    }
                                }
                            }
                            cx.request_focus();
                            return true;
                        }

                        // Check chip × hit
                        if self.can_interact() && matches!(self.display_mode, TagDisplayMode::Chips)
                        {
                            let rects = self.chip_rects.take();
                            let hit = rects
                                .iter()
                                .find(|cr| cr.close_rect.contains(Point::new(local_x, local_y)))
                                .map(|cr| cr.entry.clone());
                            self.chip_rects.set(rects);
                            if let Some(entry) = hit {
                                self.remove_chip(&entry);
                                return true;
                            }
                        }
                        // Enter capture
                        if self.can_interact() {
                            cx.request_keyboard_capture();
                        }
                        cx.request_focus();
                        return true;
                    } else if self.popup_open {
                        // Click in popup area — scrollbar check
                        if self.needs_scrollbar() {
                            let visible = self.visible_rows();
                            let content = self.filtered_ids.len() as f32 * ROW_HEIGHT;
                            let viewport = visible as f32 * ROW_HEIGHT;
                            let scroll_px = self.popup_scroll_row as f32 * ROW_HEIGHT;
                            let border = 1.0;
                            let inset_w = self.last_layout_width - border;
                            let inset_h = viewport - 2.0 * border;
                            let local_rect = RectF::new(0.0, 0.0, inset_w, inset_h);
                            let sb_layout = self
                                .scrollbar_v
                                .layout(local_rect, inset_h, content, viewport, scroll_px);
                            let popup_local =
                                Point::new(local_x, local_y - self.control_height - border);

                            if sb_layout.hit_rect.contains(popup_local) {
                                let max_scroll_rows =
                                    self.filtered_ids.len().saturating_sub(visible);
                                let max_scroll_px = content - viewport;
                                if sb_layout.thumb.contains(popup_local) {
                                    let global_rect = cx
                                        .widget_global_rect()
                                        .unwrap_or(RectF::new(0.0, 0.0, 0.0, 0.0));
                                    let popup_global_y =
                                        global_rect.y + self.control_height + border;
                                    let track_top = popup_global_y + sb_layout.track.y;
                                    let track_bottom = track_top + sb_layout.track.height;
                                    let (_, global_y) = pe.global_pos;
                                    self.scrollbar_v.begin_drag(
                                        global_y,
                                        scroll_px,
                                        track_top,
                                        track_bottom,
                                        sb_layout.thumb.height,
                                        max_scroll_px,
                                    );
                                    if let Some(button) = pe.button {
                                        cx.capture_pointer(button);
                                    }
                                } else {
                                    if popup_local.y < sb_layout.thumb.y {
                                        self.popup_scroll_row =
                                            self.popup_scroll_row.saturating_sub(visible);
                                    } else {
                                        self.popup_scroll_row =
                                            (self.popup_scroll_row + visible).min(max_scroll_rows);
                                    }
                                }
                                return true;
                            }
                        }

                        // Row hit — toggle item (preserve filter + highlight)
                        if let Some(item_index) = self.popup_row_hit(local_x, local_y) {
                            let id = self.filtered_ids[item_index];
                            self.toggle_item(id);
                            self.highlighted_index = Some(item_index);
                            return true;
                        }
                        return true;
                    }
                    false
                }
                PointerEventKind::Move => {
                    let (local_x, local_y) = pe.local_pos;
                    // Affordance icon hover tracking
                    if local_y >= 0.0 && local_y < self.control_height {
                        let new_hover = self.hit_test_affordance(local_x, local_y);
                        if new_hover != self.hovered_icon {
                            self.hovered_icon = new_hover;
                            return true;
                        }

                        // Chip × hover tracking
                        if self.can_interact() && matches!(self.display_mode, TagDisplayMode::Chips)
                        {
                            let rects = self.chip_rects.take();
                            let new_close_hover = rects
                                .iter()
                                .find(|cr| cr.close_rect.contains(Point::new(local_x, local_y)))
                                .map(|cr| cr.entry.clone());
                            self.chip_rects.set(rects);
                            if new_close_hover != self.hovered_close {
                                self.hovered_close = new_close_hover;
                                return true;
                            }
                        }
                    } else {
                        let changed = self.hovered_icon.is_some() || self.hovered_close.is_some();
                        self.hovered_icon = None;
                        self.hovered_close = None;
                        if changed {
                            return true;
                        }
                    }
                    // Popup item hover
                    if self.popup_open {
                        if let Some(item_index) = self.popup_row_hit(local_x, local_y) {
                            if self.highlighted_index != Some(item_index) {
                                self.highlighted_index = Some(item_index);
                                self.recompute_ghost();
                            }
                            return true;
                        }
                    }
                    false
                }
                PointerEventKind::Up => {
                    if self.pressed_icon.is_some() {
                        self.pressed_icon = None;
                        return true;
                    }
                    false
                }
                PointerEventKind::Scroll { delta_y, .. } if self.popup_open => {
                    let max_scroll = self.max_scroll_row();
                    let rows = (-delta_y / ROW_HEIGHT).round() as isize;
                    let rows = if rows == 0 && delta_y != 0.0 {
                        if delta_y > 0.0 { -1 } else { 1 }
                    } else {
                        rows
                    };
                    self.popup_scroll_row = (self.popup_scroll_row as isize + rows)
                        .max(0)
                        .min(max_scroll as isize)
                        as usize;
                    true
                }
                _ => false,
            },
            // Not captured, keyboard: Enter/Space enters capture
            Event::Key(ke) if ke.is_down && !self.in_capture => {
                if matches!(ke.key_code, KeyCode::Enter | KeyCode::Space) && self.can_interact() {
                    cx.request_keyboard_capture();
                    cx.request_focus();
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        self.focused = true;
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.focused = false;
        // Do NOT set in_capture = false or close popup here.
        // Framework calls on_capture_leave separately for real capture exit.
    }

    fn on_hover_leave(&mut self, _cx: &mut EventContext) {
        if self.hovered_icon.is_some()
            || self.pressed_icon.is_some()
            || self.hovered_close.is_some()
        {
            self.hovered_icon = None;
            self.pressed_icon = None;
            self.hovered_close = None;
        }
    }

    fn focusable(&self) -> bool {
        !matches!(self.policy, TagInputPolicy::ReadOnly)
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn needs_animation(&self) -> bool {
        self.chip_flash.is_some()
    }

    fn needs_update(&self) -> bool {
        self.inline_layout_dirty
    }

    fn update(&mut self, _dt: f32, cx: &mut crate::core::widget::UpdateContext) {
        if self.inline_layout_dirty {
            cx.invalidate_layout();
        }
    }

    fn wants_scroll(&self) -> bool {
        self.popup_open
    }

    fn capture_eligible(&self) -> bool {
        self.can_interact()
    }

    fn on_capture_enter(&mut self, _cx: &mut EventContext) {
        self.inline_layout_dirty = true;
        self.in_capture = true;
        self.request_inline(
            _cx,
            CompositeEditOperation::SeedForEntry {
                text: self.inline_input.text().to_owned(),
                select_all: false,
            },
            CompositeTagContinuation::Entry,
        );
    }

    fn on_capture_leave(&mut self, _cx: &mut EventContext) {
        self.inline_layout_dirty = true;
        self.in_capture = false;
        self.request_inline(
            _cx,
            CompositeEditOperation::RetireDelegation,
            CompositeTagContinuation::Retire { handled: true },
        );
        if self.popup_open {
            self.close_popup_commit();
        }
        self.set_ghost_completion(None);
    }

    fn debug_type_name(&self) -> &'static str {
        "TagInput"
    }
}

#[cfg(test)]
mod pointer_hit_tests {
    use super::*;
    use crate::core::app::RetainedSceneTestHarness;
    use crate::core::event::{
        FusedInputEvent, InteractionDomains, Modifiers, PhysicalKeyCode, PointerButtonsState,
        PointerDevice, WindowId,
    };
    use crate::core::event_router::EventRouter;
    use crate::core::icon::IconRegistry;
    use crate::core::layout::EdgeInsets;
    use crate::core::theme::Theme;
    use crate::core::widget_tree::WidgetTree;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::runtime::binding_registry::BindingRegistry;
    use crate::widgets::flex::Flex;
    use std::time::Instant;

    #[test]
    fn dropdown_pointer_bounds_tag_preserves_keyboard_and_ghost() {
        fn tag(tree: &WidgetTree, node: NodeId) -> &TagInput {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }
        fn pointer(kind: PointerEventKind, pos: Point) -> FusedInputEvent {
            match kind {
                PointerEventKind::Down => FusedInputEvent::PointerDown {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    pos,
                    device: PointerDevice::Mouse,
                    button: PointerButton::Primary,
                    click_count: 1,
                },
                PointerEventKind::Up => FusedInputEvent::PointerUp {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    pos,
                    device: PointerDevice::Mouse,
                    button: PointerButton::Primary,
                },
                PointerEventKind::Move => FusedInputEvent::PointerMove {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    pos,
                    delta: (0.0, 0.0),
                    device: PointerDevice::Mouse,
                    buttons: PointerButtonsState::default(),
                },
                _ => unreachable!(),
            }
        }

        let mut control = TagInput::new().with_items(
            (0..10)
                .map(|i| TagItem {
                    id: i + 1,
                    label: format!("Alpine {i}"),
                    group: Some("Group".into()),
                    enabled: i != 2,
                })
                .collect(),
        );
        control.open_popup();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(Flex::column().with_padding(EdgeInsets {
                top: 12.0,
                left: 40.0,
                right: 80.0,
                bottom: 0.0,
            })),
            None,
            0,
        );
        let node = tree.add_child(root, Box::new(control), None, 0);
        let mut engine = FontManagerTextEngine::new();
        RetainedSceneTestHarness::new().frame(
            &mut tree,
            root,
            Constraints::tight(Size::new(360.0, 360.0)),
            &Theme::light(),
            &mut engine,
            &IconRegistry::new(&[]),
        );
        let rect = tree.get_absolute_paint_rect(node).unwrap();
        let popup = tag(&tree, node).popup_geometry(rect).unwrap().popup;
        let mut router = EventRouter::new();
        let mut domains = InteractionDomains::new();
        let mut bindings = BindingRegistry::new();
        let mut send = |tree: &mut WidgetTree, event| {
            router.route_input_event(
                tree,
                event,
                Some(root),
                &mut engine,
                None,
                domains.primary_state_mut(),
                &mut bindings,
            );
        };
        let input = Point::new(rect.x + 3.0, rect.y + rect.height * 0.5);
        send(&mut tree, pointer(PointerEventKind::Down, input));
        send(&mut tree, pointer(PointerEventKind::Up, input));
        send(
            &mut tree,
            FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code: KeyCode::Character('a'),
                physical_key: PhysicalKeyCode::KeyA,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            },
        );
        send(
            &mut tree,
            FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                text: "a".into(),
                modifiers: Modifiers::default(),
            },
        );
        send(
            &mut tree,
            FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code: KeyCode::Down,
                physical_key: PhysicalKeyCode::ArrowDown,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            },
        );
        assert_eq!(tag(&tree, node).highlighted_index, Some(1));
        assert_eq!(
            tag(&tree, node).ghost_completion.as_deref(),
            Some("lpine 1")
        );
        let target = Point::new(popup.x + 20.0, popup.y + ROW_HEIGHT * 4.5);
        for outside in [
            Point::new(popup.x - 1.0, target.y),
            Point::new(popup.right(), target.y),
            Point::new(target.x, popup.y - 1.0),
            Point::new(target.x, popup.bottom()),
        ] {
            send(&mut tree, pointer(PointerEventKind::Move, outside));
            assert_eq!(tag(&tree, node).highlighted_index, Some(1));
            assert_eq!(
                tag(&tree, node).ghost_completion.as_deref(),
                Some("lpine 1")
            );
            assert!(tag(&tree, node).committed_set.is_empty());
        }
        let header = Point::new(target.x, popup.y + ROW_HEIGHT * 0.5);
        send(&mut tree, pointer(PointerEventKind::Down, header));
        let disabled = Point::new(target.x, popup.y + ROW_HEIGHT * 3.5);
        send(&mut tree, pointer(PointerEventKind::Down, disabled));
        assert!(tag(&tree, node).committed_set.is_empty());
        send(&mut tree, pointer(PointerEventKind::Move, target));
        assert_eq!(tag(&tree, node).highlighted_index, Some(3));
        assert_eq!(
            tag(&tree, node).ghost_completion.as_deref(),
            Some("lpine 3")
        );
        send(&mut tree, pointer(PointerEventKind::Down, target));
        assert_eq!(tag(&tree, node).committed_ids(), vec![4]);
        send(
            &mut tree,
            pointer(PointerEventKind::Up, Point::new(popup.right(), target.y)),
        );
        assert_eq!(tag(&tree, node).committed_ids(), vec![4]);
        assert!(tag(&tree, node).popup_open);
        send(
            &mut tree,
            pointer(PointerEventKind::Down, Point::new(popup.right(), target.y)),
        );
        assert_eq!(tag(&tree, node).committed_ids(), vec![4]);
        assert!(!tag(&tree, node).popup_open);
    }
}
