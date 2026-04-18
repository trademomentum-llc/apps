//! Four-tier Memory System for sophisticated reasoning.
//!
//! This module implements the memory architecture for Rational Reserve agents:
//! - **Short-term Memory**: Recent conversation context (configurable window)
//! - **Long-term Memory**: Persistent knowledge storage
//! - **Episodic Memory**: Timestamped interaction history
//! - **Semantic Memory**: Extracted patterns and learnings

use crate::MorphlexError;
use crate::types::MorphResult;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Timestamp for memory entries
pub type Timestamp = u64;

/// Get current Unix timestamp
pub fn now() -> Timestamp {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ============================================================================
// Short-term Memory
// ============================================================================

/// Short-term memory: Recent conversation context with configurable window.
///
/// Implemented as a ring buffer that maintains the most recent entries.
/// Fast access, volatile (not persisted by default).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortTermMemory {
    /// Maximum number of entries to retain
    capacity: usize,
    /// Ring buffer of recent entries
    entries: VecDeque<MemoryEntry>,
}

impl ShortTermMemory {
    /// Create new short-term memory with given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: VecDeque::with_capacity(capacity),
        }
    }

    /// Add an entry to short-term memory
    pub fn add(&mut self, content: String, metadata: Option<MemoryMetadata>) {
        let entry = MemoryEntry {
            timestamp: now(),
            content,
            metadata,
        };

        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// Get all entries in chronological order (oldest first)
    pub fn get_all(&self) -> Vec<&MemoryEntry> {
        self.entries.iter().collect()
    }

    /// Get the most recent N entries
    pub fn get_recent(&self, n: usize) -> Vec<&MemoryEntry> {
        self.entries
            .iter()
            .rev()
            .take(n)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get current entry count
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ============================================================================
// Long-term Memory
// ============================================================================

/// Long-term memory: Persistent knowledge storage.
///
/// Key-value store for durable knowledge that persists across sessions.
/// Supports tagging for organization and retrieval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongTermMemory {
    /// Stored knowledge entries
    entries: HashMap<String, KnowledgeEntry>,
    /// Tags for organization
    tags: HashMap<String, Vec<String>>,
}

impl LongTermMemory {
    /// Create new empty long-term memory
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            tags: HashMap::new(),
        }
    }

    /// Store a piece of knowledge
    pub fn store(&mut self, key: String, value: String, tags: Vec<String>) {
        let entry = KnowledgeEntry {
            key: key.clone(),
            value,
            created_at: now(),
            updated_at: now(),
            access_count: 0,
            tags: tags.clone(),
        };

        self.entries.insert(key.clone(), entry);

        // Update tag index
        for tag in tags {
            self.tags.entry(tag).or_default().push(key.clone());
        }
    }

    /// Retrieve a piece of knowledge by key
    pub fn retrieve(&mut self, key: &str) -> Option<&KnowledgeEntry> {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.access_count += 1;
            Some(entry)
        } else {
            None
        }
    }

    /// Retrieve by tag
    pub fn retrieve_by_tag(&self, tag: &str) -> Vec<&KnowledgeEntry> {
        self.tags
            .get(tag)
            .map(|keys| keys.iter().filter_map(|k| self.entries.get(k)).collect())
            .unwrap_or_default()
    }

    /// Delete a piece of knowledge
    pub fn delete(&mut self, key: &str) -> bool {
        if let Some(entry) = self.entries.remove(key) {
            // Remove from tag index
            for tag in &entry.tags {
                if let Some(keys) = self.tags.get_mut(tag) {
                    keys.retain(|k| k != key);
                }
            }
            true
        } else {
            false
        }
    }

    /// Search by keyword in content
    pub fn search(&self, query: &str) -> Vec<&KnowledgeEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .values()
            .filter(|e| e.value.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// Get all entries
    pub fn get_all(&self) -> Vec<&KnowledgeEntry> {
        self.entries.values().collect()
    }

    /// Persist to disk (JSON format for now, can use morphlex database later)
    pub fn save_to_path(&self, path: &Path) -> MorphResult<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| crate::MorphlexError::DatabaseError(e.to_string()))?;
        std::fs::write(path, json).map_err(|e| crate::MorphlexError::IoError(e))?;
        Ok(())
    }

    /// Load from disk
    pub fn load_from_path(path: &Path) -> MorphResult<Self> {
        let json = std::fs::read_to_string(path).map_err(|e| crate::MorphlexError::IoError(e))?;
        let memory = serde_json::from_str(&json)
            .map_err(|e| crate::MorphlexError::DatabaseError(e.to_string()))?;
        Ok(memory)
    }
}

impl Default for LongTermMemory {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Episodic Memory
// ============================================================================

/// Episodic memory: Timestamped interaction history.
///
/// Records complete interaction episodes with full context.
/// Used for learning from past experiences and AAR (After Action Review).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodicMemory {
    /// Stored episodes
    episodes: Vec<Episode>,
    /// Index by mission ID
    by_mission: HashMap<String, Vec<usize>>,
    /// Index by timestamp
    by_timestamp: Vec<(Timestamp, usize)>,
}

impl EpisodicMemory {
    /// Create new episodic memory
    pub fn new() -> Self {
        Self {
            episodes: Vec::new(),
            by_mission: HashMap::new(),
            by_timestamp: Vec::new(),
        }
    }

    /// Record a new episode
    pub fn record(&mut self, episode: Episode) {
        let idx = self.episodes.len();
        self.by_mission
            .entry(episode.mission_id.clone())
            .or_default()
            .push(idx);
        self.by_timestamp.push((episode.timestamp, idx));
        self.episodes.push(episode);
    }

    /// Get episodes by mission ID
    pub fn get_by_mission(&self, mission_id: &str) -> Vec<&Episode> {
        self.by_mission
            .get(mission_id)
            .map(|indices| indices.iter().map(|&i| &self.episodes[i]).collect())
            .unwrap_or_default()
    }

    /// Get episodes in time range
    pub fn get_in_range(&self, start: Timestamp, end: Timestamp) -> Vec<&Episode> {
        self.by_timestamp
            .iter()
            .filter(|(ts, _)| *ts >= start && *ts <= end)
            .map(|(_, i)| &self.episodes[*i])
            .collect()
    }

    /// Get most recent episodes
    pub fn get_recent(&self, n: usize) -> Vec<&Episode> {
        self.episodes
            .iter()
            .rev()
            .take(n)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    /// Get all episodes
    pub fn get_all(&self) -> Vec<&Episode> {
        self.episodes.iter().collect()
    }

    /// Get episode count
    pub fn len(&self) -> usize {
        self.episodes.len()
    }
}

impl Default for EpisodicMemory {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Semantic Memory
// ============================================================================

/// Semantic memory: Extracted patterns and learnings.
///
/// Abstracted knowledge derived from episodes and experiences.
/// Contains rules, patterns, schemas, and generalized knowledge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMemory {
    /// Learned patterns
    patterns: HashMap<String, Pattern>,
    /// Concept graph (simplified as adjacency list)
    concepts: HashMap<String, Concept>,
    /// Skill library
    skills: HashMap<String, Skill>,
}

impl SemanticMemory {
    /// Create new semantic memory
    pub fn new() -> Self {
        Self {
            patterns: HashMap::new(),
            concepts: HashMap::new(),
            skills: HashMap::new(),
        }
    }

    /// Extract and store a pattern from experience
    pub fn learn_pattern(&mut self, pattern: Pattern) {
        self.patterns.insert(pattern.id.clone(), pattern);
    }

    /// Get a pattern by ID
    pub fn get_pattern(&self, id: &str) -> Option<&Pattern> {
        self.patterns.get(id)
    }

    /// Find patterns matching a situation
    pub fn find_patterns(&self, context: &str) -> Vec<&Pattern> {
        let context_lower = context.to_lowercase();
        self.patterns
            .values()
            .filter(|p| {
                p.tags.iter().any(|t| context_lower.contains(t))
                    || p.description.to_lowercase().contains(&context_lower)
            })
            .collect()
    }

    /// Add or update a concept
    pub fn add_concept(&mut self, concept: Concept) {
        self.concepts.insert(concept.id.clone(), concept);
    }

    /// Get a concept by ID
    pub fn get_concept(&self, id: &str) -> Option<&Concept> {
        self.concepts.get(id)
    }

    /// Add a skill to the library
    pub fn add_skill(&mut self, skill: Skill) {
        self.skills.insert(skill.id.clone(), skill);
    }

    /// Get a skill by ID
    pub fn get_skill(&self, id: &str) -> Option<&Skill> {
        self.skills.get(id)
    }

    /// Get all skills
    pub fn get_all_skills(&self) -> Vec<&Skill> {
        self.skills.values().collect()
    }
}

impl Default for SemanticMemory {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Common Types
// ============================================================================

/// A single memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Timestamp when entry was created
    pub timestamp: Timestamp,
    /// Content of the memory
    pub content: String,
    /// Optional metadata
    pub metadata: Option<MemoryMetadata>,
}

/// Metadata for memory entries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetadata {
    /// Source of the memory (e.g., agent ID, mission ID)
    pub source: Option<String>,
    /// Importance/priority level
    pub importance: u8,
    /// Associated tags
    pub tags: Vec<String>,
}

/// A piece of knowledge in long-term memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    /// Unique identifier
    pub key: String,
    /// The knowledge content
    pub value: String,
    /// When it was created
    pub created_at: Timestamp,
    /// When it was last updated
    pub updated_at: Timestamp,
    /// Number of times accessed
    pub access_count: u64,
    /// Associated tags
    pub tags: Vec<String>,
}

/// An episode in episodic memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    /// Unique episode ID
    pub id: String,
    /// Associated mission ID
    pub mission_id: String,
    /// Timestamp of the episode
    pub timestamp: Timestamp,
    /// Episode title/summary
    pub title: String,
    /// Full episode content
    pub content: String,
    /// Participants (agent IDs)
    pub participants: Vec<String>,
    /// Outcome/result
    pub outcome: Option<String>,
    /// Lessons learned
    pub lessons: Vec<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
}

/// A learned pattern in semantic memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    /// Unique pattern ID
    pub id: String,
    /// Human-readable description
    pub description: String,
    /// Pattern type (rule, heuristic, schema, etc.)
    pub pattern_type: PatternType,
    /// Conditions for applying this pattern
    pub conditions: Vec<String>,
    /// Actions/conclusions when pattern matches
    pub conclusions: Vec<String>,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Tags for retrieval
    pub tags: Vec<String>,
    /// Source episode IDs that contributed to this pattern
    pub source_episodes: Vec<String>,
}

/// Type of pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternType {
    /// Conditional rule (if-then)
    Rule,
    /// Heuristic or rule of thumb
    Heuristic,
    /// Structural schema
    Schema,
    /// Procedural knowledge
    Procedure,
    /// Causal relationship
    Causal,
}

/// A concept in the semantic network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    /// Unique concept ID
    pub id: String,
    /// Concept name/label
    pub name: String,
    /// Definition/description
    pub definition: String,
    /// Related concept IDs with relationship types
    pub relations: Vec<(String, String)>,
    /// Examples of this concept
    pub examples: Vec<String>,
}

/// A learned skill or capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Unique skill ID
    pub id: String,
    /// Skill name
    pub name: String,
    /// Description of what this skill does
    pub description: String,
    /// Prerequisites (other skill IDs)
    pub prerequisites: Vec<String>,
    /// Steps/procedure
    pub steps: Vec<String>,
    /// Proficiency level (1-10)
    pub proficiency: u8,
}

// ============================================================================
// Unified Memory Interface
// ============================================================================

/// Complete memory system combining all four tiers.
///
/// This is the main interface that agents use to access memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySystem {
    /// Short-term memory for recent context
    pub short_term: ShortTermMemory,
    /// Long-term memory for persistent knowledge
    pub long_term: LongTermMemory,
    /// Episodic memory for interaction history
    pub episodic: EpisodicMemory,
    /// Semantic memory for learned patterns
    pub semantic: SemanticMemory,
}

impl MemorySystem {
    /// Create a new memory system with default capacities
    pub fn new(short_term_capacity: usize) -> Self {
        Self {
            short_term: ShortTermMemory::new(short_term_capacity),
            long_term: LongTermMemory::new(),
            episodic: EpisodicMemory::new(),
            semantic: SemanticMemory::new(),
        }
    }

    /// Create with custom configurations
    pub fn with_config(
        short_term_capacity: usize,
        long_term_path: Option<&Path>,
    ) -> MorphResult<Self> {
        let mut memory = Self::new(short_term_capacity);

        // Load long-term memory from disk if path provided
        if let Some(path) = long_term_path {
            if path.exists() {
                memory.long_term = LongTermMemory::load_from_path(path)?;
            }
        }

        Ok(memory)
    }

    /// Add to short-term memory
    pub fn add_short_term(&mut self, content: String, metadata: Option<MemoryMetadata>) {
        self.short_term.add(content, metadata);
    }

    /// Store in long-term memory
    pub fn store_long_term(&mut self, key: String, value: String, tags: Vec<String>) {
        self.long_term.store(key, value, tags);
    }

    /// Record an episode
    pub fn record_episode(&mut self, episode: Episode) {
        self.episodic.record(episode);
    }

    /// Learn a pattern
    pub fn learn_pattern(&mut self, pattern: Pattern) {
        self.semantic.learn_pattern(pattern);
    }

    /// Save long-term memory to disk
    pub fn save(&self, path: &Path) -> MorphResult<()> {
        self.long_term.save_to_path(path)
    }

    /// Load long-term memory from disk
    pub fn load(&mut self, path: &Path) -> MorphResult<()> {
        self.long_term = LongTermMemory::load_from_path(path)?;
        Ok(())
    }

    /// Get context for reasoning (combines relevant memories)
    pub fn get_context(&self, query: &str, recent_count: usize) -> MemoryContext {
        MemoryContext {
            recent: self.short_term.get_recent(recent_count),
            relevant_knowledge: self.long_term.search(query),
            relevant_patterns: self.semantic.find_patterns(query),
        }
    }
}

impl Default for MemorySystem {
    fn default() -> Self {
        Self::new(100)
    }
}

/// Context for reasoning -- snapshot of relevant memories
#[derive(Debug)]
pub struct MemoryContext<'a> {
    /// Recent short-term memories
    pub recent: Vec<&'a MemoryEntry>,
    /// Relevant long-term knowledge
    pub relevant_knowledge: Vec<&'a KnowledgeEntry>,
    /// Relevant semantic patterns
    pub relevant_patterns: Vec<&'a Pattern>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_term_memory_capacity() {
        let mut stm = ShortTermMemory::new(5);
        for i in 0..10 {
            stm.add(format!("entry {}", i), None);
        }
        assert_eq!(stm.len(), 5);
        let entries = stm.get_all();
        assert_eq!(entries[0].content, "entry 5");
        assert_eq!(entries[4].content, "entry 9");
    }

    #[test]
    fn test_long_term_memory_tags() {
        let mut ltm = LongTermMemory::new();
        ltm.store(
            "key1".to_string(),
            "value1".to_string(),
            vec!["tag1".to_string(), "tag2".to_string()],
        );
        ltm.store(
            "key2".to_string(),
            "value2".to_string(),
            vec!["tag1".to_string()],
        );

        let by_tag = ltm.retrieve_by_tag("tag1");
        assert_eq!(by_tag.len(), 2);
    }

    #[test]
    fn test_episodic_memory_retrieval() {
        let mut em = EpisodicMemory::new();
        em.record(Episode {
            id: "ep1".to_string(),
            mission_id: "mission1".to_string(),
            timestamp: 1000,
            title: "Test Episode".to_string(),
            content: "Test content".to_string(),
            participants: vec!["agent1".to_string()],
            outcome: Some("success".to_string()),
            lessons: vec![],
            tags: vec![],
        });

        let by_mission = em.get_by_mission("mission1");
        assert_eq!(by_mission.len(), 1);
    }

    #[test]
    fn test_memory_system_integration() {
        let mut ms = MemorySystem::new(10);

        // Add to short-term
        ms.add_short_term("Recent conversation".to_string(), None);

        // Store in long-term
        ms.store_long_term(
            "fact1".to_string(),
            "Important fact".to_string(),
            vec!["important".to_string()],
        );

        // Record episode
        ms.record_episode(Episode {
            id: "ep1".to_string(),
            mission_id: "m1".to_string(),
            timestamp: now(),
            title: "Test".to_string(),
            content: "Content".to_string(),
            participants: vec![],
            outcome: None,
            lessons: vec![],
            tags: vec![],
        });

        // Learn pattern
        ms.learn_pattern(Pattern {
            id: "p1".to_string(),
            description: "Test pattern".to_string(),
            pattern_type: PatternType::Rule,
            conditions: vec![],
            conclusions: vec![],
            confidence: 1.0,
            tags: vec!["test".to_string()],
            source_episodes: vec![],
        });

        assert_eq!(ms.short_term.len(), 1);
        assert!(ms.long_term.retrieve("fact1").is_some());
        assert_eq!(ms.episodic.len(), 1);
        assert!(ms.semantic.get_pattern("p1").is_some());
    }
}
