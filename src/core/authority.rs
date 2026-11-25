// ============================================================================
// SPEC-HAL-190: Authority Projection & Composite Widget Coordination
// ============================================================================
//
// This module implements the authority projection mechanism that allows
// sovereign widgets to own authoritative state and expose controlled,
// phase-bound access to child widgets.
//
// Invariants enforced:
// - INV-AUTH-SINGLE-OWNER: An authority has exactly one owner
// - INV-AUTH-INJECTION: Authority access is never constructed by widgets
// - INV-AUTH-PHASE-BOUND: Projections are valid only during the phase issued
// - INV-AUTH-READ-ONLY: Children cannot mutate authority directly
// - INV-AUTH-ANCESTOR-ONLY: Projection walks ancestor chain only
// - INV-AUTH-NO-SIBLING-READ: No sibling/cousin authority access
// ============================================================================

use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::core::widget_tree::NodeId;
use crate::utils::id::TreeWidgetId;

// ============================================================================
// Step 1: Authority Marker Trait (SPEC-HAL-190 Section 13.1)
// ============================================================================

/// Marker trait for types that can be projected as authority.
///
/// SPEC-HAL-190 Section 13.1: A type is projectable as authority if it
/// implements this marker trait. The trait carries no methods; it exists
/// for type discrimination in generic contexts.
///
/// # Examples
///
/// ```ignore
/// struct TabularSubstrate { /* ... */ }
/// impl Authority for TabularSubstrate {}
/// ```
///
/// # Invariants
///
/// - INV-AUTH-QUALIFY-1: Only implement for state that passes the qualification filter:
///   1. Multiple consumers need to read it
///   2. Independent lifetime (meaningful even if consumers disappear)
///   3. Semantic weight (represents meaning, not presentation)
///   4. Single lawful owner
pub trait Authority: 'static {}

// ============================================================================
// Step 2: Projection Type (SPEC-HAL-190 Section 13.3)
// ============================================================================

/// Private marker to prevent external construction of Projection.
///
/// This struct has no public fields and no public constructor, ensuring
/// that only the engine (within this crate) can create Projection instances.
#[derive(Debug)]
pub struct SealedMarker(());

/// A read-only, phase-scoped view into parent-owned authority.
///
/// SPEC-HAL-190 Section 13.3: Projections are issued by the engine during
/// phase execution. The `'phase` lifetime is tied to the context lifetime,
/// preventing storage across phases.
///
/// # Type Parameters
///
/// - `'phase`: The lifetime of the current phase (layout, paint, or event).
///   When the context dies, the projection becomes invalid.
/// - `A`: The authority type being projected. Must implement `Authority`.
///
/// # Invariants
///
/// - INV-AUTH-PROJECTION-1: No public constructor. Only the engine creates projections.
/// - INV-AUTH-PROJECTION-2: Does not implement Clone or Copy.
/// - INV-AUTH-PHASE-BOUND: Attempting to store in a struct field fails to compile.
///
/// # Example
///
/// ```ignore
/// fn layout(&mut self, cx: &mut LayoutContext, ...) -> LayoutResult {
///     if let Some(projection) = cx.authority::<TabularSubstrate>() {
///         let columns = projection.get().columns();
///         // Use columns for layout
///     }
///     // projection cannot escape this scope due to 'phase lifetime
/// }
/// ```
pub struct Projection<'phase, A: Authority> {
    inner: &'phase A,
    /// Private field with no public constructor.
    /// Prevents external construction of Projection.
    _sealed: SealedMarker,
}

impl<'phase, A: Authority> Projection<'phase, A> {
    /// Engine-only constructor.
    ///
    /// This function is pub(crate) to prevent external crates from
    /// constructing projections.
    ///
    /// INV-AUTH-PROJECTION-1: Only the engine can create projections.
    pub(crate) fn new(inner: &'phase A) -> Self {
        Self {
            inner,
            _sealed: SealedMarker(()),
        }
    }

    /// Read access to the authority.
    ///
    /// Returns an immutable reference to the authority state.
    /// The reference is bounded by the projection's phase lifetime.
    #[inline]
    pub fn get(&self) -> &A {
        self.inner
    }
}

// Explicitly NOT implementing Clone or Copy
// INV-AUTH-PROJECTION-2: Projection does not implement Clone, Copy, or any lifetime-extending trait

// ============================================================================
// Step 3: AuthorityProvider Trait (SPEC-HAL-190 Section 13.2)
// ============================================================================

/// Trait for widgets that provide (own) authority state.
///
/// SPEC-HAL-190 Section 13.2: Widgets that own authorities declare this
/// during tree construction. The engine uses this to build the authority
/// registry.
///
/// # Invariants
///
/// - INV-AUTH-REGISTER-1: A widget MUST return the same TypeId for its lifetime.
/// - INV-AUTH-REGISTER-2: Only direct ownership is permitted. A widget cannot
///   register authority it does not own.
///
/// # Example
///
/// ```ignore
/// impl AuthorityProvider for ListView {
///     fn provided_authority(&self) -> Option<(&dyn Any, TypeId)> {
///         Some((&self.substrate as &dyn Any, TypeId::of::<TabularSubstrate>()))
///     }
/// }
/// ```
pub trait AuthorityProvider {
    /// Return a type-erased reference to the authority this widget provides.
    ///
    /// Called by the engine during tree traversal setup to build the
    /// authority registry.
    ///
    /// Returns:
    /// - `Some((authority_ref, type_id))` if this widget owns an authority
    /// - `None` if this widget does not own any authority
    fn provided_authority(&self) -> Option<(&dyn Any, TypeId)> {
        None // Default: widget does not provide authority
    }

    /// Return a mutable type-erased reference to the authority.
    ///
    /// Used by the deferred mutation phase to allow authority owners
    /// to mutate their state after processing intents.
    ///
    /// Returns:
    /// - `Some((authority_ref_mut, type_id))` if this widget owns an authority
    /// - `None` if this widget does not own any authority
    fn provided_authority_mut(&mut self) -> Option<(&mut dyn Any, TypeId)> {
        None // Default: widget does not provide authority
    }
}

// ============================================================================
// Step 4: Authority Registry (SPEC-HAL-190 Section 13.2)
// ============================================================================

/// Entry in the authority registry mapping TypeId to NodeId.
///
/// The registry tracks which nodes provide which authority types.
/// This is built once per tree mutation, not per frame.
#[derive(Debug, Clone)]
pub struct AuthorityEntry {
    /// The NodeId of the widget providing this authority.
    pub node_id: NodeId,
    /// The TypeId of the authority being provided.
    pub type_id: TypeId,
}

/// Registry of authority providers in the widget tree.
///
/// SPEC-HAL-190 Section 13.2: The engine maintains this mapping from
/// (NodeId, TypeId) to authority availability.
///
/// The registry is rebuilt on tree mutation only (not per frame).
///
/// # Multi-Authority Ownership
///
/// INV-AUTH-MULTI-1: A sovereign widget MAY own multiple authorities.
///
/// This is explicitly allowed and expected for complex widgets like ListView
/// that own both TabularSubstrate (data authority) and SelectionState (selection
/// authority). Each authority type has independent lookup semantics:
/// - Children can request each authority type independently
/// - Intent routing targets owners by authority type, not widget identity
/// - Multiple authorities from the same owner are processed in undefined order
///   (they must be semantically independent per INV-INTENT-INDEPENDENCE)
#[derive(Default)]
pub struct AuthorityRegistry {
    /// Map from NodeId to list of TypeIds that node provides.
    /// A node may provide multiple authority types (INV-AUTH-MULTI-1).
    /// Used for quick lookup during ancestor chain walking.
    providers: HashMap<NodeId, Vec<TypeId>>,
}

impl AuthorityRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register that a node provides a specific authority type.
    ///
    /// Called during tree mutation when a node is added or updated.
    pub fn register(&mut self, node_id: NodeId, type_id: TypeId) {
        self.providers.entry(node_id).or_default().push(type_id);
    }

    /// Unregister all authorities for a node.
    ///
    /// Called when a node is removed from the tree.
    pub fn unregister(&mut self, node_id: NodeId) {
        self.providers.remove(&node_id);
    }

    /// Check if a node provides a specific authority type.
    pub fn provides(&self, node_id: NodeId, type_id: TypeId) -> bool {
        self.providers
            .get(&node_id)
            .map(|types| types.contains(&type_id))
            .unwrap_or(false)
    }

    /// Clear all entries (used during full tree rebuild).
    pub fn clear(&mut self) {
        self.providers.clear();
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

// ============================================================================
// Step 6: Intent Trait (SPEC-HAL-190 Section 14.2)
// ============================================================================

/// Trait for structured requests expressing desired semantic change.
///
/// SPEC-HAL-190 Section 14.2: Intents are emitted by child widgets to
/// express desired changes without performing mutation. They are
/// processed only by the authority owner.
///
/// # Examples
///
/// ```ignore
/// struct SelectRowIntent {
///     row_id: RowId,
/// }
/// impl Intent for SelectRowIntent {
///     fn as_any(&self) -> &dyn Any { self }
/// }
/// ```
///
/// # Invariants
///
/// - INV-INTENT-EMIT-1: Intents may only be emitted during event handling phase.
/// - INV-INTENT-EMIT-2: The child has no visibility into buffer contents or owner identity.
pub trait Intent: Any + Send {
    /// Downcast support for intent processing.
    ///
    /// Authority owners use this to downcast intents to concrete types
    /// during `process_intents()`.
    fn as_any(&self) -> &dyn Any;
}

// ============================================================================
// Step 7: Intent Buffer (SPEC-HAL-190 Section 14.1)
// ============================================================================

/// Buffer for pending intents targeting a specific authority owner.
///
/// SPEC-HAL-190 Section 14.1: The engine owns all intent buffers, keyed
/// by the TreeWidgetId of the authority owner widget.
///
/// # Invariants
///
/// - INV-INTENT-BUFFER-1: Buffers are owned exclusively by the engine.
/// - INV-INTENT-PROCESS-1: `process_intents` is called exactly once per frame if non-empty.
/// - INV-INTENT-PROCESS-2: Buffer is cleared before the call; owner receives owned data.
#[derive(Default)]
pub struct IntentBuffer {
    intents: Vec<Box<dyn Intent>>,
}

impl IntentBuffer {
    /// Create a new empty intent buffer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push an intent into the buffer.
    pub fn push(&mut self, intent: Box<dyn Intent>) {
        self.intents.push(intent);
    }

    /// Drain all intents from the buffer.
    ///
    /// Returns ownership of all intents, leaving the buffer empty.
    pub fn drain(&mut self) -> Vec<Box<dyn Intent>> {
        std::mem::take(&mut self.intents)
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.intents.is_empty()
    }

    /// Get the number of pending intents.
    pub fn len(&self) -> usize {
        self.intents.len()
    }
}

/// Collection of all intent buffers for the current frame.
///
/// SPEC-HAL-190 Section 14.1: The engine owns all intent buffers,
/// keyed by the TreeWidgetId of the authority owner widget.
#[derive(Default)]
pub struct IntentBuffers {
    buffers: HashMap<TreeWidgetId, IntentBuffer>,
}

impl IntentBuffers {
    /// Create a new empty intent buffer collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create a buffer for a specific owner.
    pub fn buffer_for(&mut self, owner_id: TreeWidgetId) -> &mut IntentBuffer {
        self.buffers.entry(owner_id).or_default()
    }

    /// Drain all non-empty buffers.
    ///
    /// Returns a vec of (owner_id, intents) pairs.
    /// After this call, all buffers are empty.
    pub fn drain_all(&mut self) -> Vec<(TreeWidgetId, Vec<Box<dyn Intent>>)> {
        let mut result = Vec::new();
        for (owner_id, buffer) in self.buffers.iter_mut() {
            if !buffer.is_empty() {
                result.push((*owner_id, buffer.drain()));
            }
        }
        result
    }

    /// Clear all buffers (called at frame end).
    pub fn clear(&mut self) {
        self.buffers.clear();
    }

    /// Check if all buffers are empty.
    pub fn all_empty(&self) -> bool {
        self.buffers.values().all(|b| b.is_empty())
    }
}

// ============================================================================
// Step 9: AuthorityOwner Trait (SPEC-HAL-190 Section 14.4)
// ============================================================================

/// Trait for widgets that own and mutate authority state.
///
/// SPEC-HAL-190 Section 14.4: Authority owners process pending intents
/// during the deferred mutation phase. The engine calls `process_intents`
/// after event routing completes.
///
/// # Invariants
///
/// - INV-INTENT-PROCESS-1: `process_intents` is called exactly once per frame if buffer is non-empty.
/// - INV-INTENT-PROCESS-2: Buffer is cleared before the call; owner receives owned data.
/// - INV-INTENT-INDEPENDENCE: Authorities processed in same phase must be semantically independent.
///
/// # Example
///
/// ```ignore
/// impl AuthorityOwner for ListView {
///     fn process_intents(&mut self, intents: Vec<Box<dyn Intent>>) {
///         for intent in intents {
///             if let Some(select) = intent.as_any().downcast_ref::<SelectRowIntent>() {
///                 self.substrate.select(select.row_id);
///             } else if let Some(resize) = intent.as_any().downcast_ref::<ResizeColumnIntent>() {
///                 self.substrate.resize_column(resize.column_id, resize.new_width);
///             }
///         }
///     }
/// }
/// ```
pub trait AuthorityOwner {
    /// Process pending intents.
    ///
    /// Called by the engine during the deferred mutation phase.
    /// The buffer is drained before this call; the owner receives owned intents.
    ///
    /// The owner should:
    /// - Validate intents
    /// - Order/merge/clamp/reject as needed
    /// - Mutate authority state
    /// - Call invalidation helpers if needed
    fn process_intents(&mut self, intents: Vec<Box<dyn Intent>>);
}

// ============================================================================
// Authority Lookup
// ============================================================================
//
// Authority lookup is implemented in WidgetTree::lookup_authority().
// There is exactly ONE lookup implementation (INV-AUTH-SINGLE-LOOKUP).
//
// The lookup algorithm (SPEC-HAL-190 Section 13.4):
// 1. Start from the requesting widget's NodeId
// 2. Walk the ancestor chain toward the root
// 3. For each ancestor, check if it provides the requested TypeId
// 4. Return the first (nearest) match as a Projection
//
// INV-AUTH-LOOKUP-1: Authority lookup is O(depth).
// INV-AUTH-LOOKUP-2: If multiple ancestors provide the same type, nearest wins.
// INV-AUTH-ANCESTOR-ONLY: Lookup only walks ancestors, never siblings/cousins.
// ============================================================================
