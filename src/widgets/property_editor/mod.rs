// =============================================================================
// SPEC-HAL-260A: PropertyEditor
// =============================================================================
//
// Halcyon's composite widget for hierarchical property inspection and
// editing, governed by SPEC-HAL-260A. Originally developed as
// `property_editor_v2` alongside the SPEC-HAL-260 implementation; the
// cutover is complete — v1 is deleted, SPEC-HAL-260 is superseded, and
// this module is the sole PropertyEditor.
//
// Module location and policy: SPEC-HAL-260A §4.2.

pub(crate) mod body;
pub(crate) mod core;
pub(crate) mod footer;
pub mod policy;
pub(crate) mod property_editor;
pub mod types;

// Re-export the universal cell-access types under PE v2 for ergonomic
// consumer imports. Sources of truth live in `src/engines/cell_source/`.
pub use crate::engines::cell_source::{
    AdvanceDirection, Cell, CellSource, EditExitReason, PermissionAction, PermissionQuery,
    PermissionResult, PropertyValue, RowArchetype,
};

pub use crate::engines::cell_mechanics::{EditSession, PeFocus};
pub use body::PropertyEditorBody;
pub use core::{ColumnIntent, DataLayerIntent, EditIntent, PEIntent, TreeIntent};
pub use footer::PropertyEditorFooter;
pub use policy::{
    EditDisposition, EditTrigger, EmptyAreaClickAction, HeaderClickAction, HoverAction,
    PointerDownAction, PropertyEditorPolicy, SectionCursorColumn, TabBoundary, TabRowEnd,
    WrapBehavior,
};
pub use property_editor::PropertyEditor;
pub use types::{ColumnKind, PropertyDataSource, PropertyNode, PropertyNodeKind};
