//! Rational Reserve (RR) -- swaRRm multi-agent system.
//!
//! A military-inspired swarm intelligence system where AI agents operate within
//! a hierarchical command structure to accomplish complex tasks through coordinated action.
//!
//! # Architecture
//!
//! - **Four-tier Memory**: Short-term, Long-term, Episodic, Semantic
//! - **Agent Taxonomy**: 9 specialized agent types + 4 system daemons
//! - **Military Hierarchy**: Ranks (GEN to PVT) and MOS specialties
//! - **Swarm Orchestration**: Command & Control, task delegation, SITREP reporting
//! - **System Daemons**: Continuous background operations
//!
//! # Example
//!
//! ```rust,ignore
//! use morphlex::rr::{SwarmOrchestrator, Mission, Priority};
//!
//! let orchestrator = SwarmOrchestrator::new();
//! let mission = Mission::new("Build REST API", Priority::Normal);
//! let swarm = orchestrator.spawn_swarm(mission).await?;
//! ```

pub mod agents;
pub mod comms;
pub mod daemons;
pub mod database;
pub mod hierarchy;
pub mod memory;
pub mod mission;
pub mod orchestrator;

pub use agents::*;
pub use comms::*;
pub use daemons::*;
pub use database::*;
pub use hierarchy::*;
pub use memory::*;
pub use mission::*;
pub use orchestrator::*;
