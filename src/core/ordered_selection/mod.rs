// Ordered Selection Core — Reusable selection/focus/range/transaction semantics
//
// This module extracts the pure selection model from list_state.rs into a generic
// core that can be used by TabularSubstrate, TreeView, Grid, PropertyEditor, etc.
//
// Key design principles:
// - Generic over ItemKey (no assumption that key == index)
// - All index computations go through OrderedIndex trait (the "oracle")
// - No widget/UI policy (click-empty-space, hover, pressed, activation side effects)
// - Pure reducer pattern: action + state -> (state', result)
//
// SPEC lineage: SPEC-HAL-114, SPEC-HAL-135, SPEC-HAL-136, SPEC-HAL-142
//
// =============================================================================
// ARCHITECTURE INVARIANTS — READ BEFORE MODIFYING
// =============================================================================
//
// This module defines the SINGLE AUTHORITATIVE SOURCE for selection state.
// These invariants are PERMANENT and must not be violated.
//
// INV-SEL-1: SINGLE WRITE AUTHORITY
//   - OrderedSelectionState is the ONLY selection truth
//   - All writes go through reduce_ordered_selection() or direct field access
//   - No widget may maintain shadow selection state
//   - No consumer may bypass the reducer for selection mutations
//
// INV-SEL-2: REDUCER-DRIVEN MUTATIONS
//   - OrderedSelectionAction is the closed set of selection operations
//   - Reducer is pure: (state, action, oracle) → (state', result)
//   - Side effects (callbacks, UI updates) are external to the reducer
//
// INV-SEL-3: FORBIDDEN WIDGET ACTIONS
//   - Widgets MUST NOT mutate selection directly (use intents → reducer)
//   - Widgets MUST NOT cache selection (read from authoritative source)
//   - Widgets MUST NOT assume iteration order (HashSet is unordered)
//
// INV-SEL-4: IDENTITY-BASED SELECTION
//   - Selection is by ItemKey, not by index
//   - Selection survives sort/filter/scroll (identity-stable)
//   - Stale keys (deleted items) are tolerated until next mutation
//
// =============================================================================

use std::collections::HashSet;
use std::hash::Hash;

// ============================================================================
// OrderedIndex Trait — The Oracle
// ============================================================================
//
// Provides ordered access to items without assuming key == index.
// Implementations must be allocation-free for the trait methods.
// ============================================================================

/// Oracle for ordered item access.
///
/// This trait bridges the gap between identity-based keys and position-based
/// range computations. The implementor (e.g., TabularSubstrate) provides the
/// current ordering; the selection core queries it during reduce.
///
/// # Contract
/// - `len()` returns the current item count
/// - `key_at(index)` returns the key at position `index` (panics if out of bounds)
/// - `index_of(key)` returns the position of `key`, or None if not present
///
/// # Performance
/// Implementations should be O(1) or O(log n). No allocations inside trait methods.
pub trait OrderedIndex<ItemKey> {
    /// Number of items in the ordered collection.
    fn len(&self) -> usize;

    /// Returns true if the collection is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Key at the given index. Panics if index >= len().
    fn key_at(&self, index: usize) -> ItemKey;

    /// Index of the given key, or None if not present.
    fn index_of(&self, key: ItemKey) -> Option<usize>;
}

// ============================================================================
// Selection Mode
// ============================================================================

/// Selection mode for an ordered collection.
///
/// Mode is typically immutable at runtime per SPEC-HAL-114.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum SelectionMode {
    /// No selection allowed. Selection is always empty, focus is allowed.
    #[default]
    None,
    /// At most one selected item.
    Single,
    /// Multiple selected items allowed.
    Multi,
}

// ============================================================================
// Selection Interaction Model
// ============================================================================

/// Selection interaction model.
///
/// SPEC-HAL-136: Orthogonal to SelectionMode.
/// - Immediate: Click mutates selection directly
/// - Transactional: Working set with explicit commit/cancel
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum SelectionInteractionModel {
    /// SPEC-HAL-135: Selection mutates immediately on input.
    #[default]
    Immediate,
    /// SPEC-HAL-136: Selection is a working set, requires explicit commit.
    Transactional,
}

// ============================================================================
// Ordered Selection State
// ============================================================================

/// Core selection state for an ordered collection.
///
/// Generic over `ItemKey` which must be Copy + Eq + Hash.
///
/// This struct contains:
/// - Focus key (keyboard navigation position)
/// - Selected key set (which items are selected)
/// - Anchor keys (for range selection)
/// - Range session state (for Shift+* operations)
/// - Transaction state (working vs committed selection)
///
/// It does NOT contain:
/// - Visual state (hovered, pressed)
/// - Widget callbacks
/// - Activation side effects
/// - List-specific policy (click-empty-space behavior)
#[derive(Clone, Debug)]
pub struct OrderedSelectionState<ItemKey>
where
    ItemKey: Copy + Eq + Hash,
{
    // =========================================================================
    // Focus State
    // =========================================================================
    /// Current focus key (keyboard navigation position).
    /// None means no item is focused.
    pub focus_key: Option<ItemKey>,

    // =========================================================================
    // Selection State
    // =========================================================================
    /// Currently selected items.
    /// In Immediate mode: authoritative selection.
    /// In Transactional mode during transaction: working selection.
    pub selected_keys: HashSet<ItemKey>,

    /// Selection mode (None/Single/Multi).
    pub selection_mode: SelectionMode,

    // =========================================================================
    // Anchor State (for range selection)
    // =========================================================================
    /// Persistent anchor for range selection.
    /// Set by plain clicks/focus moves.
    pub anchor_key: Option<ItemKey>,

    /// Dedicated range anchor for Shift+* operations.
    /// Set on first range-modifying action, persists during range interaction.
    pub range_anchor_key: Option<ItemKey>,

    // =========================================================================
    // Range Session State
    // =========================================================================
    /// True when a Shift-range session is active.
    range_session_active: bool,

    /// Snapshot of selection at range session start.
    /// Combined with current range to produce `base ∪ range` on each update.
    range_base_selection: HashSet<ItemKey>,

    /// Last computed range (start_index, end_index) for debugging/testing.
    range_last_range: Option<(usize, usize)>,

    // =========================================================================
    // Transaction State (SPEC-HAL-136)
    // =========================================================================
    /// Selection interaction model.
    pub interaction_model: SelectionInteractionModel,

    /// Committed selection (immutable during active transaction).
    /// In Immediate mode: unused (always empty).
    committed_selection: HashSet<ItemKey>,

    /// True when a transaction is active.
    /// In Transactional mode, begin/commit/cancel control this flag.
    /// NOTE: Unlike list_state.rs, transactional mode does NOT imply always-active.
    transaction_active: bool,

    /// Config-time lock: true after first event processed.
    /// interaction_model is fixed after this is set.
    interaction_model_locked: bool,

    /// Disabled state. When true, all actions are ignored.
    pub disabled: bool,
}

impl<ItemKey> Default for OrderedSelectionState<ItemKey>
where
    ItemKey: Copy + Eq + Hash,
{
    fn default() -> Self {
        Self {
            focus_key: None,
            selected_keys: HashSet::new(),
            selection_mode: SelectionMode::None,
            anchor_key: None,
            range_anchor_key: None,
            range_session_active: false,
            range_base_selection: HashSet::new(),
            range_last_range: None,
            interaction_model: SelectionInteractionModel::Immediate,
            committed_selection: HashSet::new(),
            transaction_active: false,
            interaction_model_locked: false,
            disabled: false,
        }
    }
}

impl<ItemKey> OrderedSelectionState<ItemKey>
where
    ItemKey: Copy + Eq + Hash,
{
    /// Create a new state with the specified selection mode.
    pub fn new(selection_mode: SelectionMode) -> Self {
        Self {
            selection_mode,
            ..Default::default()
        }
    }

    // =========================================================================
    // Focus Queries
    // =========================================================================

    /// Get the focused key.
    pub fn focused_key(&self) -> Option<ItemKey> {
        self.focus_key
    }

    /// Check if a key is focused.
    pub fn is_focused(&self, key: ItemKey) -> bool {
        self.focus_key == Some(key)
    }

    // =========================================================================
    // Selection Queries
    // =========================================================================

    /// Check if a key is selected.
    pub fn is_selected(&self, key: ItemKey) -> bool {
        self.selected_keys.contains(&key)
    }

    /// Check if selection is empty.
    pub fn is_selection_empty(&self) -> bool {
        self.selected_keys.is_empty()
    }

    /// Get the number of selected items.
    pub fn selection_count(&self) -> usize {
        self.selected_keys.len()
    }

    /// Iterate over selected keys.
    pub fn selected_keys_iter(&self) -> impl Iterator<Item = &ItemKey> {
        self.selected_keys.iter()
    }

    // =========================================================================
    // Anchor Queries
    // =========================================================================

    /// Get the persistent anchor key.
    pub fn anchor_key(&self) -> Option<ItemKey> {
        self.anchor_key
    }

    /// Get the range anchor key (for Shift+* operations).
    pub fn range_anchor_key(&self) -> Option<ItemKey> {
        self.range_anchor_key
    }

    /// Clear the range anchor.
    pub fn clear_range_anchor(&mut self) {
        self.range_anchor_key = None;
    }

    // =========================================================================
    // Range Session Queries
    // =========================================================================

    /// Check if a range session is active.
    pub fn is_range_session_active(&self) -> bool {
        self.range_session_active
    }

    /// Get the last computed range (for testing/debugging).
    pub fn range_last_range(&self) -> Option<(usize, usize)> {
        self.range_last_range
    }

    // =========================================================================
    // Range Session Management (internal)
    // =========================================================================

    /// Begin a new range session.
    ///
    /// - Sets `range_session_active = true`
    /// - Snapshots current selection to `range_base_selection`
    /// - Sets `range_anchor_key` from the provided key
    fn begin_range_session(&mut self, anchor_key: ItemKey) {
        self.range_session_active = true;
        self.range_base_selection = self.selected_keys.clone();
        self.range_anchor_key = Some(anchor_key);
        if self.anchor_key.is_none() {
            self.anchor_key = Some(anchor_key);
        }
        self.range_last_range = None;
    }

    /// Apply the current range session with a new end key.
    ///
    /// Computes `selected_keys = base ∪ range(anchor..end)` for Multi mode.
    /// Returns true if selection changed.
    fn apply_range_session<O: OrderedIndex<ItemKey>>(
        &mut self,
        end_key: ItemKey,
        oracle: &O,
    ) -> bool {
        let anchor_key = match self.range_anchor_key {
            Some(k) => k,
            None => return false,
        };

        let anchor_index = match oracle.index_of(anchor_key) {
            Some(i) => i,
            None => return false, // Anchor no longer in collection
        };

        let end_index = match oracle.index_of(end_key) {
            Some(i) => i,
            None => return false, // End key not in collection
        };

        let item_count = oracle.len();
        if item_count == 0 {
            return false;
        }

        // Compute range bounds
        let start = anchor_index.min(end_index);
        let end = anchor_index
            .max(end_index)
            .min(item_count.saturating_sub(1));

        // Record for debugging
        self.range_last_range = Some((start, end));

        // Save old selection for change detection
        let old_selection = self.selected_keys.clone();

        match self.selection_mode {
            SelectionMode::None => {
                self.selected_keys.clear();
            }
            SelectionMode::Single => {
                // Single mode: range collapses to just the end key
                self.selected_keys.clear();
                self.selected_keys.insert(end_key);
            }
            SelectionMode::Multi => {
                // Multi mode: base ∪ range
                self.selected_keys = self.range_base_selection.clone();
                for i in start..=end {
                    self.selected_keys.insert(oracle.key_at(i));
                }
            }
        }

        self.selected_keys != old_selection
    }

    /// End the current range session.
    fn end_range_session(&mut self) {
        self.range_session_active = false;
        self.range_anchor_key = None;
        self.range_base_selection.clear();
        self.range_last_range = None;
    }

    /// Finalize a range gesture completely.
    ///
    /// Ends the range session and clears focus to prevent implicit extension.
    fn finalize_range_gesture(&mut self) {
        self.end_range_session();
        self.focus_key = None;
        // In transactional mode during active transaction, also clear anchor
        if self.is_transactional() && self.transaction_active {
            self.anchor_key = None;
        }
    }

    // =========================================================================
    // Transaction Queries
    // =========================================================================

    /// Check if transactional mode is configured.
    pub fn is_transactional(&self) -> bool {
        self.interaction_model == SelectionInteractionModel::Transactional
    }

    /// Check if a transaction is currently active.
    pub fn is_transaction_active(&self) -> bool {
        self.transaction_active
    }

    /// Get the working selection (current selection being edited).
    pub fn working_selection(&self) -> &HashSet<ItemKey> {
        &self.selected_keys
    }

    /// Get the committed selection.
    pub fn committed_selection(&self) -> &HashSet<ItemKey> {
        &self.committed_selection
    }

    // =========================================================================
    // Transaction Management
    // =========================================================================

    /// Begin a transactional selection session.
    ///
    /// Returns true if transaction was started, false if:
    /// - Already in a transaction (nested transactions forbidden)
    /// - Not in transactional mode
    pub fn begin_transaction(&mut self) -> bool {
        if self.transaction_active {
            return false; // No nested transactions
        }
        if self.interaction_model != SelectionInteractionModel::Transactional {
            return false;
        }

        self.committed_selection = self.selected_keys.clone();
        self.transaction_active = true;
        true
    }

    /// Commit the transactional selection session.
    ///
    /// Returns the committed selection snapshot, or None if no transaction was active.
    pub fn commit_transaction(&mut self) -> Option<HashSet<ItemKey>> {
        if !self.transaction_active {
            return None;
        }

        self.committed_selection = self.selected_keys.clone();
        self.transaction_active = false;
        self.end_range_session();

        Some(self.committed_selection.clone())
    }

    /// Cancel the transactional selection session.
    ///
    /// Restores working selection to committed selection.
    /// Returns true if a transaction was cancelled.
    pub fn cancel_transaction(&mut self) -> bool {
        if !self.transaction_active {
            return false;
        }

        self.selected_keys = self.committed_selection.clone();
        self.transaction_active = false;
        self.end_range_session();
        self.anchor_key = None;

        true
    }

    // =========================================================================
    // Interaction Model Configuration
    // =========================================================================

    /// Set the interaction model.
    ///
    /// This is a no-op if the model is already locked (after first event).
    /// Changing the model cancels any active transaction.
    pub fn set_interaction_model(&mut self, model: SelectionInteractionModel) {
        if self.interaction_model_locked {
            debug_assert!(false, "set_interaction_model() called after lock");
            return;
        }

        if self.interaction_model == model {
            return;
        }

        if self.transaction_active {
            self.cancel_transaction();
        }

        self.interaction_model = model;

        if model == SelectionInteractionModel::Immediate {
            self.committed_selection.clear();
        }
    }

    /// Lock the interaction model (called on first event).
    pub fn lock_interaction_model(&mut self) {
        self.interaction_model_locked = true;
    }

    /// Check if interaction model is locked.
    pub fn is_interaction_model_locked(&self) -> bool {
        self.interaction_model_locked
    }
}

// ============================================================================
// Ordered Selection Action — Closed Set
// ============================================================================

/// Semantic actions for ordered selection.
///
/// Actions are device-agnostic and encode intent, not input source.
/// All selection logic lives in the reducer.
///
/// NOTE: This is the generic action set. Widget-specific actions like
/// "ClickEmptySpace" or "Activate" are NOT included here — they belong
/// in widget-level adapters that emit these core actions.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OrderedSelectionAction<ItemKey>
where
    ItemKey: Copy + Eq + Hash,
{
    // =========================================================================
    // Focus Movement (no modifier)
    // =========================================================================
    /// Move focus to the previous item.
    FocusPrevious,
    /// Move focus to the next item.
    FocusNext,
    /// Move focus to the first item.
    FocusFirst,
    /// Move focus to the last item.
    FocusLast,
    /// Move focus by a page (negative = up, positive = down).
    FocusPage { delta: isize },

    // =========================================================================
    // Focus Movement with Range Extension (Shift)
    // =========================================================================
    /// Move focus to previous, extending selection from anchor.
    ExtendPrevious,
    /// Move focus to next, extending selection from anchor.
    ExtendNext,
    /// Move focus to first, extending selection from anchor.
    ExtendFirst,
    /// Move focus to last, extending selection from anchor.
    ExtendLast,
    /// Move focus by page, extending selection from anchor.
    ExtendPage { delta: isize },

    // =========================================================================
    // Focus-Only Movement (Ctrl+Arrow equivalent)
    // =========================================================================
    /// Move focus to previous without changing selection.
    MoveFocusPrevious,
    /// Move focus to next without changing selection.
    MoveFocusNext,
    /// Move focus to first without changing selection.
    MoveFocusFirst,
    /// Move focus to last without changing selection.
    MoveFocusLast,
    /// Move focus by page without changing selection.
    MoveFocusPage { delta: isize },

    // =========================================================================
    // Direct Selection
    // =========================================================================
    /// Select a single item (clears existing selection).
    Select { key: ItemKey },
    /// Toggle selection of an item (Ctrl+Click equivalent).
    Toggle { key: ItemKey },
    /// Extend selection from anchor to key (Shift+Click equivalent).
    ExtendTo { key: ItemKey },

    // =========================================================================
    // Drag Selection
    // =========================================================================
    /// Drag selection with explicit anchor.
    DragSelect { anchor: ItemKey, current: ItemKey },
    /// End a drag gesture.
    DragEnd,

    // =========================================================================
    // Focus-Based Actions
    // =========================================================================
    /// Toggle selection of the focused item (Space key).
    ToggleFocused,
    /// Set focus to a specific key without changing selection.
    FocusTo { key: ItemKey },

    // =========================================================================
    // Bulk Actions
    // =========================================================================
    /// Clear all selection.
    ClearSelection,
    /// Select all items.
    SelectAll,
}

// ============================================================================
// Reduce Result
// ============================================================================

/// Result of a reduce operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReduceResult<ItemKey>
where
    ItemKey: Copy + Eq + Hash,
{
    /// True if selection changed.
    pub selection_changed: bool,

    /// True if focus changed.
    pub focus_changed: bool,

    /// The newly focused key (if focus changed to a specific item).
    pub new_focus: Option<ItemKey>,
}

impl<ItemKey> Default for ReduceResult<ItemKey>
where
    ItemKey: Copy + Eq + Hash,
{
    fn default() -> Self {
        Self {
            selection_changed: false,
            focus_changed: false,
            new_focus: None,
        }
    }
}

// ============================================================================
// Core Reducer
// ============================================================================

/// Pure reducer for ordered selection state.
///
/// Mutates `state` synchronously based on `action`. Uses `oracle` for
/// index <-> key conversions during range computations.
///
/// Returns what changed (for callback dispatch).
pub fn reduce_ordered_selection<ItemKey, O>(
    state: &mut OrderedSelectionState<ItemKey>,
    action: OrderedSelectionAction<ItemKey>,
    oracle: &O,
) -> ReduceResult<ItemKey>
where
    ItemKey: Copy + Eq + Hash,
    O: OrderedIndex<ItemKey>,
{
    let mut result = ReduceResult::default();

    // Disabled state ignores all actions
    if state.disabled {
        return result;
    }

    let item_count = oracle.len();

    // Empty collection: most actions are no-op
    if item_count == 0 {
        if matches!(action, OrderedSelectionAction::ClearSelection) {
            state.end_range_session();
            let had_selection = !state.selected_keys.is_empty();
            state.selected_keys.clear();
            state.anchor_key = None;
            result.selection_changed = had_selection;
        }
        return result;
    }

    match action {
        // ====================================================================
        // Focus Movement (no modifier)
        // Terminates range session. Selection unchanged.
        // ====================================================================
        OrderedSelectionAction::FocusPrevious => {
            state.end_range_session();
            let new_focus = focus_previous(state, oracle);
            if let Some(key) = new_focus {
                let changed = state.focus_key != Some(key);
                state.focus_key = Some(key);
                state.anchor_key = Some(key);
                state.range_anchor_key = None;
                result.focus_changed = changed;
                result.new_focus = Some(key);
            }
        }

        OrderedSelectionAction::FocusNext => {
            state.end_range_session();
            let new_focus = focus_next(state, oracle);
            if let Some(key) = new_focus {
                let changed = state.focus_key != Some(key);
                state.focus_key = Some(key);
                state.anchor_key = Some(key);
                state.range_anchor_key = None;
                result.focus_changed = changed;
                result.new_focus = Some(key);
            }
        }

        OrderedSelectionAction::FocusFirst => {
            state.end_range_session();
            let key = oracle.key_at(0);
            let changed = state.focus_key != Some(key);
            state.focus_key = Some(key);
            state.anchor_key = Some(key);
            state.range_anchor_key = None;
            result.focus_changed = changed;
            result.new_focus = Some(key);
        }

        OrderedSelectionAction::FocusLast => {
            state.end_range_session();
            let key = oracle.key_at(item_count - 1);
            let changed = state.focus_key != Some(key);
            state.focus_key = Some(key);
            state.anchor_key = Some(key);
            state.range_anchor_key = None;
            result.focus_changed = changed;
            result.new_focus = Some(key);
        }

        OrderedSelectionAction::FocusPage { delta } => {
            state.end_range_session();
            let new_focus = focus_page(state, oracle, delta);
            if let Some(key) = new_focus {
                let changed = state.focus_key != Some(key);
                state.focus_key = Some(key);
                state.anchor_key = Some(key);
                state.range_anchor_key = None;
                result.focus_changed = changed;
                result.new_focus = Some(key);
            }
        }

        // ====================================================================
        // Focus Movement with Range Extension (Shift)
        // Starts/continues range session. Selection = base ∪ range.
        // ====================================================================
        OrderedSelectionAction::ExtendPrevious => {
            let current_index = state.focus_key.and_then(|k| oracle.index_of(k));
            let new_index = match current_index {
                Some(idx) if idx > 0 => idx - 1,
                Some(_) => 0,
                None => item_count - 1,
            };
            let new_key = oracle.key_at(new_index);

            if !state.range_session_active {
                let anchor_key = state.focus_key.unwrap_or(new_key);
                state.begin_range_session(anchor_key);
            }

            let changed = state.focus_key != Some(new_key);
            state.focus_key = Some(new_key);
            result.focus_changed = changed;
            result.new_focus = Some(new_key);
            result.selection_changed = state.apply_range_session(new_key, oracle);
        }

        OrderedSelectionAction::ExtendNext => {
            let current_index = state.focus_key.and_then(|k| oracle.index_of(k));
            let new_index = match current_index {
                Some(idx) if idx < item_count - 1 => idx + 1,
                Some(_) => item_count - 1,
                None => 0,
            };
            let new_key = oracle.key_at(new_index);

            if !state.range_session_active {
                let anchor_key = state.focus_key.unwrap_or(new_key);
                state.begin_range_session(anchor_key);
            }

            let changed = state.focus_key != Some(new_key);
            state.focus_key = Some(new_key);
            result.focus_changed = changed;
            result.new_focus = Some(new_key);
            result.selection_changed = state.apply_range_session(new_key, oracle);
        }

        OrderedSelectionAction::ExtendFirst => {
            let new_key = oracle.key_at(0);

            if !state.range_session_active {
                let anchor_key = state.focus_key.unwrap_or(new_key);
                state.begin_range_session(anchor_key);
            }

            let changed = state.focus_key != Some(new_key);
            state.focus_key = Some(new_key);
            result.focus_changed = changed;
            result.new_focus = Some(new_key);
            result.selection_changed = state.apply_range_session(new_key, oracle);
        }

        OrderedSelectionAction::ExtendLast => {
            let new_key = oracle.key_at(item_count - 1);

            if !state.range_session_active {
                let anchor_key = state.focus_key.unwrap_or(new_key);
                state.begin_range_session(anchor_key);
            }

            let changed = state.focus_key != Some(new_key);
            state.focus_key = Some(new_key);
            result.focus_changed = changed;
            result.new_focus = Some(new_key);
            result.selection_changed = state.apply_range_session(new_key, oracle);
        }

        OrderedSelectionAction::ExtendPage { delta } => {
            let new_focus = focus_page(state, oracle, delta);
            if let Some(new_key) = new_focus {
                if !state.range_session_active {
                    let anchor_key = state.focus_key.unwrap_or(new_key);
                    state.begin_range_session(anchor_key);
                }

                let changed = state.focus_key != Some(new_key);
                state.focus_key = Some(new_key);
                result.focus_changed = changed;
                result.new_focus = Some(new_key);
                result.selection_changed = state.apply_range_session(new_key, oracle);
            }
        }

        // ====================================================================
        // Focus-Only Movement (Ctrl+Arrow)
        // Terminates range session. Selection unchanged.
        // ====================================================================
        OrderedSelectionAction::MoveFocusPrevious => {
            state.end_range_session();
            let new_focus = focus_previous(state, oracle);
            if let Some(key) = new_focus {
                let changed = state.focus_key != Some(key);
                state.focus_key = Some(key);
                result.focus_changed = changed;
                result.new_focus = Some(key);
            }
        }

        OrderedSelectionAction::MoveFocusNext => {
            state.end_range_session();
            let new_focus = focus_next(state, oracle);
            if let Some(key) = new_focus {
                let changed = state.focus_key != Some(key);
                state.focus_key = Some(key);
                result.focus_changed = changed;
                result.new_focus = Some(key);
            }
        }

        OrderedSelectionAction::MoveFocusFirst => {
            state.end_range_session();
            let key = oracle.key_at(0);
            let changed = state.focus_key != Some(key);
            state.focus_key = Some(key);
            result.focus_changed = changed;
            result.new_focus = Some(key);
        }

        OrderedSelectionAction::MoveFocusLast => {
            state.end_range_session();
            let key = oracle.key_at(item_count - 1);
            let changed = state.focus_key != Some(key);
            state.focus_key = Some(key);
            result.focus_changed = changed;
            result.new_focus = Some(key);
        }

        OrderedSelectionAction::MoveFocusPage { delta } => {
            state.end_range_session();
            let new_focus = focus_page(state, oracle, delta);
            if let Some(key) = new_focus {
                let changed = state.focus_key != Some(key);
                state.focus_key = Some(key);
                result.focus_changed = changed;
                result.new_focus = Some(key);
            }
        }

        // ====================================================================
        // Direct Selection
        // ====================================================================
        OrderedSelectionAction::Select { key } => {
            if oracle.index_of(key).is_some() {
                state.end_range_session();
                state.focus_key = Some(key);
                result.focus_changed = true;
                result.new_focus = Some(key);
                result.selection_changed = apply_single_selection(state, key);
            }
        }

        OrderedSelectionAction::Toggle { key } => {
            if oracle.index_of(key).is_some() {
                state.end_range_session();
                state.focus_key = Some(key);
                result.focus_changed = true;
                result.new_focus = Some(key);
                result.selection_changed = apply_toggle_selection(state, key);
            }
        }

        OrderedSelectionAction::ExtendTo { key } => {
            if oracle.index_of(key).is_some() {
                state.end_range_session();

                // Determine anchor: persistent anchor, focus, first selected, or target
                let anchor_key = state
                    .anchor_key
                    .or(state.focus_key)
                    .or_else(|| first_selected_key(state, oracle))
                    .unwrap_or(key);

                // In immediate mode, clear selection before range
                if !state.is_transactional() || !state.is_transaction_active() {
                    state.selected_keys.clear();
                }

                state.begin_range_session(anchor_key);
                state.focus_key = Some(key);
                result.focus_changed = true;
                result.new_focus = Some(key);
                result.selection_changed = state.apply_range_session(key, oracle);
            }
        }

        // ====================================================================
        // Drag Selection
        // ====================================================================
        OrderedSelectionAction::DragSelect { anchor, current } => {
            if oracle.index_of(anchor).is_some() && oracle.index_of(current).is_some() {
                // Detect new drag: no session OR anchor changed
                let is_new_drag =
                    !state.range_session_active || state.range_anchor_key != Some(anchor);

                if is_new_drag {
                    state.end_range_session();
                    // In immediate mode, clear selection before range
                    if !state.is_transactional() || !state.is_transaction_active() {
                        state.selected_keys.clear();
                    }
                    state.begin_range_session(anchor);
                }

                state.focus_key = Some(current);
                result.focus_changed = true;
                result.new_focus = Some(current);
                result.selection_changed = state.apply_range_session(current, oracle);
            }
        }

        OrderedSelectionAction::DragEnd => {
            state.finalize_range_gesture();
        }

        // ====================================================================
        // Focus-Based Actions
        // ====================================================================
        OrderedSelectionAction::ToggleFocused => {
            if let Some(focus_key) = state.focus_key {
                if oracle.index_of(focus_key).is_some() {
                    state.end_range_session();
                    result.selection_changed = apply_toggle_selection(state, focus_key);
                }
            }
        }

        OrderedSelectionAction::FocusTo { key } => {
            if oracle.index_of(key).is_some() {
                state.end_range_session();
                let changed = state.focus_key != Some(key);
                state.focus_key = Some(key);
                state.anchor_key = Some(key);
                state.range_anchor_key = None;
                result.focus_changed = changed;
                result.new_focus = Some(key);
            }
        }

        // ====================================================================
        // Bulk Actions
        // ====================================================================
        OrderedSelectionAction::ClearSelection => {
            state.end_range_session();
            let had_selection = !state.selected_keys.is_empty();
            state.selected_keys.clear();
            state.anchor_key = None;
            result.selection_changed = had_selection;
        }

        OrderedSelectionAction::SelectAll => {
            state.end_range_session();
            if state.selection_mode == SelectionMode::Multi {
                let old_count = state.selected_keys.len();
                state.selected_keys.clear();
                for i in 0..item_count {
                    state.selected_keys.insert(oracle.key_at(i));
                }
                result.selection_changed = state.selected_keys.len() != old_count;
            }
        }
    }

    result
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Compute the previous focus position.
fn focus_previous<ItemKey, O>(state: &OrderedSelectionState<ItemKey>, oracle: &O) -> Option<ItemKey>
where
    ItemKey: Copy + Eq + Hash,
    O: OrderedIndex<ItemKey>,
{
    let item_count = oracle.len();
    if item_count == 0 {
        return None;
    }

    let current_index = state.focus_key.and_then(|k| oracle.index_of(k));
    let new_index = match current_index {
        Some(idx) if idx > 0 => idx - 1,
        Some(_) => 0,
        None => item_count - 1,
    };
    Some(oracle.key_at(new_index))
}

/// Compute the next focus position.
fn focus_next<ItemKey, O>(state: &OrderedSelectionState<ItemKey>, oracle: &O) -> Option<ItemKey>
where
    ItemKey: Copy + Eq + Hash,
    O: OrderedIndex<ItemKey>,
{
    let item_count = oracle.len();
    if item_count == 0 {
        return None;
    }

    let current_index = state.focus_key.and_then(|k| oracle.index_of(k));
    let new_index = match current_index {
        Some(idx) if idx < item_count - 1 => idx + 1,
        Some(_) => item_count - 1,
        None => 0,
    };
    Some(oracle.key_at(new_index))
}

/// Compute focus position after page movement.
fn focus_page<ItemKey, O>(
    state: &OrderedSelectionState<ItemKey>,
    oracle: &O,
    delta: isize,
) -> Option<ItemKey>
where
    ItemKey: Copy + Eq + Hash,
    O: OrderedIndex<ItemKey>,
{
    let item_count = oracle.len();
    if item_count == 0 {
        return None;
    }

    let current_index = state
        .focus_key
        .and_then(|k| oracle.index_of(k))
        .unwrap_or(0);

    let new_index = if delta < 0 {
        current_index.saturating_sub((-delta) as usize)
    } else {
        (current_index + delta as usize).min(item_count - 1)
    };

    Some(oracle.key_at(new_index))
}

/// Apply single selection: clear existing and select the given key.
fn apply_single_selection<ItemKey>(state: &mut OrderedSelectionState<ItemKey>, key: ItemKey) -> bool
where
    ItemKey: Copy + Eq + Hash,
{
    match state.selection_mode {
        SelectionMode::None => {
            let had_selection = !state.selected_keys.is_empty();
            state.selected_keys.clear();
            state.anchor_key = Some(key);
            had_selection
        }
        SelectionMode::Single | SelectionMode::Multi => {
            let old_selection = state.selected_keys.clone();
            state.selected_keys.clear();
            state.selected_keys.insert(key);
            state.anchor_key = Some(key);
            state.selected_keys != old_selection
        }
    }
}

/// Toggle selection of a single key.
fn apply_toggle_selection<ItemKey>(state: &mut OrderedSelectionState<ItemKey>, key: ItemKey) -> bool
where
    ItemKey: Copy + Eq + Hash,
{
    match state.selection_mode {
        SelectionMode::None => false,
        SelectionMode::Single => {
            // Single mode: toggle behaves like select (no deselect to empty)
            let old_selection = state.selected_keys.clone();
            state.selected_keys.clear();
            state.selected_keys.insert(key);
            state.selected_keys != old_selection
        }
        SelectionMode::Multi => {
            let was_selected = state.selected_keys.contains(&key);
            if was_selected {
                state.selected_keys.remove(&key);
            } else {
                state.selected_keys.insert(key);
            }
            true
        }
    }
}

/// Find the first selected key by index order.
fn first_selected_key<ItemKey, O>(
    state: &OrderedSelectionState<ItemKey>,
    oracle: &O,
) -> Option<ItemKey>
where
    ItemKey: Copy + Eq + Hash,
    O: OrderedIndex<ItemKey>,
{
    let item_count = oracle.len();
    for i in 0..item_count {
        let key = oracle.key_at(i);
        if state.selected_keys.contains(&key) {
            return Some(key);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    // Tests will be added in a future task
}
