use std::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// SPEC-HAL-087 Stage 4: Type-Safe Widget Identity (Final)
// ============================================================================
//
// Two distinct ID types prevent compile-time leakage of widget-internal IDs
// into engine APIs:
//
// - TreeWidgetId: Engine-managed, used by hit-test, event routing, scene nodes
// - WidgetInternalId: Widget-local, used only inside widget implementations
//
// The tree allocates TreeWidgetId via alloc_widget_id().
// Widgets create WidgetInternalId via WidgetInternalId::new().
// These types are intentionally NOT convertible to prevent accidental leakage.
// ============================================================================

/// SPEC-HAL-087 Stage 3: Engine-managed widget identifier.
///
/// Used by all engine systems:
/// - `WidgetTree::id_to_node` mapping
/// - `WidgetNode.id`
/// - Hit-test results
/// - Event routing
/// - Scene graph overlay nodes (ScrollbarV/H)
/// - Pointer capture
/// - Focus management
///
/// Allocated ONLY by `WidgetTree::alloc_widget_id()`.
/// Never constructed directly by widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TreeWidgetId(pub u64);

impl TreeWidgetId {
    /// Create from raw value (internal use by WidgetTree only)
    #[cfg(test)]
    pub(crate) const fn from_raw(id: u64) -> Self {
        TreeWidgetId(id)
    }

    /// Get the raw u64 value (for debugging/logging)
    pub const fn raw(&self) -> u64 {
        self.0
    }
}

/// SPEC-HAL-087 Stage 3: Widget-internal identifier.
///
/// Used ONLY inside widget implementations for internal bookkeeping.
/// Must NEVER be passed to engine APIs (hit-test, event routing, scene nodes).
///
/// Allocated via `WidgetInternalId::new()` in widget constructors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WidgetInternalId(pub u64);

impl WidgetInternalId {
    /// Create a new unique WidgetInternalId (for widget-internal use)
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        WidgetInternalId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw u64 value (for debugging/logging)
    pub const fn raw(&self) -> u64 {
        self.0
    }
}

impl Default for WidgetInternalId {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_widget_internal_id_uniqueness() {
        let id1 = WidgetInternalId::new();
        let id2 = WidgetInternalId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_tree_widget_id_equality() {
        let id1 = TreeWidgetId::from_raw(42);
        let id2 = TreeWidgetId::from_raw(42);
        let id3 = TreeWidgetId::from_raw(43);
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }
}
