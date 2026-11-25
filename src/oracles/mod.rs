//! SPEC-HAL-188 Data Oracles
//!
//! This module contains canonical oracle implementations for the SPEC-HAL-188
//! data adapter contract. These oracles serve as behavior references for testing
//! and as drop-in data sources for simple use cases.
//!
//! ## Architectural Boundary
//!
//! Oracles are **reference implementations** of SPEC-HAL-188 contracts. They exist
//! to provide ground-truth behavior for testing and to serve as examples.
//!
//! **Allowed consumers:**
//! - Tests (`src/tests/**`, `tests/**`)
//! - Examples (`examples/**`)
//! - Simple/demo applications
//! - Top-level application glue (e.g., `main.rs`)
//!
//! **Forbidden consumers:**
//! - UI widgets (`src/widgets/**`) — MUST depend only on contracts, never concrete oracles
//! - Runtime (`src/runtime/**`) — MUST remain oracle-agnostic
//! - Compositor (`src/compositor/**`) — MUST remain oracle-agnostic
//!
//! Violating this boundary causes the UI layer to optimize for "nice data" and
//! breaks virtualized, remote, denied, or partial data scenarios.
//!
//! ## Available Oracles
//!
//! - `BaselineDataOracle`: SPEC-188 §15.1 baseline oracle — synchronous, in-memory,
//!   no permissions barriers, no failures, `Exact` extent, `Live` mode.

mod baseline_data_oracle;

pub use baseline_data_oracle::*;
