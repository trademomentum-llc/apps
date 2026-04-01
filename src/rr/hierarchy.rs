//! Military Hierarchy -- Ranks, MOS, and Unit Formation.
//!
//! This module defines the military-style command structure for Rational Reserve:
//! - Officer ranks (GEN, COL, MAJ, CPT, LT)
//! - NCO ranks (SGM, MSG, SGT, CPL)
//! - Enlisted ranks (SPC, PFC, PVT)
//! - Military Occupational Specialties (MOS)
//! - Unit formations (Fire Team, Squad, Platoon, Company, Battalion)

use serde::{Deserialize, Serialize};

// ============================================================================
// Ranks
// ============================================================================

/// Military rank determining position in chain of command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Rank {
    // Officer Ranks (Strategic & Tactical Leadership)
    /// General (GEN) - Swarm Commander
    GEN,
    /// Colonel (COL) - Battalion Commander
    COL,
    /// Major (MAJ) - Operations Officer
    MAJ,
    /// Captain (CPT) - Company Commander
    CPT,
    /// Lieutenant (LT) - Platoon Leader
    LT,

    // Non-Commissioned Officer Ranks (Execution Leadership)
    /// Sergeant Major (SGM) - Senior NCO
    SGM,
    /// Master Sergeant (MSG) - Section Chief
    MSG,
    /// Sergeant (SGT) - Team Leader
    SGT,
    /// Corporal (CPL) - Fire Team Leader
    CPL,

    // Enlisted Specialists
    /// Specialist (SPC) - Technical Expert
    SPC,
    /// Private First Class (PFC) - Junior Specialist
    PFC,
    /// Private (PVT) - Entry Agent
    PVT,
}

impl Rank {
    /// Get the display name for this rank
    pub fn name(self) -> &'static str {
        match self {
            Rank::GEN => "General",
            Rank::COL => "Colonel",
            Rank::MAJ => "Major",
            Rank::CPT => "Captain",
            Rank::LT => "Lieutenant",
            Rank::SGM => "Sergeant Major",
            Rank::MSG => "Master Sergeant",
            Rank::SGT => "Sergeant",
            Rank::CPL => "Corporal",
            Rank::SPC => "Specialist",
            Rank::PFC => "Private First Class",
            Rank::PVT => "Private",
        }
    }

    /// Get the abbreviation for this rank
    pub fn abbreviation(self) -> &'static str {
        match self {
            Rank::GEN => "GEN",
            Rank::COL => "COL",
            Rank::MAJ => "MAJ",
            Rank::CPT => "CPT",
            Rank::LT => "LT",
            Rank::SGM => "SGM",
            Rank::MSG => "MSG",
            Rank::SGT => "SGT",
            Rank::CPL => "CPL",
            Rank::SPC => "SPC",
            Rank::PFC => "PFC",
            Rank::PVT => "PVT",
        }
    }

    /// Check if this rank is an officer rank
    pub fn is_officer(self) -> bool {
        matches!(
            self,
            Rank::GEN | Rank::COL | Rank::MAJ | Rank::CPT | Rank::LT
        )
    }

    /// Check if this rank is an NCO rank
    pub fn is_nco(self) -> bool {
        matches!(self, Rank::SGM | Rank::MSG | Rank::SGT | Rank::CPL)
    }

    /// Check if this rank is enlisted
    pub fn is_enlisted(self) -> bool {
        matches!(self, Rank::SPC | Rank::PFC | Rank::PVT)
    }

    /// Get the typical responsibilities for this rank
    pub fn responsibilities(self) -> &'static str {
        match self {
            Rank::GEN => "Overall mission planning, resource allocation, strategic decisions",
            Rank::COL => "Coordinate multiple task forces, manage officer corps",
            Rank::MAJ => "Mission decomposition, assign objectives to captains",
            Rank::CPT => "Lead specialized teams, tactical execution",
            Rank::LT => "Direct supervision of NCOs and specialists",
            Rank::SGM => "Coordinate all enlisted activities, liaison to officers",
            Rank::MSG => "Lead specialized sections (intelligence, logistics, comms)",
            Rank::SGT => "Direct supervision of specialists, hands-on execution",
            Rank::CPL => "Lead 2-4 specialists in focused tasks",
            Rank::SPC => "Execute specific tasks (coding, analysis, data processing)",
            Rank::PFC => "Assist specialists, learn and execute simple tasks",
            Rank::PVT => "Basic task execution, data gathering, reconnaissance",
        }
    }

    /// Get all officer ranks in descending order
    pub fn officer_ranks() -> Vec<Rank> {
        vec![Rank::GEN, Rank::COL, Rank::MAJ, Rank::CPT, Rank::LT]
    }

    /// Get all NCO ranks in descending order
    pub fn nco_ranks() -> Vec<Rank> {
        vec![Rank::SGM, Rank::MSG, Rank::SGT, Rank::CPL]
    }

    /// Get all enlisted ranks in descending order
    pub fn enlisted_ranks() -> Vec<Rank> {
        vec![Rank::SPC, Rank::PFC, Rank::PVT]
    }
}

impl std::fmt::Display for Rank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name(), self.abbreviation())
    }
}

// ============================================================================
// Military Occupational Specialties (MOS)
// ============================================================================

/// Military Occupational Specialty - agent's area of expertise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MOS {
    // Intelligence (MI)
    /// 35F - Intelligence Analyst
    Intel35F,
    /// 35L - Counterintelligence
    Intel35L,
    /// 35N - SIGINT Analyst
    Intel35N,

    // Operations (OPS)
    /// 11B - Infantry (Core Execution)
    Ops11B,
    /// 12B - Combat Engineer
    Ops12B,
    /// 13B - Artillery
    Ops13B,
    /// 19D - Cavalry Scout
    Ops19D,

    // Support (SPT)
    /// 25B - IT Specialist
    Spt25B,
    /// 42A - HR Specialist
    Spt42A,
    /// 88M - Transport
    Spt88M,
    /// 92G - Logistics
    Spt92G,

    // Special Operations (SOF)
    /// 18B - SF Engineer
    Sof18B,
    /// 75th Ranger
    SofRanger,
    /// 160th SOAR
    SofSOAR,
}

impl MOS {
    /// Get the MOS code (e.g., "35F")
    pub fn code(self) -> &'static str {
        match self {
            MOS::Intel35F => "35F",
            MOS::Intel35L => "35L",
            MOS::Intel35N => "35N",
            MOS::Ops11B => "11B",
            MOS::Ops12B => "12B",
            MOS::Ops13B => "13B",
            MOS::Ops19D => "19D",
            MOS::Spt25B => "25B",
            MOS::Spt42A => "42A",
            MOS::Spt88M => "88M",
            MOS::Spt92G => "92G",
            MOS::Sof18B => "18B",
            MOS::SofRanger => "75R",
            MOS::SofSOAR => "160S",
        }
    }

    /// Get the MOS title
    pub fn title(self) -> &'static str {
        match self {
            MOS::Intel35F => "Intelligence Analyst",
            MOS::Intel35L => "Counterintelligence",
            MOS::Intel35N => "SIGINT Analyst",
            MOS::Ops11B => "Infantry (Core Execution)",
            MOS::Ops12B => "Combat Engineer",
            MOS::Ops13B => "Artillery",
            MOS::Ops19D => "Cavalry Scout",
            MOS::Spt25B => "IT Specialist",
            MOS::Spt42A => "HR Specialist",
            MOS::Spt88M => "Transport",
            MOS::Spt92G => "Logistics",
            MOS::Sof18B => "SF Engineer",
            MOS::SofRanger => "75th Ranger",
            MOS::SofSOAR => "160th SOAR",
        }
    }

    /// Get the MOS category
    pub fn category(self) -> MOSCategory {
        match self {
            MOS::Intel35F | MOS::Intel35L | MOS::Intel35N => MOSCategory::Intelligence,
            MOS::Ops11B | MOS::Ops12B | MOS::Ops13B | MOS::Ops19D => MOSCategory::Operations,
            MOS::Spt25B | MOS::Spt42A | MOS::Spt88M | MOS::Spt92G => MOSCategory::Support,
            MOS::Sof18B | MOS::SofRanger | MOS::SofSOAR => MOSCategory::SpecialOperations,
        }
    }

    /// Get the description of this MOS's responsibilities
    pub fn responsibilities(self) -> &'static str {
        match self {
            MOS::Intel35F => "Data analysis, pattern recognition, threat assessment",
            MOS::Intel35L => "Security validation, threat detection, anomaly identification",
            MOS::Intel35N => "Signal processing, log analysis, communication monitoring",
            MOS::Ops11B => "General-purpose task execution, adaptable workers",
            MOS::Ops12B => "Code generation, system building, infrastructure setup",
            MOS::Ops13B => "Heavy computation, batch processing, data bombardment",
            MOS::Ops19D => "Reconnaissance, codebase exploration, environment scanning",
            MOS::Spt25B => "System administration, deployment, DevOps",
            MOS::Spt42A => "Agent lifecycle management, swarm roster maintenance",
            MOS::Spt88M => "Data transfer, API orchestration, message routing",
            MOS::Spt92G => "Resource allocation, dependency management, supply chain",
            MOS::Sof18B => "Advanced system design, architecture, critical solutions",
            MOS::SofRanger => "Rapid response, emergency fixes, crisis intervention",
            MOS::SofSOAR => "High-speed data operations, real-time processing",
        }
    }

    /// Get all Intelligence MOS
    pub fn intelligence_mos() -> Vec<MOS> {
        vec![MOS::Intel35F, MOS::Intel35L, MOS::Intel35N]
    }

    /// Get all Operations MOS
    pub fn operations_mos() -> Vec<MOS> {
        vec![MOS::Ops11B, MOS::Ops12B, MOS::Ops13B, MOS::Ops19D]
    }

    /// Get all Support MOS
    pub fn support_mos() -> Vec<MOS> {
        vec![MOS::Spt25B, MOS::Spt42A, MOS::Spt88M, MOS::Spt92G]
    }

    /// Get all Special Operations MOS
    pub fn special_operations_mos() -> Vec<MOS> {
        vec![MOS::Sof18B, MOS::SofRanger, MOS::SofSOAR]
    }
}

impl std::fmt::Display for MOS {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} - {}", self.code(), self.title())
    }
}

/// MOS category for grouping
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MOSCategory {
    /// Intelligence
    Intelligence,
    /// Operations
    Operations,
    /// Support
    Support,
    /// Special Operations
    SpecialOperations,
}

impl MOSCategory {
    /// Get the category name
    pub fn name(self) -> &'static str {
        match self {
            MOSCategory::Intelligence => "Intelligence (MI)",
            MOSCategory::Operations => "Operations (OPS)",
            MOSCategory::Support => "Support (SPT)",
            MOSCategory::SpecialOperations => "Special Operations (SOF)",
        }
    }
}

// ============================================================================
// Unit Formations
// ============================================================================

/// Type of military unit formation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitType {
    /// Fire Team (2-4 agents)
    FireTeam,
    /// Squad (4-8 agents)
    Squad,
    /// Platoon (8-16 agents)
    Platoon,
    /// Company (16-32 agents)
    Company,
    /// Battalion (32+ agents)
    Battalion,
}

impl UnitType {
    /// Get the typical size range for this unit type
    pub fn size_range(self) -> (usize, usize) {
        match self {
            UnitType::FireTeam => (2, 4),
            UnitType::Squad => (4, 8),
            UnitType::Platoon => (8, 16),
            UnitType::Company => (16, 32),
            UnitType::Battalion => (32, 100),
        }
    }

    /// Get the typical leader rank for this unit type
    pub fn leader_rank(self) -> Rank {
        match self {
            UnitType::FireTeam => Rank::CPL,
            UnitType::Squad => Rank::SGT,
            UnitType::Platoon => Rank::LT,
            UnitType::Company => Rank::CPT,
            UnitType::Battalion => Rank::COL,
        }
    }

    /// Determine unit type based on desired size
    pub fn from_size(size: usize) -> UnitType {
        match size {
            0..=4 => UnitType::FireTeam,
            5..=8 => UnitType::Squad,
            9..=16 => UnitType::Platoon,
            17..=32 => UnitType::Company,
            _ => UnitType::Battalion,
        }
    }
}

impl std::fmt::Display for UnitType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (min, max) = self.size_range();
        write!(f, "{:?} ({}-{} agents)", self, min, max)
    }
}

/// Unit designation (e.g., "Alpha Company", "1st Platoon")
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitDesignation {
    /// Unit type
    pub unit_type: UnitType,
    /// Unit name (e.g., "Alpha", "1st", "Bravo")
    pub name: String,
    /// Parent unit (if any)
    pub parent: Option<String>,
}

impl UnitDesignation {
    /// Create a new unit designation
    pub fn new(unit_type: UnitType, name: String) -> Self {
        Self {
            unit_type,
            name,
            parent: None,
        }
    }

    /// Create with parent unit
    pub fn with_parent(unit_type: UnitType, name: String, parent: String) -> Self {
        Self {
            unit_type,
            name,
            parent: Some(parent),
        }
    }

    /// Get the full designation string
    pub fn full_designation(&self) -> String {
        if let Some(parent) = &self.parent {
            format!(
                "{} {}, {} {:?}",
                self.name,
                self.unit_type.letter(),
                parent,
                self.unit_type
            )
        } else {
            format!("{} {}", self.name, self.unit_type.letter())
        }
    }
}

impl UnitType {
    /// Get the letter designation for this unit type
    fn letter(self) -> &'static str {
        match self {
            UnitType::FireTeam => "FT",
            UnitType::Squad => "SQ",
            UnitType::Platoon => "PLT",
            UnitType::Company => "CO",
            UnitType::Battalion => "BN",
        }
    }
}

/// Unit composition specifying required MOS mix
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitComposition {
    /// Target unit type
    pub unit_type: UnitType,
    /// Required MOS with counts
    pub mos_requirements: Vec<(MOS, usize)>,
    /// Minimum rank requirements
    pub min_rank: Rank,
    /// Preferred specialties
    pub preferred_mos: Vec<MOS>,
}

impl UnitComposition {
    /// Create a fire team composition
    pub fn fire_team(mos: Vec<MOS>) -> Self {
        Self {
            unit_type: UnitType::FireTeam,
            mos_requirements: mos.into_iter().map(|m| (m, 1)).collect(),
            min_rank: Rank::CPL,
            preferred_mos: vec![],
        }
    }

    /// Create a squad composition
    pub fn squad(mos: Vec<MOS>) -> Self {
        Self {
            unit_type: UnitType::Squad,
            mos_requirements: mos.into_iter().map(|m| (m, 2)).collect(),
            min_rank: Rank::SGT,
            preferred_mos: vec![],
        }
    }

    /// Create a platoon composition
    pub fn platoon(mos: Vec<MOS>) -> Self {
        Self {
            unit_type: UnitType::Platoon,
            mos_requirements: mos.into_iter().map(|m| (m, 4)).collect(),
            min_rank: Rank::LT,
            preferred_mos: vec![],
        }
    }

    /// Create a company composition
    pub fn company(mos: Vec<MOS>) -> Self {
        Self {
            unit_type: UnitType::Company,
            mos_requirements: mos.into_iter().map(|m| (m, 8)).collect(),
            min_rank: Rank::CPT,
            preferred_mos: vec![],
        }
    }

    /// Add preferred MOS
    pub fn with_preferred(mut self, mos: Vec<MOS>) -> Self {
        self.preferred_mos = mos;
        self
    }
}

// ============================================================================
// Agent Registry Entry
// ============================================================================

/// Registry entry for an agent in the swarm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegistryEntry {
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
    /// Status (active, standby, etc.)
    pub active: bool,
}

impl AgentRegistryEntry {
    /// Create a new registry entry
    pub fn new(agent_id: String, name: String, rank: Rank, mos: MOS) -> Self {
        Self {
            agent_id,
            name,
            rank,
            mos,
            unit: None,
            commander: None,
            active: true,
        }
    }

    /// Set unit assignment
    pub fn with_unit(mut self, unit: String) -> Self {
        self.unit = Some(unit);
        self
    }

    /// Set commander
    pub fn with_commander(mut self, commander: String) -> Self {
        self.commander = Some(commander);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rank_ordering() {
        // Ranks are ordered by definition order (GEN is lowest ordinal, PVT is highest)
        // For chain of command, we use custom comparison methods
        assert_eq!(Rank::GEN as u8, 0);
        assert_eq!(Rank::PVT as u8, 11); // 12 ranks total (0-11)
        // Higher rank in hierarchy = lower ordinal value
        assert!((Rank::GEN as u8) < (Rank::COL as u8));
        assert!((Rank::PVT as u8) > (Rank::SPC as u8));
    }

    #[test]
    fn test_rank_classification() {
        assert!(Rank::GEN.is_officer());
        assert!(Rank::CPT.is_officer());
        assert!(Rank::LT.is_officer());
        assert!(!Rank::SGM.is_officer());

        assert!(Rank::SGM.is_nco());
        assert!(Rank::SGT.is_nco());
        assert!(Rank::CPL.is_nco());
        assert!(!Rank::SPC.is_nco());

        assert!(Rank::SPC.is_enlisted());
        assert!(Rank::PFC.is_enlisted());
        assert!(Rank::PVT.is_enlisted());
        assert!(!Rank::CPL.is_enlisted());
    }

    #[test]
    fn test_mos_categories() {
        assert_eq!(MOS::Intel35F.category(), MOSCategory::Intelligence);
        assert_eq!(MOS::Ops11B.category(), MOSCategory::Operations);
        assert_eq!(MOS::Spt25B.category(), MOSCategory::Support);
        assert_eq!(MOS::Sof18B.category(), MOSCategory::SpecialOperations);
    }

    #[test]
    fn test_unit_type_from_size() {
        assert_eq!(UnitType::from_size(3), UnitType::FireTeam);
        assert_eq!(UnitType::from_size(6), UnitType::Squad);
        assert_eq!(UnitType::from_size(12), UnitType::Platoon);
        assert_eq!(UnitType::from_size(24), UnitType::Company);
        assert_eq!(UnitType::from_size(50), UnitType::Battalion);
    }

    #[test]
    fn test_unit_composition() {
        let composition = UnitComposition::fire_team(vec![MOS::Ops11B, MOS::Ops12B]);
        assert_eq!(composition.unit_type, UnitType::FireTeam);
        assert_eq!(composition.mos_requirements.len(), 2);
    }
}
