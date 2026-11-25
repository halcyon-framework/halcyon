// =============================================================================
// SPEC-HAL-255: ColumnGeometryEngine — Extracted Column Geometry Resolution
// =============================================================================
//
// Extracted from src/widgets/tabular_view/geometry.rs per SPEC-HAL-255.
// This engine owns column geometry resolution, invariants, and epoch tracking.
//
// INVARIANT-1: Horizontal geometry resolution is owned exclusively by this engine.
// INVARIANT-2: No domain (Header, Body, Footer) may resolve or adjust horizontal geometry.
// INVARIANT-3: Scroll state must not influence geometry resolution.
// INVARIANT-4: Horizontal geometry is resolved exactly once per layout pass.
// INVARIANT-5: Any attempt to re-resolve geometry within the same evaluation is illegal.
// INVARIANT-6: Any attempt to read geometry before resolution is illegal.
// INVARIANT-7: Stale geometry reads are illegal.
//              Geometry is versioned per-evaluation. Reading geometry from a previous
//              evaluation epoch without re-resolution is a violation.
//              In debug builds, this MUST panic.
// =============================================================================

use crate::model::tabular::column::{ColumnId, ColumnModel, ColumnScrollDomain};

/// Resolved geometry for a single column.
///
/// SPEC-HAL-168: Resolution outputs are evaluation-local, immutable, and versioned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedColumnGeometry {
    /// Column identifier (for cross-referencing).
    pub column_id: ColumnId,
    /// Horizontal offset from TabularSubstrate origin.
    pub x_offset: f32,
    /// Resolved width after constraint application.
    pub width: f32,
    /// Scroll domain this column belongs to.
    pub scroll_domain: ColumnScrollDomain,
}

/// Read-only column slot projection.
///
/// SPEC-HAL-168: Decouples consumers (header, body, editors) from geometry internals.
/// Header slots and body slots are identical today, but this abstraction allows
/// future divergence for pinned columns, group headers, or tree indentation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColumnSlot {
    /// Column identifier.
    pub column_id: ColumnId,
    /// Horizontal position.
    pub x: f32,
    /// Width.
    pub width: f32,
    /// Scroll domain this column belongs to.
    pub scroll_domain: ColumnScrollDomain,
}

/// Column Geometry Engine.
///
/// SPEC-HAL-255: Extracted from TabularSubstrate as a standalone structural engine.
/// SPEC-HAL-168: Produced by the single resolution point during layout.
/// This engine is:
/// - evaluation-local
/// - immutable (after resolution)
/// - versioned (matches ColumnModel version)
/// - shared read-only across domains
#[derive(Debug)]
pub struct ColumnGeometryEngine {
    /// Resolved geometry per visible column (in display order).
    /// Layout: entries[0..pinned_count] are PinnedLeft,
    ///         entries[pinned_count..len-pinned_right_count] are Scrollable,
    ///         entries[len-pinned_right_count..] are PinnedRight.
    entries: Vec<ResolvedColumnGeometry>,
    /// Total content width (sum of all column widths).
    total_width: f32,
    /// Number of pinned-left columns (entries[0..pinned_count]).
    pinned_count: usize,
    /// Total width of pinned-left columns.
    pinned_width: f32,
    /// Number of pinned-right columns (entries[len-pinned_right_count..]).
    pinned_right_count: usize,
    /// Total width of pinned-right columns.
    pinned_right_width: f32,
    /// Version of the ColumnModel this was resolved from.
    model_version: u64,
    /// Evaluation epoch when this was resolved (for staleness detection).
    evaluation_epoch: u64,
    /// Whether resolution has completed for this evaluation.
    resolved: bool,
    /// Pixel-snapped available width from last resolution (for deterministic change detection).
    last_available_width: f32,
    /// Debug-only: count of full recompute executions (not short-circuits).
    #[cfg(debug_assertions)]
    full_resolve_count: u64,
    /// Debug-only: count of short-circuit hits (epoch stamp only, no recompute).
    #[cfg(debug_assertions)]
    short_circuit_count: u64,
}

/// Snap width to integer pixel grid for deterministic comparison.
///
/// SPEC-HAL-255: Float drift in available_width must not defeat the short-circuit.
impl ColumnGeometryEngine {
    /// Create an unresolved geometry engine.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            total_width: 0.0,
            pinned_count: 0,
            pinned_width: 0.0,
            pinned_right_count: 0,
            pinned_right_width: 0.0,
            model_version: 0,
            evaluation_epoch: 0,
            resolved: false,
            last_available_width: -1.0,
            #[cfg(debug_assertions)]
            full_resolve_count: 0,
            #[cfg(debug_assertions)]
            short_circuit_count: 0,
        }
    }

    /// Check if geometry has been resolved for the current evaluation.
    pub fn is_resolved(&self) -> bool {
        self.resolved
    }

    /// Get the evaluation epoch when this was resolved.
    pub fn evaluation_epoch(&self) -> u64 {
        self.evaluation_epoch
    }

    /// Get the model version this was resolved from.
    pub fn model_version(&self) -> u64 {
        self.model_version
    }

    /// Non-panicking check: is geometry resolved for the given epoch?
    ///
    /// Event-path code uses this to gate geometry reads without triggering
    /// INVARIANT-6/7 panics when geometry is invalidated between layout passes.
    pub fn is_resolved_for_epoch(&self, epoch: u64) -> bool {
        self.resolved && self.evaluation_epoch == epoch
    }

    /// Get total content width.
    ///
    /// SPEC-HAL-168 INVARIANT-6: Panics if called before resolution.
    /// SPEC-HAL-168 INVARIANT-7: Panics (debug) if geometry is stale for given epoch.
    pub fn total_width(&self, epoch: u64) -> f32 {
        self.assert_resolved_for_epoch(epoch);
        self.total_width
    }

    /// Get all resolved column geometries.
    ///
    /// SPEC-HAL-168 INVARIANT-6: Panics if called before resolution.
    /// SPEC-HAL-168 INVARIANT-7: Panics (debug) if geometry is stale for given epoch.
    pub fn columns(&self, epoch: u64) -> &[ResolvedColumnGeometry] {
        self.assert_resolved_for_epoch(epoch);
        &self.entries
    }

    /// Get resolved geometry for a specific column.
    ///
    /// SPEC-HAL-168 INVARIANT-6: Panics if called before resolution.
    /// SPEC-HAL-168 INVARIANT-7: Panics (debug) if geometry is stale for given epoch.
    pub fn get(&self, id: ColumnId, epoch: u64) -> Option<&ResolvedColumnGeometry> {
        self.assert_resolved_for_epoch(epoch);
        self.entries.iter().find(|e| e.column_id == id)
    }

    /// Get column index by ID.
    ///
    /// SPEC-HAL-168 INVARIANT-6: Panics if called before resolution.
    /// SPEC-HAL-168 INVARIANT-7: Panics (debug) if geometry is stale for given epoch.
    pub fn index_of(&self, id: ColumnId, epoch: u64) -> Option<usize> {
        self.assert_resolved_for_epoch(epoch);
        self.entries.iter().position(|e| e.column_id == id)
    }

    /// Get header column slots.
    ///
    /// SPEC-HAL-168: Decoupled projection for header domain.
    /// SPEC-HAL-168 INVARIANT-7: Panics (debug) if geometry is stale for given epoch.
    pub fn header_slots(&self, epoch: u64) -> impl Iterator<Item = ColumnSlot> + '_ {
        self.assert_resolved_for_epoch(epoch);
        self.entries.iter().map(|e| ColumnSlot {
            column_id: e.column_id,
            x: e.x_offset,
            width: e.width,
            scroll_domain: e.scroll_domain,
        })
    }

    /// Get body column slots.
    ///
    /// SPEC-HAL-168: Decoupled projection for body domain.
    /// SPEC-HAL-168 INVARIANT-7: Panics (debug) if geometry is stale for given epoch.
    pub fn body_slots(&self, epoch: u64) -> impl Iterator<Item = ColumnSlot> + '_ {
        self.assert_resolved_for_epoch(epoch);
        self.entries.iter().map(|e| ColumnSlot {
            column_id: e.column_id,
            x: e.x_offset,
            width: e.width,
            scroll_domain: e.scroll_domain,
        })
    }

    /// Get pinned column width.
    ///
    /// SPEC-HAL-168 INVARIANT-6: Panics if called before resolution.
    /// SPEC-HAL-168 INVARIANT-7: Panics (debug) if geometry is stale for given epoch.
    pub fn pinned_width(&self, epoch: u64) -> f32 {
        self.assert_resolved_for_epoch(epoch);
        self.pinned_width
    }

    /// Get pinned column count.
    ///
    /// SPEC-HAL-168 INVARIANT-6: Panics if called before resolution.
    /// SPEC-HAL-168 INVARIANT-7: Panics (debug) if geometry is stale for given epoch.
    pub fn pinned_count(&self, epoch: u64) -> usize {
        self.assert_resolved_for_epoch(epoch);
        self.pinned_count
    }

    /// Get pinned column entries.
    ///
    /// SPEC-HAL-168 INVARIANT-6: Panics if called before resolution.
    /// SPEC-HAL-168 INVARIANT-7: Panics (debug) if geometry is stale for given epoch.
    pub fn pinned_columns(&self, epoch: u64) -> &[ResolvedColumnGeometry] {
        self.assert_resolved_for_epoch(epoch);
        &self.entries[..self.pinned_count]
    }

    /// Get scrollable column entries.
    ///
    /// SPEC-HAL-168 INVARIANT-6: Panics if called before resolution.
    /// SPEC-HAL-168 INVARIANT-7: Panics (debug) if geometry is stale for given epoch.
    pub fn scrollable_columns(&self, epoch: u64) -> &[ResolvedColumnGeometry] {
        self.assert_resolved_for_epoch(epoch);
        let end = self.entries.len() - self.pinned_right_count;
        &self.entries[self.pinned_count..end]
    }

    /// Get pinned-right column count.
    ///
    /// SPEC-HAL-168 INVARIANT-6: Panics if called before resolution.
    /// SPEC-HAL-168 INVARIANT-7: Panics (debug) if geometry is stale for given epoch.
    pub fn pinned_right_count(&self, epoch: u64) -> usize {
        self.assert_resolved_for_epoch(epoch);
        self.pinned_right_count
    }

    /// Get pinned-right column width.
    ///
    /// SPEC-HAL-168 INVARIANT-6: Panics if called before resolution.
    /// SPEC-HAL-168 INVARIANT-7: Panics (debug) if geometry is stale for given epoch.
    pub fn pinned_right_width(&self, epoch: u64) -> f32 {
        self.assert_resolved_for_epoch(epoch);
        self.pinned_right_width
    }

    /// Get pinned-right column entries.
    ///
    /// SPEC-HAL-168 INVARIANT-6: Panics if called before resolution.
    /// SPEC-HAL-168 INVARIANT-7: Panics (debug) if geometry is stale for given epoch.
    pub fn pinned_right_columns(&self, epoch: u64) -> &[ResolvedColumnGeometry] {
        self.assert_resolved_for_epoch(epoch);
        let start = self.entries.len() - self.pinned_right_count;
        &self.entries[start..]
    }

    /// Get column count.
    ///
    /// SPEC-HAL-168 INVARIANT-6: Panics if called before resolution.
    /// SPEC-HAL-168 INVARIANT-7: Panics (debug) if geometry is stale for given epoch.
    pub fn len(&self, epoch: u64) -> usize {
        self.assert_resolved_for_epoch(epoch);
        self.entries.len()
    }

    /// Check if empty.
    ///
    /// SPEC-HAL-168 INVARIANT-6: Panics if called before resolution.
    /// SPEC-HAL-168 INVARIANT-7: Panics (debug) if geometry is stale for given epoch.
    pub fn is_empty(&self, epoch: u64) -> bool {
        self.assert_resolved_for_epoch(epoch);
        self.entries.is_empty()
    }

    /// Resolve column geometry from the model (full recompute).
    ///
    /// SPEC-HAL-258: Pure execution function — always performs full recompute.
    /// No internal short-circuit or self-healing logic. The substrate decides
    /// whether to call resolve() (dirty) or stamp_epoch() (clean).
    ///
    /// Columns are partitioned into two scroll domains:
    /// - PinnedLeft columns are resolved first (entries[0..pinned_count])
    /// - Scrollable columns are resolved second (entries[pinned_count..])
    ///
    /// SPEC-HAL-168 INVARIANT-5: Panics if already resolved for this evaluation.
    /// INV-RESOLVE-PURE (debug): Panics if model version and width are unchanged.
    pub(crate) fn resolve(
        &mut self,
        model: &ColumnModel,
        available_width: f32,
        evaluation_epoch: u64,
        scale_factor: f32,
    ) {
        // INVARIANT-5: Cannot re-resolve within the same evaluation
        if self.resolved && self.evaluation_epoch == evaluation_epoch {
            panic!(
                "\n\
                ╔══════════════════════════════════════════════════════════════════════╗\n\
                ║  SPEC-HAL-168 INVARIANT-5 VIOLATION: Re-resolution attempted          ║\n\
                ╠══════════════════════════════════════════════════════════════════════╣\n\
                ║  Horizontal geometry was already resolved for evaluation epoch {}.  \n\
                ║  Geometry must be resolved exactly once per layout pass.             ║\n\
                ╚══════════════════════════════════════════════════════════════════════╝",
                evaluation_epoch
            );
        }

        // Engine must be driven by substrate evaluation cycle (epoch starts at 1).
        #[cfg(debug_assertions)]
        debug_assert!(
            evaluation_epoch != 0,
            "Engine must be driven by substrate evaluation cycle"
        );

        // SPEC-HAL-258: Pure execution — no internal short-circuit.
        // The substrate decides whether to call resolve() or stamp_epoch().
        let _ = scale_factor; // Kept in signature for API stability.

        // Debug: panic if model version AND width are both unchanged.
        // This means the substrate should have called stamp_epoch() instead.
        #[cfg(debug_assertions)]
        {
            if self.resolved
                && model.version() == self.model_version
                && available_width == self.last_available_width
            {
                panic!(
                    "\n\
                    ╔══════════════════════════════════════════════════════════════════════╗\n\
                    ║  INV-RESOLVE-PURE VIOLATION: Unnecessary full recompute              ║\n\
                    ╠══════════════════════════════════════════════════════════════════════╣\n\
                    ║  resolve() called but model version ({}) and width ({}) unchanged.  \n\
                    ║  Substrate must call stamp_epoch() when geometry is not dirty.       ║\n\
                    ╚══════════════════════════════════════════════════════════════════════╝",
                    model.version(),
                    available_width
                );
            }
        }

        #[cfg(debug_assertions)]
        {
            self.full_resolve_count += 1;
        }

        self.entries.clear();

        // Collect visible columns and partition by scroll domain.
        let visible_columns: Vec<_> = model.visible_columns().collect();
        if visible_columns.is_empty() {
            self.total_width = 0.0;
            self.pinned_count = 0;
            self.pinned_width = 0.0;
            self.pinned_right_count = 0;
            self.pinned_right_width = 0.0;
            self.model_version = model.version();
            self.last_available_width = available_width;
            self.evaluation_epoch = evaluation_epoch;
            self.resolved = true;
            return;
        }

        let pinned: Vec<_> = visible_columns
            .iter()
            .filter(|c| c.scroll_domain == ColumnScrollDomain::PinnedLeft)
            .collect();
        let scrollable: Vec<_> = visible_columns
            .iter()
            .filter(|c| c.scroll_domain == ColumnScrollDomain::Scrollable)
            .collect();
        let pinned_right: Vec<_> = visible_columns
            .iter()
            .filter(|c| c.scroll_domain == ColumnScrollDomain::PinnedRight)
            .collect();

        // INV-SCROLL-DOMAIN-ELASTIC: Pinned columns (left or right) must not be elastic.
        // Elastic columns must be in the Scrollable domain.
        let pinned_elastic = pinned.iter().any(|c| c.flex.is_some())
            || pinned_right.iter().any(|c| c.flex.is_some());
        assert!(
            !pinned_elastic,
            "INV-SCROLL-DOMAIN-ELASTIC: Pinned columns must not be elastic. \
             Elastic columns must be in the Scrollable domain."
        );

        // SPEC-HAL-180 INV-COL-ELASTIC-1: At most one elastic column per layout (scoped to scrollable).
        let elastic_count = scrollable.iter().filter(|c| c.flex.is_some()).count();
        assert!(
            elastic_count <= 1,
            "INV-COL-ELASTIC-1: At most one elastic column per layout, found {}. Column IDs with flex: {:?}",
            elastic_count,
            scrollable
                .iter()
                .filter(|c| c.flex.is_some())
                .map(|c| c.id.raw())
                .collect::<Vec<_>>()
        );

        // =====================================================================
        // Phase 1: Resolve pinned-left columns (x starts at 0).
        // Pinned columns use preferred_width clamped by min/max, no flex.
        //
        // PIXEL-SNAP: Use snapped accumulator to ensure every column's
        // x_offset and width are integers. This prevents sub-pixel jitter
        // during window resize when widget rects have fractional positions.
        // =====================================================================
        // Column widths stored as logical floats. Separator x positions are
        // snapped to physical pixels in the paint layer (list_view body/header)
        // so crisp 1px lines are preserved. Cell content geometry is fractional
        // by design — the text engine handles subpixel glyph positioning.
        let mut x_cursor: f32 = 0.0;
        for col in &pinned {
            let logical_width = col.preferred_width.clamp(col.min_width, col.max_width);
            self.entries.push(ResolvedColumnGeometry {
                column_id: col.id,
                x_offset: x_cursor,
                width: logical_width,
                scroll_domain: ColumnScrollDomain::PinnedLeft,
            });
            x_cursor += logical_width;
        }
        self.pinned_count = pinned.len();
        self.pinned_width = x_cursor;

        // Compute pinned-right total width for budget calculation.
        let mut pr_width = 0.0;
        for col in &pinned_right {
            pr_width += col.preferred_width.clamp(col.min_width, col.max_width);
        }
        self.pinned_right_count = pinned_right.len();
        self.pinned_right_width = pr_width;

        // =====================================================================
        // Phase 2: Resolve scrollable columns (x starts at pinned_width).
        //
        // INV-SCROLL-DOMAIN-BUDGET: Scrollable domain available width is computed
        // after subtracting pinned-left and pinned-right widths.
        // =====================================================================
        let scrollable_available =
            (available_width - self.pinned_width - self.pinned_right_width).max(0.0);

        // Calculate fixed scrollable widths and total flex.
        let mut fixed_width_total = 0.0;
        let mut total_flex = 0.0;

        for col in &scrollable {
            if col.flex.is_none() {
                let width = col.preferred_width.clamp(col.min_width, col.max_width);
                fixed_width_total += width;
            } else {
                total_flex += col.flex.unwrap_or(0.0);
            }
        }

        // Distribute remaining space to flex columns.
        let remaining_space = (scrollable_available - fixed_width_total).max(0.0);

        for col in &scrollable {
            let logical_width = if let Some(flex) = col.flex {
                if total_flex > 0.0 {
                    let flex_share = (flex / total_flex) * remaining_space;
                    // Content-aware elastic (SPEC-HAL-180): when the
                    // column declares a content_width floor, the
                    // resolved elastic width is the maximum of its
                    // per-flex share and the content floor, then
                    // clamped by min/max. When content_width is None
                    // (default), this reduces exactly to the
                    // pre-existing math: `flex_share.clamp(min, max)`.
                    let content_floor = col.content_width.unwrap_or(0.0);
                    flex_share
                        .max(content_floor)
                        .clamp(col.min_width, col.max_width)
                } else {
                    col.preferred_width.clamp(col.min_width, col.max_width)
                }
            } else {
                col.preferred_width.clamp(col.min_width, col.max_width)
            };

            self.entries.push(ResolvedColumnGeometry {
                column_id: col.id,
                x_offset: x_cursor,
                width: logical_width,
                scroll_domain: ColumnScrollDomain::Scrollable,
            });

            x_cursor += logical_width;
        }

        // =====================================================================
        // Phase 3: Resolve pinned-right columns (x starts at available_width - pinned_right_width).
        // =====================================================================
        let mut pr_x = available_width - self.pinned_right_width;
        for col in &pinned_right {
            let width = col.preferred_width.clamp(col.min_width, col.max_width);
            self.entries.push(ResolvedColumnGeometry {
                column_id: col.id,
                x_offset: pr_x,
                width,
                scroll_domain: ColumnScrollDomain::PinnedRight,
            });
            pr_x += width;
        }

        self.total_width = x_cursor + self.pinned_right_width;

        self.model_version = model.version();
        self.last_available_width = available_width;
        self.evaluation_epoch = evaluation_epoch;
        self.resolved = true;
    }

    /// Check if geometry needs a full resolve (model or width changed, or never resolved).
    ///
    /// Returns true if `resolve()` should be called, false if `stamp_epoch()` suffices.
    /// Uses the exact same dirty condition as the INV-RESOLVE-PURE guard.
    pub(crate) fn needs_resolve(
        &self,
        model: &ColumnModel,
        available_width: f32,
        _scale_factor: f32,
    ) -> bool {
        !self.resolved
            || model.version() != self.model_version
            || available_width != self.last_available_width
    }

    /// Explicitly invalidate geometry, forcing re-resolution on next resolve() call.
    ///
    /// SPEC-HAL-255: Called only when external conditions require forced re-resolution
    /// (e.g., initial layout before any model version exists). NOT called per-frame.
    pub fn invalidate(&mut self) {
        self.resolved = false;
    }

    /// Stamp epoch without recomputing geometry.
    ///
    /// SPEC-HAL-258: Called by substrate when geometry_dirty is false — model and
    /// width are unchanged, existing geometry is valid, only epoch needs advancement.
    ///
    /// SPEC-HAL-168 INVARIANT-5: Panics if already stamped/resolved for this epoch.
    /// Requires that geometry was previously resolved (debug assertion).
    pub(crate) fn stamp_epoch(&mut self, evaluation_epoch: u64) {
        // INVARIANT-5: Cannot stamp within the same evaluation.
        if self.resolved && self.evaluation_epoch == evaluation_epoch {
            panic!(
                "\n\
                ╔══════════════════════════════════════════════════════════════════════╗\n\
                ║  SPEC-HAL-168 INVARIANT-5 VIOLATION: Re-stamp attempted               ║\n\
                ╠══════════════════════════════════════════════════════════════════════╣\n\
                ║  Geometry epoch was already stamped for evaluation epoch {}.         \n\
                ║  Geometry must be resolved or stamped exactly once per layout pass.  ║\n\
                ╚══════════════════════════════════════════════════════════════════════╝",
                evaluation_epoch
            );
        }
        debug_assert!(self.resolved, "stamp_epoch called on never-resolved engine");

        #[cfg(debug_assertions)]
        {
            self.short_circuit_count += 1;
        }
        self.evaluation_epoch = evaluation_epoch;
    }

    /// Debug-only: number of full geometry recomputes (not short-circuits).
    #[cfg(debug_assertions)]
    pub fn debug_full_resolve_count(&self) -> u64 {
        self.full_resolve_count
    }

    /// Debug-only: number of short-circuit hits (epoch stamp only).
    #[cfg(debug_assertions)]
    pub fn debug_short_circuit_count(&self) -> u64 {
        self.short_circuit_count
    }

    /// Assert that geometry has been resolved.
    ///
    /// SPEC-HAL-168 INVARIANT-6: Reading geometry before resolution is illegal.
    fn assert_resolved(&self) {
        if !self.resolved {
            panic!(
                "\n\
                ╔══════════════════════════════════════════════════════════════════════╗\n\
                ║  SPEC-HAL-168 INVARIANT-6 VIOLATION: Geometry read before resolution  ║\n\
                ╠══════════════════════════════════════════════════════════════════════╣\n\
                ║  Column geometry was accessed before resolve() was called.           ║\n\
                ║  Geometry must be resolved during layout phase before any reads.     ║\n\
                ╚══════════════════════════════════════════════════════════════════════╝"
            );
        }
    }

    /// Centralized epoch-bound access barrier.
    ///
    /// Combines INVARIANT-6 (resolved) and INVARIANT-7 (staleness) into a single
    /// guard. This is the canonical entry point for ALL geometry read paths:
    /// both GeometryView and direct engine accessors.
    ///
    /// - INVARIANT-6: Always panics if geometry not resolved.
    /// - INVARIANT-7: Panics in debug builds if geometry is stale (epoch mismatch).
    fn assert_resolved_for_epoch(&self, epoch: u64) {
        self.assert_resolved();
        self.assert_not_stale(epoch);
    }

    /// Check if geometry is stale relative to a given evaluation epoch.
    ///
    /// SPEC-HAL-168 INVARIANT-7: Stale geometry reads are illegal.
    pub fn is_stale(&self, current_epoch: u64) -> bool {
        self.evaluation_epoch != current_epoch
    }

    /// Assert that geometry is not stale.
    ///
    /// SPEC-HAL-168 INVARIANT-7: In debug builds, stale reads MUST panic.
    #[cfg(debug_assertions)]
    pub fn assert_not_stale(&self, current_epoch: u64) {
        if self.is_stale(current_epoch) {
            panic!(
                "\n\
                ╔══════════════════════════════════════════════════════════════════════╗\n\
                ║  SPEC-HAL-168 INVARIANT-7 VIOLATION: Stale geometry read detected     ║\n\
                ╠══════════════════════════════════════════════════════════════════════╣\n\
                ║  Geometry from epoch {} was read during epoch {}.                    \n\
                ║  Geometry is evaluation-local and must not be accessed across        ║\n\
                ║  evaluations without re-resolution.                                  ║\n\
                ╚══════════════════════════════════════════════════════════════════════╝",
                self.evaluation_epoch, current_epoch
            );
        }
    }

    /// Assert that geometry is not stale (no-op in release builds).
    ///
    /// SPEC-HAL-168 INVARIANT-7: Stale detection is debug-only.
    #[cfg(not(debug_assertions))]
    #[inline]
    pub fn assert_not_stale(&self, _current_epoch: u64) {
        // Intentionally empty in release builds
    }
}

impl Default for ColumnGeometryEngine {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// GeometryView: Epoch-safe read-only view of column geometry
// =============================================================================

/// Epoch-safe read-only view of column geometry.
///
/// SPEC-HAL-168: All geometry reads go through this view, which enforces:
/// - INVARIANT-6: Panics if geometry not resolved
/// - INVARIANT-7: Panics (in debug) if geometry is stale
///
/// This wrapper ensures callers cannot accidentally read stale geometry.
#[derive(Debug, Clone, Copy)]
pub struct GeometryView<'a> {
    table: &'a ColumnGeometryEngine,
    epoch: u64,
}

impl<'a> GeometryView<'a> {
    /// Create a new geometry view with epoch enforcement.
    ///
    /// Immediately validates resolved + not-stale invariants via the
    /// centralized epoch-bound access barrier.
    pub fn new(table: &'a ColumnGeometryEngine, epoch: u64) -> Self {
        table.assert_resolved_for_epoch(epoch);
        Self { table, epoch }
    }

    /// Get header column slots.
    ///
    /// SPEC-HAL-168: Decoupled projection for header domain.
    pub fn header_slots(&self) -> impl Iterator<Item = ColumnSlot> + '_ {
        self.table.entries.iter().map(|e| ColumnSlot {
            column_id: e.column_id,
            x: e.x_offset,
            width: e.width,
            scroll_domain: e.scroll_domain,
        })
    }

    /// Get body column slots.
    ///
    /// SPEC-HAL-168: Decoupled projection for body domain.
    pub fn body_slots(&self) -> impl Iterator<Item = ColumnSlot> + '_ {
        self.table.entries.iter().map(|e| ColumnSlot {
            column_id: e.column_id,
            x: e.x_offset,
            width: e.width,
            scroll_domain: e.scroll_domain,
        })
    }

    /// Get pinned column slots only.
    pub fn pinned_slots(&self) -> impl Iterator<Item = ColumnSlot> + '_ {
        self.table.entries[..self.table.pinned_count]
            .iter()
            .map(|e| ColumnSlot {
                column_id: e.column_id,
                x: e.x_offset,
                width: e.width,
                scroll_domain: e.scroll_domain,
            })
    }

    /// Get scrollable column slots only.
    pub fn scrollable_slots(&self) -> impl Iterator<Item = ColumnSlot> + '_ {
        let end = self.table.entries.len() - self.table.pinned_right_count;
        self.table.entries[self.table.pinned_count..end]
            .iter()
            .map(|e| ColumnSlot {
                column_id: e.column_id,
                x: e.x_offset,
                width: e.width,
                scroll_domain: e.scroll_domain,
            })
    }

    /// Get pinned-right column slots only.
    pub fn pinned_right_slots(&self) -> impl Iterator<Item = ColumnSlot> + '_ {
        let start = self.table.entries.len() - self.table.pinned_right_count;
        self.table.entries[start..].iter().map(|e| ColumnSlot {
            column_id: e.column_id,
            x: e.x_offset,
            width: e.width,
            scroll_domain: e.scroll_domain,
        })
    }

    /// Get pinned-right total width.
    pub fn pinned_right_width(&self) -> f32 {
        self.table.pinned_right_width
    }

    /// Get total content width.
    pub fn total_width(&self) -> f32 {
        self.table.total_width
    }

    /// Get pinned column width.
    pub fn pinned_width(&self) -> f32 {
        self.table.pinned_width
    }

    /// Get slot for a specific column by ID.
    pub fn get(&self, id: ColumnId) -> Option<ColumnSlot> {
        self.table
            .entries
            .iter()
            .find(|e| e.column_id == id)
            .map(|e| ColumnSlot {
                column_id: e.column_id,
                x: e.x_offset,
                width: e.width,
                scroll_domain: e.scroll_domain,
            })
    }

    /// Get column index by ID.
    pub fn index_of(&self, id: ColumnId) -> Option<usize> {
        self.table.entries.iter().position(|e| e.column_id == id)
    }

    /// Get column count.
    pub fn len(&self) -> usize {
        self.table.entries.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.table.entries.is_empty()
    }

    /// Get the frame epoch this view is bound to.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
}
