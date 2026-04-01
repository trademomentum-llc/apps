//! Swarm Orchestrator -- Core swarm management and C2.
//!
//! This module provides the SwarmOrchestrator that manages the lifecycle
//! of swarms, from genesis through execution to debriefing and disbandment.

use crate::rr::agents::base::*;
use crate::rr::agents::specialists::*;
use crate::rr::comms::*;
use crate::rr::hierarchy::*;
use crate::rr::memory::*;
use crate::rr::mission::*;
use crate::types::{MorphResult, MorphlexError as MorphError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique swarm identifier
pub type SwarmId = String;

/// Swarm instance representing a coordinated agent team
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Swarm {
    /// Unique swarm ID
    pub id: SwarmId,
    /// Mission being executed
    pub mission: Mission,
    /// General (swarm commander) agent ID
    pub general_id: String,
    /// All agent IDs in the swarm
    pub agent_ids: Vec<String>,
    /// Unit structure
    pub units: Vec<Unit>,
    /// Swarm status
    pub status: SwarmStatus,
    /// Communications log
    pub communications: Vec<Communication>,
    /// Timeline
    pub timeline: SwarmTimeline,
}

impl Swarm {
    /// Create a new swarm
    pub fn new(id: SwarmId, mission: Mission, general_id: String) -> Self {
        Self {
            id,
            mission,
            general_id: general_id.clone(),
            agent_ids: vec![general_id.clone()],
            units: Vec::new(),
            status: SwarmStatus::Active,
            communications: Vec::new(),
            timeline: SwarmTimeline::new(),
        }
    }

    /// Add a unit to the swarm
    pub fn add_unit(&mut self, unit: Unit) {
        self.agent_ids.extend(unit.agent_ids.clone());
        self.units.push(unit);
    }

    /// Record a communication
    pub fn record_communication(&mut self, comm: Communication) {
        self.communications.push(comm);
    }

    /// Get swarm progress
    pub fn progress(&self) -> u8 {
        self.mission.progress()
    }
}

/// Swarm status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwarmStatus {
    /// Swarm is being formed
    Forming,
    /// Swarm is active and executing
    Active,
    /// Swarm is paused
    Paused,
    /// Swarm is completing
    Completing,
    /// Swarm has completed mission
    Complete,
    /// Swarm has been disbanded
    Disbanded,
}

/// Unit within a swarm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Unit {
    /// Unit designation
    pub designation: UnitDesignation,
    /// Unit leader agent ID
    pub leader_id: String,
    /// Agent IDs in this unit
    pub agent_ids: Vec<String>,
    /// Unit type
    pub unit_type: UnitType,
}

impl Unit {
    /// Create a new unit
    pub fn new(designation: UnitDesignation, leader_id: String, agent_ids: Vec<String>) -> Self {
        let unit_type = designation.unit_type;
        Self {
            designation,
            leader_id,
            agent_ids,
            unit_type,
        }
    }
}

/// Communication record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Communication {
    /// SITREP
    SitRep(SitRep),
    /// FRAGO
    Frago(Frago),
    /// CASREP
    CasRep(CasRep),
    /// AAR
    Aar(Aar),
    /// Order
    Order(Order),
}

/// Swarm timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmTimeline {
    /// When swarm was created
    pub created_at: u64,
    /// When swarm became active
    pub activated_at: Option<u64>,
    /// When swarm completed
    pub completed_at: Option<u64>,
}

impl SwarmTimeline {
    /// Create new timeline
    pub fn new() -> Self {
        Self {
            created_at: now(),
            activated_at: None,
            completed_at: None,
        }
    }

    /// Mark as activated
    pub fn activate(&mut self) {
        self.activated_at = Some(now());
    }

    /// Mark as completed
    pub fn complete(&mut self) {
        self.completed_at = Some(now());
    }
}

impl Default for SwarmTimeline {
    fn default() -> Self {
        Self::new()
    }
}

/// Swarm Orchestrator -- manages swarm lifecycle
#[derive(Debug, Clone)]
pub struct SwarmOrchestrator {
    /// Active swarms
    pub swarms: HashMap<SwarmId, Swarm>,
    /// Agent registry
    pub agents: HashMap<String, AgentRecord>,
    /// Communication history
    pub comm_log: Vec<Communication>,
    /// Default memory capacity for agents
    pub default_memory_capacity: usize,
}

/// Agent registry record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRecord {
    /// Agent ID
    pub agent_id: String,
    /// Agent name
    pub name: String,
    /// Rank
    pub rank: Rank,
    /// MOS
    pub mos: MOS,
    /// Current swarm assignment
    pub swarm_id: Option<SwarmId>,
    /// Status
    pub status: AgentStatus,
    /// Agent type
    pub agent_type: AgentType,
}

/// Agent type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentType {
    /// SimpleAgent
    Simple,
    /// MultimodalAgent
    Multimodal,
    /// LangChainAgent
    LangChain,
    /// GuardianAgent
    Guardian,
    /// CodingAgent
    Coding,
    /// DataAnalysisAgent
    DataAnalysis,
    /// SearchReplaceAgent
    SearchReplace,
    /// DataManagementAgent
    DataManagement,
    /// DataFiltrationAgent
    DataFiltration,
}

impl AgentType {
    /// Get default rank for this agent type
    pub fn default_rank(self) -> Rank {
        match self {
            AgentType::Simple => Rank::PVT,
            AgentType::Multimodal => Rank::SPC,
            AgentType::LangChain => Rank::SGT,
            AgentType::Guardian => Rank::SGT,
            AgentType::Coding => Rank::SPC,
            AgentType::DataAnalysis => Rank::SPC,
            AgentType::SearchReplace => Rank::SPC,
            AgentType::DataManagement => Rank::SPC,
            AgentType::DataFiltration => Rank::SPC,
        }
    }

    /// Get default MOS for this agent type
    pub fn default_mos(self) -> MOS {
        match self {
            AgentType::Simple => MOS::Ops11B,
            AgentType::Multimodal => MOS::Intel35N,
            AgentType::LangChain => MOS::Sof18B,
            AgentType::Guardian => MOS::Intel35L,
            AgentType::Coding => MOS::Ops12B,
            AgentType::DataAnalysis => MOS::Intel35F,
            AgentType::SearchReplace => MOS::Ops11B,
            AgentType::DataManagement => MOS::Spt25B,
            AgentType::DataFiltration => MOS::Intel35F,
        }
    }
}

impl SwarmOrchestrator {
    /// Create a new swarm orchestrator
    pub fn new() -> Self {
        Self {
            swarms: HashMap::new(),
            agents: HashMap::new(),
            comm_log: Vec::new(),
            default_memory_capacity: 100,
        }
    }

    /// Create with custom memory capacity
    pub fn with_memory_capacity(capacity: usize) -> Self {
        Self {
            swarms: HashMap::new(),
            agents: HashMap::new(),
            comm_log: Vec::new(),
            default_memory_capacity: capacity,
        }
    }

    /// Spawn a new swarm for a mission
    pub fn spawn_swarm(&mut self, mission: Mission) -> MorphResult<&Swarm> {
        // Create General agent as swarm commander
        let general_id = format!("gen_{}_{}", mission.id, now());
        let general = SimpleAgent::new(general_id.clone(), format!("General {}", mission.id));

        // Register general
        self.register_agent(AgentRecord {
            agent_id: general_id.clone(),
            name: general.base.name.clone(),
            rank: Rank::GEN,
            mos: MOS::Ops11B,
            swarm_id: None,
            status: AgentStatus::Standby,
            agent_type: AgentType::Simple,
        });

        // Create swarm
        let swarm_id = format!("swarm_{}_{}", mission.id, now());
        let mut swarm = Swarm::new(swarm_id.clone(), mission, general_id.clone());
        swarm.status = SwarmStatus::Forming;

        // Store swarm
        self.swarms.insert(swarm_id.clone(), swarm);

        // Build initial unit structure based on mission requirements
        self.build_swarm_structure(&swarm_id)?;

        // Activate swarm
        if let Some(swarm) = self.swarms.get_mut(&swarm_id) {
            swarm.timeline.activate();
            swarm.status = SwarmStatus::Active;
        }

        Ok(self.swarms.get(&swarm_id).unwrap())
    }

    /// Build unit structure for a swarm
    fn build_swarm_structure(&mut self, swarm_id: &str) -> MorphResult<()> {
        // Get general_id first to avoid borrow conflicts
        let general_id = if let Some(swarm) = self.swarms.get(swarm_id) {
            swarm.general_id.clone()
        } else {
            return Err(crate::MorphlexError::DatabaseError(
                "Swarm not found".to_string(),
            ));
        };

        // For now, create a simple fire team structure
        // In production, this would analyze mission requirements
        let unit_designation = UnitDesignation::new(UnitType::FireTeam, "Alpha".to_string());

        // Create specialist agents for the fire team
        let coding_agent_id = format!("cod_{}_{}", swarm_id, now());
        let analysis_agent_id = format!("ana_{}_{}", swarm_id, now());

        self.register_agent(AgentRecord {
            agent_id: coding_agent_id.clone(),
            name: "Coding Specialist".to_string(),
            rank: Rank::SPC,
            mos: MOS::Ops12B,
            swarm_id: Some(swarm_id.to_string()),
            status: AgentStatus::Standby,
            agent_type: AgentType::Coding,
        });

        self.register_agent(AgentRecord {
            agent_id: analysis_agent_id.clone(),
            name: "Analysis Specialist".to_string(),
            rank: Rank::SPC,
            mos: MOS::Intel35F,
            swarm_id: Some(swarm_id.to_string()),
            status: AgentStatus::Standby,
            agent_type: AgentType::DataAnalysis,
        });

        // Create unit
        let unit = Unit::new(
            unit_designation,
            general_id,
            vec![coding_agent_id.clone(), analysis_agent_id.clone()],
        );

        if let Some(swarm) = self.swarms.get_mut(swarm_id) {
            swarm.add_unit(unit);
        }

        Ok(())
    }

    /// Register an agent
    pub fn register_agent(&mut self, record: AgentRecord) {
        self.agents.insert(record.agent_id.clone(), record);
    }

    /// Get swarm status
    pub fn get_swarm_status(&self, swarm_id: &str) -> Option<&Swarm> {
        self.swarms.get(swarm_id)
    }

    /// Get mutable swarm reference
    pub fn get_swarm_mut(&mut self, swarm_id: &str) -> Option<&mut Swarm> {
        self.swarms.get_mut(swarm_id)
    }

    /// Issue FRAGO to a swarm
    pub fn issue_frago(&mut self, swarm_id: &str, frago: Frago) -> MorphResult<()> {
        let swarm = self
            .swarms
            .get_mut(swarm_id)
            .ok_or_else(|| crate::MorphlexError::DatabaseError("Swarm not found".to_string()))?;

        swarm.record_communication(Communication::Frago(frago));
        Ok(())
    }

    /// Receive SITREP from an agent
    pub fn receive_sitrep(&mut self, sitrep: SitRep) -> MorphResult<()> {
        // Find the swarm this agent belongs to
        let agent_record = self
            .agents
            .get(&sitrep.header.from)
            .ok_or_else(|| crate::MorphlexError::DatabaseError("Agent not found".to_string()))?;

        if let Some(swarm_id) = &agent_record.swarm_id {
            if let Some(swarm) = self.swarms.get_mut(swarm_id) {
                swarm.record_communication(Communication::SitRep(sitrep.clone()));
            }
        }

        self.comm_log.push(Communication::SitRep(sitrep));
        Ok(())
    }

    /// Receive CASREP from an agent
    pub fn receive_casrep(&mut self, casrep: CasRep) -> MorphResult<()> {
        self.comm_log.push(Communication::CasRep(casrep));
        Ok(())
    }

    /// Disband a swarm and generate AAR
    pub fn disband_swarm(&mut self, swarm_id: &str) -> MorphResult<Aar> {
        let swarm = self
            .swarms
            .get(swarm_id)
            .ok_or_else(|| crate::MorphlexError::DatabaseError("Swarm not found".to_string()))?
            .clone();

        // Generate AAR
        let aar = Aar::new(
            swarm.general_id.clone(),
            "orchestrator".to_string(),
            &swarm.mission,
        )
        .with_outcome(if swarm.mission.status == MissionStatus::Complete {
            Outcome::Success
        } else {
            Outcome::PartialSuccess
        })
        .with_comparison(
            swarm.mission.objective.clone(),
            format!("Mission completed: {:?}", swarm.mission.status),
        );

        // Update swarm status
        if let Some(swarm) = self.swarms.get_mut(swarm_id) {
            swarm.timeline.complete();
            swarm.status = SwarmStatus::Disbanded;
            swarm.record_communication(Communication::Aar(aar.clone()));
        }

        // Free agents
        for agent_id in &swarm.agent_ids {
            if let Some(record) = self.agents.get_mut(agent_id) {
                record.swarm_id = None;
                record.status = AgentStatus::Standby;
            }
        }

        Ok(aar)
    }

    /// Get all active swarms
    pub fn get_active_swarms(&self) -> Vec<&Swarm> {
        self.swarms
            .values()
            .filter(|s| s.status == SwarmStatus::Active)
            .collect()
    }

    /// Get available agents (not assigned to any swarm)
    pub fn get_available_agents(&self) -> Vec<&AgentRecord> {
        self.agents
            .values()
            .filter(|a| a.swarm_id.is_none() && a.status == AgentStatus::Standby)
            .collect()
    }

    /// Get agents by MOS
    pub fn get_agents_by_mos(&self, mos: MOS) -> Vec<&AgentRecord> {
        self.agents.values().filter(|a| a.mos == mos).collect()
    }

    /// Create a specific agent type
    pub fn create_agent(&mut self, agent_type: AgentType, name: Option<String>) -> String {
        let agent_id = format!("agt_{}_{}", agent_type.as_str(), now());
        let name = name.unwrap_or_else(|| format!("{:?} Agent", agent_type));

        self.register_agent(AgentRecord {
            agent_id: agent_id.clone(),
            name: name.clone(),
            rank: agent_type.default_rank(),
            mos: agent_type.default_mos(),
            swarm_id: None,
            status: AgentStatus::Standby,
            agent_type,
        });

        agent_id
    }
}

impl Default for SwarmOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentType {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentType::Simple => "simple",
            AgentType::Multimodal => "multimodal",
            AgentType::LangChain => "langchain",
            AgentType::Guardian => "guardian",
            AgentType::Coding => "coding",
            AgentType::DataAnalysis => "analysis",
            AgentType::SearchReplace => "search",
            AgentType::DataManagement => "datamgmt",
            AgentType::DataFiltration => "filtration",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swarm_orchestrator_creation() {
        let orchestrator = SwarmOrchestrator::new();
        assert!(orchestrator.swarms.is_empty());
        assert!(orchestrator.agents.is_empty());
    }

    #[test]
    fn test_spawn_swarm() {
        let mut orchestrator = SwarmOrchestrator::new();
        let mission = Mission::new("Test mission", Priority::Routine);

        let swarm = orchestrator.spawn_swarm(mission).unwrap();
        assert_eq!(swarm.status, SwarmStatus::Active);
        assert!(!swarm.agent_ids.is_empty());
    }

    #[test]
    fn test_create_agent() {
        let mut orchestrator = SwarmOrchestrator::new();
        let agent_id = orchestrator.create_agent(AgentType::Coding, Some("Code Bot".to_string()));

        let agent = orchestrator.agents.get(&agent_id).unwrap();
        assert_eq!(agent.name, "Code Bot");
        assert_eq!(agent.agent_type, AgentType::Coding);
    }

    #[test]
    fn test_disband_swarm() {
        let mut orchestrator = SwarmOrchestrator::new();
        let mission = Mission::new("Test mission", Priority::Routine);
        let mission_id = mission.id.clone();
        let swarm = orchestrator.spawn_swarm(mission).unwrap();
        let swarm_id = swarm.id.clone();

        let aar = orchestrator.disband_swarm(&swarm_id).unwrap();
        // AAR mission_id is the original mission ID, not the swarm ID
        assert_eq!(aar.mission_id, mission_id);

        let swarm = orchestrator.get_swarm_status(&swarm_id).unwrap();
        assert_eq!(swarm.status, SwarmStatus::Disbanded);
    }
}
