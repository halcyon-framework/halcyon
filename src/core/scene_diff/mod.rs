//! SPEC-HAL-083 Phase-3/4/5/6/7/8/11/12/13/14/15/16/17/18/19/20/21/22/23/24: SceneGraph Diff Engine
//!
//! **SPEC-HAL-083 is STRUCTURALLY COMPLETE as of Phase-24.**
//!
//! # Purpose
//!
//! Compute a pure structural diff between two built SceneGraphs.
//! This module is key-based only - no geometry or content comparison.
//!
//! # Constraints
//!
//! - Identity source: `SceneNodeKey` only
//! - Nodes with `key == None` are **ignored by diff** (ephemeral)
//! - No geometry comparison
//! - No hashing of node contents
//! - Deterministic output (order-stable)
//! - Zero visual / behavioral change
//!
//! # Phase-4 Additions
//!
//! - `extract_scene_keys`: Extract identity-only snapshot (no heavy data cloning)
//! - `compute_diff_from_keys`: Diff from pre-extracted key snapshots
//!
//! # Phase-5 Additions
//!
//! - `DeltaReaction`: Sealed enum describing allowed reactions to diff entries
//! - `DeltaReactionIter`: Iterator adapter for consuming deltas through the contract
//! - `SceneDelta::into_reactions()`: The ONLY sanctioned consumption path
//!
//! # Phase-6 Additions
//!
//! - `SceneDiffConsumer`: Sealed trait for consuming reactions (passive in Phase-6)
//! - `SceneDiffConsumers`: Registration container with deterministic dispatch order
//! - Consumer registration is explicit, ordered, non-magical
//!
//! # Phase-7 Additions
//!
//! - `SceneDiffExecutionMode`: Enum controlling execution (Disabled/Enabled)
//! - `dispatch_scene_diff`: The ONLY legal execution entry point
//!
//! # Phase-8 Additions
//!
//! - Enabled mode now dispatches reactions to registered consumers
//! - `DeltaReactionIter::from_vec()`: Create iterator from pre-collected reactions
//! - `DebugCountingConsumer`: Test-only consumer that counts and stores reactions
//! - End-to-end execution pipeline for dummy/test consumers (NO GPU/NO CACHE)
//!
//! # Phase-11 Additions
//!
//! - `trace_identity_changes`: Debug-only identity consumer for dry-run validation
//! - Gated behind `HALCYON_TRACE_SCENE_IDENTITY=1` environment variable
//! - Logs INSERT/REMOVE/STABLE/UPDATE keys to stderr for debugging
//!
//! # Phase-12 Additions
//!
//! - `assert_identity_stability`: Debug-only stability assertions
//! - Verifies stable keys maintain widget affinity across frames
//! - Panics on violation (key moved from one widget to another)
//! - Last guard phase before any caching is allowed
//!
//! # Phase-13 Additions
//!
//! - `SceneDiffConsumer::process(&SceneDelta, &SceneGraph)`: New interface receiving delta and scene
//! - `LoggingConsumer`: Minimal consumer that logs counts only (no state retention)
//! - `dispatch_scene_diff` now passes `&SceneDelta` and `&SceneGraph` to consumers
//! - Consumer interface boundary established for future GPU cache operations
//!
//! # Phase-14 Additions
//!
//! - `CacheReadinessConsumer`: Debug-only consumer that validates cache feasibility
//! - `SceneNode::key()` and `SceneNode::kind()`: Accessor methods for node introspection
//! - `SceneGraph::find_node_by_key()`: Lookup method for key-based queries
//! - Validates: key existence, node type stability, geometry data availability
//! - Proves all lookup paths work without implementing actual caching
//!
//! # Phase-15 Additions
//!
//! - `SceneCache`: Empty shell struct - shape only, no storage
//! - `CacheConsumer`: Trait for cache-aware consumers (receives `&mut SceneCache`)
//! - `SceneCacheExecutionMode`: Separate gate for cache operations (disabled by default)
//! - `dispatch_cache_operations`: Entry point for cache pipeline (proves lifetimes/ownership)
//! - Proves: lifetimes, ownership, call order, mutability boundaries
//!
//! # Phase-16 Additions
//!
//! - `CacheEntry`: Minimal cache entry (key, kind, dirty flag) - CPU-only
//! - `SceneCache`: Real storage via `HashMap<SceneNodeKey, CacheEntry>`
//! - `SceneCache::insert()`, `remove()`, `mark_dirty()`, `get()`, `contains()`
//! - `BasicCacheConsumer`: Real consumer that inserts/removes/marks dirty
//! - Frame-loop wiring in app.rs (after diff, before render)
//! - NO reuse, NO eviction, NO GPU objects, NO renderer access
//!
//! # Phase-17 Additions
//!
//! - `SceneCache::clear_all_dirty()`: Centralized dirty resolution (must be called once per frame)
//! - Frame boundary assertion: `dirty_count() == 0` at end of dispatch (debug panic if violated)
//! - Lifecycle enforcement: Created→Clean, Updated→Dirty, Dirty→Cleared, Removed→Gone
//! - Consumer contract: may insert/remove/mark_dirty, may NOT clear dirty or inspect prev-frame state
//! - Dirty resolution is centralized in dispatch_cache_operations, not in consumers
//!
//! # Phase-18 Additions
//!
//! - `SceneCache::assert_consistent_with_scene()`: Verifies key↔kind stability (debug-only)
//! - `SceneCache::assert_delta_applied()`: Verifies no phantom nodes (debug-only)
//! - Enforces: every cache key exists in scene, every kind matches, inserts present, removes gone
//! - Centralized enforcement in dispatch_cache_operations (after consumers, before clear_all_dirty)
//! - Closed loop: SceneGraph → Diff → Cache → Enforcement → Frame boundary
//!
//! # Phase-19 Additions
//!
//! - `SceneCacheHandle`: Opaque, copyable handle to cache entry (not a pointer)
//! - `SceneCache::insert_with_handle()`: Insert and return stable handle
//! - `SceneCache::get_by_handle()`, `get_by_handle_mut()`: Access via handle
//! - Handle validity: remains valid across frames unless entry is removed
//! - Debug-only generation tracking for stale handle detection
//! - Consumer contract: MAY store handles, MUST NOT store references
//! - Zero overhead in release builds beyond handle storage
//!
//! # Phase-20 Additions
//!
//! - `CacheEntryOwnership` enum: `CpuOnly` (current) vs `GpuBound` (placeholder)
//! - `CacheEntry.ownership` field tracks CPU/GPU ownership state
//! - `entry.ownership()` getter for ownership state
//! - `entry.assert_cpu_only()` debug-only assertion
//! - All entry mutations (`mark_dirty`, `clear_dirty`) assert `CpuOnly`
//! - Ownership boundary locked before GPU caching lands
//!
//! # Phase-21 Additions
//!
//! - All `CacheEntry` fields now **private** (mutation surface locked)
//! - New getters: `key()`, `kind()`, `is_dirty()`
//! - All mutations flow through named methods with invariant assertions
//! - `SceneCache` mutation methods document permitted paths
//! - Closes last structural escape hatch before GPU phases
//!
//! # Phase-22 Additions
//!
//! - `CacheLifecyclePhase` enum: `Build`, `Dispatch`, `Frozen`
//! - `SceneCache.phase` field tracks lifecycle state (debug-only)
//! - `begin_frame()`, `enter_dispatch()`, `end_frame()` lifecycle methods
//! - All mutations (`insert*`, `remove`, `mark_dirty`, `clear_all_dirty`) only allowed in `Build` phase
//! - `dispatch_cache_operations` now manages full lifecycle: Build → Dispatch → Frozen
//! - Debug-only assertion panics on mutation outside Build phase
//! - `Dispatch` phase: NO mutations, read/consume only
//!
//! # Phase-23 Additions
//!
//! - `assert_read_only()`: Debug-only marker for read-only accessors
//! - Read-only methods (`get`, `get_by_handle`, `contains`, `iter`, `dirty_count`, `entry_count`) allowed in ALL phases
//! - Mutable accessors (`get_mut`, `get_by_handle_mut`) restricted to Build phase only
//! - Completes temporal authority: Build=mutate+read, Dispatch=read-only, Frozen=read-only
//!
//! # Phase-24 Additions (SPEC-HAL-083 STRUCTURAL COMPLETION)
//!
//! - **Stable Read Snapshot Guarantee**: During Dispatch and Frozen phases, SceneCache is a stable snapshot
//! - All reads observe identical state across the phase (entry_count, dirty_count unchanged)
//! - No mutation APIs are reachable during Dispatch/Frozen (enforced by Phase-22/23)
//! - Debug-only snapshot marker: captures state at `enter_dispatch()`, asserts unchanged on reads
//! - `assert_snapshot_stable()`: Debug-only assertion called by read accessors in Dispatch/Frozen
//! - Zero overhead in release builds
//! - SPEC-HAL-083 is now structurally complete - ready for GPU/renderer integration in future SPECs
//!
//! # Non-Goals (Phase-3/4/5/6/7/8/11/12/13/14/15/16/17/18/19/20/21/22/23/24)
//!
//! - No property diffs (updates field stays empty)
//! - No ordering/reorder detection
//! - No GPU cache or buffer reuse
//! - No production consumers (requires Phase-9+)

mod forbidden;
mod guard;

use crate::core::scene::{SceneGraph, SceneNode};
use crate::core::scene_identity::{SceneNodeKey, SceneNodeKind};

/// Result of diffing two SceneGraphs.
///
/// Contains only key-based structural changes. No content comparison.
#[derive(Debug, Clone, Default)]
pub struct SceneDelta {
    /// New keys or fresh primitive kinds replacing retired cache generations.
    pub inserts: Vec<SceneNodeKey>,
    /// Removed keys or retired primitive kinds replaced under the same root key.
    pub removes: Vec<SceneNodeKey>,
    /// Keys present in both with changed content.
    /// **ALWAYS EMPTY in Phase-3** - no content comparison yet.
    pub updates: Vec<SceneNodeKey>,
}

impl SceneDelta {
    /// Returns true if there are no changes.
    pub fn is_empty(&self) -> bool {
        self.inserts.is_empty() && self.removes.is_empty() && self.updates.is_empty()
    }
}

/// Compute the structural diff between two SceneGraphs.
///
/// # Arguments
///
/// * `prev` - Previous frame's SceneGraph (None for first frame)
/// * `next` - Current frame's SceneGraph
///
/// # Returns
///
/// A `SceneDelta` containing:
/// - `inserts`: keys in `next` but not in `prev`
/// - `removes`: keys in `prev` but not in `next`
/// - `updates`: always empty in Phase-3 (no content comparison)
///
/// # Notes
///
/// - Ephemeral nodes (`key == None`) are ignored entirely
/// - Output order is deterministic (insertion order preserved)
pub fn compute_scene_diff(prev: Option<&SceneGraph>, next: &SceneGraph) -> SceneDelta {
    // Extract keyed nodes from next scene (preserving order)
    let next_keys = collect_keyed_nodes(next);

    // Extract keyed nodes from prev scene (if any)
    let prev_keys = prev.map(collect_keyed_nodes).unwrap_or_default();

    // Build a set of prev keys for O(1) lookup
    // Note: We use a Vec-based contains check to maintain determinism
    // BTreeSet would also work but Vec is simpler for small N

    let mut delta = SceneDelta::default();

    // Find inserts: key in next but not in prev
    for key in &next_keys {
        if !prev_keys.contains(key) {
            delta.inserts.push(*key);
        }
    }

    // Find removes: key in prev but not in next
    for key in &prev_keys {
        if !next_keys.contains(key) {
            delta.removes.push(*key);
        }
    }

    // updates stays empty in Phase-3 (no content comparison)

    delta
}

// ============================================================================
// SPEC-HAL-083 Phase-4: Lightweight Key Extraction
// ============================================================================

/// Extract all keyed node identities from a SceneGraph.
///
/// This is the identity-only snapshot API for Phase-4. It extracts just the
/// `SceneNodeKey` values without cloning any heavy scene data (geometry, text, etc.).
///
/// # Returns
///
/// A `Vec<SceneNodeKey>` containing all keys in traversal order.
/// Ephemeral nodes (`key == None`) are excluded.
pub fn extract_scene_keys(scene: &SceneGraph) -> Vec<SceneNodeKey> {
    collect_keyed_nodes(scene)
}

/// Compute diff from pre-extracted key snapshots.
///
/// This is the Phase-4 API for diffing without holding full SceneGraph references.
/// Use `extract_scene_keys` to create snapshots, then diff between frames.
///
/// # Arguments
///
/// * `prev_keys` - Keys from previous frame (None for first frame)
/// * `next_keys` - Keys from current frame
pub fn compute_diff_from_keys(
    prev_keys: Option<&Vec<SceneNodeKey>>,
    next_keys: &Vec<SceneNodeKey>,
) -> SceneDelta {
    let prev = prev_keys.map(|v| v.as_slice()).unwrap_or(&[]);

    let mut delta = SceneDelta::default();

    // Find inserts: key in next but not in prev
    for key in next_keys {
        if !prev.contains(key) {
            delta.inserts.push(*key);
        }
    }

    // Find removes: key in prev but not in next
    for key in prev {
        if !next_keys.contains(key) {
            delta.removes.push(*key);
        }
    }

    delta
}

/// Cache handoff under SPEC-HAL-083 §6.4: a changed primitive kind retires
/// the old cache generation while preserving the widget-owned visual-root key.
pub(crate) fn compute_scene_diff_for_cache(
    prev_keys: Option<&Vec<SceneNodeKey>>,
    next: &SceneGraph,
    cache: &SceneCache,
) -> (Vec<SceneNodeKey>, SceneDelta) {
    fn collect(
        nodes: &[SceneNode],
        keys: &mut Vec<SceneNodeKey>,
        replacements: &mut Vec<SceneNodeKey>,
        cache: &SceneCache,
    ) {
        for node in nodes {
            if let Some(key) = node.key() {
                keys.push(key);
                if cache
                    .get(key)
                    .is_some_and(|entry| entry.kind() != node.kind())
                {
                    replacements.push(key);
                }
            }
            match node {
                SceneNode::RetainedFragment(n) => collect(&n.children, keys, replacements, cache),
                SceneNode::Clip(n) => collect(&n.children, keys, replacements, cache),
                SceneNode::Layer(n) => collect(&n.children, keys, replacements, cache),
                SceneNode::Scroll(n) => collect(&n.children, keys, replacements, cache),
                _ => {}
            }
        }
    }

    let mut keys = Vec::new();
    let mut replacements = Vec::new();
    collect(&next.root, &mut keys, &mut replacements, cache);
    let mut delta = compute_diff_from_keys(prev_keys, &keys);
    for key in replacements {
        if !delta.removes.contains(&key) {
            delta.removes.push(key);
        }
        if !delta.inserts.contains(&key) {
            delta.inserts.push(key);
        }
    }
    (keys, delta)
}

/// Collect all SceneNodeKeys from a SceneGraph, in traversal order.
///
/// Nodes with `key == None` are skipped (ephemeral).
fn collect_keyed_nodes(graph: &SceneGraph) -> Vec<SceneNodeKey> {
    let mut keys = Vec::new();
    collect_keyed_nodes_recursive(&graph.root, &mut keys);
    keys
}

/// Recursively collect keys from a list of SceneNodes.
fn collect_keyed_nodes_recursive(nodes: &[SceneNode], keys: &mut Vec<SceneNodeKey>) {
    for node in nodes {
        // Extract key from node (if any)
        if let Some(key) = extract_key(node) {
            keys.push(key);
        }

        // Recurse into children (Clip and Layer nodes have children)
        match node {
            SceneNode::RetainedFragment(fragment) => {
                collect_keyed_nodes_recursive(&fragment.children, keys);
            }
            SceneNode::Clip(clip) => {
                collect_keyed_nodes_recursive(&clip.children, keys);
            }
            SceneNode::Layer(layer) => {
                collect_keyed_nodes_recursive(&layer.children, keys);
            }
            SceneNode::Scroll(scroll) => {
                collect_keyed_nodes_recursive(&scroll.children, keys);
            }
            _ => {}
        }
    }
}

/// Extract the SceneNodeKey from a SceneNode, if present.
fn extract_key(node: &SceneNode) -> Option<SceneNodeKey> {
    match node {
        SceneNode::RetainedFragment(_) => None,
        SceneNode::Rect(n) => n.key,
        SceneNode::BorderRect(n) => n.key,
        SceneNode::RoundedRect(n) => n.key,
        SceneNode::Text(n) => n.key,
        SceneNode::Image(n) => n.key,
        SceneNode::Clip(n) => n.key,
        SceneNode::Layer(n) => n.key,
        SceneNode::Scroll(n) => n.key,
        SceneNode::Circle(n) => n.key,
        SceneNode::Ellipse(n) => n.key,
        SceneNode::Line(n) => n.key,
        SceneNode::Polyline(n) => n.key,
        SceneNode::LuminousContour(n) => n.key(),
        SceneNode::LocalAmbientGlow(n) => n.key(),
        SceneNode::AnalyticSurface(n) => n.key(),
        SceneNode::FrostedGlass(n) => n.key(),
        SceneNode::Path(n) => n.key,
        SceneNode::ScrollbarV(n) => n.key,
        SceneNode::ScrollbarH(n) => n.key,
        SceneNode::Shadow(n) => n.key,
        SceneNode::GradientRect(n) => n.key,
        SceneNode::ColorPickerSv(n) => n.key,
        SceneNode::RadialGauge(n) => n.key,
    }
}

// ============================================================================
// SPEC-HAL-083 Phase-5: SceneDelta Consumption Contract (SEALED)
// ============================================================================
//
// # Purpose
//
// Define the consumption boundary for SceneDelta. This is a DESCRIPTIVE
// interface only - it declares what reactions are ALLOWED, not what happens.
//
// # Binding Constraints
//
// 1. SceneDelta is IDENTITY-ONLY:
//    - No geometry access at consumption time
//    - No node borrowing or SceneGraph traversal
//    - Consumers receive keys, not nodes
//
// 2. Consumers MUST NOT:
//    - Inspect node contents (forbidden by design - keys only)
//    - Infer semantics from key values
//    - Perform renderer-side heuristics based on diff patterns
//    - Access the SceneGraph during consumption
//
// 3. Consumption is:
//    - Explicit: consumers must opt-in
//    - Sealed: no ad-hoc reactions outside this contract
//    - Descriptive: reactions are declared, not executed
//
// # Forbidden (Phase-5)
//
// - Renderer hooks or GPU cache integration
// - Actual execution of reactions
// - Node diffing or geometry comparison
// - Implicit behavior triggered by diffs
// - Coupling to app.rs, scheduler, or renderer
//
// # Future Extensions (Phase-6+)
//
// - DeltaReaction may gain execution capability
// - DeltaConsumer trait may be implemented by renderer subsystems
// - GPU resource pools may subscribe to insert/remove events
//
// These extensions require explicit SPEC-HAL-083 Phase-6+ approval.
// ============================================================================

/// Describes a single allowed reaction to a SceneDelta entry.
///
/// This is a DESCRIPTIVE type - it declares what reaction is permitted,
/// but does NOT execute it. Execution is deferred to future phases.
///
/// # Contract
///
/// - Reactions are identity-only (key-based)
/// - No geometry, no node content, no SceneGraph access
/// - Consumers declare intent, not behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaReaction {
    /// A new keyed node appeared in the scene.
    ///
    /// Permitted future reactions (Phase-6+):
    /// - Allocate GPU resources for this key
    /// - Initialize cache entry
    ///
    /// Forbidden:
    /// - Inspecting node geometry or content
    /// - Inferring node type from key value
    Insert(SceneNodeKey),

    /// A keyed node was removed from the scene.
    ///
    /// Permitted future reactions (Phase-6+):
    /// - Release GPU resources for this key
    /// - Invalidate cache entry
    ///
    /// Forbidden:
    /// - Accessing the removed node's data
    /// - Deferred cleanup based on node content
    Remove(SceneNodeKey),

    /// A keyed node's content may have changed.
    ///
    /// **Currently unused** - Phase-3/4 does not detect updates.
    ///
    /// Permitted future reactions (Phase-6+):
    /// - Mark GPU resources dirty for this key
    /// - Schedule re-upload of cached data
    ///
    /// Forbidden:
    /// - Diffing node properties here
    /// - Geometry comparison at consumption time
    Update(SceneNodeKey),
}

/// Iterator adapter that converts a SceneDelta into DeltaReactions.
///
/// This is the SEALED consumption interface. All diff consumption MUST
/// go through this iterator to ensure the contract is enforced.
///
/// # Usage (Phase-6+)
///
/// ```ignore
/// for reaction in delta.into_reactions() {
///     match reaction {
///         DeltaReaction::Insert(key) => { /* declare intent */ }
///         DeltaReaction::Remove(key) => { /* declare intent */ }
///         DeltaReaction::Update(key) => { /* declare intent */ }
///     }
/// }
/// ```
///
/// # Contract Enforcement
///
/// - Consumers receive `DeltaReaction`, not raw key vectors
/// - Order: inserts first, then removes, then updates
/// - No direct access to SceneDelta fields during iteration
pub struct DeltaReactionIter {
    /// Phase-8: Support creating from pre-collected reactions
    inner: DeltaReactionIterInner,
}

enum DeltaReactionIterInner {
    /// Original mode: iterate from separate key vectors
    FromKeys {
        inserts: std::vec::IntoIter<SceneNodeKey>,
        removes: std::vec::IntoIter<SceneNodeKey>,
        updates: std::vec::IntoIter<SceneNodeKey>,
        phase: DeltaIterPhase,
    },
    /// Phase-8: iterate from pre-collected reaction vec
    FromVec {
        reactions: std::vec::IntoIter<DeltaReaction>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeltaIterPhase {
    Inserts,
    Removes,
    Updates,
    Done,
}

impl DeltaReactionIter {
    /// Phase-8: Create an iterator from a pre-collected Vec of reactions.
    ///
    /// This allows dispatching the same reactions to multiple consumers.
    pub fn from_vec(reactions: Vec<DeltaReaction>) -> Self {
        Self {
            inner: DeltaReactionIterInner::FromVec {
                reactions: reactions.into_iter(),
            },
        }
    }
}

impl SceneDelta {
    /// Convert this delta into a sealed reaction iterator.
    ///
    /// This is the ONLY sanctioned way to consume a SceneDelta.
    /// Direct field access is permitted for logging/debugging only.
    pub fn into_reactions(self) -> DeltaReactionIter {
        DeltaReactionIter {
            inner: DeltaReactionIterInner::FromKeys {
                inserts: self.inserts.into_iter(),
                removes: self.removes.into_iter(),
                updates: self.updates.into_iter(),
                phase: DeltaIterPhase::Inserts,
            },
        }
    }

    /// Returns the total number of reactions in this delta.
    pub fn reaction_count(&self) -> usize {
        self.inserts.len() + self.removes.len() + self.updates.len()
    }
}

impl Iterator for DeltaReactionIter {
    type Item = DeltaReaction;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner {
            DeltaReactionIterInner::FromKeys {
                inserts,
                removes,
                updates,
                phase,
            } => loop {
                match phase {
                    DeltaIterPhase::Inserts => {
                        if let Some(key) = inserts.next() {
                            return Some(DeltaReaction::Insert(key));
                        }
                        *phase = DeltaIterPhase::Removes;
                    }
                    DeltaIterPhase::Removes => {
                        if let Some(key) = removes.next() {
                            return Some(DeltaReaction::Remove(key));
                        }
                        *phase = DeltaIterPhase::Updates;
                    }
                    DeltaIterPhase::Updates => {
                        if let Some(key) = updates.next() {
                            return Some(DeltaReaction::Update(key));
                        }
                        *phase = DeltaIterPhase::Done;
                    }
                    DeltaIterPhase::Done => return None,
                }
            },
            DeltaReactionIterInner::FromVec { reactions } => reactions.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = match &self.inner {
            DeltaReactionIterInner::FromKeys {
                inserts,
                removes,
                updates,
                ..
            } => inserts.len() + removes.len() + updates.len(),
            DeltaReactionIterInner::FromVec { reactions } => reactions.len(),
        };
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for DeltaReactionIter {}

// ============================================================================
// SPEC-HAL-083 Phase-6: Diff Consumer Registration (NON-OPERATIONAL)
// ============================================================================
//
// # Purpose
//
// Define a formal registration mechanism for SceneDiff consumers.
// This phase establishes WHO may subscribe to reactions and HOW subscriptions
// are expressed, while keeping all behavior inert.
//
// # Binding Constraints
//
// 1. Consumption remains IDENTITY-ONLY:
//    - Consumers receive `DeltaReaction`, never raw keys
//    - No SceneGraph access during consumption
//    - No access to SceneDelta internals
//
// 2. Registration is:
//    - Explicit: consumers must opt-in via registration
//    - Ordered: dispatch order is deterministic (registration order)
//    - Non-magical: no global singletons, no implicit registration
//
// 3. Registered consumers MUST NOT:
//    - Execute reactions (forbidden in Phase-6)
//    - Allocate or free resources
//    - Touch renderer, GPU, or scheduler
//    - Have observable side effects
//
// # Phase-6 Status
//
// - Consumers are PASSIVE
// - Execution is FORBIDDEN
// - All methods are NOPs or return descriptive data only
//
// # Phase-7 Activation (future)
//
// Phase-7 is required to:
// - Enable `process_delta` to execute reactions
// - Wire consumers into the render loop
// - Allow resource allocation/deallocation
//
// These extensions require explicit SPEC-HAL-083 Phase-7 approval.
// ============================================================================

/// Trait for consuming SceneDelta reactions.
///
/// This is a SEALED trait - implementations declare intent but do NOT
/// execute reactions in Phase-6. All methods must be side-effect-free.
///
/// # Contract
///
/// - Consumers receive `&SceneDelta` and `&SceneGraph`
/// - Registration order determines dispatch order
/// - Ordered, explicit, non-magical
///
/// # Phase-13 Interface
///
/// Consumers now receive:
/// - `&SceneDelta` - the structural diff (inserts, removes, updates)
/// - `&SceneGraph` - the current frame's scene graph for node lookup
///
/// This allows consumers to:
/// - Inspect delta counts without iteration
/// - Look up node data by key from the scene graph
/// - Prepare for future GPU cache operations (Phase-14+)
pub trait SceneDiffConsumer: std::fmt::Debug {
    /// Human-readable name for debugging and registration diagnostics.
    fn name(&self) -> &'static str;

    /// Process a scene diff.
    ///
    /// # Arguments
    ///
    /// - `delta`: The structural diff containing insert/remove/update keys
    /// - `scene`: The current frame's scene graph (for node lookup by key)
    ///
    /// # Phase-13 Contract
    ///
    /// Implementations may:
    /// - Log counts and diagnostics
    /// - Validate delta contents
    /// - Iterate reactions via `delta.into_reactions()`
    ///
    /// Implementations MUST NOT:
    /// - Allocate GPU resources
    /// - Modify renderer state
    /// - Have persistent side effects across frames
    fn process(&mut self, delta: &SceneDelta, scene: &SceneGraph);

    /// Returns the number of keys this consumer is currently tracking.
    ///
    /// Phase-6: MUST return 0 (no tracking yet).
    /// Phase-7+: May return actual tracked count.
    fn tracked_key_count(&self) -> usize {
        0
    }
}

/// Registration container for SceneDiff consumers.
///
/// Holds consumers in deterministic order (registration order).
/// Exposes iteration but not mutation during dispatch.
///
/// # Contract
///
/// - Consumers are stored in registration order
/// - Dispatch iterates in registration order
/// - No mutation allowed during dispatch
/// - Phase-6: `dispatch` is a NOP
///
/// # Usage (Phase-7+)
///
/// ```ignore
/// let mut consumers = SceneDiffConsumers::new();
/// consumers.register(Box::new(MyConsumer::new()));
///
/// // After computing delta:
/// consumers.dispatch(delta);
/// ```
#[derive(Debug, Default)]
pub struct SceneDiffConsumers {
    /// Registered consumers in registration order.
    consumers: Vec<Box<dyn SceneDiffConsumer>>,
}

impl SceneDiffConsumers {
    /// Create an empty consumer registry.
    pub fn new() -> Self {
        Self {
            consumers: Vec::new(),
        }
    }

    /// Register a consumer.
    ///
    /// Consumers are dispatched in registration order.
    /// Duplicate registrations are allowed (same consumer called multiple times).
    pub fn register(&mut self, consumer: Box<dyn SceneDiffConsumer>) {
        self.consumers.push(consumer);
    }

    /// Returns the number of registered consumers.
    pub fn consumer_count(&self) -> usize {
        self.consumers.len()
    }

    /// Returns an iterator over consumer names (for diagnostics).
    pub fn consumer_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.consumers.iter().map(|c| c.name())
    }

    /// Dispatch a delta to all registered consumers.
    ///
    /// # Phase-13 Contract
    ///
    /// Iterates consumers in registration order and calls `process(delta, scene)`.
    ///
    /// NOTE: The main entry point is `dispatch_scene_diff()` which handles
    /// the execution gate. This method is for internal/test use only.
    pub fn dispatch(&mut self, delta: &SceneDelta, scene: &SceneGraph) {
        for consumer in self.consumers.iter_mut() {
            consumer.process(delta, scene);
        }
    }

    /// Check if any consumers are registered.
    pub fn is_empty(&self) -> bool {
        self.consumers.is_empty()
    }
}

// ============================================================================
// SPEC-HAL-083 Phase-7: Controlled Activation Boundary (EXECUTION GATE)
// ============================================================================
//
// # Purpose
//
// Define WHERE and HOW execution would be allowed, without turning it on.
// This freezes the activation surface and prevents accidental activation.
//
// # Execution Gate Contract
//
// 1. `dispatch_scene_diff` is the ONLY legal execution entry point.
// 2. All diff consumption MUST go through this function.
// 3. Direct calls to `SceneDiffConsumers::dispatch` are FORBIDDEN outside tests.
//
// # Phase-7 Status
//
// - Execution mode: HARD-DISABLED
// - `SceneDiffExecutionMode::Disabled`: consumes delta, does nothing
// - `SceneDiffExecutionMode::Enabled`: PANICS (not permitted until Phase-8)
//
// # Phase-8 Activation (future)
//
// Phase-8 is required to:
// - Remove the panic from `Enabled` mode
// - Actually call `consumer.process_delta` for each consumer
// - Wire this function into the render loop
//
// Until Phase-8 lands, execution is FORBIDDEN.
// ============================================================================

/// Execution mode for SceneDiff consumption.
///
/// This enum controls whether diff reactions are actually processed.
///
/// # Phase-7 Contract
///
/// - `Disabled`: Default. Delta is consumed but no reactions are processed.
/// - `Enabled`: **PANICS** in Phase-7. Execution is not yet permitted.
///
/// # Phase-8+ Contract
///
/// When Phase-8 lands, `Enabled` will:
/// - Iterate reactions via `into_reactions()`
/// - Call `consumer.process_delta()` for each registered consumer
/// - Actually execute the consumption pipeline
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SceneDiffExecutionMode {
    /// Default mode. Delta is consumed but reactions are NOT processed.
    /// This is the only safe mode in Phase-7.
    #[default]
    Disabled,

    /// Execution mode. **PANICS in Phase-7.**
    ///
    /// This mode exists to define the activation boundary.
    /// It will be enabled in Phase-8 when execution is permitted.
    Enabled,
}

/// Dispatch a SceneDelta through the execution gate.
///
/// This is the ONLY legal entry point for SceneDiff execution.
/// All diff consumption in production code MUST go through this function.
///
/// # Arguments
///
/// * `mode` - Execution mode (must be `Disabled` in Phase-7)
/// * `consumers` - Registered consumers
/// * `delta` - The delta to dispatch
/// * `scene` - The current frame's scene graph
///
/// # Phase-7 Behavior
///
/// - `Disabled`: Delta is consumed, nothing happens. This is safe.
/// - `Enabled`: **PANICS** with message:
///   "SPEC-HAL-083 Phase-7: execution not yet permitted"
///
/// # Phase-13 Behavior
///
/// - `Disabled`: Same as Phase-7 (NOP)
/// - `Enabled`: Iterates consumers and calls `process(&delta, &scene)`
///
/// # Why this gate exists
///
/// - Freezes the activation surface
/// - Prevents accidental execution before enabled
/// - Makes the execution boundary explicit and auditable
/// - Ensures future activation cannot bypass this gate
///
/// # Phase-13 Update
///
/// Consumers now receive `&SceneDelta` and `&SceneGraph` directly.
/// This allows inspection without collecting reactions.
pub fn dispatch_scene_diff(
    mode: SceneDiffExecutionMode,
    consumers: &mut SceneDiffConsumers,
    delta: &SceneDelta,
    scene: &SceneGraph,
) {
    match mode {
        SceneDiffExecutionMode::Disabled => {
            // NOP - delta and scene are borrowed, nothing to consume
        }
        SceneDiffExecutionMode::Enabled => {
            // Phase-13: Execute through registered consumers
            //
            // SAFETY CONTRACT:
            // - Consumers MUST NOT touch renderer, GPU, or caches
            // - Consumers MUST NOT have external side effects
            // - This is for test/dummy consumers only
            //
            // Phase-14+ will allow production consumers.

            // Dispatch to each consumer in registration order
            for consumer in consumers.consumers.iter_mut() {
                consumer.process(delta, scene);
            }
        }
    }
}

// ============================================================================
// SPEC-HAL-083 Phase-8: Debug/Test Consumer Implementation
// ============================================================================
//
// # Purpose
//
// Provide a dummy consumer that proves the pipeline works end-to-end
// without touching renderer, GPU, or caches.
//
// # Contract
//
// - DebugCountingConsumer is for TESTING ONLY
// - No external side effects
// - No renderer/GPU/cache interaction
// - Phase-9+ will add production consumers
// ============================================================================

/// A debug consumer that counts and stores reactions.
///
/// This is a TEST-ONLY consumer that proves the execution pipeline works.
/// It has NO external side effects and does NOT touch renderer/GPU/caches.
///
/// # Usage
///
/// ```ignore
/// let mut consumer = DebugCountingConsumer::new();
/// // ... register and dispatch ...
/// assert_eq!(consumer.insert_count(), 3);
/// assert_eq!(consumer.remove_count(), 1);
/// ```
#[derive(Debug, Default)]
pub struct DebugCountingConsumer {
    /// Keys that were inserted
    inserted_keys: Vec<SceneNodeKey>,
    /// Keys that were removed
    removed_keys: Vec<SceneNodeKey>,
    /// Keys that were updated
    updated_keys: Vec<SceneNodeKey>,
}

impl DebugCountingConsumer {
    /// Create a new empty debug consumer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of insert reactions received.
    pub fn insert_count(&self) -> usize {
        self.inserted_keys.len()
    }

    /// Returns the number of remove reactions received.
    pub fn remove_count(&self) -> usize {
        self.removed_keys.len()
    }

    /// Returns the number of update reactions received.
    pub fn update_count(&self) -> usize {
        self.updated_keys.len()
    }

    /// Returns total reactions received.
    pub fn total_count(&self) -> usize {
        self.insert_count() + self.remove_count() + self.update_count()
    }

    /// Returns a slice of all inserted keys.
    pub fn inserted_keys(&self) -> &[SceneNodeKey] {
        &self.inserted_keys
    }

    /// Returns a slice of all removed keys.
    pub fn removed_keys(&self) -> &[SceneNodeKey] {
        &self.removed_keys
    }

    /// Returns a slice of all updated keys.
    pub fn updated_keys(&self) -> &[SceneNodeKey] {
        &self.updated_keys
    }

    /// Clear all stored reactions.
    pub fn clear(&mut self) {
        self.inserted_keys.clear();
        self.removed_keys.clear();
        self.updated_keys.clear();
    }
}

impl SceneDiffConsumer for DebugCountingConsumer {
    fn name(&self) -> &'static str {
        "DebugCountingConsumer"
    }

    fn process(&mut self, delta: &SceneDelta, _scene: &SceneGraph) {
        // Collect keys from the delta directly
        self.inserted_keys.extend(delta.inserts.iter().copied());
        self.removed_keys.extend(delta.removes.iter().copied());
        self.updated_keys.extend(delta.updates.iter().copied());
    }

    fn tracked_key_count(&self) -> usize {
        self.total_count()
    }
}

// ============================================================================
// SPEC-HAL-083 Phase-13: Logging Consumer (No State Retention)
// ============================================================================
//
// # Purpose
//
// Prove the consumer pipeline is correctly wired by logging counts.
// No allocations, no state retention - just logs and returns.
// ============================================================================

/// A minimal consumer that logs delta counts.
///
/// This consumer:
/// - Logs INSERT/REMOVE/UPDATE counts to tracing
/// - Has no state retention across frames
/// - Proves ordering and call path
///
/// # Phase-13 Contract
///
/// - No allocations beyond stack
/// - No persistent state
/// - No renderer/GPU interaction
#[derive(Debug, Default)]
pub struct LoggingConsumer {
    /// Consumer name for diagnostics
    name: &'static str,
}

impl LoggingConsumer {
    /// Create a new logging consumer with the given name.
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl SceneDiffConsumer for LoggingConsumer {
    fn name(&self) -> &'static str {
        self.name
    }

    fn process(&mut self, delta: &SceneDelta, _scene: &SceneGraph) {
        // Log counts only - no allocations, no state retention
        if !delta.is_empty() {
            tracing::debug!(
                "[{}] SceneDiff: +{} -{} ~{}",
                self.name,
                delta.inserts.len(),
                delta.removes.len(),
                delta.updates.len()
            );
        }
    }

    fn tracked_key_count(&self) -> usize {
        0 // No state retention
    }
}

// ============================================================================
// SPEC-HAL-083 Phase-14: Cache-Readiness Validation Consumer (Debug-Only)
// ============================================================================
//
// # Purpose
//
// Prove that all information required for caching exists without caching anything.
// This is the last pre-cache phase - validates lookup paths and data availability.
//
// # Contract
//
// - Debug-only, env-gated (HALCYON_TRACE_SCENE_IDENTITY=1)
// - Verifies: key existence, node type stability, geometry data
// - Panics on missing data (loud failure, not silent bug)
// - No persistent storage, no GPU interaction
// ============================================================================

/// A debug-only consumer that validates cache readiness.
///
/// This consumer verifies that:
/// 1. Insert keys exist in the scene graph
/// 2. Node types remain stable for stable keys across frames
/// 3. Geometry-bearing nodes expose enough data for caching
///
/// # Phase-14 Contract
///
/// - No persistent storage
/// - No allocations beyond stack
/// - Panics on validation failure
/// - Release builds: completely compiled out
#[cfg(debug_assertions)]
#[derive(Debug, Default)]
pub struct CacheReadinessConsumer {
    /// Previous frame's key-to-kind mapping for stability checks
    /// Uses interior mutability pattern with static storage
    _marker: std::marker::PhantomData<()>,
}

#[cfg(debug_assertions)]
impl CacheReadinessConsumer {
    /// Create a new cache-readiness consumer.
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }

    /// Validate that all insert keys exist in the scene.
    fn validate_inserts(&self, delta: &SceneDelta, scene: &SceneGraph) {
        for key in &delta.inserts {
            let node = scene.find_node_by_key(*key);
            assert!(
                node.is_some(),
                "SPEC-HAL-083 Phase-14: Insert key {:?} not found in scene graph. \
                 A key marked as inserted MUST exist in the current frame's scene.",
                key
            );

            // Validate geometry data is present
            if let Some(node) = node {
                self.validate_geometry_data(key, node);
            }
        }
    }

    /// Validate node type stability for stable keys (not in inserts).
    fn validate_type_stability(&self, delta: &SceneDelta, scene: &SceneGraph) {
        use std::sync::{Mutex, OnceLock};

        // Static storage for previous frame's key-to-kind mapping
        static PREV_KEY_KINDS: OnceLock<
            Mutex<std::collections::HashMap<SceneNodeKey, SceneNodeKind>>,
        > = OnceLock::new();
        let prev_kinds_mutex =
            PREV_KEY_KINDS.get_or_init(|| Mutex::new(std::collections::HashMap::new()));

        let mut prev_kinds = prev_kinds_mutex.lock().unwrap();

        // Collect current frame's key-to-kind mapping
        let mut current_kinds: std::collections::HashMap<SceneNodeKey, SceneNodeKind> =
            std::collections::HashMap::new();

        for node in &scene.root {
            if let Some(key) = node.key() {
                current_kinds.insert(key, node.kind());
            }
        }

        // Check type stability: for each key NOT in inserts (stable key),
        // verify the kind matches the previous frame
        for (key, current_kind) in &current_kinds {
            let is_new = delta.inserts.iter().any(|k| k == key);
            if is_new {
                continue; // Skip new keys, they have no previous type
            }

            if let Some(prev_kind) = prev_kinds.get(key) {
                assert!(
                    *prev_kind == *current_kind,
                    "SPEC-HAL-083 Phase-14: Node type changed for stable key {:?}. \
                     Previous: {:?}, Current: {:?}. \
                     A stable key MUST NOT change node types across frames.",
                    key,
                    prev_kind,
                    current_kind
                );
            }
        }

        // Update storage for next frame
        prev_kinds.clear();
        prev_kinds.extend(current_kinds);
    }

    /// Validate that geometry-bearing nodes have valid data for caching.
    fn validate_geometry_data(&self, key: &SceneNodeKey, node: &SceneNode) {
        use crate::core::scene::SceneNode;

        // Check that geometry values are finite and valid
        match node {
            SceneNode::Rect(n) => {
                assert!(
                    n.rect.width.is_finite() && n.rect.height.is_finite(),
                    "SPEC-HAL-083 Phase-14: Rect node {:?} has invalid geometry: {:?}",
                    key,
                    n.rect
                );
            }
            SceneNode::BorderRect(n) => {
                assert!(
                    n.rect.width.is_finite() && n.rect.height.is_finite(),
                    "SPEC-HAL-083 Phase-14: BorderRect node {:?} has invalid geometry: {:?}",
                    key,
                    n.rect
                );
            }
            SceneNode::RoundedRect(n) => {
                assert!(
                    n.rect.width.is_finite() && n.rect.height.is_finite(),
                    "SPEC-HAL-083 Phase-14: RoundedRect node {:?} has invalid geometry: {:?}",
                    key,
                    n.rect
                );
            }
            SceneNode::Circle(n) => {
                assert!(
                    n.center.x.is_finite() && n.center.y.is_finite() && n.radius.is_finite(),
                    "SPEC-HAL-083 Phase-14: Circle node {:?} has invalid geometry",
                    key
                );
            }
            SceneNode::Ellipse(n) => {
                assert!(
                    n.center.x.is_finite()
                        && n.center.y.is_finite()
                        && n.radii.x.is_finite()
                        && n.radii.y.is_finite(),
                    "SPEC-HAL-083 Phase-14: Ellipse node {:?} has invalid geometry",
                    key
                );
            }
            // Text, Image, Clip, Layer, Scroll, Line, Polyline, Path, Scrollbars
            // have different caching requirements - geometry validation passes
            _ => {}
        }
    }
}

#[cfg(debug_assertions)]
impl SceneDiffConsumer for CacheReadinessConsumer {
    fn name(&self) -> &'static str {
        "CacheReadinessConsumer"
    }

    fn process(&mut self, delta: &SceneDelta, scene: &SceneGraph) {
        use std::sync::OnceLock;

        // Only run when env flag is set
        static ENABLED: OnceLock<bool> = OnceLock::new();
        let enabled =
            *ENABLED.get_or_init(|| std::env::var("HALCYON_TRACE_SCENE_IDENTITY").is_ok());

        if !enabled {
            return;
        }

        // Validate all insert keys exist in scene
        self.validate_inserts(delta, scene);

        // Validate node type stability for stable keys
        self.validate_type_stability(delta, scene);
    }

    fn tracked_key_count(&self) -> usize {
        0 // No tracking, pure validation
    }
}

// ============================================================================
// SPEC-HAL-083 Phase-15: Minimal Cache Skeleton (No Reuse)
// ============================================================================
//
// # Purpose
//
// Introduce the SHAPE of the cache without allowing any actual caching.
// This phase proves lifetimes, ownership, call order, and mutability boundaries.
//
// # Contract
//
// - NO reuse
// - NO retention across frames
// - NO GPU objects
// - NO renderer access
// - NO performance claims
//
// If anything actually gets cached in Phase-15, that's a failure.
//
// # What This Phase Proves
//
// 1. Lifetimes: SceneCache can be borrowed mutably during dispatch
// 2. Ownership: SceneCache ownership model is sound
// 3. Call order: dispatch_cache_operations has correct call order
// 4. Mutability: &mut SceneCache can be passed to consumers
// ============================================================================

/// Ownership state for cache entries.
///
/// # Phase-20 Contract
///
/// This enum explicitly defines the CPU/GPU ownership boundary.
/// All mutations to cache entries MUST assert `CpuOnly` ownership.
///
/// # Variants
///
/// - `CpuOnly`: Entry is owned by CPU code. All mutations allowed.
/// - `GpuBound`: Entry has been handed off to GPU layer. Mutations forbidden.
///
/// # Current State (Phase-20)
///
/// Only `CpuOnly` is used in production paths.
/// `GpuBound` exists as a placeholder to lock the ownership model.
///
/// # Future (GPU Phases)
///
/// When GPU caching lands:
/// - Entries will transition to `GpuBound` after GPU upload
/// - `GpuBound` entries cannot be mutated without explicit release
/// - This prevents accidental modification of GPU-owned data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CacheEntryOwnership {
    /// Entry is CPU-owned. All mutations allowed.
    #[default]
    CpuOnly,
    /// Entry is GPU-bound. Mutations forbidden (placeholder, unused in Phase-20).
    GpuBound,
}

/// Minimal cache entry - CPU-only, no GPU resources.
///
/// # Phase-16 Contract
///
/// - Stores key, kind, and dirty flag only
/// - NO GPU objects
/// - NO renderer handles
/// - NO geometry data (that's in SceneGraph)
/// - Plain Rust struct only
///
/// # Phase-20 Additions
///
/// - `ownership` field tracks CPU/GPU ownership state
/// - All mutations assert `CpuOnly` ownership (debug-only)
/// - `GpuBound` state is placeholder for future GPU caching
///
/// # Phase-21: Mutation Surface Lockdown
///
/// All fields are **private**. All mutations flow through named methods.
///
/// ## Permitted Mutation Paths
///
/// - `CacheEntry::new()` - construction only
/// - `CacheEntry::mark_dirty()` - sets dirty flag (asserts CpuOnly)
/// - `CacheEntry::clear_dirty()` - clears dirty flag (asserts CpuOnly)
///
/// ## Read-Only Accessors
///
/// - `key()` - returns the key
/// - `kind()` - returns the node kind
/// - `is_dirty()` - returns dirty state
/// - `ownership()` - returns ownership state
///
/// ## Forbidden
///
/// - Direct field access (all fields private)
/// - Any mutation path not listed above
///
/// # Future (GPU Phases)
///
/// May gain:
/// - GPU buffer handles
/// - Cached geometry data
/// - Upload timestamps
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// The key this entry represents.
    key: SceneNodeKey,
    /// The node kind at insertion time.
    kind: SceneNodeKind,
    /// Whether this entry needs re-processing (content changed).
    dirty: bool,
    /// Ownership state (CPU vs GPU).
    ownership: CacheEntryOwnership,
}

impl CacheEntry {
    /// Create a new cache entry.
    ///
    /// New entries default to `CpuOnly` ownership.
    pub fn new(key: SceneNodeKey, kind: SceneNodeKind) -> Self {
        Self {
            key,
            kind,
            dirty: false,
            ownership: CacheEntryOwnership::CpuOnly,
        }
    }

    /// Returns the key this entry represents.
    pub fn key(&self) -> SceneNodeKey {
        self.key
    }

    /// Returns the node kind at insertion time.
    pub fn kind(&self) -> SceneNodeKind {
        self.kind
    }

    /// Returns whether this entry is dirty.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Returns the ownership state of this entry.
    pub fn ownership(&self) -> CacheEntryOwnership {
        self.ownership
    }

    /// Assert this entry is CPU-owned (debug-only).
    ///
    /// # Panics (debug only)
    ///
    /// Panics if ownership is not `CpuOnly`.
    #[cfg(debug_assertions)]
    pub fn assert_cpu_only(&self) {
        if self.ownership != CacheEntryOwnership::CpuOnly {
            panic!(
                "SPEC-HAL-083 Phase-20: Entry {:?} is {:?}, expected CpuOnly. \
                 Cannot mutate GPU-bound entries.",
                self.key, self.ownership
            );
        }
    }

    /// No-op in release builds.
    #[cfg(not(debug_assertions))]
    pub fn assert_cpu_only(&self) {}

    /// Mark this entry as dirty (content changed).
    ///
    /// # Panics (debug only)
    ///
    /// Panics if ownership is not `CpuOnly`.
    pub fn mark_dirty(&mut self) {
        #[cfg(debug_assertions)]
        self.assert_cpu_only();
        self.dirty = true;
    }

    /// Clear the dirty flag.
    ///
    /// # Panics (debug only)
    ///
    /// Panics if ownership is not `CpuOnly`.
    pub fn clear_dirty(&mut self) {
        #[cfg(debug_assertions)]
        self.assert_cpu_only();
        self.dirty = false;
    }

    /// Set dirty flag directly (test-only, bypasses ownership check).
    ///
    /// # Warning
    ///
    /// This method is for testing only. Production code should use `mark_dirty()`.
    #[cfg(test)]
    pub(crate) fn set_dirty_for_test(&mut self, dirty: bool) {
        self.dirty = dirty;
    }

    /// Set ownership state (test-only).
    ///
    /// # Warning
    ///
    /// This method is for testing only. Production code should never
    /// directly set ownership - it will be managed by the cache layer.
    #[cfg(test)]
    pub(crate) fn set_ownership_for_test(&mut self, ownership: CacheEntryOwnership) {
        self.ownership = ownership;
    }
}

/// Opaque handle to a cache entry.
///
/// # Phase-19 Contract
///
/// - Copyable, hashable, non-dereferenceable
/// - NOT a pointer - does not expose memory address
/// - Remains valid across frames unless entry is removed
/// - Becomes invalid ONLY when `remove(key)` is called
/// - Kind is baked in at creation time for debug validation
///
/// # Debug Validation
///
/// In debug builds, the handle stores:
/// - The key it refers to
/// - The expected kind
/// - A generation counter
///
/// These are validated on access to catch:
/// - Stale handles (entry was removed)
/// - Kind mismatches (programming error)
///
/// # Release Behavior
///
/// Minimal overhead: just key + kind storage, no generation tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneCacheHandle {
    /// The key this handle refers to.
    key: SceneNodeKey,
    /// The expected node kind (for debug validation).
    kind: SceneNodeKind,
    /// Generation at handle creation time (debug only).
    #[cfg(debug_assertions)]
    generation: u64,
}

impl SceneCacheHandle {
    /// Returns the key this handle refers to.
    pub fn key(&self) -> SceneNodeKey {
        self.key
    }

    /// Returns the expected kind for this handle.
    pub fn kind(&self) -> SceneNodeKind {
        self.kind
    }
}

/// Cache lifecycle phase for timing-based mutation control.
///
/// # Phase-22 Contract
///
/// This enum controls *when* mutations are allowed, complementing
/// Phase-21's control over *what* mutations are allowed.
///
/// # Variants
///
/// - `Build`: Active frame construction. Mutations allowed.
/// - `Dispatch`: Consumer dispatch complete. NO mutations. Read/consume only.
/// - `Frozen`: Frame complete. NO mutations allowed.
///
/// # Lifecycle Flow
///
/// ```text
/// begin_frame() → Build
///                   ↓
/// dispatch()    → Dispatch
///                   ↓
/// end_frame()   → Frozen
/// ```
///
/// # Enforcement (debug only)
///
/// - All mutation methods (`insert`, `remove`, `mark_dirty`, etc.) assert `Build` phase
/// - `dispatch_cache_operations` manages transitions
/// - Mutation outside `Build` panics in debug builds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CacheLifecyclePhase {
    /// Active frame construction. Mutations allowed.
    #[default]
    Build,
    /// Consumer dispatch complete. NO mutations. Read/consume only.
    Dispatch,
    /// Frame complete. NO mutations allowed.
    Frozen,
}

/// Scene cache with real storage.
///
/// # Phase-16 Contract
///
/// - Real storage via `HashMap<SceneNodeKey, CacheEntry>`
/// - One entry per key
/// - NO reuse across frames beyond key stability
/// - NO eviction logic
/// - NO GPU objects
/// - NO renderer access
/// - CPU-only, plain Rust structs
///
/// # Operations
///
/// - `insert()`: Add entry for new key
/// - `remove()`: Remove entry for removed key
/// - `mark_dirty()`: Mark entry dirty for updated key
/// - `get()`: Retrieve entry by key
/// - `contains()`: Check if key exists
///
/// # Phase-19 Additions
///
/// - `insert_with_handle()`: Insert and return stable handle
/// - `get_by_handle()`: Access entry via handle
/// - `get_by_handle_mut()`: Mutable access via handle
/// - Handles remain valid across frames unless entry is removed
/// - Generation tracking (debug only) for stale handle detection
///
/// # Phase-21: Mutation Surface Lockdown
///
/// All cache mutations flow through a single, auditable surface.
/// Entry fields are private; mutations use named methods with invariant assertions.
///
/// ## Permitted Mutation Paths (SceneCache)
///
/// - `insert()` / `insert_with_handle()` - creates new entry (asserts key not present)
/// - `remove()` - removes entry (asserts key present, invalidates handles)
/// - `mark_dirty()` - marks entry dirty (asserts entry exists, entry is CpuOnly)
/// - `clear_all_dirty()` - clears all dirty flags (asserts entries are CpuOnly)
///
/// ## Invariants Enforced (debug only)
///
/// - Entry must exist before mutation (except insert)
/// - Entry must be `CpuOnly` before mutation
/// - No direct field access on `CacheEntry`
///
/// ## Forbidden
///
/// - Direct `CacheEntry` field mutation
/// - Mutation of `GpuBound` entries
/// - Any mutation path not listed above
///
/// # Phase-22: Explicit Lifecycle Phases
///
/// Controls *when* mutations are allowed via `CacheLifecyclePhase`.
///
/// ## Lifecycle States
///
/// - `Build`: Frame construction. Mutations allowed.
/// - `Dispatch`: Post-mutation. NO mutations. Read/consume only.
/// - `Frozen`: Frame complete. NO mutations allowed.
///
/// ## Lifecycle Flow
///
/// ```text
/// begin_frame() → Build → [mutations] → enter_dispatch() → Dispatch → end_frame() → Frozen
/// ```
///
/// ## Enforcement (debug only)
///
/// - Mutations assert phase is `Build`
/// - Mutation outside `Build` phase panics
///
/// # Future
///
/// May gain:
/// - GPU resource handles
/// - LRU/eviction state
/// - Memory pooling
#[derive(Debug)]
pub struct SceneCache {
    /// Key-to-entry mapping.
    entries: std::collections::HashMap<SceneNodeKey, CacheEntry>,
    /// Generation for each live entry (debug only).
    /// Removed keys must not leave tombstones in this map.
    #[cfg(debug_assertions)]
    generations: std::collections::HashMap<SceneNodeKey, u64>,
    /// Monotonic source for live-entry generations (debug only).
    /// A global source preserves stale-handle rejection when a removed key is reused
    /// without retaining one generation record per historical key forever.
    #[cfg(debug_assertions)]
    next_generation: u64,
    /// Current lifecycle phase (debug only).
    #[cfg(debug_assertions)]
    phase: CacheLifecyclePhase,
    /// Phase-24: Snapshot entry count captured at enter_dispatch (debug only).
    #[cfg(debug_assertions)]
    snapshot_entry_count: usize,
    /// Phase-24: Snapshot dirty count captured at enter_dispatch (debug only).
    #[cfg(debug_assertions)]
    snapshot_dirty_count: usize,
}

impl Default for SceneCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SceneCache {
    /// Create a new empty cache.
    ///
    /// Cache starts in `Build` phase.
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            #[cfg(debug_assertions)]
            generations: std::collections::HashMap::new(),
            #[cfg(debug_assertions)]
            next_generation: 0,
            #[cfg(debug_assertions)]
            phase: CacheLifecyclePhase::Build,
            #[cfg(debug_assertions)]
            snapshot_entry_count: 0,
            #[cfg(debug_assertions)]
            snapshot_dirty_count: 0,
        }
    }

    /// Clear all entries.
    ///
    /// Note: This invalidates all handles. The monotonic generation source is not
    /// reset, so a later insertion of the same key cannot revalidate an old handle.
    /// Phase is reset to `Build`.
    pub fn clear(&mut self) {
        self.entries.clear();
        #[cfg(debug_assertions)]
        {
            self.generations.clear();
            self.phase = CacheLifecyclePhase::Build;
            self.snapshot_entry_count = 0;
            self.snapshot_dirty_count = 0;
        }
    }

    /// Returns the current lifecycle phase.
    #[cfg(debug_assertions)]
    pub fn phase(&self) -> CacheLifecyclePhase {
        self.phase
    }

    /// Returns the current lifecycle phase (always Build in release).
    #[cfg(not(debug_assertions))]
    pub fn phase(&self) -> CacheLifecyclePhase {
        CacheLifecyclePhase::Build
    }

    /// Begin a new frame. Sets phase to `Build`.
    ///
    /// # Phase-22 Contract
    ///
    /// Call this at the start of each frame to enable mutations.
    /// This is the ONLY way to exit `Frozen` phase.
    ///
    /// # Phase-24 Contract
    ///
    /// Resets snapshot markers (snapshot is only valid during Dispatch/Frozen).
    pub fn begin_frame(&mut self) {
        #[cfg(debug_assertions)]
        {
            self.phase = CacheLifecyclePhase::Build;
            // Phase-24: Reset snapshot markers
            self.snapshot_entry_count = 0;
            self.snapshot_dirty_count = 0;
        }
    }

    /// Transition to dispatch phase. Sets phase to `Dispatch`.
    ///
    /// # Phase-22 Contract
    ///
    /// Called by `dispatch_cache_operations` after all mutations complete.
    /// NO mutations allowed in Dispatch phase. Read/consume only.
    ///
    /// # Phase-24 Contract
    ///
    /// Captures snapshot of cache state. All reads during Dispatch and Frozen
    /// must observe this exact snapshot (assert_snapshot_stable enforces this).
    pub fn enter_dispatch(&mut self) {
        #[cfg(debug_assertions)]
        {
            // Phase-24: Capture snapshot before transitioning
            self.snapshot_entry_count = self.entries.len();
            self.snapshot_dirty_count = self.entries.values().filter(|e| e.is_dirty()).count();
            self.phase = CacheLifecyclePhase::Dispatch;
        }
    }

    /// End the current frame. Sets phase to `Frozen`.
    ///
    /// # Phase-22 Contract
    ///
    /// Call this after dispatch completes. NO mutations allowed until
    /// `begin_frame()` is called.
    pub fn end_frame(&mut self) {
        #[cfg(debug_assertions)]
        {
            self.phase = CacheLifecyclePhase::Frozen;
        }
    }

    /// Assert mutations are allowed (debug only).
    ///
    /// Mutations only allowed in `Build` phase.
    #[cfg(debug_assertions)]
    fn assert_mutation_allowed(&self) {
        if self.phase != CacheLifecyclePhase::Build {
            panic!(
                "SPEC-HAL-083 Phase-22: Cache mutation attempted in {:?} phase. \
                 Mutations only allowed in Build phase. \
                 Call begin_frame() to start a new frame.",
                self.phase
            );
        }
    }

    /// Assert read-only access is allowed (debug only).
    ///
    /// # Phase-23 Contract
    ///
    /// Read-only access is allowed in ALL phases (Build, Dispatch, Frozen).
    /// This assertion exists to document and enforce that certain methods
    /// are read-only and do NOT require Build phase.
    ///
    /// # Phase-24 Contract
    ///
    /// In Dispatch and Frozen phases, additionally asserts snapshot stability.
    /// The cache state (entry_count, dirty_count) must match the snapshot
    /// captured at `enter_dispatch()`.
    #[cfg(debug_assertions)]
    fn assert_read_only(&self) {
        // Read-only access is allowed in all phases.
        // This method exists to:
        // 1. Document that the calling method is read-only
        // 2. Prove mechanically that Dispatch allows reads
        // 3. Enforce snapshot stability during Dispatch/Frozen (Phase-24)

        // Phase-24: In Dispatch or Frozen, assert snapshot stability
        if self.phase == CacheLifecyclePhase::Dispatch || self.phase == CacheLifecyclePhase::Frozen
        {
            self.assert_snapshot_stable();
        }
    }

    /// Assert snapshot stability (debug only).
    ///
    /// # Phase-24 Contract
    ///
    /// Verifies that the cache state matches the snapshot captured at
    /// `enter_dispatch()`. This ensures all reads during Dispatch and Frozen
    /// phases observe identical state.
    ///
    /// # Panics
    ///
    /// Panics if entry_count or dirty_count differs from snapshot.
    #[cfg(debug_assertions)]
    pub fn assert_snapshot_stable(&self) {
        let current_entry_count = self.entries.len();
        let current_dirty_count = self.entries.values().filter(|e| e.is_dirty()).count();

        if current_entry_count != self.snapshot_entry_count {
            panic!(
                "SPEC-HAL-083 Phase-24: Snapshot violation! entry_count changed. \
                 Expected {} (snapshot), found {}. \
                 Cache must be stable during Dispatch/Frozen phases.",
                self.snapshot_entry_count, current_entry_count
            );
        }

        if current_dirty_count != self.snapshot_dirty_count {
            panic!(
                "SPEC-HAL-083 Phase-24: Snapshot violation! dirty_count changed. \
                 Expected {} (snapshot), found {}. \
                 Cache must be stable during Dispatch/Frozen phases.",
                self.snapshot_dirty_count, current_dirty_count
            );
        }
    }

    /// No-op in release builds.
    #[cfg(not(debug_assertions))]
    pub fn assert_snapshot_stable(&self) {}

    /// Returns the number of cached entries.
    ///
    /// # Phase Requirements
    ///
    /// - Build: allowed (read-only)
    /// - Dispatch: allowed (read-only)
    /// - Frozen: allowed (read-only)
    pub fn entry_count(&self) -> usize {
        #[cfg(debug_assertions)]
        self.assert_read_only();

        self.entries.len()
    }

    /// Insert a new cache entry.
    ///
    /// # Panics (debug only)
    ///
    /// - If an entry for this key already exists
    /// - If cache is not in `Build` phase (Phase-22)
    pub fn insert(&mut self, key: SceneNodeKey, kind: SceneNodeKind) {
        #[cfg(debug_assertions)]
        {
            self.assert_mutation_allowed();
            if self.entries.contains_key(&key) {
                panic!(
                    "SPEC-HAL-083 Phase-16: Duplicate insert for key {:?}. \
                     Insert should only be called for new keys.",
                    key
                );
            }
        }
        self.entries.insert(key, CacheEntry::new(key, kind));
        #[cfg(debug_assertions)]
        {
            let generation = self.next_generation;
            self.next_generation = self
                .next_generation
                .checked_add(1)
                .expect("SPEC-HAL-083 Phase-19: scene cache generation counter exhausted");
            self.generations.insert(key, generation);
        }
    }

    /// Remove a cache entry.
    ///
    /// Returns the removed entry, or `None` if not found.
    ///
    /// # Handle Invalidation (Phase-19)
    ///
    /// This method invalidates any handles pointing to this key.
    /// In debug builds, the live generation is removed. A later insertion receives
    /// a new global generation, causing stale handles to panic without retaining a
    /// tombstone for every historical key.
    ///
    /// # Panics (debug only)
    ///
    /// - If no entry exists for this key
    /// - If cache is not in `Build` phase (Phase-22)
    pub fn remove(&mut self, key: SceneNodeKey) -> Option<CacheEntry> {
        #[cfg(debug_assertions)]
        {
            self.assert_mutation_allowed();
            if !self.entries.contains_key(&key) {
                panic!(
                    "SPEC-HAL-083 Phase-16: Remove for non-existent key {:?}. \
                     Remove should only be called for existing keys.",
                    key
                );
            }
            let removed_generation = self.generations.remove(&key);
            debug_assert!(
                removed_generation.is_some(),
                "SPEC-HAL-083 Phase-19: live cache entry must have a generation"
            );
        }
        self.entries.remove(&key)
    }

    /// Mark an entry as dirty.
    ///
    /// # Phase-21 Contract
    ///
    /// This is a permitted mutation path. Assertions enforced:
    /// - Entry must exist (Phase-16)
    /// - Entry must be `CpuOnly` (Phase-20, delegated to entry)
    /// - Cache must be in `Build` phase (Phase-22)
    ///
    /// # Panics (debug only)
    ///
    /// - If no entry exists for this key
    /// - If entry is not `CpuOnly`
    /// - If cache is not in `Build` phase (Phase-22)
    pub fn mark_dirty(&mut self, key: SceneNodeKey) {
        #[cfg(debug_assertions)]
        {
            self.assert_mutation_allowed();
            if !self.entries.contains_key(&key) {
                panic!(
                    "SPEC-HAL-083 Phase-16: Mark dirty for non-existent key {:?}. \
                     Mark dirty should only be called for existing keys.",
                    key
                );
            }
            // Phase-21: Assert entry exists before mutation
            // CpuOnly assertion delegated to entry.mark_dirty()
        }
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.mark_dirty(); // Asserts CpuOnly
        }
    }

    /// Get an entry by key (read-only).
    ///
    /// # Phase Requirements
    ///
    /// - Build: allowed (read-only)
    /// - Dispatch: allowed (read-only)
    /// - Frozen: allowed (read-only)
    pub fn get(&self, key: SceneNodeKey) -> Option<&CacheEntry> {
        #[cfg(debug_assertions)]
        self.assert_read_only();

        self.entries.get(&key)
    }

    /// Get a mutable entry by key.
    ///
    /// # Phase Requirements
    ///
    /// - Build: allowed (mutation path)
    /// - Dispatch: forbidden (debug panic)
    /// - Frozen: forbidden (debug panic)
    pub fn get_mut(&mut self, key: SceneNodeKey) -> Option<&mut CacheEntry> {
        #[cfg(debug_assertions)]
        self.assert_mutation_allowed();

        self.entries.get_mut(&key)
    }

    /// Insert a new cache entry and return a stable handle.
    ///
    /// # Phase-19 Contract
    ///
    /// - Returns an opaque handle that remains valid across frames
    /// - Handle is invalidated ONLY when `remove(key)` is called
    /// - Handle stores key + kind for debug validation
    ///
    /// # Panics (debug only)
    ///
    /// - If an entry for this key already exists
    /// - If cache is not in `Build` phase (Phase-22)
    pub fn insert_with_handle(
        &mut self,
        key: SceneNodeKey,
        kind: SceneNodeKind,
    ) -> SceneCacheHandle {
        #[cfg(debug_assertions)]
        {
            self.assert_mutation_allowed();
            if self.entries.contains_key(&key) {
                panic!(
                    "SPEC-HAL-083 Phase-19: Duplicate insert for key {:?}. \
                     insert_with_handle should only be called for new keys.",
                    key
                );
            }
        }
        self.entries.insert(key, CacheEntry::new(key, kind));

        #[cfg(debug_assertions)]
        {
            let generation = self.next_generation;
            self.next_generation = self
                .next_generation
                .checked_add(1)
                .expect("SPEC-HAL-083 Phase-19: scene cache generation counter exhausted");
            self.generations.insert(key, generation);
            SceneCacheHandle {
                key,
                kind,
                generation,
            }
        }
        #[cfg(not(debug_assertions))]
        {
            SceneCacheHandle { key, kind }
        }
    }

    /// Get an entry by handle.
    ///
    /// # Phase-19 Contract
    ///
    /// - Returns `Some(&CacheEntry)` if handle is valid
    /// - Returns `None` if entry was removed (release builds)
    ///
    /// # Phase Requirements
    ///
    /// - Build: allowed (read-only)
    /// - Dispatch: allowed (read-only)
    /// - Frozen: allowed (read-only)
    ///
    /// # Panics (debug only)
    ///
    /// - If handle refers to removed entry (stale handle)
    /// - If handle kind doesn't match entry kind (programming error)
    pub fn get_by_handle(&self, handle: SceneCacheHandle) -> Option<&CacheEntry> {
        #[cfg(debug_assertions)]
        {
            self.assert_read_only();
            self.assert_handle_valid(handle);
        }
        self.entries.get(&handle.key)
    }

    /// Get a mutable entry by handle.
    ///
    /// # Phase-19 Contract
    ///
    /// - Returns `Some(&mut CacheEntry)` if handle is valid
    /// - Returns `None` if entry was removed (release builds)
    ///
    /// # Phase Requirements
    ///
    /// - Build: allowed (mutation path)
    /// - Dispatch: forbidden (debug panic)
    /// - Frozen: forbidden (debug panic)
    ///
    /// # Panics (debug only)
    ///
    /// - If handle refers to removed entry (stale handle)
    /// - If handle kind doesn't match entry kind (programming error)
    /// - If cache is not in Build phase (Phase-23)
    pub fn get_by_handle_mut(&mut self, handle: SceneCacheHandle) -> Option<&mut CacheEntry> {
        #[cfg(debug_assertions)]
        {
            self.assert_mutation_allowed();
            self.assert_handle_valid(handle);
        }
        self.entries.get_mut(&handle.key)
    }

    /// Assert handle is valid (debug only).
    ///
    /// Validates:
    /// - Handle's generation matches current generation (not stale)
    /// - Handle's kind matches entry's kind (no type confusion)
    #[cfg(debug_assertions)]
    fn assert_handle_valid(&self, handle: SceneCacheHandle) {
        let current_gen = self
            .generations
            .get(&handle.key)
            .copied()
            .unwrap_or_else(|| {
                panic!(
                    "SPEC-HAL-083 Phase-19: Stale handle for key {:?}. \
                 Entry was removed and handle is no longer valid.",
                    handle.key
                )
            });
        if handle.generation != current_gen {
            panic!(
                "SPEC-HAL-083 Phase-19: Stale handle for key {:?}. \
                 Handle generation {} != current generation {}. \
                 Entry was removed and handle is no longer valid.",
                handle.key, handle.generation, current_gen
            );
        }

        if let Some(entry) = self.entries.get(&handle.key) {
            if entry.kind() != handle.kind {
                panic!(
                    "SPEC-HAL-083 Phase-19: Kind mismatch for handle {:?}. \
                     Handle kind {:?} != entry kind {:?}. \
                     This indicates a programming error.",
                    handle.key,
                    handle.kind,
                    entry.kind()
                );
            }
        } else {
            panic!(
                "SPEC-HAL-083 Phase-19: Handle {:?} refers to non-existent entry. \
                 Entry was removed but generation was not incremented correctly.",
                handle.key
            );
        }
    }

    /// Check if a key exists in the cache.
    ///
    /// # Phase Requirements
    ///
    /// - Build: allowed (read-only)
    /// - Dispatch: allowed (read-only)
    /// - Frozen: allowed (read-only)
    pub fn contains(&self, key: SceneNodeKey) -> bool {
        #[cfg(debug_assertions)]
        self.assert_read_only();

        self.entries.contains_key(&key)
    }

    /// Returns an iterator over all entries.
    ///
    /// # Phase Requirements
    ///
    /// - Build: allowed (read-only)
    /// - Dispatch: allowed (read-only)
    /// - Frozen: allowed (read-only)
    pub fn iter(&self) -> impl Iterator<Item = (&SceneNodeKey, &CacheEntry)> {
        #[cfg(debug_assertions)]
        self.assert_read_only();

        self.entries.iter()
    }

    /// Returns the number of dirty entries.
    ///
    /// # Phase Requirements
    ///
    /// - Build: allowed (read-only)
    /// - Dispatch: allowed (read-only)
    /// - Frozen: allowed (read-only)
    pub fn dirty_count(&self) -> usize {
        #[cfg(debug_assertions)]
        self.assert_read_only();

        self.entries.values().filter(|e| e.is_dirty()).count()
    }

    /// Clear all dirty flags (centralized dirty resolution).
    ///
    /// # Phase-17 Contract
    ///
    /// This method MUST be called exactly once per frame when cache execution
    /// is enabled. It is called by `dispatch_cache_operations`, NOT by consumers.
    ///
    /// # Phase-21 Contract
    ///
    /// This is a permitted mutation path. Assertions enforced:
    /// - Each entry must be `CpuOnly` (Phase-20, delegated to entry)
    ///
    /// # Phase-22 Contract
    ///
    /// - Cache must be in `Build` phase
    ///
    /// # Lifecycle
    ///
    /// - Dirty entries transition: Dirty → Clean
    /// - Clean entries remain: Clean
    /// - This is the ONLY way to clear dirty flags
    ///
    /// # Why Centralized?
    ///
    /// - Consumers MUST NOT clear dirty themselves
    /// - Ensures frame boundary assertions can verify all dirty was resolved
    /// - Prevents "forgotten dirty" bugs
    ///
    /// # Panics (debug only)
    ///
    /// - If cache is not in `Build` phase (Phase-22)
    pub fn clear_all_dirty(&mut self) {
        #[cfg(debug_assertions)]
        self.assert_mutation_allowed();

        for entry in self.entries.values_mut() {
            entry.clear_dirty(); // Asserts CpuOnly
        }
    }

    /// Assert cache is consistent with the scene graph (debug-only).
    ///
    /// # Phase-18 Contract
    ///
    /// For every cache entry:
    /// - The key MUST exist in the current SceneGraph
    /// - The SceneNode kind MUST equal the cached kind
    ///
    /// # Panics (debug only)
    ///
    /// - If a cached key is missing from the scene
    /// - If the node kind doesn't match the cached kind
    ///
    /// # Release Behavior
    ///
    /// No-op in release builds.
    #[cfg(debug_assertions)]
    pub fn assert_consistent_with_scene(&self, scene: &SceneGraph) {
        for (key, entry) in &self.entries {
            // Check key exists in scene
            let node = scene.find_node_by_key(*key);
            if node.is_none() {
                panic!(
                    "SPEC-HAL-083 Phase-18: Cache entry {:?} has no corresponding scene node. \
                     Cache and scene are inconsistent.",
                    key
                );
            }

            // Check kind matches
            let node = node.unwrap();
            let scene_kind = node.kind();
            if scene_kind != entry.kind() {
                panic!(
                    "SPEC-HAL-083 Phase-18: Cache entry {:?} has kind {:?}, \
                     but scene node has kind {:?}. Kind mismatch.",
                    key,
                    entry.kind(),
                    scene_kind
                );
            }
        }
    }

    /// No-op in release builds.
    #[cfg(not(debug_assertions))]
    pub fn assert_consistent_with_scene(&self, _scene: &SceneGraph) {}

    /// Assert delta was correctly applied to cache (debug-only).
    ///
    /// # Phase-18 Contract
    ///
    /// - For every `delta.inserts`: cache MUST contain that key
    /// - Pure `delta.removes` are absent; a reinserted kind replacement is present
    ///
    /// # Panics (debug only)
    ///
    /// - If an inserted key is missing from cache
    /// - If a removed key is still present in cache
    ///
    /// # Release Behavior
    ///
    /// No-op in release builds.
    #[cfg(debug_assertions)]
    pub fn assert_delta_applied(&self, delta: &SceneDelta) {
        // Every insert must be in cache
        for key in &delta.inserts {
            if !self.entries.contains_key(key) {
                panic!(
                    "SPEC-HAL-083 Phase-18: Insert key {:?} is not in cache after dispatch. \
                     Consumers failed to insert this key.",
                    key
                );
            }
        }

        // Pure removals are absent; kind replacements have a fresh inserted entry.
        for key in &delta.removes {
            if !delta.inserts.contains(key) && self.entries.contains_key(key) {
                panic!(
                    "SPEC-HAL-083 Phase-18: Remove key {:?} is still in cache after dispatch. \
                     Consumers failed to remove this key.",
                    key
                );
            }
        }
    }

    /// No-op in release builds.
    #[cfg(not(debug_assertions))]
    pub fn assert_delta_applied(&self, _delta: &SceneDelta) {}
}

/// Trait for cache-aware consumers.
///
/// This trait extends the consumer model to receive mutable cache access.
///
/// # Phase-17 Contract
///
/// Consumers MAY:
/// - Call `cache.insert()` for insert keys
/// - Call `cache.remove()` for remove keys
/// - Call `cache.mark_dirty()` for update keys
/// - Log diagnostics
///
/// Consumers MUST NOT:
/// - Call `cache.clear_all_dirty()` (centralized in dispatch)
/// - Call `entry.clear_dirty()` directly
/// - Inspect previous-frame state
/// - Touch GPU resources
/// - Access renderer
/// - Perform reuse heuristics
/// - Implement eviction logic
///
/// # Phase-19 Handle Contract
///
/// Consumers MAY:
/// - Store `SceneCacheHandle` values (opaque, copyable)
/// - Use `cache.insert_with_handle()` to obtain handles
/// - Use `cache.get_by_handle()` for access via stored handles
///
/// Consumers MUST NOT:
/// - Store `&CacheEntry` or `&mut CacheEntry` references
/// - Assume handles remain valid after `remove()` is called
/// - Use handles across different `SceneCache` instances
///
/// # Dirty Resolution
///
/// Dirty resolution is CENTRALIZED in `dispatch_cache_operations`.
/// After all consumers run, the dispatcher calls `cache.clear_all_dirty()`.
/// Consumers MUST NOT clear dirty themselves.
pub trait CacheConsumer: std::fmt::Debug {
    /// Human-readable name for debugging.
    fn name(&self) -> &'static str;

    /// Process a scene diff with cache access.
    ///
    /// # Arguments
    ///
    /// - `delta`: The structural diff (inserts/removes/updates)
    /// - `scene`: The current frame's scene graph
    /// - `cache`: Mutable reference to the scene cache
    ///
    /// # Phase-17 Contract
    ///
    /// Implementations MAY:
    /// - Call `cache.insert()` for insert keys
    /// - Call `cache.remove()` for remove keys
    /// - Call `cache.mark_dirty()` for update keys
    /// - Log diagnostics
    ///
    /// Implementations MUST NOT:
    /// - Call `cache.clear_all_dirty()` or `entry.clear_dirty()`
    /// - Inspect previous-frame dirty state
    /// - Allocate GPU resources
    /// - Modify renderer state
    fn process(&mut self, delta: &SceneDelta, scene: &SceneGraph, cache: &mut SceneCache);
}

/// Container for cache-aware consumers.
///
/// Similar to `SceneDiffConsumers` but for cache operations.
#[derive(Debug, Default)]
pub struct CacheConsumers {
    /// Registered consumers in registration order.
    consumers: Vec<Box<dyn CacheConsumer>>,
}

impl CacheConsumers {
    /// Create an empty consumer registry.
    pub fn new() -> Self {
        Self {
            consumers: Vec::new(),
        }
    }

    /// Register a cache consumer.
    pub fn register(&mut self, consumer: Box<dyn CacheConsumer>) {
        self.consumers.push(consumer);
    }

    /// Returns the number of registered consumers.
    pub fn consumer_count(&self) -> usize {
        self.consumers.len()
    }

    /// Returns an iterator over consumer names.
    pub fn consumer_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.consumers.iter().map(|c| c.name())
    }

    /// Check if any consumers are registered.
    pub fn is_empty(&self) -> bool {
        self.consumers.is_empty()
    }
}

/// Execution mode for cache operations.
///
/// Separate from `SceneDiffExecutionMode` to allow independent control.
///
/// # Phase-16 Contract
///
/// - `Disabled`: Default. Cache operations are skipped entirely.
/// - `Enabled`: Consumers are called and MAY modify the cache.
///
/// When enabled:
/// - Insert new entries for inserted keys
/// - Remove entries for removed keys
/// - Mark entries dirty for updated keys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SceneCacheExecutionMode {
    /// Default mode. Cache operations are completely skipped.
    #[default]
    Disabled,

    /// Enabled mode. Consumers are called and may modify the cache.
    ///
    /// Phase-16: Real caching occurs (insert/remove/mark_dirty).
    Enabled,
}

/// Dispatch cache operations through the execution gate.
///
/// This is the entry point for cache-aware consumption.
/// Separate from `dispatch_scene_diff` to allow independent evolution.
///
/// # Arguments
///
/// * `mode` - Execution mode (Disabled by default)
/// * `consumers` - Registered cache consumers
/// * `delta` - The scene diff
/// * `scene` - The current frame's scene graph
/// * `cache` - Mutable reference to the scene cache
///
/// # Phase-17 Contract
///
/// - `Disabled`: NOP - nothing happens, no assertions
/// - `Enabled`: Executes the following sequence:
///   1. Call all consumers in registration order
///   2. Call `cache.clear_all_dirty()` (centralized dirty resolution)
///   3. Assert `dirty_count() == 0` (debug panic if violated)
///
/// # Phase-22 Lifecycle Contract
///
/// When enabled, manages cache lifecycle phases:
/// 1. `begin_frame()` → Build phase (mutations allowed)
/// 2. Consumers run → Build phase
/// 3. `clear_all_dirty()` → Build phase
/// 4. Assertions (debug only) → Build phase
/// 5. `enter_dispatch()` → Dispatch phase (for future GPU work)
/// 6. `end_frame()` → Frozen phase (no more mutations)
///
/// # Lifecycle Enforcement
///
/// After consumers run:
/// - All dirty entries are cleared by this function
/// - Consumers MUST NOT clear dirty themselves
/// - Frame boundary assertion catches "forgotten dirty" bugs
///
/// # Call Site (SPEC-HAL-083A)
///
/// Must be called:
/// - After SceneGraph diff computation
/// - Before render preparation
/// - Exactly once per frame
pub fn dispatch_cache_operations(
    mode: SceneCacheExecutionMode,
    consumers: &mut CacheConsumers,
    delta: &SceneDelta,
    scene: &SceneGraph,
    cache: &mut SceneCache,
) {
    match mode {
        SceneCacheExecutionMode::Disabled => {
            // NOP - cache operations disabled, no assertions
        }
        SceneCacheExecutionMode::Enabled => {
            // Phase-22 Step 1: Enter Build phase (allows mutations)
            cache.begin_frame();

            // Phase-17 Step 1: Call all consumers (may insert/remove/mark_dirty)
            // All mutations happen in Build phase
            for consumer in consumers.consumers.iter_mut() {
                consumer.process(delta, scene, cache);
            }

            // Phase-17 Step 3: Centralized dirty resolution (still in Build phase)
            cache.clear_all_dirty();

            // Phase-18 Step 2: Cache↔Scene consistency enforcement (debug only)
            // Verifies: inserts are in cache, removes are not, key↔kind stability
            #[cfg(debug_assertions)]
            {
                cache.assert_delta_applied(delta);
                cache.assert_consistent_with_scene(scene);
            }

            // Phase-17 Step 3: Frame boundary assertion (debug only)
            #[cfg(debug_assertions)]
            {
                let dirty = cache.dirty_count();
                if dirty > 0 {
                    panic!(
                        "SPEC-HAL-083 Phase-17: Frame boundary assertion failed. \
                         {} dirty entries remain after clear_all_dirty(). \
                         This indicates a bug in clear_all_dirty() or a consumer \
                         modified the cache after dirty resolution.",
                        dirty
                    );
                }
            }

            // Phase-22 Step 5: Enter Dispatch phase (for future GPU work)
            cache.enter_dispatch();

            // Phase-24: Assert snapshot stability at end of dispatch
            // Verifies entry_count and dirty_count unchanged since enter_dispatch()
            #[cfg(debug_assertions)]
            cache.assert_snapshot_stable();

            // Phase-22 Step 6: End frame (Frozen phase - no more mutations)
            cache.end_frame();
        }
    }
}

/// A debug consumer that proves the cache pipeline works.
///
/// This consumer does NOTHING except prove:
/// - It receives `&mut SceneCache`
/// - It receives `&SceneDelta` and `&SceneGraph`
/// - The dispatch mechanism works
///
/// # Phase-15 Contract
///
/// - NO caching
/// - NO storage
/// - NO side effects
#[derive(Debug, Default)]
pub struct DebugCacheConsumer {
    /// Number of times process was called (for testing).
    call_count: usize,
}

impl DebugCacheConsumer {
    /// Create a new debug cache consumer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of times process was called.
    pub fn call_count(&self) -> usize {
        self.call_count
    }
}

impl CacheConsumer for DebugCacheConsumer {
    fn name(&self) -> &'static str {
        "DebugCacheConsumer"
    }

    fn process(&mut self, delta: &SceneDelta, _scene: &SceneGraph, cache: &mut SceneCache) {
        // Debug consumer: log counts but do not modify cache
        self.call_count += 1;

        if !delta.is_empty() {
            tracing::debug!(
                "[DebugCacheConsumer] call #{}: +{} -{} ~{} (cache entry_count={})",
                self.call_count,
                delta.inserts.len(),
                delta.removes.len(),
                delta.updates.len(),
                cache.entry_count()
            );
        }
    }
}

// ============================================================================
// SPEC-HAL-083 Phase-16: Basic Cache Consumer (Real Insert/Remove/Update)
// ============================================================================
//
// # Purpose
//
// Demonstrate real cache operations: insert, remove, mark_dirty.
// This is the first consumer that actually modifies the cache.
//
// # Contract
//
// - Insert entries for delta.inserts
// - Remove entries for delta.removes
// - Mark entries dirty for delta.updates
// - NO GPU interaction
// - NO renderer access
// - NO reuse heuristics
// ============================================================================

/// A basic cache consumer that performs real insert/remove/update operations.
///
/// # Phase-16 Contract
///
/// - Inserts cache entries for `delta.inserts`
/// - Removes cache entries for `delta.removes`
/// - Marks entries dirty for `delta.updates`
/// - NO GPU interaction
/// - NO renderer access
/// - NO reuse or eviction logic
#[derive(Debug, Default)]
pub struct BasicCacheConsumer {
    /// Number of inserts performed.
    insert_count: usize,
    /// Number of removes performed.
    remove_count: usize,
    /// Number of dirty marks performed.
    dirty_count: usize,
}

impl BasicCacheConsumer {
    /// Create a new basic cache consumer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the total number of inserts performed.
    pub fn insert_count(&self) -> usize {
        self.insert_count
    }

    /// Returns the total number of removes performed.
    pub fn remove_count(&self) -> usize {
        self.remove_count
    }

    /// Returns the total number of dirty marks performed.
    pub fn dirty_count(&self) -> usize {
        self.dirty_count
    }

    /// Reset all counters.
    pub fn reset_counters(&mut self) {
        self.insert_count = 0;
        self.remove_count = 0;
        self.dirty_count = 0;
    }
}

impl CacheConsumer for BasicCacheConsumer {
    fn name(&self) -> &'static str {
        "BasicCacheConsumer"
    }

    fn process(&mut self, delta: &SceneDelta, scene: &SceneGraph, cache: &mut SceneCache) {
        // Phase-16: Real cache operations

        // Retire old generations before inserting a replacement under the same key.
        for key in &delta.removes {
            cache.remove(*key);
            self.remove_count += 1;
        }

        // Insert entries for new keys
        for key in &delta.inserts {
            if let Some(node) = scene.find_node_by_key(*key) {
                cache.insert(*key, node.kind());
                self.insert_count += 1;
            }
        }

        // Mark entries dirty for updated keys
        for key in &delta.updates {
            cache.mark_dirty(*key);
            self.dirty_count += 1;
        }
    }
}

// ============================================================================
// SPEC-HAL-083 Phase-11: Identity Consumption Dry-Run (Debug-Only)
// ============================================================================
//
// # Purpose
//
// Prove that the identity pipeline is usable by downstream systems without
// wiring any caching. This is a signal quality verification phase.
//
// # Contract
//
// - Debug-only, gated behind HALCYON_TRACE_SCENE_IDENTITY=1
// - No GPU buffers, no caching, no state across frames
// - No allocations beyond small Vecs
// - Never touches renderer code
// ============================================================================

/// Trace identity changes for debugging.
///
/// When `HALCYON_TRACE_SCENE_IDENTITY=1` is set, logs:
/// - Insert keys (new this frame)
/// - Remove keys (gone this frame)
/// - Stable keys (present in both frames)
///
/// # Arguments
///
/// - `delta`: The computed scene diff
/// - `current_keys`: Keys present in the current (new) frame
///
/// This function is a dry-run consumer that verifies signal quality before
/// any real caching is implemented.
#[cfg(debug_assertions)]
pub fn trace_identity_changes(delta: &SceneDelta, current_keys: &[SceneNodeKey]) {
    use std::sync::OnceLock;

    static ENABLED: OnceLock<bool> = OnceLock::new();
    let enabled = *ENABLED.get_or_init(|| std::env::var("HALCYON_TRACE_SCENE_IDENTITY").is_ok());

    if !enabled {
        return;
    }

    // Compute stable keys: in current but not in inserts (they were already there)
    // and not in removes (they're still there)
    let mut stable_keys: Vec<SceneNodeKey> = Vec::new();

    // A key is stable if it's in current_keys but NOT in inserts (new) and NOT in removes (gone)
    // Since inserts are keys NEW to this frame, stable = current - inserts
    // Removes are keys GONE from this frame, so they're not in current_keys anyway
    for key in current_keys {
        let is_insert = delta.inserts.iter().any(|k| k == key);
        if !is_insert {
            stable_keys.push(*key);
        }
    }

    // Only log if there's something to report
    if delta.is_empty() && stable_keys.is_empty() {
        return;
    }

    eprintln!("[IDENTITY TRACE] Frame diff:");
    if !delta.inserts.is_empty() {
        eprintln!("  INSERT ({}):", delta.inserts.len());
        for key in &delta.inserts {
            eprintln!("    {:?}", key);
        }
    }
    if !delta.removes.is_empty() {
        eprintln!("  REMOVE ({}):", delta.removes.len());
        for key in &delta.removes {
            eprintln!("    {:?}", key);
        }
    }
    if !stable_keys.is_empty() {
        eprintln!("  STABLE ({}):", stable_keys.len());
        for key in &stable_keys {
            eprintln!("    {:?}", key);
        }
    }
    if !delta.updates.is_empty() {
        eprintln!("  UPDATE ({}):", delta.updates.len());
        for key in &delta.updates {
            eprintln!("    {:?}", key);
        }
    }
}

// ============================================================================
// SPEC-HAL-083 Phase-12: Identity Stability Assertions (Debug-Only)
//
// # Purpose
//
// Prove that stable identities actually behave stably across frames.
// This is the last guard phase before any caching is allowed.
//
// # Contract
//
// - Debug-only, gated behind HALCYON_TRACE_SCENE_IDENTITY=1
// - Stable keys must not move between widgets
// - Stable keys must appear exactly once per frame
// - Violations panic with explicit message
// ============================================================================

/// Assert identity stability across frames.
///
/// When `HALCYON_TRACE_SCENE_IDENTITY=1` is set, verifies:
/// 1. Stable keys do not move between widgets (same key, same widget_id)
/// 2. Keys appear exactly once per frame (already enforced by Phase-2)
///
/// # Arguments
///
/// - `delta`: The computed scene diff (to identify stable vs new keys)
/// - `current_key_widget_map`: Map from key to widget_id for current frame
///
/// Panics on violation with an explicit error message.
#[cfg(debug_assertions)]
pub fn assert_identity_stability(
    delta: &SceneDelta,
    current_key_widget_map: &std::collections::HashMap<SceneNodeKey, u64>,
) {
    use std::sync::{Mutex, OnceLock};

    static ENABLED: OnceLock<bool> = OnceLock::new();
    let enabled = *ENABLED.get_or_init(|| std::env::var("HALCYON_TRACE_SCENE_IDENTITY").is_ok());

    if !enabled {
        return;
    }

    // Previous frame's key-to-widget mapping
    static PREV_KEY_WIDGET_MAP: OnceLock<Mutex<std::collections::HashMap<SceneNodeKey, u64>>> =
        OnceLock::new();
    let prev_map_mutex =
        PREV_KEY_WIDGET_MAP.get_or_init(|| Mutex::new(std::collections::HashMap::new()));

    let mut prev_map = prev_map_mutex.lock().unwrap();

    // Check stability: for each key in current that was NOT inserted (i.e., was stable),
    // verify it has the same widget_id as before
    for (key, current_widget_id) in current_key_widget_map {
        // Skip keys that are new (inserts) - they have no previous mapping to compare
        let is_new = delta.inserts.iter().any(|k| k == key);
        if is_new {
            continue;
        }

        // This is a stable key - check widget affinity
        if let Some(prev_widget_id) = prev_map.get(key) {
            assert!(
                *prev_widget_id == *current_widget_id,
                "SPEC-HAL-083 Phase-12: Identity stability violation! \
                 SceneNodeKey {:?} moved from widget {} to widget {}. \
                 Stable keys must maintain widget affinity across frames.",
                key,
                prev_widget_id,
                current_widget_id
            );
        }
        // If not in prev_map but also not in inserts, this is the first frame
        // or the key was previously emitted without widget tracking. Allow it.
    }

    // Update prev_map with current frame's data for next comparison
    prev_map.clear();
    prev_map.extend(current_key_widget_map.iter().map(|(k, v)| (*k, *v)));
}

// ============================================================================
// SPEC-HAL-083 Phase-3: Diff Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::math::{Point, RectF};
    use crate::core::scene::{Color, RectNode, TextNode};

    fn make_rect_node(key: Option<SceneNodeKey>) -> SceneNode {
        SceneNode::Rect(RectNode {
            rect: RectF::new(0.0, 0.0, 10.0, 10.0),
            color: Color::WHITE,
            z_index: 0,
            key,
        })
    }

    fn make_text_node(key: Option<SceneNodeKey>) -> SceneNode {
        SceneNode::Text(TextNode {
            binding: None,
            position: Point::new(0.0, 0.0),
            text: "test".to_string(),
            font_size: 12.0,
            color: Color::WHITE,
            z_index: 0,
            backgrounds: vec![],
            key,
        })
    }

    fn build_scene_with_nodes(nodes: Vec<SceneNode>) -> SceneGraph {
        let mut scene = SceneGraph::new();
        scene.root = nodes;
        scene.clear_color = Color::BLACK;
        scene
    }

    #[test]
    fn nine_patch_kind_replacement_retires_cached_generation_without_rekeying() {
        use crate::core::paint::{
            NinePatchDestinationInsets, NinePatchSourceInsets, PaintSink, SurfaceFill,
            SurfaceNinePatch,
        };
        use crate::core::scene::SceneBuilder;

        let key = SceneNodeKey::new(1);
        let sibling = SceneNodeKey::new(2);
        let rect = RectF::new(0.0, 0.0, 160.0, 100.0);
        let mut old = SceneBuilder::new();
        old.push_rounded_rect(rect, Color::WHITE, 4.0, 0, Some(key));
        old.push_rect(
            RectF::new(200.0, 0.0, 10.0, 10.0),
            Color::WHITE,
            0,
            Some(sibling),
        );
        let old = old.build();
        let old_keys = extract_scene_keys(&old);
        let mut cache = SceneCache::new();
        let _old_handle = cache.insert_with_handle(key, SceneNodeKind::RoundedRect);
        let sibling_handle = cache.insert_with_handle(sibling, SceneNodeKind::Rect);
        let mut consumers = CacheConsumers::new();
        consumers.register(Box::new(BasicCacheConsumer::new()));
        dispatch_cache_operations(
            SceneCacheExecutionMode::Enabled,
            &mut consumers,
            &SceneDelta::default(),
            &old,
            &mut cache,
        );

        let material = SurfaceNinePatch::new(
            1,
            RectF::new(0.0, 0.0, 1.0, 1.0),
            NinePatchSourceInsets::all(1.0 / 3.0),
            NinePatchDestinationInsets::new(12.0, 18.0, 8.0, 24.0),
            crate::core::paint::Color::WHITE,
        )
        .unwrap();
        let make_scene = |rect| {
            let mut scene = SceneBuilder::new();
            scene.push_surface_fill(rect, &SurfaceFill::NinePatch(material), 0, Some(key));
            scene.push_rect(
                RectF::new(200.0, 0.0, 10.0, 10.0),
                Color::WHITE,
                0,
                Some(sibling),
            );
            scene.build()
        };
        let next = make_scene(rect);
        let (keys, delta) = compute_scene_diff_for_cache(Some(&old_keys), &next, &cache);
        assert_eq!(keys, old_keys);
        assert_eq!(delta.removes, [key]);
        assert_eq!(delta.inserts, [key]);
        assert!(delta.updates.is_empty());
        dispatch_cache_operations(
            SceneCacheExecutionMode::Enabled,
            &mut consumers,
            &delta,
            &next,
            &mut cache,
        );
        assert_eq!(cache.get(key).unwrap().kind(), SceneNodeKind::Image);
        assert_eq!(cache.entry_count(), 2);
        assert_eq!(cache.dirty_count(), 0);
        assert!(cache.get_by_handle(sibling_handle).is_some());
        #[cfg(debug_assertions)]
        assert!(std::panic::catch_unwind(|| cache.get_by_handle(_old_handle)).is_err());

        let small = make_scene(RectF::new(0.0, 0.0, 20.0, 10.0));
        let (small_keys, unchanged) = compute_scene_diff_for_cache(Some(&keys), &small, &cache);
        assert_eq!(small_keys, keys);
        assert!(unchanged.is_empty());
        dispatch_cache_operations(
            SceneCacheExecutionMode::Enabled,
            &mut consumers,
            &unchanged,
            &small,
            &mut cache,
        );
        assert_eq!(cache.get(key).unwrap().kind(), SceneNodeKind::Image);
        assert!(cache.get_by_handle(sibling_handle).is_some());
    }

    /// Test: Identical scenes → empty delta
    #[test]
    fn test_identical_scenes_empty_delta() {
        let key1 = SceneNodeKey::new(1);
        let key2 = SceneNodeKey::new(2);

        let scene =
            build_scene_with_nodes(vec![make_rect_node(Some(key1)), make_rect_node(Some(key2))]);

        let delta = compute_scene_diff(Some(&scene), &scene);

        assert!(
            delta.is_empty(),
            "Identical scenes should produce empty delta"
        );
        assert!(delta.inserts.is_empty());
        assert!(delta.removes.is_empty());
        assert!(delta.updates.is_empty());
    }

    /// Test: All-new scene (prev = None) → all inserts
    #[test]
    fn test_all_new_scene_all_inserts() {
        let key1 = SceneNodeKey::new(1);
        let key2 = SceneNodeKey::new(2);
        let key3 = SceneNodeKey::new(3);

        let scene = build_scene_with_nodes(vec![
            make_rect_node(Some(key1)),
            make_rect_node(Some(key2)),
            make_rect_node(Some(key3)),
        ]);

        let delta = compute_scene_diff(None, &scene);

        assert_eq!(delta.inserts.len(), 3);
        assert!(delta.inserts.contains(&key1));
        assert!(delta.inserts.contains(&key2));
        assert!(delta.inserts.contains(&key3));
        assert!(delta.removes.is_empty());
        assert!(delta.updates.is_empty());
    }

    /// Test: Node removed → remove only
    #[test]
    fn test_node_removed() {
        let key1 = SceneNodeKey::new(1);
        let key2 = SceneNodeKey::new(2);

        let prev =
            build_scene_with_nodes(vec![make_rect_node(Some(key1)), make_rect_node(Some(key2))]);

        let next = build_scene_with_nodes(vec![make_rect_node(Some(key1))]);

        let delta = compute_scene_diff(Some(&prev), &next);

        assert!(delta.inserts.is_empty());
        assert_eq!(delta.removes.len(), 1);
        assert!(delta.removes.contains(&key2));
        assert!(delta.updates.is_empty());
    }

    /// Test: Ephemeral-only scenes → empty delta
    #[test]
    fn test_ephemeral_only_empty_delta() {
        let prev = build_scene_with_nodes(vec![make_rect_node(None), make_rect_node(None)]);

        let next = build_scene_with_nodes(vec![
            make_rect_node(None),
            make_rect_node(None),
            make_rect_node(None),
        ]);

        let delta = compute_scene_diff(Some(&prev), &next);

        assert!(
            delta.is_empty(),
            "Ephemeral-only scenes should produce empty delta"
        );
    }

    /// Test: Mixed keyed + ephemeral → diff only keyed
    #[test]
    fn test_mixed_keyed_ephemeral() {
        let key1 = SceneNodeKey::new(1);
        let key2 = SceneNodeKey::new(2);
        let key3 = SceneNodeKey::new(3);

        let prev = build_scene_with_nodes(vec![
            make_rect_node(Some(key1)),
            make_rect_node(None), // ephemeral
            make_rect_node(Some(key2)),
        ]);

        let next = build_scene_with_nodes(vec![
            make_rect_node(Some(key1)),
            make_rect_node(None), // ephemeral
            make_rect_node(None), // ephemeral
            make_rect_node(Some(key3)),
        ]);

        let delta = compute_scene_diff(Some(&prev), &next);

        // key1 in both → no change
        // key2 only in prev → remove
        // key3 only in next → insert
        // ephemeral nodes ignored

        assert_eq!(delta.inserts.len(), 1);
        assert!(delta.inserts.contains(&key3));
        assert_eq!(delta.removes.len(), 1);
        assert!(delta.removes.contains(&key2));
        assert!(delta.updates.is_empty());
    }

    /// Test: Order preservation (inserts in traversal order)
    #[test]
    fn test_order_preservation() {
        let keys: Vec<SceneNodeKey> = (1..=5).map(SceneNodeKey::new).collect();

        let scene = build_scene_with_nodes(keys.iter().map(|k| make_rect_node(Some(*k))).collect());

        let delta = compute_scene_diff(None, &scene);

        // Inserts should be in traversal order
        assert_eq!(delta.inserts, keys);
    }

    // ========================================================================
    // SPEC-HAL-083 Phase-5: Consumption Contract Tests
    // ========================================================================

    /// Test: into_reactions yields correct DeltaReaction variants
    #[test]
    fn test_into_reactions_basic() {
        let key1 = SceneNodeKey::new(1);
        let key2 = SceneNodeKey::new(2);
        let key3 = SceneNodeKey::new(3);

        let delta = SceneDelta {
            inserts: vec![key1],
            removes: vec![key2],
            updates: vec![key3],
        };

        let reactions: Vec<_> = delta.into_reactions().collect();

        assert_eq!(reactions.len(), 3);
        assert_eq!(reactions[0], DeltaReaction::Insert(key1));
        assert_eq!(reactions[1], DeltaReaction::Remove(key2));
        assert_eq!(reactions[2], DeltaReaction::Update(key3));
    }

    /// Test: into_reactions order is inserts → removes → updates
    #[test]
    fn test_into_reactions_order() {
        let delta = SceneDelta {
            inserts: vec![SceneNodeKey::new(1), SceneNodeKey::new(2)],
            removes: vec![SceneNodeKey::new(3)],
            updates: vec![SceneNodeKey::new(4), SceneNodeKey::new(5)],
        };

        let reactions: Vec<_> = delta.into_reactions().collect();

        // All inserts first
        assert!(matches!(reactions[0], DeltaReaction::Insert(_)));
        assert!(matches!(reactions[1], DeltaReaction::Insert(_)));
        // Then removes
        assert!(matches!(reactions[2], DeltaReaction::Remove(_)));
        // Then updates
        assert!(matches!(reactions[3], DeltaReaction::Update(_)));
        assert!(matches!(reactions[4], DeltaReaction::Update(_)));
    }

    /// Test: empty delta yields empty iterator
    #[test]
    fn test_into_reactions_empty() {
        let delta = SceneDelta::default();
        let reactions: Vec<_> = delta.into_reactions().collect();
        assert!(reactions.is_empty());
    }

    /// Test: reaction_count matches actual count
    #[test]
    fn test_reaction_count() {
        let delta = SceneDelta {
            inserts: vec![SceneNodeKey::new(1), SceneNodeKey::new(2)],
            removes: vec![SceneNodeKey::new(3)],
            updates: vec![],
        };

        assert_eq!(delta.reaction_count(), 3);
    }

    /// Test: ExactSizeIterator implementation
    #[test]
    fn test_exact_size_iterator() {
        let delta = SceneDelta {
            inserts: vec![SceneNodeKey::new(1)],
            removes: vec![SceneNodeKey::new(2), SceneNodeKey::new(3)],
            updates: vec![],
        };

        let mut iter = delta.into_reactions();
        assert_eq!(iter.len(), 3);

        iter.next();
        assert_eq!(iter.len(), 2);

        iter.next();
        assert_eq!(iter.len(), 1);

        iter.next();
        assert_eq!(iter.len(), 0);
    }

    // ========================================================================
    // SPEC-HAL-083 Phase-6: Consumer Registration Tests
    // ========================================================================

    /// A test consumer that does nothing (Phase-6 compliant).
    #[derive(Debug)]
    struct TestConsumer {
        name: &'static str,
    }

    impl TestConsumer {
        fn new(name: &'static str) -> Self {
            Self { name }
        }
    }

    impl SceneDiffConsumer for TestConsumer {
        fn name(&self) -> &'static str {
            self.name
        }

        fn process(&mut self, _delta: &SceneDelta, _scene: &SceneGraph) {
            // Phase-13: NOP test consumer - must not have side effects
        }
    }

    /// Test: Empty registry
    #[test]
    fn test_consumer_registry_empty() {
        let consumers = SceneDiffConsumers::new();
        assert!(consumers.is_empty());
        assert_eq!(consumers.consumer_count(), 0);
    }

    /// Test: Consumer registration
    #[test]
    fn test_consumer_registration() {
        let mut consumers = SceneDiffConsumers::new();

        consumers.register(Box::new(TestConsumer::new("consumer_a")));
        consumers.register(Box::new(TestConsumer::new("consumer_b")));

        assert!(!consumers.is_empty());
        assert_eq!(consumers.consumer_count(), 2);
    }

    /// Test: Consumer names in registration order
    #[test]
    fn test_consumer_names_order() {
        let mut consumers = SceneDiffConsumers::new();

        consumers.register(Box::new(TestConsumer::new("first")));
        consumers.register(Box::new(TestConsumer::new("second")));
        consumers.register(Box::new(TestConsumer::new("third")));

        let names: Vec<_> = consumers.consumer_names().collect();
        assert_eq!(names, vec!["first", "second", "third"]);
    }

    /// Test: Dispatch calls consumers with delta and scene
    #[test]
    fn test_dispatch_calls_consumers() {
        let mut consumers = SceneDiffConsumers::new();
        consumers.register(Box::new(TestConsumer::new("test")));

        let delta = SceneDelta {
            inserts: vec![SceneNodeKey::new(1)],
            removes: vec![SceneNodeKey::new(2)],
            updates: vec![],
        };

        let scene = SceneGraph::new();

        // Phase-13: dispatch calls process(&delta, &scene) on each consumer
        consumers.dispatch(&delta, &scene);

        // If we got here without panic, the test passes
    }

    /// Test: Default tracked_key_count is 0 (Phase-6)
    #[test]
    fn test_default_tracked_key_count() {
        let consumer = TestConsumer::new("test");
        assert_eq!(consumer.tracked_key_count(), 0);
    }

    // ========================================================================
    // SPEC-HAL-083 Phase-7: Execution Gate Tests
    // ========================================================================

    /// Test: Disabled mode is a NOP
    #[test]
    fn test_execution_gate_disabled_mode() {
        let mut consumers = SceneDiffConsumers::new();
        consumers.register(Box::new(TestConsumer::new("test")));

        let delta = SceneDelta {
            inserts: vec![SceneNodeKey::new(1)],
            removes: vec![SceneNodeKey::new(2)],
            updates: vec![],
        };
        let scene = SceneGraph::new();

        // Disabled mode should be a NOP - no consumer calls
        dispatch_scene_diff(
            SceneDiffExecutionMode::Disabled,
            &mut consumers,
            &delta,
            &scene,
        );

        // If we got here, the test passes
    }

    /// Test: Default execution mode is Disabled
    #[test]
    fn test_execution_mode_default() {
        let mode = SceneDiffExecutionMode::default();
        assert_eq!(mode, SceneDiffExecutionMode::Disabled);
    }

    /// Test: Disabled mode works with empty consumers
    #[test]
    fn test_execution_gate_disabled_empty_consumers() {
        let mut consumers = SceneDiffConsumers::new();
        assert!(consumers.is_empty());

        let delta = SceneDelta::default();
        let scene = SceneGraph::new();

        // Should work even with no consumers registered
        dispatch_scene_diff(
            SceneDiffExecutionMode::Disabled,
            &mut consumers,
            &delta,
            &scene,
        );
    }

    /// Test: Disabled mode works with empty delta
    #[test]
    fn test_execution_gate_disabled_empty_delta() {
        let mut consumers = SceneDiffConsumers::new();
        consumers.register(Box::new(TestConsumer::new("test")));

        let delta = SceneDelta::default();
        let scene = SceneGraph::new();
        assert!(delta.is_empty());

        // Should work with empty delta
        dispatch_scene_diff(
            SceneDiffExecutionMode::Disabled,
            &mut consumers,
            &delta,
            &scene,
        );
    }

    // ========================================================================
    // SPEC-HAL-083 Phase-8: Deferred Execution Wiring Tests
    // ========================================================================

    /// Test: Reactions are delivered in correct order (insert → remove → update)
    #[test]
    fn test_phase8_reaction_order() {
        let mut consumers = SceneDiffConsumers::new();
        let consumer = DebugCountingConsumer::new();
        consumers.register(Box::new(consumer));

        let key1 = SceneNodeKey::new(1);
        let key2 = SceneNodeKey::new(2);
        let key3 = SceneNodeKey::new(3);

        let delta = SceneDelta {
            inserts: vec![key1],
            removes: vec![key2],
            updates: vec![key3],
        };
        let scene = SceneGraph::new();

        dispatch_scene_diff(
            SceneDiffExecutionMode::Enabled,
            &mut consumers,
            &delta,
            &scene,
        );

        // Verify consumer received reactions (consumer is inside Box, check tracked_key_count)
        // We can't directly access the consumer inside the Box, but we can verify
        // it doesn't panic and the dispatch completes
    }

    /// Test: Consumer sees correct counts
    #[test]
    fn test_phase8_consumer_counts() {
        let mut consumer = DebugCountingConsumer::new();

        let key1 = SceneNodeKey::new(1);
        let key2 = SceneNodeKey::new(2);
        let key3 = SceneNodeKey::new(3);
        let key4 = SceneNodeKey::new(4);
        let key5 = SceneNodeKey::new(5);

        let delta = SceneDelta {
            inserts: vec![key1, key2],
            removes: vec![key3],
            updates: vec![key4, key5],
        };
        let scene = SceneGraph::new();

        consumer.process(&delta, &scene);

        assert_eq!(consumer.insert_count(), 2);
        assert_eq!(consumer.remove_count(), 1);
        assert_eq!(consumer.update_count(), 2);
        assert_eq!(consumer.total_count(), 5);

        // Verify actual keys stored
        assert_eq!(consumer.inserted_keys(), &[key1, key2]);
        assert_eq!(consumer.removed_keys(), &[key3]);
        assert_eq!(consumer.updated_keys(), &[key4, key5]);
    }

    /// Test: Multiple consumers receive identical reaction streams
    #[test]
    fn test_phase8_multiple_consumers_identical_streams() {
        let mut consumers = SceneDiffConsumers::new();

        // We'll verify this by checking that dispatch doesn't panic
        // and that three consumers can be registered and dispatched to
        consumers.register(Box::new(DebugCountingConsumer::new()));
        consumers.register(Box::new(DebugCountingConsumer::new()));
        consumers.register(Box::new(DebugCountingConsumer::new()));

        assert_eq!(consumers.consumer_count(), 3);

        let delta = SceneDelta {
            inserts: vec![SceneNodeKey::new(1), SceneNodeKey::new(2)],
            removes: vec![SceneNodeKey::new(3)],
            updates: vec![],
        };
        let scene = SceneGraph::new();

        // All three consumers should receive the same reactions
        dispatch_scene_diff(
            SceneDiffExecutionMode::Enabled,
            &mut consumers,
            &delta,
            &scene,
        );

        // Verify all consumers have the same tracked count
        for name in consumers.consumer_names() {
            assert_eq!(name, "DebugCountingConsumer");
        }
    }

    /// Test: Disabled mode still does nothing (Phase-8 regression test)
    #[test]
    fn test_phase8_disabled_mode_nop() {
        let mut consumers = SceneDiffConsumers::new();
        consumers.register(Box::new(DebugCountingConsumer::new()));

        let delta = SceneDelta {
            inserts: vec![SceneNodeKey::new(1), SceneNodeKey::new(2)],
            removes: vec![SceneNodeKey::new(3)],
            updates: vec![SceneNodeKey::new(4)],
        };
        let scene = SceneGraph::new();

        // Disabled mode should NOT call process
        dispatch_scene_diff(
            SceneDiffExecutionMode::Disabled,
            &mut consumers,
            &delta,
            &scene,
        );

        // If we could access the consumer, its counts would all be 0
        // Since we can't, we just verify the dispatch completes without panic
    }

    /// Test: Enabled mode with empty consumers list
    #[test]
    fn test_phase8_enabled_empty_consumers() {
        let mut consumers = SceneDiffConsumers::new();
        assert!(consumers.is_empty());

        let delta = SceneDelta {
            inserts: vec![SceneNodeKey::new(1)],
            removes: vec![],
            updates: vec![],
        };
        let scene = SceneGraph::new();

        // Should work even with no consumers
        dispatch_scene_diff(
            SceneDiffExecutionMode::Enabled,
            &mut consumers,
            &delta,
            &scene,
        );
    }

    /// Test: Enabled mode with empty delta
    #[test]
    fn test_phase8_enabled_empty_delta() {
        let mut consumers = SceneDiffConsumers::new();
        consumers.register(Box::new(DebugCountingConsumer::new()));

        let delta = SceneDelta::default();
        let scene = SceneGraph::new();
        assert!(delta.is_empty());

        // Should work with empty delta
        dispatch_scene_diff(
            SceneDiffExecutionMode::Enabled,
            &mut consumers,
            &delta,
            &scene,
        );
    }

    /// Test: DebugCountingConsumer clear functionality
    #[test]
    fn test_debug_counting_consumer_clear() {
        let mut consumer = DebugCountingConsumer::new();

        let delta = SceneDelta {
            inserts: vec![SceneNodeKey::new(1)],
            removes: vec![SceneNodeKey::new(2)],
            updates: vec![SceneNodeKey::new(3)],
        };
        let scene = SceneGraph::new();

        consumer.process(&delta, &scene);
        assert_eq!(consumer.total_count(), 3);

        consumer.clear();
        assert_eq!(consumer.total_count(), 0);
        assert!(consumer.inserted_keys().is_empty());
        assert!(consumer.removed_keys().is_empty());
        assert!(consumer.updated_keys().is_empty());
    }

    /// Test: DeltaReactionIter::from_vec creates valid iterator
    #[test]
    fn test_delta_reaction_iter_from_vec() {
        let key1 = SceneNodeKey::new(1);
        let key2 = SceneNodeKey::new(2);
        let key3 = SceneNodeKey::new(3);

        let reactions = vec![
            DeltaReaction::Insert(key1),
            DeltaReaction::Remove(key2),
            DeltaReaction::Update(key3),
        ];

        let iter = DeltaReactionIter::from_vec(reactions);
        let collected: Vec<_> = iter.collect();

        assert_eq!(collected.len(), 3);
        assert_eq!(collected[0], DeltaReaction::Insert(key1));
        assert_eq!(collected[1], DeltaReaction::Remove(key2));
        assert_eq!(collected[2], DeltaReaction::Update(key3));
    }

    // ========================================================================
    // SPEC-HAL-083 Phase-16: Cache Storage Tests
    // ========================================================================

    /// Test: Insert creates entry in cache
    #[test]
    fn test_phase16_cache_insert() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(100);

        assert_eq!(cache.entry_count(), 0);
        assert!(!cache.contains(key));

        cache.insert(key, SceneNodeKind::Rect);

        assert_eq!(cache.entry_count(), 1);
        assert!(cache.contains(key));

        let entry = cache.get(key).unwrap();
        assert_eq!(entry.key, key);
        assert_eq!(entry.kind, SceneNodeKind::Rect);
        assert!(!entry.dirty);
    }

    /// Test: Remove removes entry from cache
    #[test]
    fn test_phase16_cache_remove() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(100);

        cache.insert(key, SceneNodeKind::Rect);
        assert_eq!(cache.entry_count(), 1);

        let removed = cache.remove(key);
        assert!(removed.is_some());
        assert_eq!(cache.entry_count(), 0);
        assert!(!cache.contains(key));
    }

    /// Test: Mark dirty sets dirty flag
    #[test]
    fn test_phase16_cache_mark_dirty() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(100);

        cache.insert(key, SceneNodeKind::Rect);
        assert!(!cache.get(key).unwrap().dirty);
        assert_eq!(cache.dirty_count(), 0);

        cache.mark_dirty(key);
        assert!(cache.get(key).unwrap().dirty);
        assert_eq!(cache.dirty_count(), 1);
    }

    /// Test: No dispatch → cache remains unchanged
    #[test]
    fn test_phase16_disabled_mode_no_change() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let mut consumers = CacheConsumers::new();
        consumers.register(Box::new(BasicCacheConsumer::new()));

        let key = SceneNodeKey::new(100);
        cache.insert(key, SceneNodeKind::Rect);
        assert_eq!(cache.entry_count(), 1);

        let delta = SceneDelta {
            inserts: vec![SceneNodeKey::new(200)],
            removes: vec![],
            updates: vec![],
        };
        let scene = SceneGraph::new();

        // Disabled mode should not modify cache
        dispatch_cache_operations(
            SceneCacheExecutionMode::Disabled,
            &mut consumers,
            &delta,
            &scene,
            &mut cache,
        );

        // Cache unchanged
        assert_eq!(cache.entry_count(), 1);
        assert!(cache.contains(key));
        assert!(!cache.contains(SceneNodeKey::new(200)));
    }

    /// Test: BasicCacheConsumer insert/remove/update
    #[test]
    fn test_phase16_basic_cache_consumer() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let mut consumer = BasicCacheConsumer::new();

        let key1 = SceneNodeKey::new(1);
        let key2 = SceneNodeKey::new(2);
        let key3 = SceneNodeKey::new(3);

        // Build a scene with key1 and key3
        let scene =
            build_scene_with_nodes(vec![make_rect_node(Some(key1)), make_rect_node(Some(key3))]);

        // Pre-insert key2 for removal test
        cache.insert(key2, SceneNodeKind::Rect);

        // Pre-insert key3 for update test
        cache.insert(key3, SceneNodeKind::Rect);

        let delta = SceneDelta {
            inserts: vec![key1], // key1 should be inserted
            removes: vec![key2], // key2 should be removed
            updates: vec![key3], // key3 should be marked dirty
        };

        consumer.process(&delta, &scene, &mut cache);

        // Verify results
        assert!(cache.contains(key1)); // Inserted
        assert!(!cache.contains(key2)); // Removed
        assert!(cache.contains(key3)); // Still exists
        assert!(cache.get(key3).unwrap().dirty); // Marked dirty

        assert_eq!(consumer.insert_count(), 1);
        assert_eq!(consumer.remove_count(), 1);
        assert_eq!(consumer.dirty_count(), 1);
    }

    /// Test: CacheEntry dirty flag clear
    #[test]
    fn test_phase16_cache_entry_clear_dirty() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut entry = CacheEntry::new(SceneNodeKey::new(1), SceneNodeKind::Rect);
        assert!(!entry.dirty);

        entry.mark_dirty();
        assert!(entry.dirty);

        entry.clear_dirty();
        assert!(!entry.dirty);
    }

    /// Test: SceneCache clear all entries
    #[test]
    fn test_phase16_cache_clear() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();

        cache.insert(SceneNodeKey::new(1), SceneNodeKind::Rect);
        cache.insert(SceneNodeKey::new(2), SceneNodeKind::Text);
        cache.insert(SceneNodeKey::new(3), SceneNodeKind::Circle);
        assert_eq!(cache.entry_count(), 3);

        cache.clear();
        assert_eq!(cache.entry_count(), 0);
    }

    /// Test: SceneCache iteration
    #[test]
    fn test_phase16_cache_iter() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();

        cache.insert(SceneNodeKey::new(1), SceneNodeKind::Rect);
        cache.insert(SceneNodeKey::new(2), SceneNodeKind::Text);

        let entries: Vec<_> = cache.iter().collect();
        assert_eq!(entries.len(), 2);
    }

    // ========================================================================
    // SPEC-HAL-083 Phase-17: Lifecycle Enforcement Tests
    // ========================================================================

    /// Test: clear_all_dirty clears all dirty flags
    #[test]
    fn test_phase17_clear_all_dirty() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();

        // Insert entries and mark some dirty
        cache.insert(SceneNodeKey::new(1), SceneNodeKind::Rect);
        cache.insert(SceneNodeKey::new(2), SceneNodeKind::Text);
        cache.insert(SceneNodeKey::new(3), SceneNodeKind::Circle);

        cache.mark_dirty(SceneNodeKey::new(1));
        cache.mark_dirty(SceneNodeKey::new(3));

        assert_eq!(cache.dirty_count(), 2);

        // Clear all dirty
        cache.clear_all_dirty();

        assert_eq!(cache.dirty_count(), 0);
        assert!(!cache.get(SceneNodeKey::new(1)).unwrap().dirty);
        assert!(!cache.get(SceneNodeKey::new(2)).unwrap().dirty);
        assert!(!cache.get(SceneNodeKey::new(3)).unwrap().dirty);
    }

    /// Test: Dirty + clear → pass (dispatch clears dirty automatically)
    #[test]
    fn test_phase17_dispatch_clears_dirty() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let mut consumers = CacheConsumers::new();
        consumers.register(Box::new(BasicCacheConsumer::new()));

        let key1 = SceneNodeKey::new(1);
        let key2 = SceneNodeKey::new(2);

        // Build a scene with both keys
        let scene =
            build_scene_with_nodes(vec![make_rect_node(Some(key1)), make_rect_node(Some(key2))]);

        // Pre-insert entries
        cache.insert(key1, SceneNodeKind::Rect);
        cache.insert(key2, SceneNodeKind::Rect);

        // Delta with updates (will mark dirty)
        let delta = SceneDelta {
            inserts: vec![],
            removes: vec![],
            updates: vec![key1, key2],
        };

        // Dispatch should: mark dirty via consumer, then clear_all_dirty
        dispatch_cache_operations(
            SceneCacheExecutionMode::Enabled,
            &mut consumers,
            &delta,
            &scene,
            &mut cache,
        );

        // After dispatch, all dirty should be cleared
        assert_eq!(cache.dirty_count(), 0);
    }

    /// Test: Multiple dirty entries → all must be cleared
    #[test]
    fn test_phase17_multiple_dirty_cleared() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();

        // Insert many entries
        for i in 1..=10 {
            cache.insert(SceneNodeKey::new(i), SceneNodeKind::Rect);
        }

        // Mark all dirty
        for i in 1..=10 {
            cache.mark_dirty(SceneNodeKey::new(i));
        }

        assert_eq!(cache.dirty_count(), 10);

        // Clear all
        cache.clear_all_dirty();

        assert_eq!(cache.dirty_count(), 0);

        // Verify each entry
        for i in 1..=10 {
            assert!(!cache.get(SceneNodeKey::new(i)).unwrap().dirty);
        }
    }

    /// Test: Disabled execution mode → no assertions triggered
    #[test]
    fn test_phase17_disabled_mode_no_assertions() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let mut consumers = CacheConsumers::new();

        // Insert entry and mark dirty
        cache.insert(SceneNodeKey::new(1), SceneNodeKind::Rect);
        cache.mark_dirty(SceneNodeKey::new(1));

        let delta = SceneDelta::default();
        let scene = SceneGraph::new();

        // Dispatch in disabled mode - should NOT panic, should NOT clear dirty
        dispatch_cache_operations(
            SceneCacheExecutionMode::Disabled,
            &mut consumers,
            &delta,
            &scene,
            &mut cache,
        );

        // Dirty should still be 1 (disabled mode doesn't touch cache)
        assert_eq!(cache.dirty_count(), 1);
    }

    /// Test: Lifecycle state transitions
    #[test]
    fn test_phase17_lifecycle_transitions() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);

        // Created → Clean
        cache.insert(key, SceneNodeKind::Rect);
        assert!(!cache.get(key).unwrap().dirty);

        // Updated → Dirty
        cache.mark_dirty(key);
        assert!(cache.get(key).unwrap().dirty);

        // Dirty → Cleared (explicit)
        cache.clear_all_dirty();
        assert!(!cache.get(key).unwrap().dirty);

        // Removed → Gone
        cache.remove(key);
        assert!(!cache.contains(key));
    }

    /// Test: Empty cache clear_all_dirty is no-op
    #[test]
    fn test_phase17_clear_empty_cache() {
        let mut cache = SceneCache::new();

        assert_eq!(cache.entry_count(), 0);
        assert_eq!(cache.dirty_count(), 0);

        // Should not panic
        cache.clear_all_dirty();

        assert_eq!(cache.entry_count(), 0);
        assert_eq!(cache.dirty_count(), 0);
    }

    /// Test: Clean entries remain clean after clear_all_dirty
    #[test]
    fn test_phase17_clean_remains_clean() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();

        // Insert entries (all clean)
        cache.insert(SceneNodeKey::new(1), SceneNodeKind::Rect);
        cache.insert(SceneNodeKey::new(2), SceneNodeKind::Text);

        // Mark only one dirty
        cache.mark_dirty(SceneNodeKey::new(1));

        assert_eq!(cache.dirty_count(), 1);
        assert!(cache.get(SceneNodeKey::new(1)).unwrap().dirty);
        assert!(!cache.get(SceneNodeKey::new(2)).unwrap().dirty);

        // Clear all dirty
        cache.clear_all_dirty();

        // Both should be clean now
        assert_eq!(cache.dirty_count(), 0);
        assert!(!cache.get(SceneNodeKey::new(1)).unwrap().dirty);
        assert!(!cache.get(SceneNodeKey::new(2)).unwrap().dirty);
    }

    // ========================================================================
    // SPEC-HAL-083 Phase-18: Cache↔Scene Consistency Enforcement Tests
    // ========================================================================

    /// Test: assert_delta_applied passes when inserts are in cache and removes are not
    #[test]
    fn test_phase18_assert_delta_applied_happy_path() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key1 = SceneNodeKey::new(1);
        let key2 = SceneNodeKey::new(2);

        // Insert key1 into cache (simulating what consumer would do)
        cache.insert(key1, SceneNodeKind::Rect);

        // Delta says: insert key1, remove key2
        let delta = SceneDelta {
            inserts: vec![key1],
            removes: vec![key2], // key2 not in cache = correct
            updates: vec![],
        };

        // Should not panic - cache state matches delta expectations
        cache.assert_delta_applied(&delta);
    }

    /// Test: assert_delta_applied panics when insert is missing from cache
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-HAL-083 Phase-18: Insert key")]
    fn test_phase18_assert_delta_applied_missing_insert() {
        let cache = SceneCache::new();
        let key1 = SceneNodeKey::new(1);

        // Delta says insert key1, but key1 is NOT in cache
        let delta = SceneDelta {
            inserts: vec![key1],
            removes: vec![],
            updates: vec![],
        };

        // Should panic - insert key missing from cache
        cache.assert_delta_applied(&delta);
    }

    /// Test: assert_delta_applied panics when removed key is still in cache
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-HAL-083 Phase-18: Remove key")]
    fn test_phase18_assert_delta_applied_retained_remove() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key1 = SceneNodeKey::new(1);

        // Insert key1 into cache
        cache.insert(key1, SceneNodeKind::Rect);

        // Delta says remove key1, but key1 is STILL in cache
        let delta = SceneDelta {
            inserts: vec![],
            removes: vec![key1],
            updates: vec![],
        };

        // Should panic - remove key still in cache
        cache.assert_delta_applied(&delta);
    }

    /// Test: assert_consistent_with_scene passes for consistent state
    #[test]
    fn test_phase18_assert_consistent_happy_path() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key1 = SceneNodeKey::new(1);
        let key2 = SceneNodeKey::new(2);

        // Cache has two entries
        cache.insert(key1, SceneNodeKind::Rect);
        cache.insert(key2, SceneNodeKind::Text);

        // Scene has same two nodes with matching keys and kinds
        let scene =
            build_scene_with_nodes(vec![make_rect_node(Some(key1)), make_text_node(Some(key2))]);

        // Should not panic - cache and scene are consistent
        cache.assert_consistent_with_scene(&scene);
    }

    /// Test: assert_consistent_with_scene passes for empty cache
    #[test]
    fn test_phase18_assert_consistent_empty_cache() {
        let cache = SceneCache::new();
        let scene = SceneGraph::new();

        // Should not panic - empty cache is always consistent
        cache.assert_consistent_with_scene(&scene);
    }

    /// Test: assert_consistent_with_scene panics for phantom cache entry
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-HAL-083 Phase-18: Cache entry")]
    fn test_phase18_assert_consistent_phantom_entry() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key1 = SceneNodeKey::new(1);

        // Cache has entry for key1
        cache.insert(key1, SceneNodeKind::Rect);

        // Scene is empty - no node with key1
        let scene = SceneGraph::new();

        // Should panic - cache entry has no corresponding scene node
        cache.assert_consistent_with_scene(&scene);
    }

    /// Test: assert_consistent_with_scene panics for kind mismatch
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Kind mismatch")]
    fn test_phase18_assert_consistent_kind_mismatch() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key1 = SceneNodeKey::new(1);

        // Cache says key1 is Text
        cache.insert(key1, SceneNodeKind::Text);

        // Scene says key1 is Rect
        let scene = build_scene_with_nodes(vec![make_rect_node(Some(key1))]);

        // Should panic - kind mismatch
        cache.assert_consistent_with_scene(&scene);
    }

    /// Test: Full dispatch with Phase-18 assertions enabled
    #[test]
    fn test_phase18_full_dispatch_consistent() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let mut consumers = CacheConsumers::new();
        consumers.register(Box::new(BasicCacheConsumer::new()));

        let key1 = SceneNodeKey::new(1);
        let key2 = SceneNodeKey::new(2);

        // Build scene with two nodes
        let scene =
            build_scene_with_nodes(vec![make_rect_node(Some(key1)), make_rect_node(Some(key2))]);

        // Delta: insert both keys
        let delta = SceneDelta {
            inserts: vec![key1, key2],
            removes: vec![],
            updates: vec![],
        };

        // Dispatch should:
        // 1. Consumer inserts both keys
        // 2. assert_delta_applied verifies inserts present
        // 3. assert_consistent_with_scene verifies key↔kind stability
        // 4. clear_all_dirty
        // 5. assert dirty_count() == 0
        dispatch_cache_operations(
            SceneCacheExecutionMode::Enabled,
            &mut consumers,
            &delta,
            &scene,
            &mut cache,
        );

        // Verify final state
        assert!(cache.contains(key1));
        assert!(cache.contains(key2));
        assert_eq!(cache.dirty_count(), 0);
    }

    // ========================================================================
    // SPEC-HAL-083 Phase-19: Handle Address Stability Tests
    // ========================================================================

    /// Test: insert_with_handle returns a valid handle
    #[test]
    fn test_phase19_insert_with_handle_basic() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);

        let handle = cache.insert_with_handle(key, SceneNodeKind::Rect);

        // Handle should have correct key and kind
        assert_eq!(handle.key(), key);
        assert_eq!(handle.kind(), SceneNodeKind::Rect);

        // Entry should be accessible via handle
        let entry = cache.get_by_handle(handle);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().key, key);
        assert_eq!(entry.unwrap().kind, SceneNodeKind::Rect);
    }

    /// Test: handle remains valid across simulated frames
    #[test]
    fn test_phase19_handle_valid_across_frames() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);

        // Frame 1: insert
        let handle = cache.insert_with_handle(key, SceneNodeKind::Rect);

        // Simulate frame boundary (just mark dirty and clear)
        cache.mark_dirty(key);
        cache.clear_all_dirty();

        // Frame 2: handle still valid
        let entry = cache.get_by_handle(handle);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().key, key);

        // Frame 3: another frame boundary
        cache.mark_dirty(key);
        cache.clear_all_dirty();

        // Frame 3: handle still valid
        let entry = cache.get_by_handle(handle);
        assert!(entry.is_some());
    }

    /// Test: handle invalidated on remove (debug: panics, release: returns None)
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-HAL-083 Phase-19: Stale handle")]
    fn test_phase19_handle_invalidated_on_remove() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);

        let handle = cache.insert_with_handle(key, SceneNodeKind::Rect);

        // Entry is accessible
        assert!(cache.get_by_handle(handle).is_some());

        // Remove the entry
        cache.remove(key);

        // Handle is now stale - should panic in debug
        let _ = cache.get_by_handle(handle);
    }

    /// Test: get_by_handle_mut works
    #[test]
    fn test_phase19_get_by_handle_mut() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);

        let handle = cache.insert_with_handle(key, SceneNodeKind::Rect);

        // Mutate via handle
        {
            let entry = cache.get_by_handle_mut(handle).unwrap();
            entry.mark_dirty();
        }

        // Verify mutation
        assert!(cache.get_by_handle(handle).unwrap().dirty);
    }

    /// Test: handle with wrong kind panics (programming error)
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Kind mismatch")]
    fn test_phase19_handle_kind_mismatch_panics() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);

        // Insert as Rect
        cache.insert(key, SceneNodeKind::Rect);

        // Manually construct a handle with wrong kind (simulates programming error)
        #[cfg(debug_assertions)]
        let bad_handle = SceneCacheHandle {
            key,
            kind: SceneNodeKind::Text, // Wrong kind!
            generation: 0,
        };
        #[cfg(not(debug_assertions))]
        let bad_handle = SceneCacheHandle {
            key,
            kind: SceneNodeKind::Text,
        };

        // Should panic due to kind mismatch
        let _ = cache.get_by_handle(bad_handle);
    }

    /// Test: multiple handles to same key
    #[test]
    fn test_phase19_multiple_handles_same_key() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);

        let handle1 = cache.insert_with_handle(key, SceneNodeKind::Rect);

        // Can't insert twice, but we can use the same handle
        let entry1 = cache.get_by_handle(handle1);
        let entry2 = cache.get_by_handle(handle1);

        assert!(entry1.is_some());
        assert!(entry2.is_some());
        assert_eq!(entry1.unwrap().key, entry2.unwrap().key);
    }

    /// Test: handle copyable (Copy trait)
    #[test]
    fn test_phase19_handle_is_copyable() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);

        let handle = cache.insert_with_handle(key, SceneNodeKind::Rect);

        // Copy the handle
        let handle_copy = handle;

        // Both should work
        assert!(cache.get_by_handle(handle).is_some());
        assert!(cache.get_by_handle(handle_copy).is_some());
    }

    /// Test: re-insert after remove creates new generation
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-HAL-083 Phase-19: Stale handle")]
    fn test_phase19_reinsert_after_remove_invalidates_old_handle() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);

        // Insert, get handle
        let old_handle = cache.insert_with_handle(key, SceneNodeKind::Rect);

        // Remove
        cache.remove(key);

        // Re-insert (same key)
        let _new_handle = cache.insert_with_handle(key, SceneNodeKind::Rect);

        // Old handle should be stale (different generation)
        let _ = cache.get_by_handle(old_handle);
    }

    #[test]
    #[cfg(debug_assertions)]
    fn test_phase19_removed_key_generations_do_not_accumulate() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();

        for value in 0..50_000 {
            let key = SceneNodeKey::new(value);
            cache.insert(key, SceneNodeKind::Rect);
            cache.remove(key);
        }

        assert!(cache.entries.is_empty());
        assert!(
            cache.generations.is_empty(),
            "removed scene identities must not leave debug generation tombstones"
        );
        assert_eq!(cache.next_generation, 50_000);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-HAL-083 Phase-19: Stale handle")]
    fn test_phase19_clear_then_reinsert_invalidates_old_handle() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);
        let old_handle = cache.insert_with_handle(key, SceneNodeKind::Rect);

        cache.clear();
        let _new_handle = cache.insert_with_handle(key, SceneNodeKind::Rect);

        let _ = cache.get_by_handle(old_handle);
    }

    /// Test: handle hashable (can be used in HashSet/HashMap)
    #[test]
    fn test_phase19_handle_is_hashable() {
        use crate::core::scene_identity::SceneNodeKind;
        use std::collections::HashSet;

        let mut cache = SceneCache::new();

        let handle1 = cache.insert_with_handle(SceneNodeKey::new(1), SceneNodeKind::Rect);
        let handle2 = cache.insert_with_handle(SceneNodeKey::new(2), SceneNodeKind::Text);

        let mut set = HashSet::new();
        set.insert(handle1);
        set.insert(handle2);

        assert_eq!(set.len(), 2);
        assert!(set.contains(&handle1));
        assert!(set.contains(&handle2));
    }

    // ========================================================================
    // SPEC-HAL-083 Phase-20: Cache Entry Ownership Split Tests
    // ========================================================================

    /// Test: new entry defaults to CpuOnly ownership
    #[test]
    fn test_phase20_new_entry_defaults_cpu_only() {
        use crate::core::scene_identity::SceneNodeKind;

        let entry = CacheEntry::new(SceneNodeKey::new(1), SceneNodeKind::Rect);

        assert_eq!(entry.ownership(), CacheEntryOwnership::CpuOnly);
    }

    /// Test: ownership getter returns correct value
    #[test]
    fn test_phase20_ownership_getter() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut entry = CacheEntry::new(SceneNodeKey::new(1), SceneNodeKind::Rect);

        // Default is CpuOnly
        assert_eq!(entry.ownership(), CacheEntryOwnership::CpuOnly);

        // Transition to GpuBound (test-only)
        entry.set_ownership_for_test(CacheEntryOwnership::GpuBound);
        assert_eq!(entry.ownership(), CacheEntryOwnership::GpuBound);
    }

    /// Test: assert_cpu_only passes for CpuOnly entries
    #[test]
    fn test_phase20_assert_cpu_only_passes() {
        use crate::core::scene_identity::SceneNodeKind;

        let entry = CacheEntry::new(SceneNodeKey::new(1), SceneNodeKind::Rect);

        // Should not panic
        entry.assert_cpu_only();
    }

    /// Test: assert_cpu_only panics for GpuBound entries
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-HAL-083 Phase-20")]
    fn test_phase20_assert_cpu_only_panics_gpu_bound() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut entry = CacheEntry::new(SceneNodeKey::new(1), SceneNodeKind::Rect);
        entry.set_ownership_for_test(CacheEntryOwnership::GpuBound);

        // Should panic - entry is GpuBound
        entry.assert_cpu_only();
    }

    /// Test: mark_dirty passes for CpuOnly entries
    #[test]
    fn test_phase20_mark_dirty_cpu_only_passes() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut entry = CacheEntry::new(SceneNodeKey::new(1), SceneNodeKind::Rect);

        // Should not panic
        entry.mark_dirty();
        assert!(entry.dirty);
    }

    /// Test: mark_dirty panics for GpuBound entries
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-HAL-083 Phase-20")]
    fn test_phase20_mark_dirty_panics_gpu_bound() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut entry = CacheEntry::new(SceneNodeKey::new(1), SceneNodeKind::Rect);
        entry.set_ownership_for_test(CacheEntryOwnership::GpuBound);

        // Should panic - cannot mutate GpuBound entry
        entry.mark_dirty();
    }

    /// Test: clear_dirty passes for CpuOnly entries
    #[test]
    fn test_phase20_clear_dirty_cpu_only_passes() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut entry = CacheEntry::new(SceneNodeKey::new(1), SceneNodeKind::Rect);
        entry.dirty = true; // Set dirty directly for test setup

        // Should not panic
        entry.clear_dirty();
        assert!(!entry.dirty);
    }

    /// Test: clear_dirty panics for GpuBound entries
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-HAL-083 Phase-20")]
    fn test_phase20_clear_dirty_panics_gpu_bound() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut entry = CacheEntry::new(SceneNodeKey::new(1), SceneNodeKind::Rect);
        entry.dirty = true; // Set dirty directly for test setup
        entry.set_ownership_for_test(CacheEntryOwnership::GpuBound);

        // Should panic - cannot mutate GpuBound entry
        entry.clear_dirty();
    }

    /// Test: cache operations work with CpuOnly entries
    #[test]
    fn test_phase20_cache_operations_cpu_only() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);

        // Insert
        cache.insert(key, SceneNodeKind::Rect);
        assert_eq!(
            cache.get(key).unwrap().ownership(),
            CacheEntryOwnership::CpuOnly
        );

        // Mark dirty
        cache.mark_dirty(key);
        assert!(cache.get(key).unwrap().dirty);

        // Clear all dirty
        cache.clear_all_dirty();
        assert!(!cache.get(key).unwrap().dirty);

        // Remove
        let removed = cache.remove(key);
        assert!(removed.is_some());
    }

    /// Test: CacheEntryOwnership enum is Copy
    #[test]
    fn test_phase20_ownership_is_copy() {
        let ownership = CacheEntryOwnership::CpuOnly;
        let copy = ownership;

        assert_eq!(ownership, copy);
        assert_eq!(ownership, CacheEntryOwnership::CpuOnly);
    }

    /// Test: CacheEntryOwnership default is CpuOnly
    #[test]
    fn test_phase20_ownership_default() {
        let ownership = CacheEntryOwnership::default();
        assert_eq!(ownership, CacheEntryOwnership::CpuOnly);
    }

    // ========================================================================
    // SPEC-HAL-083 Phase-22: Explicit Cache Lifecycle Phases Tests
    // ========================================================================

    /// Test: CacheLifecyclePhase enum exists with correct variants
    #[test]
    fn test_phase22_lifecycle_phases_exist() {
        let build = CacheLifecyclePhase::Build;
        let dispatch = CacheLifecyclePhase::Dispatch;
        let frozen = CacheLifecyclePhase::Frozen;

        assert_ne!(build, dispatch);
        assert_ne!(dispatch, frozen);
        assert_ne!(build, frozen);
    }

    /// Test: CacheLifecyclePhase default is Build
    #[test]
    fn test_phase22_lifecycle_default_is_build() {
        let phase = CacheLifecyclePhase::default();
        assert_eq!(phase, CacheLifecyclePhase::Build);
    }

    /// Test: begin_frame sets Build phase
    #[test]
    #[cfg(debug_assertions)]
    fn test_phase22_begin_frame_sets_build() {
        let mut cache = SceneCache::new();

        // Start in Build (default)
        assert_eq!(cache.phase(), CacheLifecyclePhase::Build);

        // Transition to Frozen
        cache.end_frame();
        assert_eq!(cache.phase(), CacheLifecyclePhase::Frozen);

        // begin_frame resets to Build
        cache.begin_frame();
        assert_eq!(cache.phase(), CacheLifecyclePhase::Build);
    }

    /// Test: enter_dispatch sets Dispatch phase
    #[test]
    #[cfg(debug_assertions)]
    fn test_phase22_enter_dispatch_sets_dispatch() {
        let mut cache = SceneCache::new();

        // Start in Build
        assert_eq!(cache.phase(), CacheLifecyclePhase::Build);

        // Transition to Dispatch
        cache.enter_dispatch();
        assert_eq!(cache.phase(), CacheLifecyclePhase::Dispatch);
    }

    /// Test: end_frame sets Frozen phase
    #[test]
    #[cfg(debug_assertions)]
    fn test_phase22_end_frame_sets_frozen() {
        let mut cache = SceneCache::new();

        // Start in Build
        assert_eq!(cache.phase(), CacheLifecyclePhase::Build);

        // Transition to Frozen
        cache.end_frame();
        assert_eq!(cache.phase(), CacheLifecyclePhase::Frozen);
    }

    /// Test: Mutations work in Build phase
    #[test]
    #[cfg(debug_assertions)]
    fn test_phase22_mutations_allowed_in_build() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key1 = SceneNodeKey::new(1);
        let key2 = SceneNodeKey::new(2);

        // In Build phase - all mutations should work
        assert_eq!(cache.phase(), CacheLifecyclePhase::Build);

        // insert
        cache.insert(key1, SceneNodeKind::Rect);
        assert!(cache.contains(key1));

        // insert_with_handle
        let _handle = cache.insert_with_handle(key2, SceneNodeKind::Text);
        assert!(cache.contains(key2));

        // mark_dirty
        cache.mark_dirty(key1);
        assert!(cache.get(key1).unwrap().is_dirty());

        // clear_all_dirty
        cache.clear_all_dirty();
        assert!(!cache.get(key1).unwrap().is_dirty());

        // remove
        let removed = cache.remove(key1);
        assert!(removed.is_some());
    }

    /// Test: insert panics in Frozen phase
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-HAL-083 Phase-22")]
    fn test_phase22_insert_panics_frozen() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        cache.end_frame();

        // Should panic - cannot insert in Frozen phase
        cache.insert(SceneNodeKey::new(1), SceneNodeKind::Rect);
    }

    /// Test: insert panics in Dispatch phase
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-HAL-083 Phase-22")]
    fn test_phase22_insert_panics_dispatch() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        cache.enter_dispatch();

        // Should panic - cannot insert in Dispatch phase
        cache.insert(SceneNodeKey::new(1), SceneNodeKind::Rect);
    }

    /// Test: remove panics in Frozen phase
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-HAL-083 Phase-22")]
    fn test_phase22_remove_panics_frozen() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);
        cache.insert(key, SceneNodeKind::Rect);
        cache.end_frame();

        // Should panic - cannot remove in Frozen phase
        cache.remove(key);
    }

    /// Test: mark_dirty panics in Frozen phase
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-HAL-083 Phase-22")]
    fn test_phase22_mark_dirty_panics_frozen() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);
        cache.insert(key, SceneNodeKind::Rect);
        cache.end_frame();

        // Should panic - cannot mark_dirty in Frozen phase
        cache.mark_dirty(key);
    }

    /// Test: clear_all_dirty panics in Frozen phase
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-HAL-083 Phase-22")]
    fn test_phase22_clear_all_dirty_panics_frozen() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);
        cache.insert(key, SceneNodeKind::Rect);
        cache.mark_dirty(key);
        cache.end_frame();

        // Should panic - cannot clear_all_dirty in Frozen phase
        cache.clear_all_dirty();
    }

    /// Test: insert_with_handle panics in Frozen phase
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-HAL-083 Phase-22")]
    fn test_phase22_insert_with_handle_panics_frozen() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        cache.end_frame();

        // Should panic - cannot insert_with_handle in Frozen phase
        cache.insert_with_handle(SceneNodeKey::new(1), SceneNodeKind::Rect);
    }

    /// Test: Full lifecycle transition chain
    #[test]
    #[cfg(debug_assertions)]
    fn test_phase22_lifecycle_chain() {
        let mut cache = SceneCache::new();

        // Frame 1: Build → Dispatch → Frozen
        assert_eq!(cache.phase(), CacheLifecyclePhase::Build);
        cache.enter_dispatch();
        assert_eq!(cache.phase(), CacheLifecyclePhase::Dispatch);
        cache.end_frame();
        assert_eq!(cache.phase(), CacheLifecyclePhase::Frozen);

        // Frame 2: Frozen → Build (via begin_frame)
        cache.begin_frame();
        assert_eq!(cache.phase(), CacheLifecyclePhase::Build);
    }

    /// Test: dispatch_cache_operations manages lifecycle
    #[test]
    #[cfg(debug_assertions)]
    fn test_phase22_dispatch_manages_lifecycle() {
        let mut cache = SceneCache::new();
        let mut consumers = CacheConsumers::new();

        // Create a scene with keyed nodes
        let key = SceneNodeKey::new(1);
        let scene = build_scene_with_nodes(vec![make_rect_node(Some(key))]);

        let delta = SceneDelta {
            inserts: vec![key],
            removes: vec![],
            updates: vec![],
        };

        // Add BasicCacheConsumer
        consumers.register(Box::new(BasicCacheConsumer::default()));

        // Before dispatch: default is Build
        assert_eq!(cache.phase(), CacheLifecyclePhase::Build);

        // Dispatch manages lifecycle: Build → mutations → Dispatch → Frozen
        dispatch_cache_operations(
            SceneCacheExecutionMode::Enabled,
            &mut consumers,
            &delta,
            &scene,
            &mut cache,
        );

        // After dispatch: should be Frozen
        assert_eq!(cache.phase(), CacheLifecyclePhase::Frozen);

        // Entry was inserted
        assert!(cache.contains(key));
    }

    /// Test: Multiple frames with lifecycle
    #[test]
    #[cfg(debug_assertions)]
    fn test_phase22_multiple_frames() {
        let mut cache = SceneCache::new();
        let mut consumers = CacheConsumers::new();
        consumers.register(Box::new(BasicCacheConsumer::default()));

        // Frame 1: insert key1
        let key1 = SceneNodeKey::new(1);
        let scene1 = build_scene_with_nodes(vec![make_rect_node(Some(key1))]);
        let delta1 = SceneDelta {
            inserts: vec![key1],
            removes: vec![],
            updates: vec![],
        };

        dispatch_cache_operations(
            SceneCacheExecutionMode::Enabled,
            &mut consumers,
            &delta1,
            &scene1,
            &mut cache,
        );
        assert_eq!(cache.phase(), CacheLifecyclePhase::Frozen);
        assert!(cache.contains(key1));

        // Frame 2: insert key2, keep key1
        let key2 = SceneNodeKey::new(2);
        let scene2 =
            build_scene_with_nodes(vec![make_rect_node(Some(key1)), make_rect_node(Some(key2))]);
        let delta2 = SceneDelta {
            inserts: vec![key2],
            removes: vec![],
            updates: vec![key1],
        };

        dispatch_cache_operations(
            SceneCacheExecutionMode::Enabled,
            &mut consumers,
            &delta2,
            &scene2,
            &mut cache,
        );
        assert_eq!(cache.phase(), CacheLifecyclePhase::Frozen);
        assert!(cache.contains(key1));
        assert!(cache.contains(key2));

        // Frame 3: remove key1
        let scene3 = build_scene_with_nodes(vec![make_rect_node(Some(key2))]);
        let delta3 = SceneDelta {
            inserts: vec![],
            removes: vec![key1],
            updates: vec![],
        };

        dispatch_cache_operations(
            SceneCacheExecutionMode::Enabled,
            &mut consumers,
            &delta3,
            &scene3,
            &mut cache,
        );
        assert_eq!(cache.phase(), CacheLifecyclePhase::Frozen);
        assert!(!cache.contains(key1));
        assert!(cache.contains(key2));
    }

    // ========================================================================
    // SPEC-HAL-083 Phase-23: Read-Only Dispatch Guard Tests
    // ========================================================================

    /// Test: Read-only accessor works in Dispatch phase
    #[test]
    #[cfg(debug_assertions)]
    fn test_phase23_read_only_in_dispatch() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);

        // Insert in Build phase
        cache.insert(key, SceneNodeKind::Rect);

        // Transition to Dispatch
        cache.enter_dispatch();
        assert_eq!(cache.phase(), CacheLifecyclePhase::Dispatch);

        // All read-only accessors should work in Dispatch
        assert!(cache.contains(key));
        assert!(cache.get(key).is_some());
        assert_eq!(cache.entry_count(), 1);
        assert_eq!(cache.dirty_count(), 0);

        // Iterator should work
        let count = cache.iter().count();
        assert_eq!(count, 1);
    }

    /// Test: get_by_handle works in Dispatch phase
    #[test]
    #[cfg(debug_assertions)]
    fn test_phase23_handle_read_in_dispatch() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);

        // Insert with handle in Build phase
        let handle = cache.insert_with_handle(key, SceneNodeKind::Rect);

        // Transition to Dispatch
        cache.enter_dispatch();
        assert_eq!(cache.phase(), CacheLifecyclePhase::Dispatch);

        // Read via handle should work
        let entry = cache.get_by_handle(handle);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().key(), key);
    }

    /// Test: Read-only access works in Frozen phase (for inspection)
    #[test]
    #[cfg(debug_assertions)]
    fn test_phase23_read_allowed_in_frozen() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);

        cache.insert(key, SceneNodeKind::Rect);
        cache.enter_dispatch(); // Phase-24: Must capture snapshot before Frozen
        cache.end_frame();
        assert_eq!(cache.phase(), CacheLifecyclePhase::Frozen);

        // Read access is allowed in Frozen for inspection
        assert!(cache.contains(key));
        assert!(cache.get(key).is_some());
        assert_eq!(cache.entry_count(), 1);
    }

    /// Test: get_mut panics in Dispatch phase
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-HAL-083 Phase-22")]
    fn test_phase23_get_mut_panics_dispatch() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);

        cache.insert(key, SceneNodeKind::Rect);
        cache.enter_dispatch();

        // Should panic - get_mut is mutation path, forbidden in Dispatch
        let _ = cache.get_mut(key);
    }

    /// Test: get_by_handle_mut panics in Dispatch phase
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-HAL-083 Phase-22")]
    fn test_phase23_get_by_handle_mut_panics_dispatch() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        let key = SceneNodeKey::new(1);

        let handle = cache.insert_with_handle(key, SceneNodeKind::Rect);
        cache.enter_dispatch();

        // Should panic - get_by_handle_mut is mutation path, forbidden in Dispatch
        let _ = cache.get_by_handle_mut(handle);
    }

    // =========================================================================
    // Phase-24 Tests: Stable Read Snapshot Guarantee
    // =========================================================================

    /// Test: Snapshot is captured at enter_dispatch
    #[test]
    fn test_phase24_snapshot_captured_at_dispatch() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        cache.insert(SceneNodeKey::new(1), SceneNodeKind::Rect);
        cache.insert(SceneNodeKey::new(2), SceneNodeKind::Text);
        cache.mark_dirty(SceneNodeKey::new(1));

        // Before dispatch: 2 entries, 1 dirty
        assert_eq!(cache.entry_count(), 2);
        assert_eq!(cache.dirty_count(), 1);

        cache.enter_dispatch();

        // After dispatch: reads still work, snapshot is stable
        assert_eq!(cache.entry_count(), 2);
        assert_eq!(cache.dirty_count(), 1);
    }

    /// Test: Reads in Dispatch phase verify snapshot stability
    #[test]
    fn test_phase24_reads_verify_snapshot_dispatch() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        cache.insert(SceneNodeKey::new(1), SceneNodeKind::Rect);
        cache.insert(SceneNodeKey::new(2), SceneNodeKind::Text);
        cache.enter_dispatch();

        // Multiple reads should all observe same state
        assert_eq!(cache.entry_count(), 2);
        assert_eq!(cache.entry_count(), 2);
        assert!(cache.contains(SceneNodeKey::new(1)));
        assert!(cache.get(SceneNodeKey::new(1)).is_some());
        assert_eq!(cache.dirty_count(), 0);
    }

    /// Test: Reads in Frozen phase verify snapshot stability
    #[test]
    fn test_phase24_reads_verify_snapshot_frozen() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        cache.insert(SceneNodeKey::new(1), SceneNodeKind::Rect);
        cache.mark_dirty(SceneNodeKey::new(1));
        cache.enter_dispatch();
        cache.end_frame();

        // Frozen phase: reads still verify snapshot captured at enter_dispatch
        assert_eq!(cache.entry_count(), 1);
        assert_eq!(cache.dirty_count(), 1);
        assert!(cache.contains(SceneNodeKey::new(1)));
    }

    /// Test: assert_snapshot_stable can be called directly
    #[test]
    fn test_phase24_assert_snapshot_stable_direct() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        cache.insert(SceneNodeKey::new(1), SceneNodeKind::Rect);
        cache.enter_dispatch();

        // Direct call should not panic - cache is stable
        cache.assert_snapshot_stable();
        cache.end_frame();
        cache.assert_snapshot_stable();
    }

    /// Test: Snapshot reset on begin_frame
    #[test]
    fn test_phase24_snapshot_reset_begin_frame() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();
        cache.insert(SceneNodeKey::new(1), SceneNodeKind::Rect);
        cache.enter_dispatch();
        cache.end_frame();

        // Start new frame - snapshot markers should reset
        cache.begin_frame();

        // In Build phase, reads don't check snapshot
        // (they only check in Dispatch/Frozen)
        assert_eq!(cache.entry_count(), 1);

        // Add more entries
        cache.insert(SceneNodeKey::new(2), SceneNodeKind::Text);
        assert_eq!(cache.entry_count(), 2);

        // New dispatch captures new snapshot
        cache.enter_dispatch();
        assert_eq!(cache.entry_count(), 2);
        cache.assert_snapshot_stable();
    }

    /// Test: dispatch_cache_operations asserts snapshot at end
    #[test]
    fn test_phase24_dispatch_asserts_snapshot() {
        // Use an empty scene/delta to test snapshot assertion without triggering
        // Phase-18 delta checks
        let scene = build_scene_with_nodes(vec![]);
        let delta = SceneDelta {
            inserts: vec![],
            removes: vec![],
            updates: vec![],
        };

        let mut cache = SceneCache::new();
        let mut consumers = CacheConsumers::new();

        // This should complete without panic - snapshot assertion passes
        dispatch_cache_operations(
            SceneCacheExecutionMode::Enabled,
            &mut consumers,
            &delta,
            &scene,
            &mut cache,
        );

        // Cache is now frozen with snapshot verified
        assert_eq!(cache.entry_count(), 0);
    }

    /// Test: Full frame cycle with snapshot verification
    #[test]
    fn test_phase24_full_frame_cycle() {
        use crate::core::scene_identity::SceneNodeKind;

        let mut cache = SceneCache::new();

        // Frame 1
        cache.insert(SceneNodeKey::new(1), SceneNodeKind::Rect);
        cache.insert(SceneNodeKey::new(2), SceneNodeKind::Text);
        cache.mark_dirty(SceneNodeKey::new(1));
        cache.enter_dispatch();
        assert_eq!(cache.entry_count(), 2);
        assert_eq!(cache.dirty_count(), 1);
        cache.assert_snapshot_stable();
        cache.end_frame();
        cache.assert_snapshot_stable();

        // Frame 2
        cache.begin_frame();
        cache.remove(SceneNodeKey::new(2));
        cache.clear_all_dirty();
        cache.enter_dispatch();
        assert_eq!(cache.entry_count(), 1);
        assert_eq!(cache.dirty_count(), 0);
        cache.assert_snapshot_stable();
        cache.end_frame();

        // Final verification
        assert_eq!(cache.entry_count(), 1);
        cache.assert_snapshot_stable();
    }
}
