//! Communication Protocols -- SITREP, FRAGO, CASREP, AAR.
//!
//! This module defines the standardized communication formats for
//! Rational Reserve agent coordination.

use super::agents::base::AgentStatus;
use super::hierarchy::*;
use super::memory::now;
use super::mission::*;
use crate::types::MorphResult;
use serde::{Deserialize, Serialize};

/// Unique communication ID
pub type CommId = String;

/// Base communication header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationHeader {
    /// Unique communication ID
    pub id: CommId,
    /// Communication type
    pub comm_type: CommType,
    /// Sender agent ID
    pub from: String,
    /// Recipient agent ID
    pub to: String,
    /// Timestamp
    pub timestamp: u64,
    /// Mission ID (if applicable)
    pub mission_id: Option<String>,
    /// Priority
    pub priority: Priority,
}

impl CommunicationHeader {
    /// Create a new communication header
    pub fn new(comm_type: CommType, from: String, to: String, priority: Priority) -> Self {
        Self {
            id: format!("c_{}_{}", now(), uuid_simple()),
            comm_type,
            from,
            to,
            timestamp: now(),
            mission_id: None,
            priority,
        }
    }

    /// Set mission ID
    pub fn with_mission(mut self, mission_id: String) -> Self {
        self.mission_id = Some(mission_id);
        self
    }
}

/// Communication type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommType {
    /// Situation Report (SITREP)
    SitRep,
    /// Fragmentary Order (FRAGO)
    Frago,
    /// Casualty Report (CASREP)
    CasRep,
    /// After Action Review (AAR)
    Aar,
    /// Order
    Order,
    /// Request
    Request,
    /// Response
    Response,
}

// ============================================================================
// SITREP - Situation Report
// ============================================================================

/// Situation Report - Periodic status update from subordinates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SitRep {
    /// Communication header
    #[serde(flatten)]
    pub header: CommunicationHeader,
    /// Agent's current status
    pub status: AgentStatus,
    /// Current mission ID
    pub mission_id: Option<String>,
    /// Report content
    pub content: String,
    /// Progress percentage (0-100)
    pub progress: u8,
    /// Current blockers
    pub blockers: Vec<String>,
    /// Subordinates' status (for NCOs/officers)
    pub subordinates_status: Vec<SubordinateStatus>,
    /// Resource usage
    pub resource_usage: Option<ResourceUsage>,
}

impl SitRep {
    /// Create a new SITREP
    pub fn new(from: String, to: String, status: AgentStatus, content: String) -> Self {
        Self {
            header: CommunicationHeader::new(CommType::SitRep, from, to, Priority::Routine),
            status,
            mission_id: None,
            content,
            progress: 0,
            blockers: Vec::new(),
            subordinates_status: Vec::new(),
            resource_usage: None,
        }
    }

    /// Set mission ID
    pub fn with_mission(mut self, mission_id: String) -> Self {
        self.header.mission_id = Some(mission_id.clone());
        self.mission_id = Some(mission_id);
        self
    }

    /// Set progress
    pub fn with_progress(mut self, progress: u8) -> Self {
        self.progress = progress;
        self
    }

    /// Add blocker
    pub fn with_blocker(mut self, blocker: String) -> Self {
        self.blockers.push(blocker);
        self
    }

    /// Set subordinates status
    pub fn with_subordinates(mut self, status: Vec<SubordinateStatus>) -> Self {
        self.subordinates_status = status;
        self
    }
}

/// Subordinate status summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubordinateStatus {
    /// Agent ID
    pub agent_id: String,
    /// Current status
    pub status: AgentStatus,
    /// Progress percentage
    pub progress: u8,
}

/// Resource usage report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// LLM calls used
    pub llm_calls: u64,
    /// Tokens consumed
    pub tokens_used: u64,
    /// Compute time (seconds)
    pub compute_seconds: u64,
    /// Memory used (MB)
    pub memory_mb: u64,
}

// ============================================================================
// FRAGO - Fragmentary Order
// ============================================================================

/// Fragmentary Order - Mid-mission adjustment from superiors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frago {
    /// Communication header
    #[serde(flatten)]
    pub header: CommunicationHeader,
    /// Order content/instructions
    pub content: String,
    /// Changes from original order
    pub changes: Vec<Change>,
    /// Effective time
    pub effective_immediately: bool,
    /// Acknowledgment required
    pub ack_required: bool,
}

impl Frago {
    /// Create a new FRAGO
    pub fn new(from: String, to: String, content: String) -> Self {
        Self {
            header: CommunicationHeader::new(CommType::Frago, from, to, Priority::Priority),
            content,
            changes: Vec::new(),
            effective_immediately: true,
            ack_required: true,
        }
    }

    /// Add a change
    pub fn with_change(mut self, change: Change) -> Self {
        self.changes.push(change);
        self
    }

    /// Set effective immediately flag
    pub fn with_immediate_effect(mut self, immediate: bool) -> Self {
        self.effective_immediately = immediate;
        self
    }
}

/// Type of change in a FRAGO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Change {
    /// Change objective
    ObjectiveChange { old: String, new: String },
    /// Change priority
    PriorityChange { old: Priority, new: Priority },
    /// Add resources
    ResourceAddition { resource: String, amount: u64 },
    /// Change deadline
    DeadlineChange { new_deadline: u64 },
    /// Add/remove constraints
    ConstraintChange { action: String, constraint: String },
    /// Reassign personnel
    PersonnelChange {
        agent_id: String,
        action: PersonnelAction,
    },
}

/// Personnel action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PersonnelAction {
    /// Add to mission
    Add,
    /// Remove from mission
    Remove,
    /// Reassign to different task
    Reassign { new_task: String },
}

// ============================================================================
// CASREP - Casualty Report
// ============================================================================

/// Casualty Report - Agent failure/error notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CasRep {
    /// Communication header
    #[serde(flatten)]
    pub header: CommunicationHeader,
    /// Affected agent ID
    pub affected_agent: String,
    /// Failure type
    pub failure_type: FailureType,
    /// Severity level
    pub severity: Severity,
    /// Error description
    pub description: String,
    /// Impact assessment
    pub impact: ImpactAssessment,
    /// Recovery actions taken
    pub recovery_actions: Vec<String>,
    /// Assistance requested
    pub assistance_needed: Option<String>,
}

impl CasRep {
    /// Create a new CASREP
    pub fn new(
        from: String,
        to: String,
        affected_agent: String,
        failure_type: FailureType,
        description: String,
    ) -> Self {
        Self {
            header: CommunicationHeader::new(CommType::CasRep, from, to, Priority::Urgent),
            affected_agent,
            failure_type,
            severity: Severity::Moderate,
            description,
            impact: ImpactAssessment::default(),
            recovery_actions: Vec::new(),
            assistance_needed: None,
        }
    }

    /// Set severity
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    /// Set impact
    pub fn with_impact(mut self, impact: ImpactAssessment) -> Self {
        self.impact = impact;
        self
    }

    /// Add recovery action
    pub fn with_recovery_action(mut self, action: String) -> Self {
        self.recovery_actions.push(action);
        self
    }

    /// Request assistance
    pub fn request_assistance(mut self, assistance: String) -> Self {
        self.assistance_needed = Some(assistance);
        self
    }
}

/// Failure type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailureType {
    /// Agent crashed/error
    AgentError { error: String },
    /// Resource exhaustion
    ResourceExhaustion { resource: String },
    /// Timeout
    Timeout { timeout_seconds: u64 },
    /// Security violation
    SecurityViolation { violation: String },
    /// Communication failure
    CommunicationFailure,
    /// Dependency failure
    DependencyFailure { dependency: String },
}

/// Severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    /// Minor - minimal impact
    Minor,
    /// Moderate - noticeable impact
    Moderate,
    /// Serious - significant impact
    Serious,
    /// Critical - mission-threatening
    Critical,
    /// Catastrophic - mission failure
    Catastrophic,
}

/// Impact assessment
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImpactAssessment {
    /// Mission impact (0-100)
    pub mission_impact: u8,
    /// Timeline impact (delay in seconds)
    pub timeline_delay: u64,
    /// Resource impact (additional resources needed)
    pub additional_resources: ResourceAllocation,
    /// Affected tasks
    pub affected_tasks: Vec<String>,
}

/// Resource allocation (simplified for CASREP)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceAllocation {
    /// Additional LLM calls needed
    pub llm_calls: u64,
    /// Additional compute time needed
    pub compute_seconds: u64,
}

// ============================================================================
// AAR - After Action Review
// ============================================================================

/// After Action Review - Post-mission analysis and learning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aar {
    /// Communication header
    #[serde(flatten)]
    pub header: CommunicationHeader,
    /// Mission ID
    pub mission_id: String,
    /// Mission objective
    pub objective: String,
    /// Actual outcome
    pub outcome: Outcome,
    /// What was supposed to happen
    pub intended: String,
    /// What actually happened
    pub actual: String,
    /// What went well
    pub successes: Vec<String>,
    /// What could be improved
    pub areas_for_improvement: Vec<String>,
    /// Lessons learned
    pub lessons_learned: Vec<Lesson>,
    /// Recommendations
    pub recommendations: Vec<String>,
    /// Participant feedback
    pub participant_feedback: Vec<ParticipantFeedback>,
    /// Metrics
    pub metrics: MissionMetrics,
}

impl Aar {
    /// Create a new AAR
    pub fn new(from: String, to: String, mission: &Mission) -> Self {
        Self {
            header: CommunicationHeader::new(CommType::Aar, from, to, Priority::Routine),
            mission_id: mission.id.clone(),
            objective: mission.objective.clone(),
            outcome: Outcome::default(),
            intended: String::new(),
            actual: String::new(),
            successes: Vec::new(),
            areas_for_improvement: Vec::new(),
            lessons_learned: Vec::new(),
            recommendations: Vec::new(),
            participant_feedback: Vec::new(),
            metrics: MissionMetrics::default(),
        }
    }

    /// Set outcome
    pub fn with_outcome(mut self, outcome: Outcome) -> Self {
        self.outcome = outcome;
        self
    }

    /// Set intended vs actual
    pub fn with_comparison(mut self, intended: String, actual: String) -> Self {
        self.intended = intended;
        self.actual = actual;
        self
    }

    /// Add success
    pub fn with_success(mut self, success: String) -> Self {
        self.successes.push(success);
        self
    }

    /// Add area for improvement
    pub fn with_improvement(mut self, improvement: String) -> Self {
        self.areas_for_improvement.push(improvement);
        self
    }

    /// Add lesson learned
    pub fn with_lesson(mut self, lesson: Lesson) -> Self {
        self.lessons_learned.push(lesson);
        self
    }

    /// Add recommendation
    pub fn with_recommendation(mut self, recommendation: String) -> Self {
        self.recommendations.push(recommendation);
        self
    }
}

/// Mission outcome
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum Outcome {
    /// Mission completed successfully
    Success,
    /// Mission completed with issues
    PartialSuccess,
    /// Mission failed
    #[default]
    Failure,
    /// Mission cancelled
    Cancelled,
}

/// Lesson learned
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lesson {
    /// Lesson description
    pub description: String,
    /// Category
    pub category: LessonCategory,
    /// Applicability (which future missions)
    pub applicability: Vec<String>,
    /// Confidence level (0.0 to 1.0)
    pub confidence: f64,
}

/// Lesson category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LessonCategory {
    /// Tactical lesson
    Tactical,
    /// Technical lesson
    Technical,
    /// Process lesson
    Process,
    /// Communication lesson
    Communication,
    /// Resource lesson
    Resource,
}

/// Participant feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantFeedback {
    /// Agent ID
    pub agent_id: String,
    /// Feedback content
    pub feedback: String,
    /// Rating (1-10)
    pub rating: Option<u8>,
}

/// Mission metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MissionMetrics {
    /// Total duration (seconds)
    pub duration_seconds: u64,
    /// Total LLM calls
    pub llm_calls: u64,
    /// Total tokens used
    pub tokens_used: u64,
    /// Task completion rate (0.0 to 1.0)
    pub task_completion_rate: f64,
    /// Agent utilization (0.0 to 1.0)
    pub agent_utilization: f64,
    /// Cost (in credits)
    pub cost: f64,
}

// ============================================================================
// Order
// ============================================================================

/// Order from superior to subordinate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// Communication header
    #[serde(flatten)]
    pub header: CommunicationHeader,
    /// Order content
    pub content: String,
    /// Task to execute
    pub task: Option<Task>,
    /// Deadline (timestamp)
    pub deadline: Option<u64>,
    /// Rules of engagement
    pub rules_of_engagement: Vec<String>,
    /// Acknowledgment received
    pub acknowledged: bool,
}

impl Order {
    /// Create a new order
    pub fn new(from: String, to: String, content: String) -> Self {
        Self {
            header: CommunicationHeader::new(CommType::Order, from, to, Priority::Routine),
            content,
            task: None,
            deadline: None,
            rules_of_engagement: Vec::new(),
            acknowledged: false,
        }
    }

    /// Set task
    pub fn with_task(mut self, task: Task) -> Self {
        self.task = Some(task);
        self
    }

    /// Set deadline
    pub fn with_deadline(mut self, deadline: u64) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Add rule of engagement
    pub fn with_roe(mut self, roe: String) -> Self {
        self.rules_of_engagement.push(roe);
        self
    }
}

// ============================================================================
// Delegation
// ============================================================================

/// Task delegation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delegation {
    /// Task ID being delegated
    pub task_id: TaskId,
    /// Delegating agent ID
    pub from: String,
    /// Receiving agent ID
    pub to: String,
    /// Timestamp
    pub timestamp: u64,
    /// Task details
    pub task: Task,
}

// ============================================================================
// Request/Response
// ============================================================================

/// Request from agent to superior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Communication header
    #[serde(flatten)]
    pub header: CommunicationHeader,
    /// Request type
    pub request_type: RequestType,
    /// Request content
    pub content: String,
    /// Justification
    pub justification: String,
}

impl Request {
    /// Create a new request
    pub fn new(from: String, to: String, request_type: RequestType, content: String) -> Self {
        Self {
            header: CommunicationHeader::new(CommType::Request, from, to, Priority::Routine),
            request_type,
            content,
            justification: String::new(),
        }
    }

    /// Set justification
    pub fn with_justification(mut self, justification: String) -> Self {
        self.justification = justification;
        self
    }
}

/// Request type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestType {
    /// Request additional resources
    Resources { resource: String, amount: u64 },
    /// Request assistance
    Assistance { type_needed: String },
    /// Request clarification
    Clarification { question: String },
    /// Request permission
    Permission { action: String },
    /// Request extension
    Extension { additional_time: u64 },
}

/// Response to a request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Communication header
    #[serde(flatten)]
    pub header: CommunicationHeader,
    /// Original request ID
    pub request_id: String,
    /// Approved or denied
    pub approved: bool,
    /// Response content
    pub content: String,
    /// Conditions (if approved with conditions)
    pub conditions: Vec<String>,
}

impl Response {
    /// Create an approval response
    pub fn approve(request_id: String, from: String, to: String, content: String) -> Self {
        Self {
            header: CommunicationHeader::new(CommType::Response, from, to, Priority::Routine),
            request_id,
            approved: true,
            content,
            conditions: Vec::new(),
        }
    }

    /// Create a denial response
    pub fn deny(request_id: String, from: String, to: String, content: String) -> Self {
        Self {
            header: CommunicationHeader::new(CommType::Response, from, to, Priority::Routine),
            request_id,
            approved: false,
            content,
            conditions: Vec::new(),
        }
    }

    /// Add conditions
    pub fn with_conditions(mut self, conditions: Vec<String>) -> Self {
        self.conditions = conditions;
        self
    }
}

// Simple UUID-like generator
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
    fn test_sitrep_creation() {
        let sitrep = SitRep::new(
            "agent1".to_string(),
            "commander".to_string(),
            AgentStatus::Active,
            "Mission in progress".to_string(),
        );
        assert_eq!(sitrep.header.comm_type, CommType::SitRep);
        assert_eq!(sitrep.status, AgentStatus::Active);
    }

    #[test]
    fn test_frago_with_changes() {
        let frago = Frago::new(
            "commander".to_string(),
            "agent1".to_string(),
            "Change priority to authentication".to_string(),
        )
        .with_change(Change::PriorityChange {
            old: Priority::Routine,
            new: Priority::Urgent,
        });
        assert_eq!(frago.changes.len(), 1);
    }

    #[test]
    fn test_casrep() {
        let casrep = CasRep::new(
            "agent1".to_string(),
            "commander".to_string(),
            "agent2".to_string(),
            FailureType::AgentError {
                error: "Timeout".to_string(),
            },
            "Agent timed out during execution".to_string(),
        )
        .with_severity(Severity::Serious);
        assert_eq!(casrep.severity, Severity::Serious);
        assert_eq!(casrep.header.comm_type, CommType::CasRep);
    }

    #[test]
    fn test_aar() {
        use super::super::mission::Mission;
        let mission = Mission::new("Test mission", Priority::Routine);
        let mut aar = Aar::new("commander".to_string(), "archive".to_string(), &mission);
        aar = aar.with_outcome(Outcome::Success);
        aar = aar.with_success("Completed ahead of schedule".to_string());
        aar = aar.with_improvement("Better resource estimation needed".to_string());

        assert_eq!(aar.outcome, Outcome::Success);
        assert_eq!(aar.successes.len(), 1);
        assert_eq!(aar.areas_for_improvement.len(), 1);
    }
}
