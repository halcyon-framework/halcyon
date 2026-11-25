// SPEC-HAL-219: TabBar Selection Intent Model
// External selection sink. Written by widget, read by caller.
// Widget retains zero knowledge of emitted values.
// No persistence, no history, no binding.

use std::cell::Cell;
use std::rc::Rc;

use crate::widgets::tab_bar::TabId;

/// External selection sink. Written by TabBar widget, read by caller.
/// Widget retains zero knowledge of emitted values after write.
pub type TabBarSink = Rc<Cell<Option<TabId>>>;

/// Create a new TabBar selection sink.
pub fn tab_bar_sink() -> TabBarSink {
    Rc::new(Cell::new(None))
}
