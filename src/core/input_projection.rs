// SPEC-HAL-131C: Input Projection Contract
//
// Defines the projection snapshot for input handling (hit-testing, pointer events,
// focus targeting). Input operates on this snapshot, NOT on realized widgets.
//
// Key invariants:
// - INV-INPUT-PROJECTION-1: Input queries projection snapshot, never realized tree
// - INV-INPUT-PROJECTION-2: Input resolution does not mutate projection or realization
// - INV-INPUT-IDENTITY-1: Input targets logical identity, stable across realization churn
//
// The projection is computed during layout_tree() and is read-only for the
// remainder of the frame.

use crate::core::math::RectF;
use crate::utils::id::TreeWidgetId;

/// SPEC-HAL-131C: Stable logical item identity.
///
/// INV-INPUT-IDENTITY-1: This ID is stable across realization and unrealization cycles.
/// It corresponds to RowId from SPEC-HAL-131B but is generalized for input purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LogicalItemId(pub u64);

impl LogicalItemId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }
}

/// SPEC-HAL-131C: A single entry in the input projection.
///
/// Represents a logical item that can receive input, regardless of whether
/// it's currently realized as a widget.
#[derive(Debug, Clone)]
pub struct InputProjectionEntry {
    /// Stable logical identity (INV-INPUT-IDENTITY-1)
    pub logical_id: LogicalItemId,
    /// Bounds in content space (before scroll transform)
    pub content_bounds: RectF,
    /// If realized, the widget that represents this item
    pub realized_widget: Option<TreeWidgetId>,
    /// Logical index in the container (for ordered containers like List)
    pub logical_index: usize,
}

/// SPEC-HAL-131C: Input projection snapshot for a virtualizing container.
///
/// INV-PROJECTION-SNAPSHOT-1: Computed during layout_tree(), read-only after.
/// INV-INPUT-PROJECTION-1: Input queries this, not the realized tree.
#[derive(Debug, Clone, Default)]
pub struct InputProjection {
    /// The owning container's widget ID
    pub container_id: Option<TreeWidgetId>,
    /// Absolute bounds of the container's viewport
    pub viewport_bounds: RectF,
    /// Scroll offset applied to content
    pub scroll_offset: (f32, f32),
    /// All logical items (realized and unrealized)
    entries: Vec<InputProjectionEntry>,
    /// Total content extent (for bounds checking)
    pub content_extent: (f32, f32),
}

impl InputProjection {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the container that owns this projection.
    pub fn set_container(&mut self, container_id: TreeWidgetId, viewport_bounds: RectF) {
        self.container_id = Some(container_id);
        self.viewport_bounds = viewport_bounds;
    }

    /// Set the scroll offset.
    pub fn set_scroll_offset(&mut self, offset_x: f32, offset_y: f32) {
        self.scroll_offset = (offset_x, offset_y);
    }

    /// Set the content extent.
    pub fn set_content_extent(&mut self, width: f32, height: f32) {
        self.content_extent = (width, height);
    }

    /// Clear all entries (for rebuild).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Add a logical item to the projection.
    ///
    /// Called during layout_tree() to register all items (realized and unrealized).
    pub fn add_entry(&mut self, entry: InputProjectionEntry) {
        self.entries.push(entry);
    }

    /// Get all entries (read-only access).
    pub fn entries(&self) -> &[InputProjectionEntry] {
        &self.entries
    }

    /// Get entry count.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Hit-test a point in viewport coordinates.
    ///
    /// INV-INPUT-PROJECTION-1: This is the primary input query path.
    /// INV-INPUT-PROJECTION-2: This is read-only, never mutates state.
    ///
    /// Returns the entry at the given point, or None if no hit.
    /// The point is in viewport-local coordinates.
    pub fn hit_test(&self, local_x: f32, local_y: f32) -> Option<&InputProjectionEntry> {
        // Transform viewport coords to content coords
        let content_x = local_x + self.scroll_offset.0;
        let content_y = local_y + self.scroll_offset.1;

        // Search entries in reverse order (front-to-back in paint order)
        for entry in self.entries.iter().rev() {
            if entry.content_bounds.contains_point(content_x, content_y) {
                return Some(entry);
            }
        }

        None
    }

    /// Find entry by logical ID.
    ///
    /// INV-INPUT-IDENTITY-1: Used for focus targeting by stable identity.
    pub fn find_by_id(&self, id: LogicalItemId) -> Option<&InputProjectionEntry> {
        self.entries.iter().find(|e| e.logical_id == id)
    }

    /// Find entry by logical index.
    pub fn find_by_index(&self, index: usize) -> Option<&InputProjectionEntry> {
        self.entries.iter().find(|e| e.logical_index == index)
    }

    /// Get the logical item at a given Y position (for vertical lists).
    ///
    /// Uses binary search for efficient lookup with variable heights.
    pub fn item_at_y(&self, content_y: f32) -> Option<&InputProjectionEntry> {
        // Binary search for item containing content_y
        let idx = self
            .entries
            .partition_point(|e| e.content_bounds.y + e.content_bounds.height <= content_y);
        if idx < self.entries.len() {
            let entry = &self.entries[idx];
            if entry.content_bounds.y <= content_y {
                return Some(entry);
            }
        }
        None
    }
}

/// SPEC-HAL-131C: Extended hit-test result with logical identity.
///
/// INV-INPUT-IDENTITY-1: For virtualized content, logical_item_id is always set.
/// For non-virtualized content, it may be None.
#[derive(Debug, Clone)]
pub struct ProjectedHitResult {
    /// Logical identity of the hit target (stable across realization)
    pub logical_item_id: Option<LogicalItemId>,
    /// Logical index in container (for ordered containers)
    pub logical_index: Option<usize>,
    /// Widget ID if the item is currently realized
    pub realized_widget_id: Option<TreeWidgetId>,
    /// The container that owns this projection
    pub container_id: TreeWidgetId,
    /// Local position within the item's bounds
    pub local_pos: (f32, f32),
    /// The item's bounds in content space
    pub item_bounds: RectF,
}

/// SPEC-HAL-131C: Trait for widgets that provide input projection data.
///
/// Widgets that virtualize content (like List) should implement this trait
/// to allow the engine to build the input projection during layout.
///
/// INV-PROJECTION-SNAPSHOT-1: This is called during layout_tree() only.
pub trait InputProjectionProvider {
    /// Get the total number of logical items.
    fn logical_item_count(&self) -> usize;

    /// Get the bounds of a logical item in content space.
    ///
    /// Returns None if the index is out of bounds.
    fn logical_item_bounds(&self, index: usize) -> Option<RectF>;

    /// Get the logical ID for an item at the given index.
    ///
    /// INV-INPUT-IDENTITY-1: This ID must be stable across realization cycles.
    fn logical_item_id(&self, index: usize) -> Option<LogicalItemId>;

    /// Get the realized widget ID for an item, if it's currently realized.
    fn realized_widget_for_item(&self, index: usize) -> Option<TreeWidgetId>;

    /// Get the range of currently realized items.
    ///
    /// # Authority Contract (SPEC-HAL-134 Phase 4 Tripwire)
    ///
    /// **`realized_range` is AUTHORITATIVE.** It is derived from `visible_range`
    /// via prefetch expansion and represents the single source of truth for
    /// which items are currently realized.
    ///
    /// ## Rules for New Consumers
    ///
    /// **ANY NEW RUNTIME CONSUMER** of this method MUST:
    /// 1. Add a `debug_assert!` validating the range (well-formed, within bounds)
    /// 2. Document the consumption in SPEC-HAL-131B or SPEC-HAL-131C
    /// 3. Update the consumer audit list in the SPEC
    ///
    /// ## Forbidden Behaviors
    ///
    /// Consumers MUST NOT:
    /// - Recompute or re-derive the range
    /// - Clamp, extend, or modify the range
    /// - Apply "corrections" or "compensations"
    /// - Reinterpret the range based on external state
    ///
    /// ## Audit Trigger
    ///
    /// `grep "realized_range()"` is a manual audit trigger.
    /// Any new hit requires consumer review, assertion, and SPEC update.
    fn realized_range(&self) -> std::ops::Range<usize>;

    /// Get the content width for items.
    fn content_width(&self) -> f32;
}

impl ProjectedHitResult {
    /// Check if this hit is on an unrealized item.
    pub fn is_unrealized(&self) -> bool {
        self.realized_widget_id.is_none()
    }
}

// ============================================================================
// SPEC-HAL-131E: Projected Action Routing
// ============================================================================

/// Action types for projected input dispatch.
///
/// These actions operate on logical identity (LogicalItemId), not widget identity.
/// They can be dispatched to either realized widgets or containers for unrealized items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectedAction {
    /// Selection action (click with optional modifiers)
    Select {
        /// Whether Shift modifier was held
        shift: bool,
        /// Whether Ctrl/Cmd modifier was held
        ctrl_or_cmd: bool,
    },
    /// Focus action (set keyboard focus to this item)
    Focus,
    /// Activation action (double-click, Enter, or primary action)
    Activate,
}

impl ProjectedAction {
    /// Create a Select action from modifiers.
    pub fn select_with_modifiers(modifiers: crate::core::event::Modifiers) -> Self {
        Self::Select {
            shift: modifiers.shift,
            ctrl_or_cmd: modifiers.ctrl || modifiers.meta,
        }
    }
}

// ============================================================================
// SPEC-HAL-131E: Focus Anchor Hint
// ============================================================================

/// Anchor hint emitted on projected focus changes.
///
/// INV-FOCUS-ANCHOR-1: Anchor hint is logical identity, not realized widget.
/// INV-FOCUS-ANCHOR-2: Anchor hint does NOT trigger realization during input.
/// INV-FOCUS-ANCHOR-3: Anchor hint is consumed on next scroll/layout pass only.
/// INV-FOCUS-ANCHOR-4: Setting anchor does NOT mutate scroll position during input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusAnchorHint {
    /// Logical identity of the focused item (stable across realization).
    pub logical_id: LogicalItemId,
    /// Logical index in the container.
    pub logical_index: usize,
}

impl FocusAnchorHint {
    pub const fn new(logical_id: LogicalItemId, logical_index: usize) -> Self {
        Self {
            logical_id,
            logical_index,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logical_item_id_equality() {
        let id1 = LogicalItemId::new(42);
        let id2 = LogicalItemId::new(42);
        let id3 = LogicalItemId::new(99);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_projection_hit_test_realized() {
        let mut proj = InputProjection::new();
        proj.set_container(TreeWidgetId(1), RectF::new(0.0, 0.0, 200.0, 500.0));
        proj.set_scroll_offset(0.0, 0.0);

        // Add a realized item
        proj.add_entry(InputProjectionEntry {
            logical_id: LogicalItemId::new(100),
            content_bounds: RectF::new(0.0, 0.0, 200.0, 50.0),
            realized_widget: Some(TreeWidgetId(10)),
            logical_index: 0,
        });

        let hit = proj.hit_test(100.0, 25.0);
        assert!(hit.is_some());
        let entry = hit.unwrap();
        assert_eq!(entry.logical_id, LogicalItemId::new(100));
        assert!(entry.realized_widget.is_some());
    }

    #[test]
    fn test_projection_hit_test_unrealized() {
        let mut proj = InputProjection::new();
        proj.set_container(TreeWidgetId(1), RectF::new(0.0, 0.0, 200.0, 500.0));
        proj.set_scroll_offset(0.0, 100.0); // Scrolled down 100px

        // Add an unrealized item at content Y=150
        proj.add_entry(InputProjectionEntry {
            logical_id: LogicalItemId::new(3),
            content_bounds: RectF::new(0.0, 150.0, 200.0, 50.0),
            realized_widget: None, // Unrealized!
            logical_index: 3,
        });

        // Hit at viewport Y=50 -> content Y=150
        let hit = proj.hit_test(100.0, 50.0);
        assert!(hit.is_some());
        let entry = hit.unwrap();
        assert_eq!(entry.logical_id, LogicalItemId::new(3));
        assert!(entry.realized_widget.is_none()); // INV-INPUT-IDENTITY-1: logical ID is stable
    }

    #[test]
    fn test_projection_hit_test_miss() {
        let mut proj = InputProjection::new();
        proj.set_container(TreeWidgetId(1), RectF::new(0.0, 0.0, 200.0, 500.0));

        proj.add_entry(InputProjectionEntry {
            logical_id: LogicalItemId::new(1),
            content_bounds: RectF::new(0.0, 0.0, 200.0, 50.0),
            realized_widget: None,
            logical_index: 0,
        });

        // Hit below the item
        let hit = proj.hit_test(100.0, 100.0);
        assert!(hit.is_none());
    }

    #[test]
    fn test_find_by_id() {
        let mut proj = InputProjection::new();

        proj.add_entry(InputProjectionEntry {
            logical_id: LogicalItemId::new(1),
            content_bounds: RectF::new(0.0, 0.0, 100.0, 50.0),
            realized_widget: None,
            logical_index: 0,
        });
        proj.add_entry(InputProjectionEntry {
            logical_id: LogicalItemId::new(2),
            content_bounds: RectF::new(0.0, 50.0, 100.0, 50.0),
            realized_widget: Some(TreeWidgetId(5)),
            logical_index: 1,
        });

        let found = proj.find_by_id(LogicalItemId::new(2));
        assert!(found.is_some());
        assert_eq!(found.unwrap().logical_index, 1);

        let not_found = proj.find_by_id(LogicalItemId::new(99));
        assert!(not_found.is_none());
    }
}
