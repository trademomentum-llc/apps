//! Base agent types and traits for Rational Reserve.
//!
//! This module defines the core agent abstraction that all specialized agents inherit from.
//! Each agent has a military rank, MOS (Military Occupational Specialty), and operates
//! within the chain of command.

use crate::rr::comms::*;
use crate::rr::hierarchy::*;
use crate::rr::memory::*;
use crate::rr::mission::*;
use crate::types::MorphResult;
use serde::{Deserialize, Serialize};

/// Unique agent identifier
pub type AgentId = String;

/// Agent status in the lifecycle
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    /// Agent is available for assignment
    Standby,
    /// Agent has received orders and is processing
    Active,
    /// Agent is fully engaged in a task
    Engaged,
    /// Agent has completed its mission
    Complete,
    /// Agent is unavailable (error, offline, etc.)
    Unavailable,
}

/// Base trait for all Rational Reserve agents.
///
/// This trait defines the interface that all agents must implement,
/// regardless of their specialization or rank.
pub trait RRAgentTrait: Send + Sync {
    /// Get the agent's unique identifier
    fn id(&self) -> &AgentId;

    /// Get the agent's military rank
    fn rank(&self) -> crate::rr::hierarchy::Rank;

    /// Get the agent's MOS (Military Occupational Specialty)
    fn mos(&self) -> &crate::rr::hierarchy::MOS;

    /// Get the agent's current status
    fn status(&self) -> &AgentStatus;

    /// Set the agent's status
    fn set_status(&mut self, status: AgentStatus);

    /// Get the ID of the commanding officer (if any)
    fn commander(&self) -> Option<&AgentId>;

    /// Set the commanding officer
    fn set_commander(&mut self, commander_id: Option<AgentId>);

    /// Get direct subordinate agent IDs
    fn subordinates(&self) -> &[AgentId];

    /// Add a subordinate
    fn add_subordinate(&mut self, subordinate_id: AgentId);

    /// Remove a subordinate
    fn remove_subordinate(&mut self, subordinate_id: &str);

    /// Receive an order from a superior officer
    fn receive_order(&mut self, order: &crate::rr::comms::Order) -> MorphResult<()>;

    /// Execute the current mission/task
    fn execute_mission(
        &mut self,
        mission: &crate::rr::mission::Mission,
    ) -> MorphResult<crate::rr::mission::Result>;

    /// Report status (SITREP)
    fn report_sitrep(&self) -> crate::rr::comms::SitRep;

    /// Delegate a task to a subordinate
    fn delegate_task(
        &self,
        task: &crate::rr::mission::Task,
        subordinate_id: &str,
    ) -> MorphResult<crate::rr::comms::Delegation>;

    /// Get the agent's memory system
    fn memory(&self) -> &MemorySystem;

    /// Get mutable access to memory
    fn memory_mut(&mut self) -> &mut MemorySystem;

    /// Get the agent's specialized capabilities
    fn capabilities(&self) -> &AgentCapabilities;
}

/// Agent capabilities and specializations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentCapabilities {
    /// Can process text input
    pub text_processing: bool,
    /// Can process images
    pub image_processing: bool,
    /// Can process audio
    pub audio_processing: bool,
    /// Can execute code
    pub code_execution: bool,
    /// Can perform data analysis
    pub data_analysis: bool,
    /// Can perform search and replace operations
    pub search_replace: bool,
    /// Can perform CRUD operations
    pub crud_operations: bool,
    /// Can filter and validate data
    pub data_filtration: bool,
    /// Can monitor for safety/ethics
    pub safety_monitoring: bool,
    /// Can use external tools (ReAct pattern)
    pub tool_integration: bool,
}

impl AgentCapabilities {
    /// Create capabilities for a simple conversational agent
    pub fn simple() -> Self {
        Self {
            text_processing: true,
            ..Default::default()
        }
    }

    /// Create capabilities for a multimodal agent
    pub fn multimodal() -> Self {
        Self {
            text_processing: true,
            image_processing: true,
            audio_processing: true,
            ..Default::default()
        }
    }

    /// Create capabilities for a coding agent
    pub fn coding() -> Self {
        Self {
            text_processing: true,
            code_execution: true,
            data_analysis: true,
            ..Default::default()
        }
    }

    /// Create capabilities for a data analysis agent
    pub fn data_analysis() -> Self {
        Self {
            text_processing: true,
            data_analysis: true,
            crud_operations: true,
            ..Default::default()
        }
    }

    /// Create capabilities for a search/replace agent
    pub fn search_replace() -> Self {
        Self {
            text_processing: true,
            search_replace: true,
            ..Default::default()
        }
    }

    /// Create capabilities for a data management agent
    pub fn data_management() -> Self {
        Self {
            text_processing: true,
            crud_operations: true,
            data_filtration: true,
            ..Default::default()
        }
    }

    /// Create capabilities for a data filtration agent
    pub fn data_filtration() -> Self {
        Self {
            text_processing: true,
            data_filtration: true,
            data_analysis: true,
            ..Default::default()
        }
    }

    /// Create capabilities for a guardian agent
    pub fn guardian() -> Self {
        Self {
            text_processing: true,
            safety_monitoring: true,
            data_analysis: true,
            ..Default::default()
        }
    }

    /// Create capabilities for a LangChain-style agent
    pub fn langchain() -> Self {
        Self {
            text_processing: true,
            tool_integration: true,
            code_execution: true,
            data_analysis: true,
            ..Default::default()
        }
    }
}

/// Base implementation struct that specialized agents can embed.
///
/// This provides default implementations for common agent functionality.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RRAgentBase {
    /// Unique agent identifier
    pub id: AgentId,
    /// Military rank
    pub rank: Rank,
    /// Military Occupational Specialty
    pub mos: MOS,
    /// Current operational status
    pub status: AgentStatus,
    /// Commanding officer's agent ID
    pub commander: Option<AgentId>,
    /// Direct subordinate agent IDs
    pub subordinates: Vec<AgentId>,
    /// Agent's memory system
    pub memory: MemorySystem,
    /// Agent's capabilities
    pub capabilities: AgentCapabilities,
    /// Current mission (if any)
    pub current_mission: Option<Mission>,
    /// Agent's display name
    pub name: String,
    /// Agent's unit designation
    pub unit: Option<String>,
}

impl RRAgentBase {
    /// Create a new agent base
    pub fn new(
        id: AgentId,
        rank: Rank,
        mos: MOS,
        name: String,
        capabilities: AgentCapabilities,
    ) -> Self {
        Self {
            id,
            rank,
            mos,
            status: AgentStatus::Standby,
            commander: None,
            subordinates: Vec::new(),
            memory: MemorySystem::new(100),
            capabilities,
            current_mission: None,
            name,
            unit: None,
        }
    }

    /// Create with custom short-term memory capacity
    pub fn with_memory_capacity(
        id: AgentId,
        rank: Rank,
        mos: MOS,
        name: String,
        capabilities: AgentCapabilities,
        memory_capacity: usize,
    ) -> Self {
        Self {
            id,
            rank,
            mos,
            status: AgentStatus::Standby,
            commander: None,
            subordinates: Vec::new(),
            memory: MemorySystem::new(memory_capacity),
            capabilities,
            current_mission: None,
            name,
            unit: None,
        }
    }

    /// Default implementation for receiving an order
    pub fn base_receive_order(&mut self, order: &crate::rr::comms::Order) -> MorphResult<()> {
        // Validate chain of command (simplified - in production would verify signature)
        if let Some(cmdr) = &self.commander
            && &order.header.from != cmdr {
                return Err(crate::MorphlexError::DatabaseError(
                    "Order received from non-commanding officer".to_string(),
                ));
            }

        // Store order in short-term memory
        self.memory.add_short_term(
            format!("Order received: {}", order.content),
            Some(MemoryMetadata {
                source: Some(order.header.from.clone()),
                importance: order.header.priority as u8,
                tags: vec![order.header.mission_id.clone().unwrap_or_default()],
            }),
        );

        // Update status
        self.status = AgentStatus::Active;

        Ok(())
    }

    /// Default SITREP generation
    pub fn base_report_sitrep(&self) -> crate::rr::comms::SitRep {
        crate::rr::comms::SitRep {
            header: crate::rr::comms::CommunicationHeader::new(
                crate::rr::comms::CommType::SitRep,
                self.id.clone(),
                self.commander
                    .clone()
                    .unwrap_or_else(|| "commander".to_string()),
                crate::rr::mission::Priority::Routine,
            ),
            status: self.status.clone(),
            mission_id: self.current_mission.as_ref().map(|m| m.id.clone()),
            content: format!(
                "Agent {} ({}) - Status: {:?}",
                self.name, self.rank, self.status
            ),
            progress: match &self.status {
                AgentStatus::Standby => 0,
                AgentStatus::Active => 25,
                AgentStatus::Engaged => 50,
                AgentStatus::Complete => 100,
                AgentStatus::Unavailable => 0,
            },
            blockers: vec![],
            subordinates_status: self
                .subordinates
                .iter()
                .map(|id| SubordinateStatus {
                    agent_id: id.clone(),
                    status: AgentStatus::Standby, // Would need to query actual status
                    progress: 0,
                })
                .collect(),
            resource_usage: None,
        }
    }

    /// Default task delegation
    pub fn base_delegate_task(
        &self,
        task: &crate::rr::mission::Task,
        subordinate_id: &str,
    ) -> MorphResult<crate::rr::comms::Delegation> {
        // Verify subordinate exists
        if !self.subordinates.iter().any(|id| id == subordinate_id) {
            return Err(crate::MorphlexError::DatabaseError(format!(
                "Agent {} is not a subordinate",
                subordinate_id
            )));
        }

        Ok(crate::rr::comms::Delegation {
            task_id: task.id.clone(),
            from: self.id.clone(),
            to: subordinate_id.to_string(),
            timestamp: now(),
            task: task.clone(),
        })
    }
}

// Blanket implementation of RRAgentTrait for RRAgentBase
impl RRAgentTrait for RRAgentBase {
    fn id(&self) -> &AgentId {
        &self.id
    }

    fn rank(&self) -> crate::rr::hierarchy::Rank {
        self.rank
    }

    fn mos(&self) -> &crate::rr::hierarchy::MOS {
        &self.mos
    }

    fn status(&self) -> &AgentStatus {
        &self.status
    }

    fn set_status(&mut self, status: AgentStatus) {
        self.status = status;
    }

    fn commander(&self) -> Option<&AgentId> {
        self.commander.as_ref()
    }

    fn set_commander(&mut self, commander_id: Option<AgentId>) {
        self.commander = commander_id;
    }

    fn subordinates(&self) -> &[AgentId] {
        &self.subordinates
    }

    fn add_subordinate(&mut self, subordinate_id: AgentId) {
        self.subordinates.push(subordinate_id);
    }

    fn remove_subordinate(&mut self, subordinate_id: &str) {
        self.subordinates.retain(|id| id != subordinate_id);
    }

    fn receive_order(&mut self, order: &crate::rr::comms::Order) -> MorphResult<()> {
        self.base_receive_order(order)
    }

    fn execute_mission(
        &mut self,
        mission: &crate::rr::mission::Mission,
    ) -> MorphResult<crate::rr::mission::Result> {
        // Default implementation - specialized agents override this
        self.current_mission = Some(mission.clone());
        self.status = AgentStatus::Engaged;

        // Record episode
        self.memory.record_episode(Episode {
            id: format!("ep_{}_{}", self.id, mission.id),
            mission_id: mission.id.clone(),
            timestamp: now(),
            title: format!("Mission: {}", mission.objective),
            content: mission.description.clone(),
            participants: vec![self.id.clone()],
            outcome: None,
            lessons: vec![],
            tags: vec![],
        });

        Ok(crate::rr::mission::Result::Pending)
    }

    fn report_sitrep(&self) -> crate::rr::comms::SitRep {
        self.base_report_sitrep()
    }

    fn delegate_task(
        &self,
        task: &crate::rr::mission::Task,
        subordinate_id: &str,
    ) -> MorphResult<crate::rr::comms::Delegation> {
        self.base_delegate_task(task, subordinate_id)
    }

    fn memory(&self) -> &MemorySystem {
        &self.memory
    }

    fn memory_mut(&mut self) -> &mut MemorySystem {
        &mut self.memory
    }

    fn capabilities(&self) -> &AgentCapabilities {
        &self.capabilities
    }
}

/// Macro to implement RRAgentTrait for specialized agents that embed RRAgentBase
#[macro_export]
macro_rules! impl_rr_agent_trait {
    ($struct_name:ident) => {
        impl RRAgentTrait for $struct_name {
            fn id(&self) -> &AgentId {
                &self.base.id
            }

            fn rank(&self) -> $crate::rr::hierarchy::Rank {
                self.base.rank
            }

            fn mos(&self) -> &$crate::rr::hierarchy::MOS {
                &self.base.mos
            }

            fn status(&self) -> &$crate::rr::agents::base::AgentStatus {
                &self.base.status
            }

            fn set_status(&mut self, status: $crate::rr::agents::base::AgentStatus) {
                self.base.status = status;
            }

            fn commander(&self) -> Option<&AgentId> {
                self.base.commander.as_ref()
            }

            fn set_commander(&mut self, commander_id: Option<AgentId>) {
                self.base.commander = commander_id;
            }

            fn subordinates(&self) -> &[AgentId] {
                &self.base.subordinates
            }

            fn add_subordinate(&mut self, subordinate_id: AgentId) {
                self.base.subordinates.push(subordinate_id);
            }

            fn remove_subordinate(&mut self, subordinate_id: &str) {
                self.base.subordinates.retain(|id| id != subordinate_id);
            }

            fn receive_order(
                &mut self,
                order: &$crate::rr::comms::Order,
            ) -> $crate::types::MorphResult<()> {
                self.base.base_receive_order(order)
            }

            fn execute_mission(
                &mut self,
                mission: &$crate::rr::mission::Mission,
            ) -> $crate::types::MorphResult<$crate::rr::mission::Result> {
                self.base_execute_mission(mission)
            }

            fn report_sitrep(&self) -> $crate::rr::comms::SitRep {
                self.base.base_report_sitrep()
            }

            fn delegate_task(
                &self,
                task: &$crate::rr::mission::Task,
                subordinate_id: &str,
            ) -> $crate::types::MorphResult<$crate::rr::comms::Delegation> {
                self.base.base_delegate_task(task, subordinate_id)
            }

            fn memory(&self) -> &$crate::rr::memory::MemorySystem {
                &self.base.memory
            }

            fn memory_mut(&mut self) -> &mut $crate::rr::memory::MemorySystem {
                &mut self.base.memory
            }

            fn capabilities(&self) -> &$crate::rr::agents::base::AgentCapabilities {
                &self.base.capabilities
            }
        }
    };
}

pub use impl_rr_agent_trait;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_base_creation() {
        let agent = RRAgentBase::new(
            "agent_001".to_string(),
            Rank::SPC,
            MOS::Ops11B,
            "Private Alpha".to_string(),
            AgentCapabilities::simple(),
        );

        assert_eq!(agent.id(), "agent_001");
        assert_eq!(agent.rank(), Rank::SPC);
        assert_eq!(agent.status(), &AgentStatus::Standby);
    }

    #[test]
    fn test_agent_subordinate_management() {
        let mut agent = RRAgentBase::new(
            "commander".to_string(),
            Rank::SGT,
            MOS::Ops11B,
            "Sergeant".to_string(),
            AgentCapabilities::simple(),
        );

        agent.add_subordinate("sub1".to_string());
        agent.add_subordinate("sub2".to_string());

        assert_eq!(agent.subordinates().len(), 2);

        agent.remove_subordinate("sub1");
        assert_eq!(agent.subordinates().len(), 1);
    }

    #[test]
    fn test_agent_status_transitions() {
        let mut agent = RRAgentBase::new(
            "agent".to_string(),
            Rank::SPC,
            MOS::Ops11B,
            "Agent".to_string(),
            AgentCapabilities::simple(),
        );

        assert_eq!(agent.status(), &AgentStatus::Standby);

        agent.set_status(AgentStatus::Active);
        assert_eq!(agent.status(), &AgentStatus::Active);

        agent.set_status(AgentStatus::Engaged);
        assert_eq!(agent.status(), &AgentStatus::Engaged);

        agent.set_status(AgentStatus::Complete);
        assert_eq!(agent.status(), &AgentStatus::Complete);
    }
}
