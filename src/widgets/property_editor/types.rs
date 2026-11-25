// =============================================================================
// SPEC-HAL-260A §5: PropertyEditor Data Model
// =============================================================================
//
// PropertyValue, Cell, the CellSource trait, and the permission gate
// types (PermissionQuery / PermissionAction / PermissionResult) live in
// `src/engines/cell_source/mod.rs` — universal cell-access foundation
// shared across cell-bearing widgets (PropertyEditor, TreeView, future
// flat data grids, future spreadsheets).
//
// This file holds the PE-specific types:
//   - PropertyNode: tree-structural row metadata (node_id, label, parent
//     linkage, section flag). NO value — values come from the CellSource
//     via cell(row_id, column_id).
//   - PropertyDataSource: extends CellSource with tree topology (nodes()).
//   - ColumnKind.

use crate::engines::cell_source::CellSource;

// ----------------------------------------------------------------------------
// §5.2 PropertyNode + §5.5.1 PropertyNodeKind
// ----------------------------------------------------------------------------

/// PE-specific row archetype (SPEC-HAL-260A §5.5.1). Finer-grained than
/// the core binary `RowArchetype { Data, Band }` — PE distinguishes two
/// visual band levels (Section and SubSection) that paint with distinct
/// backgrounds. PE's `PropertyDataSource` impl projects this to the core
/// `RowArchetype` at the trait boundary: `Section` and `SubSection` both
/// map to `RowArchetype::Band`; `Data` maps to `RowArchetype::Data`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PropertyNodeKind {
    /// A property row hosting cells. `CellSource::cell()` returns `Some`
    /// per Data column when the consumer populates. Cell-mode focus,
    /// selection, and edit apply.
    Data,
    /// A top-level grouping band. No cells. Paints with
    /// `list_colors.header_background`. May host a disclosure toggle.
    Section,
    /// A nested grouping band within a Section. No cells. Paints with
    /// `list_colors.subheader_background`. INV-ARCHETYPE-HIERARCHY
    /// (§5.5.1): `parent_id` must resolve to a `Section` node — no
    /// SubSection at root, no SubSection under a Data row, no SubSection
    /// nested under another SubSection.
    SubSection,
}

/// A single property row in the PropertyEditor tree.
///
/// Tree-structural metadata ONLY — identity, label, parent linkage, row
/// archetype. Cell values are NOT carried by PropertyNode; they come from
/// the `CellSource` via `cell(row_id, column_id)` (see
/// `src/engines/cell_source/mod.rs`).
pub struct PropertyNode {
    /// Stable identifier. Maps 1:1 to HTS NodeMeta.node_id.
    pub node_id: u64,
    /// Display name shown in the tree column.
    pub label: String,
    /// Parent node_id (None for root-level properties).
    pub parent_id: Option<u64>,
    /// Row archetype (SPEC-HAL-260A §5.5.1). Governs paint, hit-test,
    /// focus, navigation, selection, and edit behavior. PE's
    /// `PropertyDataSource` impl projects this to the core `RowArchetype`
    /// (`{Data, Band}`) at the `CellSource::row_archetype` trait boundary.
    pub kind: PropertyNodeKind,
}

// ----------------------------------------------------------------------------
// §5.3 PropertyDataSource trait
// ----------------------------------------------------------------------------

/// Data source for PropertyEditor.
///
/// Extends `CellSource` (universal cell access — `cell()`, `commit_cell()`,
/// `validate()`, `revision()`, `permission_check()`) with PE-specific
/// tree topology (`nodes()`).
pub trait PropertyDataSource: CellSource {
    /// Return the full property tree (rows as PropertyNode topology). Cell
    /// values come separately via `CellSource::cell()`.
    fn nodes(&self) -> Vec<PropertyNode>;
}

// ----------------------------------------------------------------------------
// §5.4 ColumnKind
// ----------------------------------------------------------------------------

/// Column classification for cell-mode addressing and editability rules.
///
/// `#[non_exhaustive]` per SPEC-HAL-260A §5.4: future widening (e.g.,
/// `Indicator`, `Action`, `Name`) must not break trait users.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ColumnKind {
    /// Holds property values; subject to per-cell editability.
    Data,
    /// Structural or informational (tree label, future Indicator / Action
    /// columns). Not editable. May render disclosure toggles. May be a
    /// cursor target on Section rows.
    NonData,
}
