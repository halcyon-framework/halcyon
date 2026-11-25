//! SPEC-HAL-084: Pass Executor Implementations
//!
//! Concrete pass executors for the RenderGraph. Each executor implements
//! PassExecutor and encodes GPU commands for a specific rendering pass.

pub mod main_pass;
pub mod overlay_pass;
pub(crate) mod pipeline_dispatch;

pub use main_pass::MainPassExecutor;
pub use overlay_pass::OverlayPassExecutor;
