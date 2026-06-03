//! Mission and Task Structures.
//!
//! This module defines the mission specification and task decomposition
//! for Rational Reserve operations.

use super::hierarchy::*;
use super::memory::now;
use crate::types::MorphResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique mission identifier
pub type MissionId = String;

/// Unique task identifier
pub type TaskId = String;

/// Mission priority level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum Priority {
    /// Routine priority
    #[default]
    Routine = 0,
    /// Priority mission
    Priority = 1,
    /// Urgent mission
    Urgent = 2,
}


/// Mission constraints
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MissionConstraints {
    /// Maximum time allowed (in seconds)
    pub max_time: Option<u64>,
    /// Maximum cost (in credits/tokens)
    pub max_cost: Option<u64>,
    /// Quality requirements (0.0 to 1.0)
    pub min_quality: Option<f64>,
    /// Required security clearance
    pub clearance: Option<SecurityLevel>,
    /// Restricted actions
    pub restricted_actions: Vec<String>,
}

/// Security clearance level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityLevel {
    /// Unclassified
    Unclassified,
    /// Confidential
    Confidential,
    /// Secret
    Secret,
    /// Top Secret
    TopSecret,
}

/// Resource allocation for a mission
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceAllocation {
    /// LLM call quota
    pub llm_calls: Option<u64>,
    /// Compute quota (CPU seconds)
    pub compute_seconds: Option<u64>,
    /// Memory quota (MB)
    pub memory_mb: Option<u64>,
    /// API call quota
    pub api_calls: Option<u64>,
    /// Token budget
    pub token_budget: Option<u64>,
}

/// Mission specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mission {
    /// Unique mission ID
    pub id: MissionId,
    /// Natural language objective
    pub objective: String,
    /// Detailed description
    pub description: String,
    /// Mission constraints
    pub constraints: MissionConstraints,
    /// Allocated resources
    pub resources: ResourceAllocation,
    /// Priority level
    pub priority: Priority,
    /// Task decomposition
    pub tasks: Vec<Task>,
    /// Current status
    pub status: MissionStatus,
    /// Timeline
    pub timeline: MissionTimeline,
    /// Assigned agent IDs
    pub assigned_agents: Vec<String>,
    /// Parent mission (if sub-mission)
    pub parent_mission: Option<MissionId>,
    /// Sub-missions
    pub sub_missions: Vec<MissionId>,
}

impl Mission {
    /// Create a new mission
    pub fn new(objective: impl Into<String>, priority: Priority) -> Self {
        let objective = objective.into();
        let id = format!("m_{}_{}", now(), uuid_simple());

        Self {
            id,
            objective: objective.clone(),
            description: objective,
            constraints: MissionConstraints::default(),
            resources: ResourceAllocation::default(),
            priority,
            tasks: Vec::new(),
            status: MissionStatus::Pending,
            timeline: MissionTimeline::new(),
            assigned_agents: Vec::new(),
            parent_mission: None,
            sub_missions: Vec::new(),
        }
    }

    /// Set mission description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set constraints
    pub fn with_constraints(mut self, constraints: MissionConstraints) -> Self {
        self.constraints = constraints;
        self
    }

    /// Set resources
    pub fn with_resources(mut self, resources: ResourceAllocation) -> Self {
        self.resources = resources;
        self
    }

    /// Add a task to the mission
    pub fn add_task(&mut self, task: Task) {
        self.tasks.push(task);
    }

    /// Decompose mission into tasks (simplified - would use LLM in production)
    pub fn decompose(&mut self) -> MorphResult<()> {
        // In production, this would use an LLM to decompose the objective
        // For now, create a placeholder task
        let task = Task::new(format!("Execute: {}", self.objective), self.priority);
        self.tasks.push(task);
        Ok(())
    }

    /// Update mission status based on task completion
    pub fn update_status(&mut self) {
        if self.tasks.is_empty() {
            self.status = MissionStatus::Complete;
            self.timeline.complete();
            return;
        }

        let completed = self
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Complete)
            .count();
        let total = self.tasks.len();

        if completed == total {
            self.status = MissionStatus::Complete;
            self.timeline.complete();
        } else if completed > 0 {
            self.status = MissionStatus::InProgress;
        } else if self.status == MissionStatus::Pending {
            self.status = MissionStatus::InProgress;
        }
    }

    /// Get mission progress (0-100)
    pub fn progress(&self) -> u8 {
        if self.tasks.is_empty() {
            return if self.status == MissionStatus::Complete {
                100
            } else {
                0
            };
        }

        let completed = self
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Complete)
            .count();
        ((completed as f64 / self.tasks.len() as f64) * 100.0) as u8
    }
}

/// Mission status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionStatus {
    /// Mission is pending activation
    Pending,
    /// Mission is in progress
    InProgress,
    /// Mission is on hold
    OnHold,
    /// Mission is complete
    Complete,
    /// Mission was cancelled
    Cancelled,
    /// Mission failed
    Failed,
}

/// Mission timeline tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionTimeline {
    /// When the mission was created
    pub created_at: u64,
    /// When the mission started
    pub started_at: Option<u64>,
    /// When the mission was completed
    pub completed_at: Option<u64>,
    /// Estimated completion time
    pub estimated_completion: Option<u64>,
}

impl MissionTimeline {
    /// Create a new timeline
    pub fn new() -> Self {
        let now = now();
        Self {
            created_at: now,
            started_at: None,
            completed_at: None,
            estimated_completion: None,
        }
    }

    /// Mark mission as started
    pub fn start(&mut self) {
        self.started_at = Some(now());
    }

    /// Mark mission as complete
    pub fn complete(&mut self) {
        self.completed_at = Some(now());
    }

    /// Set estimated completion time
    pub fn set_estimated_completion(&mut self, seconds_from_now: u64) {
        self.estimated_completion = Some(now() + seconds_from_now);
    }

    /// Get elapsed time in seconds
    pub fn elapsed(&self) -> u64 {
        self.started_at.map(|start| now() - start).unwrap_or(0)
    }

    /// Get total duration (if complete)
    pub fn duration(&self) -> Option<u64> {
        match (self.started_at, self.completed_at) {
            (Some(start), Some(end)) => Some(end - start),
            _ => None,
        }
    }
}

impl Default for MissionTimeline {
    fn default() -> Self {
        Self::new()
    }
}

/// Atomic task unit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task ID
    pub id: TaskId,
    /// Task description
    pub description: String,
    /// Task type
    pub task_type: TaskType,
    /// Required MOS for this task
    pub required_mos: Option<MOS>,
    /// Minimum rank required
    pub min_rank: Option<Rank>,
    /// Priority
    pub priority: Priority,
    /// Task status
    pub status: TaskStatus,
    /// Assigned agent ID
    pub assigned_to: Option<String>,
    /// Parent task ID
    pub parent_task: Option<TaskId>,
    /// Sub-tasks
    pub sub_tasks: Vec<TaskId>,
    /// Dependencies (task IDs that must complete first)
    pub dependencies: Vec<TaskId>,
    /// Task result
    pub result: Option<TaskResult>,
    /// Error message (if failed)
    pub error: Option<String>,
}

impl Task {
    /// Create a new task
    pub fn new(description: impl Into<String>, priority: Priority) -> Self {
        let id = format!("t_{}_{}", now(), uuid_simple());
        Self {
            id,
            description: description.into(),
            task_type: TaskType::General,
            required_mos: None,
            min_rank: None,
            priority,
            status: TaskStatus::Pending,
            assigned_to: None,
            parent_task: None,
            sub_tasks: Vec::new(),
            dependencies: Vec::new(),
            result: None,
            error: None,
        }
    }

    /// Set task type
    pub fn with_type(mut self, task_type: TaskType) -> Self {
        self.task_type = task_type;
        self
    }

    /// Set required MOS
    pub fn with_mos(mut self, mos: MOS) -> Self {
        self.required_mos = Some(mos);
        self
    }

    /// Set minimum rank
    pub fn with_min_rank(mut self, rank: Rank) -> Self {
        self.min_rank = Some(rank);
        self
    }

    /// Add dependency
    pub fn with_dependency(mut self, task_id: TaskId) -> Self {
        self.dependencies.push(task_id);
        self
    }

    /// Check if task is ready to execute (dependencies met)
    pub fn is_ready(&self, completed_tasks: &[TaskId]) -> bool {
        self.dependencies
            .iter()
            .all(|dep| completed_tasks.contains(dep))
    }
}

/// Task type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    /// General purpose task
    General,
    /// Intelligence/analysis task
    Intelligence,
    /// Code generation task
    CodeGeneration,
    /// Data processing task
    DataProcessing,
    /// Search/reconnaissance task
    Reconnaissance,
    /// Communication task
    Communication,
    /// Quality assurance task
    QualityAssurance,
    /// Security review task
    SecurityReview,
}

/// Task status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Task is pending
    Pending,
    /// Task is in progress
    InProgress,
    /// Task is complete
    Complete,
    /// Task is blocked
    Blocked,
    /// Task failed
    Failed,
}

/// Task result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Result content
    pub content: String,
    /// Output artifacts (file paths, data references)
    pub artifacts: Vec<String>,
    /// Metrics (time taken, tokens used, etc.)
    pub metrics: HashMap<String, u64>,
}

impl TaskResult {
    /// Create a new result
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            artifacts: Vec::new(),
            metrics: HashMap::new(),
        }
    }

    /// Add an artifact
    pub fn with_artifact(mut self, artifact: impl Into<String>) -> Self {
        self.artifacts.push(artifact.into());
        self
    }

    /// Add a metric
    pub fn with_metric(mut self, key: impl Into<String>, value: u64) -> Self {
        self.metrics.insert(key.into(), value);
        self
    }
}

/// Mission execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Result {
    /// Mission is pending
    Pending,
    /// Mission completed successfully
    Success { output: String },
    /// Mission failed
    Failure { error: String },
}

// Simple UUID-like generator for IDs
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{:08x}", nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mission_creation() {
        let mission = Mission::new("Build REST API", Priority::Routine);
        assert_eq!(mission.priority, Priority::Routine);
        assert_eq!(mission.status, MissionStatus::Pending);
        assert!(mission.id.starts_with("m_"));
    }

    #[test]
    fn test_mission_decomposition() {
        let mut mission = Mission::new("Test objective", Priority::Urgent);
        mission.decompose().unwrap();
        assert_eq!(mission.tasks.len(), 1);
    }

    #[test]
    fn test_mission_progress() {
        let mut mission = Mission::new("Test", Priority::Routine);
        mission.add_task(Task::new("Task 1", Priority::Routine));
        mission.add_task(Task::new("Task 2", Priority::Routine));

        assert_eq!(mission.progress(), 0);

        mission.tasks[0].status = TaskStatus::Complete;
        assert_eq!(mission.progress(), 50);

        mission.tasks[1].status = TaskStatus::Complete;
        assert_eq!(mission.progress(), 100);
    }

    #[test]
    fn test_task_dependencies() {
        let mut task = Task::new("Dependent task", Priority::Routine);
        task.dependencies.push("task1".to_string());
        task.dependencies.push("task2".to_string());

        assert!(!task.is_ready(&[]));
        assert!(!task.is_ready(&["task1".to_string()]));
        assert!(task.is_ready(&["task1".to_string(), "task2".to_string()]));
    }
}
