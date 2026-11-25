// =============================================================================
// SPEC-HAL-260A §11: PropertyEditor Interaction Policy
// =============================================================================
//
// Per-instance interaction policy. Each field is a closed enum governing one
// interaction axis. Defaults match the §11.1 table exactly.

// ----------------------------------------------------------------------------
// §11 PropertyEditorPolicy
// ----------------------------------------------------------------------------

/// Per-instance interaction policy. Carried by `PropertyEditorCore`; consulted
/// by event handlers, the cell-mode navigation reducer (§10), the intent
/// dispatch pipeline (§12), and the edit lifecycle (§7).
#[derive(Clone, Debug)]
pub struct PropertyEditorPolicy {
    /// Wrap behavior on row-axis navigation past first/last row.
    pub wrap_row_axis: WrapBehavior,
    /// Wrap behavior on column-axis navigation past first/last column.
    pub wrap_column_axis: WrapBehavior,
    /// Tab behavior when the cursor is in the last cell of a row.
    pub tab_at_row_end: TabRowEnd,
    /// Tab behavior when the cursor is in PE's last cell overall.
    pub tab_at_last_cell: TabBoundary,
    /// What input triggers an edit-begin on an editable cell.
    pub edit_trigger: EditTrigger,
    /// Pointer Down on a cell: move cursor only, or move + begin edit.
    pub pointer_down_on_cell: PointerDownAction,
    /// Hover visual: none, or row highlight.
    pub hover: HoverAction,
    /// Disposition of an active edit when PE itself loses focus (engine-state
    /// blur). OS-deactivation blur is NOT a trigger — see §11.3 and
    /// INV-PE-SOVEREIGNTY.
    pub blur_disposition: EditDisposition,
    /// Disposition of an active edit when the user clicks elsewhere within PE
    /// (different cell / row).
    pub click_elsewhere_disposition: EditDisposition,
    /// Which column the cursor occupies on a Section row.
    pub section_row_cursor_column: SectionCursorColumn,
    /// Header click action. `Sort` emits a DataLayerIntent (§12); no
    /// in-PE sorting (§11.2).
    pub header_click: HeaderClickAction,
    /// Click in the empty area below the last row.
    pub empty_area_click: EmptyAreaClickAction,
}

// ----------------------------------------------------------------------------
// Sub-enums (SPEC-HAL-260A §11)
// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Debug)]
pub enum WrapBehavior {
    NoWrap,
    Wrap,
}

#[derive(Copy, Clone, Debug)]
pub enum TabRowEnd {
    WrapToNextRow,
    StayInRow,
}

#[derive(Copy, Clone, Debug)]
pub enum TabBoundary {
    NoOp,
    WrapToFirstCell,
}

#[derive(Copy, Clone, Debug)]
pub enum EditTrigger {
    SingleClick,
    DoubleClick,
    EnterOrI,
    ManualOnly,
}

#[derive(Copy, Clone, Debug)]
pub enum PointerDownAction {
    MoveCursor,
    MoveCursorAndBeginEdit,
}

#[derive(Copy, Clone, Debug)]
pub enum HoverAction {
    None,
    Highlight,
}

#[derive(Copy, Clone, Debug)]
pub enum EditDisposition {
    Commit,
    Cancel,
}

#[derive(Copy, Clone, Debug)]
pub enum SectionCursorColumn {
    FirstNonData,
    FirstColumn,
}

#[derive(Copy, Clone, Debug)]
pub enum HeaderClickAction {
    None,
    Sort,
}

#[derive(Copy, Clone, Debug)]
pub enum EmptyAreaClickAction {
    FocusEditor,
    NoOp,
}

// ----------------------------------------------------------------------------
// §11.1 Defaults
// ----------------------------------------------------------------------------

impl Default for PropertyEditorPolicy {
    fn default() -> Self {
        Self {
            wrap_row_axis: WrapBehavior::NoWrap,
            wrap_column_axis: WrapBehavior::NoWrap,
            tab_at_row_end: TabRowEnd::WrapToNextRow,
            tab_at_last_cell: TabBoundary::NoOp,
            edit_trigger: EditTrigger::DoubleClick,
            pointer_down_on_cell: PointerDownAction::MoveCursor,
            hover: HoverAction::Highlight,
            blur_disposition: EditDisposition::Commit,
            click_elsewhere_disposition: EditDisposition::Commit,
            section_row_cursor_column: SectionCursorColumn::FirstNonData,
            header_click: HeaderClickAction::None,
            empty_area_click: EmptyAreaClickAction::FocusEditor,
        }
    }
}
