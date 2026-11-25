//! EditorBridge: Consumer-side contract for editor widgets.
//!
//! The bridge between editor widgets (ComboBox, future editors) and the
//! TabularSubstrate intent system. EditorBridge is the only place that:
//! - Downcasts editor widgets (via WidgetDowncast)
//! - Polls `take_committed()`
//! - Constructs `EditIntent`
//!
//! ComboBox knows nothing about rows, columns, or payloads.
//! TabularSubstrate knows nothing about ComboBox.
//! EditorBridge is the seam.

use crate::model::selection_resolver::CommittedValue;
use crate::widgets::combo_box::ComboBox;
use crate::widgets::tabular_view::{ColumnId, EditIntent, EditPayload, RowId};

use crate::core::widget::Widget;

/// EditorBridge: owns one editor widget, polls for committed values,
/// converts to `EditPayload`, emits `EditIntent`.
///
/// This is the consumer-side contract for resolver-driven editor widgets.
/// It is not a widget. It does not render. It does not layout.
pub struct EditorBridge {
    row_id: RowId,
    column_id: ColumnId,
    editor: Box<dyn Widget>,
}

impl EditorBridge {
    pub fn new(row_id: RowId, column_id: ColumnId, editor: Box<dyn Widget>) -> Self {
        Self {
            row_id,
            column_id,
            editor,
        }
    }

    /// Poll the editor widget for a committed value.
    /// If committed, converts to `EditPayload` and returns `EditIntent::RequestCommitEdit`.
    /// Returns `None` if no commit occurred this cycle.
    pub fn poll(&mut self) -> Option<EditIntent> {
        // WidgetDowncast → take_committed() → EditPayload → EditIntent
        let combo = self.editor.as_any_mut().downcast_mut::<ComboBox>()?;
        let committed = combo.take_committed()?;
        let payload = committed_value_to_payload(committed);
        Some(EditIntent::RequestCommitEdit {
            row_id: self.row_id,
            column_id: self.column_id,
            payload,
        })
    }

    pub fn editor(&self) -> &dyn Widget {
        &*self.editor
    }

    pub fn editor_mut(&mut self) -> &mut dyn Widget {
        &mut *self.editor
    }

    pub fn row_id(&self) -> RowId {
        self.row_id
    }

    pub fn column_id(&self) -> ColumnId {
        self.column_id
    }

    /// Consume the bridge and return the owned editor widget.
    pub fn into_editor(self) -> Box<dyn Widget> {
        self.editor
    }
}

/// Convert `CommittedValue` to `EditPayload`.
///
/// `ExistingOption` → `EditPayload::Number` (raw `OptionId` as f64).
/// `CustomValue` → `EditPayload::String`.
fn committed_value_to_payload(value: CommittedValue) -> EditPayload {
    match value {
        CommittedValue::ExistingOption(id) => EditPayload::Number(id.raw() as f64),
        CommittedValue::CustomValue(text) => EditPayload::String(text),
    }
}
