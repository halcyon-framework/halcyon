// SPEC-HAL-220: Breadcrumbs Navigation Intent Model
// External navigation intent sink. Written by widget, read by caller.
// Widget retains zero knowledge of emitted values.
// No persistence, no history, no binding.

use std::cell::Cell;
use std::rc::Rc;

use crate::widgets::breadcrumbs::CrumbId;

/// External navigation intent sink. Written by Breadcrumbs widget, read by caller.
/// Widget retains zero knowledge of emitted values after write.
pub type BreadcrumbsSink = Rc<Cell<Option<CrumbId>>>;

/// Create a new Breadcrumbs navigation intent sink.
pub fn breadcrumbs_sink() -> BreadcrumbsSink {
    Rc::new(Cell::new(None))
}
