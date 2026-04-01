//! Rational Reserve Agents -- specialized AI agents with military hierarchy.
//!
//! This module provides the agent taxonomy for the swaRRm system:
//! - Base agent trait and types
//! - Nine specialized agent implementations
//! - Rank and MOS (Military Occupational Specialty) assignments

pub mod base;
pub mod specialists;

pub use base::*;
pub use specialists::*;
