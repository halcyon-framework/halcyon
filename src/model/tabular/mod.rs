//! Tabular model types shared between engines and widgets.
//!
//! Column types (ColumnId, Column, ColumnModel, ColumnScrollDomain),
//! engine-grade row height oracle (RowHeights), and row expansion
//! state model (RowExpansion) live here.

pub mod column;
pub mod row_expansion;
pub mod row_heights;
