//! SPEC-R-003: GPU Path Rendering Pipeline Modules
//!
//! This module contains pipeline implementations for shape rendering.
//! The shape module owns the analytic SDF shader/pipeline contract; renderer
//! buffer generation and draw routing are integrated separately.

pub(crate) mod analytic_surface;
pub mod path_pipeline;
pub mod shape_pipeline;
pub mod text_pipeline;
