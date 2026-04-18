//! RR Database Schema -- Persistence for swarm data.
//!
//! This module defines the database schema and operations for
//! Rational Reserve persistent storage.

use crate::rr::agents::base::AgentStatus;
use crate::rr::comms::*;
use crate::rr::hierarchy::*;
use crate::rr::memory::now;
use crate::rr::mission::*;
use crate::rr::orchestrator::*;
use crate::types::{MorphResult, MorphlexError as MorphError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// RR Database containing all swarm data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RRDatabase {
    /// Database version
    pub version: u32,
    /// Swarm records
    pub swarms: HashMap<String, SwarmRecord>,
    /// Agent records
    pub agents: HashMap<String, AgentDBRecord>,
    /// Unit records
    pub units: HashMap<String, UnitDBRecord>,
    /// Mission records
    pub missions: HashMap<String, MissionDBRecord>,
    /// Task records
    pub tasks: HashMap<String, TaskDBRecord>,
    /// Communication records
    pub communications: HashMap<String, CommunicationDBRecord>,
    /// AAR archive
    pub aar_archive: Vec<AarDBRecord>,
}

impl RRDatabase {
    /// Create a new empty database
    pub fn new() -> Self {
        Self {
            version: 1,
            swarms: HashMap::new(),
            agents: HashMap::new(),
            units: HashMap::new(),
            missions: HashMap::new(),
            tasks: HashMap::new(),
            communications: HashMap::new(),
            aar_archive: Vec::new(),
        }
    }

    /// Load database from JSON file
    pub fn load_from_path(path: &Path) -> MorphResult<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }

        let json = std::fs::read_to_string(path).map_err(|e| crate::MorphlexError::IoError(e))?;

        serde_json::from_str(&json).map_err(|e| {
            crate::MorphlexError::DatabaseError(format!("Failed to parse database: {}", e))
        })
    }

    /// Save database to JSON file
    pub fn save_to_path(&self, path: &Path) -> MorphResult<()> {
        let json = serde_json::to_string_pretty(self).map_err(|e| {
            crate::MorphlexError::DatabaseError(format!("Failed to serialize database: {}", e))
        })?;

        std::fs::write(path, json).map_err(|e| crate::MorphlexError::IoError(e))?;

        Ok(())
    }

    /// Record a swarm
    pub fn record_swarm(&mut self, swarm: &SwarmRecord) {
        self.swarms.insert(swarm.id.clone(), swarm.clone());
    }

    /// Record an agent
    pub fn record_agent(&mut self, agent: &AgentDBRecord) {
        self.agents.insert(agent.agent_id.clone(), agent.clone());
    }

    /// Record a mission
    pub fn record_mission(&mut self, mission: &MissionDBRecord) {
        self.missions.insert(mission.id.clone(), mission.clone());
    }

    /// Record a task
    pub fn record_task(&mut self, task: &TaskDBRecord) {
        self.tasks.insert(task.id.clone(), task.clone());
    }

    /// Record a communication
    pub fn record_communication(&mut self, comm: &CommunicationDBRecord) {
        self.communications.insert(comm.id.clone(), comm.clone());
    }

    /// Archive an AAR
    pub fn archive_aar(&mut self, aar: &AarDBRecord) {
        self.aar_archive.push(aar.clone());
    }

    /// Get swarm by ID
    pub fn get_swarm(&self, swarm_id: &str) -> Option<&SwarmRecord> {
        self.swarms.get(swarm_id)
    }

    /// Get agent by ID
    pub fn get_agent(&self, agent_id: &str) -> Option<&AgentDBRecord> {
        self.agents.get(agent_id)
    }

    /// Get mission by ID
    pub fn get_mission(&self, mission_id: &str) -> Option<&MissionDBRecord> {
        self.missions.get(mission_id)
    }

    /// Get tasks by mission ID
    pub fn get_tasks_by_mission(&self, mission_id: &str) -> Vec<&TaskDBRecord> {
        self.tasks
            .values()
            .filter(|t| t.mission_id == mission_id)
            .collect()
    }

    /// Get communications by swarm ID
    pub fn get_comms_by_swarm(&self, swarm_id: &str) -> Vec<&CommunicationDBRecord> {
        self.communications
            .values()
            .filter(|c| c.swarm_id.as_deref() == Some(swarm_id))
            .collect()
    }

    /// Get AARs by mission ID
    pub fn get_aars_by_mission(&self, mission_id: &str) -> Vec<&AarDBRecord> {
        self.aar_archive
            .iter()
            .filter(|a| a.mission_id == mission_id)
            .collect()
    }

    /// Get statistics
    pub fn get_stats(&self) -> DatabaseStats {
        DatabaseStats {
            total_swarms: self.swarms.len(),
            total_agents: self.agents.len(),
            total_missions: self.missions.len(),
            total_tasks: self.tasks.len(),
            total_communications: self.communications.len(),
            total_aars: self.aar_archive.len(),
            active_swarms: self
                .swarms
                .values()
                .filter(|s| s.status == SwarmStatus::Active)
                .count(),
            completed_missions: self
                .missions
                .values()
                .filter(|m| m.status == MissionStatus::Complete)
                .count(),
        }
    }

    /// Compact database (remove old completed swarms)
    pub fn compact(&mut self, keep_recent: usize) -> MorphResult<usize> {
        let mut removed = 0;

        // Find completed swarms older than keep_recent
        let completed_swarm_ids: Vec<_> = self
            .swarms
            .iter()
            .filter(|(_, s)| {
                s.status == SwarmStatus::Complete || s.status == SwarmStatus::Disbanded
            })
            .map(|(id, _)| id.clone())
            .collect();

        // Remove old ones (keep only keep_recent)
        for swarm_id in completed_swarm_ids.iter().skip(keep_recent) {
            self.swarms.remove(swarm_id);
            removed += 1;
        }

        Ok(removed)
    }
}

impl Default for RRDatabase {
    fn default() -> Self {
        Self::new()
    }
}

/// Swarm record for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmRecord {
    /// Swarm ID
    pub id: String,
    /// Mission ID
    pub mission_id: String,
    /// General agent ID
    pub general_id: String,
    /// Agent IDs
    pub agent_ids: Vec<String>,
    /// Unit IDs
    pub unit_ids: Vec<String>,
    /// Status
    pub status: SwarmStatus,
    /// Created timestamp
    pub created_at: u64,
    /// Completed timestamp
    pub completed_at: Option<u64>,
    /// Progress (0-100)
    pub progress: u8,
}

impl From<&Swarm> for SwarmRecord {
    fn from(swarm: &Swarm) -> Self {
        Self {
            id: swarm.id.clone(),
            mission_id: swarm.mission.id.clone(),
            general_id: swarm.general_id.clone(),
            agent_ids: swarm.agent_ids.clone(),
            unit_ids: swarm
                .units
                .iter()
                .map(|u| u.designation.name.clone())
                .collect(),
            status: swarm.status,
            created_at: swarm.timeline.created_at,
            completed_at: swarm.timeline.completed_at,
            progress: swarm.progress(),
        }
    }
}

/// Agent record for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDBRecord {
    /// Agent ID
    pub agent_id: String,
    /// Agent name
    pub name: String,
    /// Rank
    pub rank: Rank,
    /// MOS
    pub mos: MOS,
    /// Unit assignment
    pub unit: Option<String>,
    /// Commander ID
    pub commander: Option<String>,
    /// Current swarm ID
    pub swarm_id: Option<String>,
    /// Status
    pub status: AgentStatus,
    /// Agent type
    pub agent_type: AgentType,
    /// Created timestamp
    pub created_at: u64,
    /// Missions completed
    pub missions_completed: u64,
    /// Performance score (0.0 to 1.0)
    pub performance_score: f64,
}

/// Unit record for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitDBRecord {
    /// Unit ID (designation name)
    pub id: String,
    /// Swarm ID
    pub swarm_id: String,
    /// Unit type
    pub unit_type: UnitType,
    /// Leader agent ID
    pub leader_id: String,
    /// Agent IDs
    pub agent_ids: Vec<String>,
    /// Created timestamp
    pub created_at: u64,
}

/// Mission record for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionDBRecord {
    /// Mission ID
    pub id: String,
    /// Objective
    pub objective: String,
    /// Description
    pub description: String,
    /// Priority
    pub priority: Priority,
    /// Status
    pub status: MissionStatus,
    /// Swarm ID
    pub swarm_id: Option<String>,
    /// Created timestamp
    pub created_at: u64,
    /// Started timestamp
    pub started_at: Option<u64>,
    /// Completed timestamp
    pub completed_at: Option<u64>,
    /// Progress (0-100)
    pub progress: u8,
    /// Task count
    pub task_count: usize,
}

impl From<&Mission> for MissionDBRecord {
    fn from(mission: &Mission) -> Self {
        Self {
            id: mission.id.clone(),
            objective: mission.objective.clone(),
            description: mission.description.clone(),
            priority: mission.priority,
            status: mission.status,
            swarm_id: None,
            created_at: mission.timeline.created_at,
            started_at: mission.timeline.started_at,
            completed_at: mission.timeline.completed_at,
            progress: mission.progress(),
            task_count: mission.tasks.len(),
        }
    }
}

/// Task record for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDBRecord {
    /// Task ID
    pub id: String,
    /// Mission ID
    pub mission_id: String,
    /// Description
    pub description: String,
    /// Task type
    pub task_type: TaskType,
    /// Required MOS
    pub required_mos: Option<MOS>,
    /// Assigned agent ID
    pub assigned_to: Option<String>,
    /// Status
    pub status: TaskStatus,
    /// Parent task ID
    pub parent_task: Option<String>,
    /// Dependencies
    pub dependencies: Vec<String>,
    /// Result content
    pub result: Option<String>,
    /// Error message
    pub error: Option<String>,
    /// Created timestamp
    pub created_at: u64,
    /// Completed timestamp
    pub completed_at: Option<u64>,
}

impl From<&Task> for TaskDBRecord {
    fn from(task: &Task) -> Self {
        Self {
            id: task.id.clone(),
            mission_id: String::new(), // Would need to be set by caller
            description: task.description.clone(),
            task_type: task.task_type,
            required_mos: task.required_mos,
            assigned_to: task.assigned_to.clone(),
            status: task.status,
            parent_task: task.parent_task.clone(),
            dependencies: task.dependencies.clone(),
            result: task.result.as_ref().map(|r| r.content.clone()),
            error: task.error.clone(),
            created_at: now(),
            completed_at: None,
        }
    }
}

/// Communication record for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationDBRecord {
    /// Communication ID
    pub id: String,
    /// Swarm ID
    pub swarm_id: Option<String>,
    /// From agent ID
    pub from: String,
    /// To agent ID
    pub to: String,
    /// Communication type
    pub comm_type: CommType,
    /// Content summary
    pub content_summary: String,
    /// Timestamp
    pub timestamp: u64,
    /// Priority
    pub priority: Priority,
    /// Full serialized content (JSON)
    pub full_content: String,
}

/// AAR record for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AarDBRecord {
    /// AAR ID
    pub id: String,
    /// Mission ID
    pub mission_id: String,
    /// Swarm ID
    pub swarm_id: Option<String>,
    /// Objective
    pub objective: String,
    /// Outcome
    pub outcome: Outcome,
    /// Successes
    pub successes: Vec<String>,
    /// Areas for improvement
    pub improvements: Vec<String>,
    /// Lessons learned (descriptions only)
    pub lessons: Vec<String>,
    /// Metrics
    pub metrics: MissionMetrics,
    /// Created timestamp
    pub created_at: u64,
}

impl From<&Aar> for AarDBRecord {
    fn from(aar: &Aar) -> Self {
        Self {
            id: aar.header.id.clone(),
            mission_id: aar.mission_id.clone(),
            swarm_id: aar.header.mission_id.clone(),
            objective: aar.objective.clone(),
            outcome: aar.outcome.clone(),
            successes: aar.successes.clone(),
            improvements: aar.areas_for_improvement.clone(),
            lessons: aar
                .lessons_learned
                .iter()
                .map(|l| l.description.clone())
                .collect(),
            metrics: aar.metrics.clone(),
            created_at: now(),
        }
    }
}

/// Database statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DatabaseStats {
    /// Total swarms
    pub total_swarms: usize,
    /// Total agents
    pub total_agents: usize,
    /// Total missions
    pub total_missions: usize,
    /// Total tasks
    pub total_tasks: usize,
    /// Total communications
    pub total_communications: usize,
    /// Total AARs
    pub total_aars: usize,
    /// Active swarms
    pub active_swarms: usize,
    /// Completed missions
    pub completed_missions: usize,
}

impl std::fmt::Display for DatabaseStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Rational Reserve Database Statistics")?;
        writeln!(f, "=====================================")?;
        writeln!(
            f,
            "Swarms: {} ({} active)",
            self.total_swarms, self.active_swarms
        )?;
        writeln!(f, "Agents: {}", self.total_agents)?;
        writeln!(
            f,
            "Missions: {} ({} completed)",
            self.total_missions, self.completed_missions
        )?;
        writeln!(f, "Tasks: {}", self.total_tasks)?;
        writeln!(f, "Communications: {}", self.total_communications)?;
        writeln!(f, "AAR Archive: {}", self.total_aars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_database_creation() {
        let db = RRDatabase::new();
        assert_eq!(db.version, 1);
        assert!(db.swarms.is_empty());
    }

    #[test]
    fn test_database_save_load() {
        let mut db = RRDatabase::new();

        // Add a test swarm record
        let swarm_record = SwarmRecord {
            id: "test_swarm".to_string(),
            mission_id: "test_mission".to_string(),
            general_id: "gen1".to_string(),
            agent_ids: vec!["agent1".to_string()],
            unit_ids: vec![],
            status: SwarmStatus::Active,
            created_at: now(),
            completed_at: None,
            progress: 0,
        };
        db.record_swarm(&swarm_record);

        // Save to temp file
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("rr_test_db.json");
        db.save_to_path(&db_path).unwrap();

        // Load from file
        let loaded_db = RRDatabase::load_from_path(&db_path).unwrap();
        assert_eq!(loaded_db.swarms.len(), 1);
        assert!(loaded_db.get_swarm("test_swarm").is_some());

        // Cleanup
        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn test_database_stats() {
        let mut db = RRDatabase::new();

        let swarm_record = SwarmRecord {
            id: "swarm1".to_string(),
            mission_id: "mission1".to_string(),
            general_id: "gen1".to_string(),
            agent_ids: vec![],
            unit_ids: vec![],
            status: SwarmStatus::Active,
            created_at: now(),
            completed_at: None,
            progress: 50,
        };
        db.record_swarm(&swarm_record);

        let mission_record = MissionDBRecord {
            id: "mission1".to_string(),
            objective: "Test".to_string(),
            description: "Test mission".to_string(),
            priority: Priority::Routine,
            status: MissionStatus::InProgress,
            swarm_id: Some("swarm1".to_string()),
            created_at: now(),
            started_at: Some(now()),
            completed_at: None,
            progress: 50,
            task_count: 5,
        };
        db.record_mission(&mission_record);

        let stats = db.get_stats();
        assert_eq!(stats.total_swarms, 1);
        assert_eq!(stats.active_swarms, 1);
        assert_eq!(stats.total_missions, 1);
    }
}
