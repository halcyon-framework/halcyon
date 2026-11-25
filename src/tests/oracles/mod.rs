//! Test oracles re-exports.
//!
//! Oracles have been relocated to `crate::oracles`. This module re-exports
//! them for backward compatibility with existing test imports.

#![cfg(test)]

pub use crate::oracles::*;
