// ============================================================================
// SPEC-HAL-020: Action System - Stage 1 (Core Types & Registry)
// Stage 2: Action Handlers & Execution
// ============================================================================

use crate::core::event::{KeyCode, Modifiers};
use crate::runtime::binding_registry::{
    Binding, BindingLayer, BindingPriority, BindingRegistry, InputPattern,
};
use std::collections::HashMap;

// ============================================================================
// ActionId
// ============================================================================

/// Unique identifier for an action.
///
/// This is a simple u64 wrapper used as a stable reference to registered actions.
/// Actions are identified by this ID throughout the action system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActionId(pub u64);

/// SPEC-HAL-176: Well-known engine action IDs.
///
/// These IDs are reserved for engine-level actions that are executed
/// directly by the event router (which has full context: tree, domain_state,
/// focus manager). They do NOT use ActionHandler — the router matches
/// on the ActionId and executes the appropriate logic.
///
/// App-level actions (theme toggle, command palette) use IDs starting at 1000+.
impl ActionId {
    // -- Spatial navigation (SPEC-HAL-176 §6) --
    pub const FOCUS_LEFT: Self = Self(100);
    pub const FOCUS_RIGHT: Self = Self(101);
    pub const FOCUS_UP: Self = Self(102);
    pub const FOCUS_DOWN: Self = Self(103);

    // -- Structural navigation (SPEC-HAL-126) --
    pub const FOCUS_NEXT: Self = Self(110);
    pub const FOCUS_PREV: Self = Self(111);

    // -- Mode transitions (SPEC-HAL-176 §5) --
    pub const ENTER_WIDGET: Self = Self(120);
    pub const RELEASE_CAPTURE: Self = Self(121);
    pub const EXIT_SCOPE: Self = Self(122);

    // -- Activation (SPEC-HAL-176 §5.4) --
    pub const ACTIVATE: Self = Self(130);

    // -- Plot actions (SPEC-HAL-297) --
    // Range 2000-2099 reserved for Plot/Chart tools.
    // 2000-2009: Tool activation
    pub const PLOT_TOOL_POINTER: Self = Self(2000);
    pub const PLOT_TOOL_MEASURE: Self = Self(2001);
    pub const PLOT_TOOL_HLINE: Self = Self(2002);
    pub const PLOT_TOOL_RECT: Self = Self(2003);
    pub const PLOT_TOOL_TRENDLINE: Self = Self(2004);
    pub const PLOT_TOOL_FIBONACCI: Self = Self(2005);

    // 2010-2019: Plot navigation/view actions
    /// Clear selection / deactivate tool.
    pub const PLOT_CLEAR: Self = Self(2010);
    /// Reset view to default (Home or double-click). Re-enables auto-follow.
    pub const PLOT_RESET_VIEW: Self = Self(2011);
    /// Zoom in at viewport center.
    pub const PLOT_ZOOM_IN: Self = Self(2012);
    /// Zoom out from viewport center.
    pub const PLOT_ZOOM_OUT: Self = Self(2013);
    /// Toggle toolbar visibility.
    pub const PLOT_TOGGLE_TOOLBAR: Self = Self(2014);
    // 2020-2049: Reserved for consumer-defined tool actions.

    // -- Node flow editor actions (SPEC-HAL-294 §7) --
    // Range 2200-2299 reserved for node graph / node flow editor actions.
    pub const NODE_FLOW_DELETE_SELECTED: Self = Self(2200);
    pub const NODE_FLOW_SELECT_ALL: Self = Self(2201);
    pub const NODE_FLOW_DUPLICATE_SELECTED: Self = Self(2202);
    pub const NODE_FLOW_COPY_SELECTED: Self = Self(2203);
    pub const NODE_FLOW_PASTE: Self = Self(2204);
    pub const NODE_FLOW_CUT_SELECTED: Self = Self(2205);
    pub const NODE_FLOW_UNDO: Self = Self(2206);
    pub const NODE_FLOW_REDO: Self = Self(2207);
    pub const NODE_FLOW_OPEN_SEARCH: Self = Self(2208);
    pub const NODE_FLOW_CANCEL: Self = Self(2209);
    pub const NODE_FLOW_ZOOM_TO_FIT: Self = Self(2210);
    pub const NODE_FLOW_RESET_VIEW: Self = Self(2211);
    pub const NODE_FLOW_ZOOM_IN: Self = Self(2212);
    pub const NODE_FLOW_ZOOM_OUT: Self = Self(2213);
    pub const NODE_FLOW_NUDGE_LEFT: Self = Self(2220);
    pub const NODE_FLOW_NUDGE_RIGHT: Self = Self(2221);
    pub const NODE_FLOW_NUDGE_UP: Self = Self(2222);
    pub const NODE_FLOW_NUDGE_DOWN: Self = Self(2223);
    pub const NODE_FLOW_NUDGE_LEFT_FAST: Self = Self(2224);
    pub const NODE_FLOW_NUDGE_RIGHT_FAST: Self = Self(2225);
    pub const NODE_FLOW_NUDGE_UP_FAST: Self = Self(2226);
    pub const NODE_FLOW_NUDGE_DOWN_FAST: Self = Self(2227);

    /// Check if this is an engine-level action that the event router handles.
    pub fn is_engine_action(self) -> bool {
        self.0 >= 100 && self.0 < 200
    }

    /// Whether this action is spatial navigation (widget gets first dibs on TextInput).
    ///
    /// Spatial nav actions are deferred to the widget-first-dibs + fallback path
    /// for TextInput events. Mode transitions (ENTER_WIDGET, etc.) are consumed
    /// immediately by dispatch_engine_action.
    pub fn is_spatial_navigation(self) -> bool {
        matches!(self.0, 100..=103 | 110..=111)
    }
}

// ============================================================================
// ActionHandler (Stage 2)
// ============================================================================

/// Stage 2: Function pointer type for action handlers.
///
/// Action handlers take mutable access to AppContext and execute the action's logic.
/// This is a function pointer (not a closure) for simplicity and Send-ability.
pub type ActionHandler = fn(&mut crate::core::app::AppContext);

// ============================================================================
// Shortcut (data only)
// ============================================================================

/// Keyboard shortcut definition for actions.
///
/// Represents a key combination (key + modifiers) that can trigger an action.
/// This is a data-only struct - no parsing logic or helpers in Stage 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub key: crate::core::event::BindingKey,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

impl Shortcut {
    /// Admission for training/recording from observed input, distinct from generic bindings.
    pub fn from_physical_response(
        observation: Option<&crate::core::event::KeyEvent>,
        reservation: Option<crate::core::event::KeyboardKeyReservation>,
    ) -> Result<Self, crate::core::event::PhysicalBindingUnavailable> {
        crate::core::event::physical_binding_support(observation, reservation)?;
        let key = observation.expect("physical response admission requires an observation");
        Self::from_physical_key_and_modifiers(key.physical_key, key.modifiers)
            .map_err(|_| crate::core::event::PhysicalBindingUnavailable::UnknownPhysicalKey)
    }

    pub fn from_physical_key_and_modifiers(
        key: crate::core::event::PhysicalKeyCode,
        modifiers: Modifiers,
    ) -> Result<Self, crate::core::event::UnknownPhysicalKey> {
        if key == crate::core::event::PhysicalKeyCode::Unknown {
            return Err(crate::core::event::UnknownPhysicalKey);
        }
        Ok(Self {
            key: crate::core::event::BindingKey::Physical(key),
            ctrl: modifiers.ctrl,
            alt: modifiers.alt,
            shift: modifiers.shift,
            meta: modifiers.meta,
        })
    }

    pub fn matches_key_event(&self, event: &crate::core::event::KeyEvent) -> bool {
        self.key.matches(event.key_code, event.physical_key)
            && self.ctrl == event.modifiers.ctrl
            && self.alt == event.modifiers.alt
            && self.shift == event.modifiers.shift
            && self.meta == event.modifiers.meta
    }

    /// Create a new shortcut with the given key and modifier flags.
    pub fn new(key: KeyCode, ctrl: bool, alt: bool, shift: bool, meta: bool) -> Self {
        Self {
            key: key.into(),
            ctrl,
            alt,
            shift,
            meta,
        }
    }

    /// Create a shortcut with no modifiers.
    pub fn key_only(key: KeyCode) -> Self {
        Self {
            key: key.into(),
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        }
    }

    /// Stage 2: Create a shortcut from key code and Modifiers struct.
    ///
    /// This helper bridges the gap between fused input events (which use Modifiers)
    /// and the action system's Shortcut representation.
    pub fn from_key_and_modifiers(key: KeyCode, modifiers: Modifiers) -> Self {
        Self {
            key: key.into(),
            ctrl: modifiers.ctrl,
            alt: modifiers.alt,
            shift: modifiers.shift,
            meta: modifiers.meta,
        }
    }

    /// Stage 3: Create a Ctrl+Shift+key shortcut.
    ///
    /// Convenience helper for registering common shortcut patterns.
    pub fn ctrl_shift(key: KeyCode) -> Self {
        Self {
            key: key.into(),
            ctrl: true,
            alt: false,
            shift: true,
            meta: false,
        }
    }

    /// Stage 7: Create a platform-aware command shortcut.
    ///
    /// Uses Cmd (meta) on macOS, Ctrl on Windows/Linux.
    /// This respects platform conventions for standard shortcuts.
    ///
    /// # Arguments
    /// * `key` - The key code (e.g., KeyCode::Character('p'))
    /// * `shift` - Whether Shift is required
    ///
    /// # Examples
    /// ```ignore
    /// // Command Palette: Cmd+Shift+P (macOS) or Ctrl+Shift+P (Windows/Linux)
    /// let shortcut = Shortcut::platform_command(KeyCode::Character('p'), true);
    /// ```
    pub fn platform_command(key: KeyCode, shift: bool) -> Self {
        #[cfg(target_os = "macos")]
        {
            Self {
                key: key.into(),
                ctrl: false,
                alt: false,
                shift,
                meta: true, // Cmd on macOS
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            Self {
                key: key.into(),
                ctrl: true,
                alt: false,
                shift,
                meta: false,
            }
        }
    }

    /// Format as human-readable display string: "Ctrl+Shift+S", "Alt+F4", "Enter".
    ///
    /// Canonical format used by PropertyEditor keybind display,
    /// command palette, and serialization. Order: Ctrl → Alt → Shift → Meta → Key.
    pub fn to_display_string(&self) -> String {
        let mut result = String::new();
        if self.ctrl {
            result.push_str("Ctrl+");
        }
        if self.alt {
            result.push_str("Alt+");
        }
        if self.shift {
            result.push_str("Shift+");
        }
        if self.meta {
            result.push_str("Meta+");
        }
        let key = match self.key {
            crate::core::event::BindingKey::Logical(key) => key,
            crate::core::event::BindingKey::Physical(key) => {
                result.push_str(&format!("Physical:{key:?}"));
                return result;
            }
        };
        match key {
            KeyCode::Character(c) => {
                for uc in c.to_uppercase() {
                    result.push(uc);
                }
            }
            KeyCode::Enter => result.push_str("Enter"),
            KeyCode::Space => result.push_str("Space"),
            KeyCode::Tab => result.push_str("Tab"),
            KeyCode::Backspace => result.push_str("Backspace"),
            KeyCode::Delete => result.push_str("Delete"),
            KeyCode::Escape => result.push_str("Escape"),
            KeyCode::Left => result.push_str("Left"),
            KeyCode::Right => result.push_str("Right"),
            KeyCode::Up => result.push_str("Up"),
            KeyCode::Down => result.push_str("Down"),
            KeyCode::Home => result.push_str("Home"),
            KeyCode::End => result.push_str("End"),
            KeyCode::PageUp => result.push_str("PageUp"),
            KeyCode::PageDown => result.push_str("PageDown"),
            KeyCode::F1 => result.push_str("F1"),
            KeyCode::F2 => result.push_str("F2"),
            KeyCode::F3 => result.push_str("F3"),
            KeyCode::F4 => result.push_str("F4"),
            KeyCode::F5 => result.push_str("F5"),
            KeyCode::F6 => result.push_str("F6"),
            KeyCode::F7 => result.push_str("F7"),
            KeyCode::F8 => result.push_str("F8"),
            KeyCode::F9 => result.push_str("F9"),
            KeyCode::F10 => result.push_str("F10"),
            KeyCode::F11 => result.push_str("F11"),
            KeyCode::F12 => result.push_str("F12"),
            _ => result.push_str(&format!("{:?}", key)),
        }
        result
    }
}

// ============================================================================
// ActionScope
// ============================================================================

/// Scope in which an action is valid.
///
/// - `Global`: Action is available throughout the application
/// - `Screen`: Action is only available on the current screen
/// - `Widget`: Action is only available when a specific widget has focus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionScope {
    Global,
    Screen,
    Widget,
}

// ============================================================================
// Action
// ============================================================================

/// Definition of an action in the action system.
///
/// An action represents a named operation that can be triggered by shortcuts,
/// the command palette, or programmatically.
///
/// Stage 2: Includes handler function for execution.
pub struct Action {
    pub id: ActionId,
    pub name: &'static str,
    pub description: &'static str,
    pub category: &'static str,
    pub shortcut: Option<Shortcut>,
    pub scope: ActionScope,
    /// Stage 2: Handler function that executes this action's logic.
    pub handler: ActionHandler,
}

// ============================================================================
// ActionRegistry
// ============================================================================

/// Central registry for all actions.
///
/// Stage 1: Just storage. No lookup logic beyond basic insertion.
/// - `actions`: Maps ActionId to Action definition
/// - `shortcut_map`: Maps Shortcut to ActionId for quick shortcut resolution
///
/// Stage 4: Added metadata index for querying and search.
/// - `list`: Stable, ordered list of all ActionIds (registration order)
pub struct ActionRegistry {
    pub actions: HashMap<ActionId, Action>,
    // Deprecated: use binding_registry for shortcut lookup. Retained for existing callers.
    pub shortcut_map: HashMap<Shortcut, ActionId>,
    /// Stage 4: Ordered list of action IDs for stable iteration and search.
    pub list: Vec<ActionId>,
    /// SPEC-HAL-020C: Layered binding tables with conflict resolution.
    pub binding_registry: BindingRegistry,
}

impl ActionRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            actions: HashMap::new(),
            shortcut_map: HashMap::new(),
            list: Vec::new(),
            binding_registry: BindingRegistry::new(),
        }
    }

    /// Add an action to the registry.
    ///
    /// If the action has a shortcut, also registers the shortcut mapping.
    /// Stage 4: Also adds to the ordered list for stable iteration.
    pub fn add(&mut self, action: Action) {
        let id = action.id;

        // If action has a shortcut, register the mapping
        if let Some(shortcut) = action.shortcut {
            self.shortcut_map.insert(shortcut, id);

            // SPEC-HAL-020C: Also register in binding_registry as System/Normal
            self.binding_registry.add(Binding {
                pattern: InputPattern::from(&shortcut),
                action_id: id,
                layer: BindingLayer::System,
                priority: BindingPriority::Normal,
                tag: None,
            });
        }

        // Insert the action
        self.actions.insert(id, action);

        // Stage 4: Add to ordered list
        self.list.push(id);
    }

    /// Stage 2: Find an action ID by its keyboard shortcut.
    ///
    /// Returns Some(ActionId) if a shortcut is bound, None otherwise.
    /// This is an O(1) lookup using the shortcut_map.
    pub fn find_action_by_shortcut(&self, shortcut: &Shortcut) -> Option<ActionId> {
        self.shortcut_map.get(shortcut).copied()
    }

    /// Stage 2: Execute an action by its ID.
    ///
    /// Looks up the action in the registry and calls its handler with the given context.
    /// If the action is not found, this is a silent no-op.
    pub fn execute(&self, id: ActionId, cx: &mut crate::core::app::AppContext) {
        if let Some(action) = self.actions.get(&id) {
            (action.handler)(cx);
        }
    }

    /// Stage 2: Try to execute an action by shortcut.
    ///
    /// Looks up the shortcut, executes the bound action if found, and returns true.
    /// If no action is bound to this shortcut, returns false.
    ///
    /// This is the primary entry point for keyboard shortcut dispatch.
    pub fn try_execute_shortcut(
        &self,
        shortcut: &Shortcut,
        cx: &mut crate::core::app::AppContext,
    ) -> bool {
        if let Some(id) = self.find_action_by_shortcut(shortcut) {
            self.execute(id, cx);
            true
        } else {
            false
        }
    }

    /// Stage 4: Return all registered actions in registration order.
    ///
    /// This iterator respects the order in which actions were added via `add()`.
    /// No allocations are performed; the iterator filters over the action list.
    pub fn all(&self) -> impl Iterator<Item = &Action> {
        self.list.iter().filter_map(|id| self.actions.get(id))
    }

    /// Stage 4: Return actions filtered by scope.
    ///
    /// Returns an iterator over actions matching the given scope (Global, Screen, or Widget).
    /// Actions are returned in registration order.
    pub fn by_scope(&self, scope: ActionScope) -> impl Iterator<Item = &Action> {
        self.list
            .iter()
            .filter_map(|id| self.actions.get(id))
            .filter(move |a| a.scope == scope)
    }

    /// Stage 4: Case-insensitive prefix/substring search on name and description.
    ///
    /// This is a basic search implementation for Stage 4. Fuzzy matching will be added in Stage 5.
    ///
    /// # Search behavior:
    /// - Empty query: returns all actions in registration order
    /// - Non-empty query: case-insensitive match on:
    ///   - Name starts with query (prefix match)
    ///   - Name contains query (substring match)
    ///   - Description contains query (substring match)
    ///
    /// # Performance:
    /// - No allocations except for `to_lowercase()` conversions
    /// - Caller provides a mutable Vec buffer for output
    /// - Results respect registration order
    pub fn search_basic<'a>(&'a self, query: &str, out: &mut Vec<&'a Action>) {
        out.clear();

        if query.is_empty() {
            // Empty query: return all actions
            for id in &self.list {
                if let Some(a) = self.actions.get(id) {
                    out.push(a);
                }
            }
            return;
        }

        let q = query.to_lowercase();

        for id in &self.list {
            let a = match self.actions.get(id) {
                Some(a) => a,
                None => continue,
            };

            let name = a.name.to_lowercase();
            let desc = a.description.to_lowercase();

            if name.starts_with(&q) || name.contains(&q) || desc.contains(&q) {
                out.push(a);
            }
        }
    }

    /// Stage 5: Fuzzy matching score for a query against text.
    ///
    /// Returns a positive score if the query matches the text, 0 otherwise.
    ///
    /// # Scoring hierarchy:
    /// - Prefix match: +300
    /// - Substring match: +150
    /// - Fuzzy (ordered character match): +10 per matched char, +1 per small gap
    ///   - Breaks if gap > 6 characters
    ///
    /// All matching is case-insensitive.
    fn fuzzy_score(query: &str, text: &str) -> i32 {
        if query.is_empty() {
            return 1;
        }

        let query_lower = query.to_lowercase();
        let text_lower = text.to_lowercase();

        // 1. Prefix match: +300
        if text_lower.starts_with(&query_lower) {
            return 300;
        }

        // 2. Substring match: +150
        if text_lower.contains(&query_lower) {
            return 150;
        }

        // 3. Fuzzy (ordered character match)
        let query_chars: Vec<char> = query_lower.chars().collect();
        let text_chars: Vec<char> = text_lower.chars().collect();

        let mut score = 0;
        let mut text_idx = 0;
        let mut last_match_idx = 0;

        for (query_idx, &q_char) in query_chars.iter().enumerate() {
            // Find next occurrence of query char in text
            let mut found = false;
            while text_idx < text_chars.len() {
                if text_chars[text_idx] == q_char {
                    found = true;
                    // Check gap since last match
                    if query_idx > 0 {
                        let gap = text_idx - last_match_idx - 1;
                        if gap > 6 {
                            // Gap too large, break fuzzy scoring
                            return 0;
                        }
                        score += 1; // Small gap bonus
                    }
                    score += 10; // Character match
                    last_match_idx = text_idx;
                    text_idx += 1;
                    break;
                }
                text_idx += 1;
            }

            if !found {
                // Character not found in remaining text
                return 0;
            }
        }

        score
    }

    /// Stage 5: Fuzzy search with ranked results.
    ///
    /// Returns actions sorted by relevance score (descending).
    /// Each result is a tuple of (action reference, score).
    ///
    /// # Scoring:
    /// - Name match score × 2
    /// - Description match score × 1
    /// - Actions with score > 0 are included
    /// - Results sorted by total score (descending)
    /// - Equal scores preserve registration order (stable sort)
    ///
    /// Empty query returns all actions with score=1.
    pub fn search_fuzzy<'a>(&'a self, query: &str, out: &mut Vec<(&'a Action, i32)>) {
        out.clear();

        if query.is_empty() {
            // Empty query: return all actions with score=1
            for id in &self.list {
                if let Some(a) = self.actions.get(id) {
                    out.push((a, 1));
                }
            }
            return;
        }

        // Score each action
        for id in &self.list {
            let a = match self.actions.get(id) {
                Some(a) => a,
                None => continue,
            };

            let name_score = Self::fuzzy_score(query, a.name);
            let desc_score = Self::fuzzy_score(query, a.description);
            let total_score = (name_score * 2) + desc_score;

            if total_score > 0 {
                out.push((a, total_score));
            }
        }

        // Sort by score descending (stable sort preserves registration order for equal scores)
        out.sort_by(|a, b| b.1.cmp(&a.1));
    }

    /// Stage 5: Unified search interface.
    ///
    /// Dispatches to basic or fuzzy search based on query length:
    /// - Query length < 2: uses basic search (Stage 4)
    /// - Query length >= 2: uses fuzzy search (Stage 5)
    ///
    /// Returns only action references (scores discarded).
    pub fn search<'a>(&'a self, query: &str, out: &mut Vec<&'a Action>) {
        if query.len() < 2 {
            // Short query: use basic search
            self.search_basic(query, out);
        } else {
            // Longer query: use fuzzy search
            let mut ranked: Vec<(&Action, i32)> = Vec::new();
            self.search_fuzzy(query, &mut ranked);
            out.clear();
            out.extend(ranked.iter().map(|(action, _score)| *action));
        }
    }
}

impl Default for ActionRegistry {
    fn default() -> Self {
        Self::new()
    }
}
