// =============================================================================
// SPEC-HAL-335: TemporalSubstrate — Engine Modules
// =============================================================================
//
// Structural engines for time-positioned projection. Engines own invariants
// and resolve logic; they have no widget, layout, or rendering dependencies.
// =============================================================================

pub mod containment;
pub mod descriptors;
pub mod emphasis;
pub mod hts_rows;
pub mod interval_projection;
pub mod lane_layout;
pub mod substrate;
pub mod time_geometry;

pub use containment::{ContainmentPolicy, ContainmentSource, RootedContainmentSource};
pub use descriptors::{
    ArrowDescriptor, IntervalDescriptor, IntervalEdge, IntervalId, IntervalLayer, IntervalSource,
    IntervalStyle, MarkerDescriptor, MarkerStyle, RowBandDescriptor, TemporalDescriptors,
    TemporalInvalidationEvent,
};
pub use emphasis::{EmphasisRelation, EmphasisRole, EmphasisState};
pub use hts_rows::{RowGeometry, hts_rows};
pub use interval_projection::{IntervalProjection, TemporalFrameSnapshot};
pub use lane_layout::{LaneLayoutEngine, LanePolicy, MAX_LANES};
pub use substrate::{SharedTemporalSubstrate, TemporalIntent, TemporalSubstrate, TimeContext};
pub use time_geometry::{
    HeaderCell, HeaderTier, TimeGeometryEngine, TimeGeometryResolution, TimeScale, WorkingCalendar,
};
