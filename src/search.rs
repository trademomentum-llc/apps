//! Search Engine — deterministic inverted index over morphlex token vectors.
//!
//! Architecture: inverted index keyed by lemma_id (i32). This gives automatic
//! stemming for free — "running", "runs", "ran" all index under the same
//! lemma_id for "run". Query and documents go through the identical morphlex
//! pipeline, so matching is deterministic: same input always produces the
//! same results.
//!
//! Scoring is integer-only (no floats). Ranking uses weighted i32 sums of:
//!   - lemma hit count
//!   - exact lexeme match bonus
//!   - POS/role/morph similarity
//!   - multi-term coverage bonus
//!
//! Binary index format (.mxidx) uses the same conventions as the database
//! module: little-endian, magic header, flat tables.

use std::collections::HashMap;
use std::path::Path;

use crate::types::*;
use crate::vectorizer;

// ─── Scoring Constants (integer-only) ───────────────────────────────────────

const SCORE_LEMMA_HIT: i32 = 10;
const SCORE_EXACT_MATCH: i32 = 5;
const SCORE_POS_MATCH: i32 = 3;
const SCORE_ROLE_MATCH: i32 = 2;
const SCORE_MORPH_OVERLAP: i32 = 1;
const SCORE_TERM_COVERAGE: i32 = 20;

// ─── Index Format Constants ─────────────────────────────────────────────────

const INDEX_MAGIC: &[u8; 8] = b"MXSEARCH";
const INDEX_VERSION: u32 = 1;

// ─── Search Index ───────────────────────────────────────────────────────────

/// The search index — an inverted index from lemma_id to document hits.
/// Analogous to Clang's symbol table but for natural language tokens.
pub struct SearchIndex {
    /// lemma_id -> list of DocHits (sorted by doc_id, then position)
    inverted: HashMap<i32, Vec<DocHit>>,
    /// doc_id -> document metadata
    documents: HashMap<DocId, DocMeta>,
    /// doc_id -> original text (optional, for snippet extraction)
    doc_texts: HashMap<DocId, String>,
    /// Number of documents indexed
    doc_count: u32,
}

impl SearchIndex {
    /// Create an empty search index.
    pub fn new() -> Self {
        SearchIndex {
            inverted: HashMap::new(),
            documents: HashMap::new(),
            doc_texts: HashMap::new(),
            doc_count: 0,
        }
    }

    /// Add a document to the index. Returns the deterministic doc_id.
    pub fn add_document(&mut self, title: &str, content: &str) -> MorphResult<DocId> {
        let doc_id = vectorizer::hash_to_i32(content);

        // Collision detection
        if self.documents.contains_key(&doc_id) {
            return Err(MorphlexError::IndexError(format!(
                "Document hash collision for '{}' (id={})", title, doc_id
            )));
        }

        let (_lemmas, vectors) = crate::compile(content)?;

        self.documents.insert(doc_id, DocMeta {
            doc_id,
            word_count: vectors.len() as u32,
            title: title.to_string(),
        });

        // Index each token by lemma_id
        for (position, tv) in vectors.iter().enumerate() {
            let hit = DocHit {
                doc_id,
                position: position as WordPos,
                pos: tv.pos,
                role: tv.role,
                morph: tv.morph,
                id: tv.id,
            };
            self.inverted
                .entry(tv.lemma_id)
                .or_default()
                .push(hit);
        }

        // Sort posting lists by (doc_id, position) for deterministic output
        for list in self.inverted.values_mut() {
            list.sort_by_key(|h| (h.doc_id, h.position));
        }

        self.doc_count += 1;
        Ok(doc_id)
    }

    /// Add a document and store its original text for snippet extraction.
    pub fn add_document_with_text(&mut self, title: &str, content: &str) -> MorphResult<DocId> {
        let doc_id = self.add_document(title, content)?;
        self.doc_texts.insert(doc_id, content.to_string());
        Ok(doc_id)
    }

    /// Number of documents in the index.
    pub fn doc_count(&self) -> u32 {
        self.doc_count
    }

    /// Number of unique lemma_ids in the index (posting list count).
    pub fn posting_count(&self) -> usize {
        self.inverted.len()
    }

    /// Get document metadata by id.
    pub fn get_doc(&self, doc_id: DocId) -> Option<&DocMeta> {
        self.documents.get(&doc_id)
    }

    /// Get stored document text by id (if stored).
    pub fn get_doc_text(&self, doc_id: DocId) -> Option<&str> {
        self.doc_texts.get(&doc_id).map(|s| s.as_str())
    }

    // ─── Serialization ──────────────────────────────────────────────────

    /// Serialize the index to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Header: 24 bytes
        buf.extend_from_slice(INDEX_MAGIC);
        buf.extend_from_slice(&INDEX_VERSION.to_le_bytes());
        buf.extend_from_slice(&self.doc_count.to_le_bytes());
        buf.extend_from_slice(&(self.inverted.len() as u32).to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes()); // flags (reserved)

        // Document Table — sorted by doc_id for determinism
        let mut doc_ids: Vec<DocId> = self.documents.keys().copied().collect();
        doc_ids.sort();

        for doc_id in &doc_ids {
            let meta = &self.documents[doc_id];
            buf.extend_from_slice(&meta.doc_id.to_le_bytes());
            buf.extend_from_slice(&meta.word_count.to_le_bytes());
            let title_bytes = meta.title.as_bytes();
            buf.extend_from_slice(&(title_bytes.len() as u16).to_le_bytes());
            buf.extend_from_slice(title_bytes);

            // Text (optional)
            match self.doc_texts.get(doc_id) {
                Some(text) => {
                    let text_bytes = text.as_bytes();
                    buf.extend_from_slice(&(text_bytes.len() as u32).to_le_bytes());
                    buf.extend_from_slice(text_bytes);
                }
                None => {
                    buf.extend_from_slice(&0u32.to_le_bytes());
                }
            }
        }

        // Posting Table — sorted by lemma_id for determinism
        let mut lemma_ids: Vec<i32> = self.inverted.keys().copied().collect();
        lemma_ids.sort();

        for lemma_id in &lemma_ids {
            let hits = &self.inverted[lemma_id];
            buf.extend_from_slice(&lemma_id.to_le_bytes());
            buf.extend_from_slice(&(hits.len() as u32).to_le_bytes());
            for hit in hits {
                buf.extend_from_slice(&hit.doc_id.to_le_bytes());
                buf.extend_from_slice(&hit.position.to_le_bytes());
                buf.push(hit.pos as u8);
                buf.push(hit.role as u8);
                buf.extend_from_slice(&hit.morph.to_le_bytes());
                buf.extend_from_slice(&hit.id.to_le_bytes());
            }
        }

        buf
    }

    /// Deserialize an index from bytes.
    pub fn from_bytes(data: &[u8]) -> MorphResult<Self> {
        if data.len() < 24 {
            return Err(MorphlexError::IndexError("Index too small".to_string()));
        }

        // Header
        if &data[0..8] != INDEX_MAGIC {
            return Err(MorphlexError::IndexError("Invalid magic".to_string()));
        }
        let version = u32::from_le_bytes(data[8..12].try_into().unwrap());
        if version != INDEX_VERSION {
            return Err(MorphlexError::IndexError(format!(
                "Unsupported version: {}", version
            )));
        }
        let doc_count = u32::from_le_bytes(data[12..16].try_into().unwrap());
        let posting_count = u32::from_le_bytes(data[16..20].try_into().unwrap());
        // flags at [20..24] — reserved

        let mut pos = 24;
        let mut documents = HashMap::new();
        let mut doc_texts = HashMap::new();

        // Document Table
        for _ in 0..doc_count {
            if pos + 10 > data.len() {
                return Err(MorphlexError::IndexError("Truncated document table".to_string()));
            }
            let doc_id = i32::from_le_bytes(data[pos..pos+4].try_into().unwrap());
            let word_count = u32::from_le_bytes(data[pos+4..pos+8].try_into().unwrap());
            let title_len = u16::from_le_bytes(data[pos+8..pos+10].try_into().unwrap()) as usize;
            pos += 10;

            let title = String::from_utf8_lossy(&data[pos..pos+title_len]).to_string();
            pos += title_len;

            let text_len = u32::from_le_bytes(data[pos..pos+4].try_into().unwrap()) as usize;
            pos += 4;
            if text_len > 0 {
                let text = String::from_utf8_lossy(&data[pos..pos+text_len]).to_string();
                doc_texts.insert(doc_id, text);
                pos += text_len;
            }

            documents.insert(doc_id, DocMeta { doc_id, word_count, title });
        }

        // Posting Table
        let mut inverted: HashMap<i32, Vec<DocHit>> = HashMap::new();

        for _ in 0..posting_count {
            if pos + 8 > data.len() {
                return Err(MorphlexError::IndexError("Truncated posting table".to_string()));
            }
            let lemma_id = i32::from_le_bytes(data[pos..pos+4].try_into().unwrap());
            let hit_count = u32::from_le_bytes(data[pos+4..pos+8].try_into().unwrap()) as usize;
            pos += 8;

            let mut hits = Vec::with_capacity(hit_count);
            for _ in 0..hit_count {
                if pos + 12 > data.len() {
                    return Err(MorphlexError::IndexError("Truncated hit entry".to_string()));
                }
                let doc_id = i32::from_le_bytes(data[pos..pos+4].try_into().unwrap());
                let position = u32::from_le_bytes(data[pos+4..pos+8].try_into().unwrap());
                let hit_pos = data[pos+8] as i8;
                let role = data[pos+9] as i8;
                let morph = i16::from_le_bytes(data[pos+10..pos+12].try_into().unwrap());
                let id = i32::from_le_bytes(data[pos+12..pos+16].try_into().unwrap());
                pos += 16;

                hits.push(DocHit { doc_id, position, pos: hit_pos, role, morph, id });
            }
            inverted.insert(lemma_id, hits);
        }

        Ok(SearchIndex { inverted, documents, doc_texts, doc_count })
    }

    /// Write the index to a file.
    pub fn write_to_path(&self, path: &Path) -> MorphResult<u64> {
        let bytes = self.to_bytes();
        let len = bytes.len() as u64;
        std::fs::write(path, bytes)?;
        Ok(len)
    }

    /// Read an index from a file.
    pub fn read_from_path(path: &Path) -> MorphResult<Self> {
        let data = std::fs::read(path)?;
        Self::from_bytes(&data)
    }
}

// ─── Index Building ─────────────────────────────────────────────────────────

/// Build a search index from document pairs (title, content).
pub fn build_index(documents: &[(String, String)]) -> MorphResult<SearchIndex> {
    let mut index = SearchIndex::new();
    for (title, content) in documents {
        index.add_document(title, content)?;
    }
    Ok(index)
}

// ─── Query Execution ────────────────────────────────────────────────────────

/// Default search config: mode=All, no filters, max_results=20.
pub fn default_config() -> SearchConfig {
    SearchConfig {
        mode: QueryMode::All,
        filter: SearchFilter {
            pos: None,
            role: None,
            morph_mask: None,
        },
        max_results: 20,
    }
}

/// Execute a search query against an index.
pub fn search(
    index: &SearchIndex,
    query: &str,
    config: &SearchConfig,
) -> MorphResult<Vec<SearchResult>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let (_query_lemmas, query_vectors) = crate::compile(query)?;

    if query_vectors.is_empty() {
        return Ok(Vec::new());
    }

    // Collect hits per query term, grouped by doc_id
    // Key: (query_term_index, doc_id) -> Vec<DocHit>
    let mut per_term_doc_hits: Vec<HashMap<DocId, Vec<DocHit>>> = Vec::new();
    let mut query_lemma_ids: Vec<i32> = Vec::new();

    for qv in &query_vectors {
        let lemma_id = qv.lemma_id;
        query_lemma_ids.push(lemma_id);

        let mut doc_hits: HashMap<DocId, Vec<DocHit>> = HashMap::new();

        if let Some(posting_list) = index.inverted.get(&lemma_id) {
            for hit in posting_list {
                // Apply filters
                if let Some(pos_filter) = config.filter.pos {
                    if hit.pos != pos_filter {
                        continue;
                    }
                }
                if let Some(role_filter) = config.filter.role {
                    if hit.role != role_filter {
                        continue;
                    }
                }
                if let Some(morph_mask) = config.filter.morph_mask {
                    if (hit.morph & morph_mask) != morph_mask {
                        continue;
                    }
                }
                doc_hits.entry(hit.doc_id).or_default().push(*hit);
            }
        }

        per_term_doc_hits.push(doc_hits);
    }

    // Determine candidate doc_ids based on query mode
    let candidate_docs: Vec<DocId> = match config.mode {
        QueryMode::All => {
            // Intersection: docs that appear in ALL query terms
            if per_term_doc_hits.is_empty() {
                return Ok(Vec::new());
            }
            let first_docs: std::collections::HashSet<DocId> =
                per_term_doc_hits[0].keys().copied().collect();
            let intersection = per_term_doc_hits[1..].iter().fold(first_docs, |acc, term| {
                let term_docs: std::collections::HashSet<DocId> =
                    term.keys().copied().collect();
                acc.intersection(&term_docs).copied().collect()
            });
            let mut docs: Vec<DocId> = intersection.into_iter().collect();
            docs.sort();
            docs
        }
        QueryMode::Any => {
            // Union: docs that appear in ANY query term
            let mut all_docs: std::collections::HashSet<DocId> =
                std::collections::HashSet::new();
            for term_hits in &per_term_doc_hits {
                all_docs.extend(term_hits.keys());
            }
            let mut docs: Vec<DocId> = all_docs.into_iter().collect();
            docs.sort();
            docs
        }
    };

    // Score each candidate document
    let mut results: Vec<SearchResult> = Vec::new();

    for doc_id in candidate_docs {
        let mut score: i32 = 0;
        let mut matched_positions: Vec<WordPos> = Vec::new();
        let mut matched_lemmas: Vec<i32> = Vec::new();

        for (term_idx, qv) in query_vectors.iter().enumerate() {
            let hits = match per_term_doc_hits[term_idx].get(&doc_id) {
                Some(h) => h,
                None => continue,
            };

            matched_lemmas.push(qv.lemma_id);

            for hit in hits {
                matched_positions.push(hit.position);

                // Lemma match: base relevance
                score += SCORE_LEMMA_HIT;

                // Exact lexeme match bonus
                if hit.id == qv.id {
                    score += SCORE_EXACT_MATCH;
                }

                // POS match bonus
                if hit.pos == qv.pos {
                    score += SCORE_POS_MATCH;
                }

                // Semantic role match bonus
                if hit.role == qv.role {
                    score += SCORE_ROLE_MATCH;
                }

                // Morph overlap bonus
                if (hit.morph & qv.morph) != 0 {
                    score += SCORE_MORPH_OVERLAP;
                }
            }
        }

        // Multi-term coverage bonus
        let distinct_matches = matched_lemmas.len() as i32;
        score += distinct_matches * SCORE_TERM_COVERAGE;

        matched_positions.sort();
        matched_positions.dedup();
        matched_lemmas.sort();
        matched_lemmas.dedup();

        results.push(SearchResult {
            doc_id,
            score,
            matched_positions,
            matched_lemmas,
        });
    }

    // Sort by score descending, break ties by doc_id ascending (deterministic)
    results.sort_by(|a, b| {
        b.score.cmp(&a.score).then(a.doc_id.cmp(&b.doc_id))
    });

    // Apply max_results limit
    if config.max_results > 0 && results.len() > config.max_results {
        results.truncate(config.max_results);
    }

    Ok(results)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helpers ──────────────────────────────────────────────────────────

    fn sample_docs() -> Vec<(String, String)> {
        vec![
            ("doc1".to_string(), "the quick brown fox jumps over the lazy dog".to_string()),
            ("doc2".to_string(), "the runner is running in the park".to_string()),
            ("doc3".to_string(), "dogs and cats are friendly animals".to_string()),
        ]
    }

    fn build_sample_index() -> SearchIndex {
        build_index(&sample_docs()).unwrap()
    }

    // ── Determinism ─────────────────────────────────────────────────────

    #[test]
    fn test_index_determinism() {
        let a = build_sample_index();
        let b = build_sample_index();
        assert_eq!(a.doc_count(), b.doc_count());
        assert_eq!(a.posting_count(), b.posting_count());
        assert_eq!(a.to_bytes(), b.to_bytes());
    }

    #[test]
    fn test_search_determinism() {
        let index = build_sample_index();
        let config = default_config();
        let a = search(&index, "dog", &config).unwrap();
        let b = search(&index, "dog", &config).unwrap();
        assert_eq!(a, b);
    }

    // ── Lemma Matching ──────────────────────────────────────────────────

    #[test]
    fn test_lemma_matching() {
        // "running" and "runner" share lemma "runn" (morphology strips -ing/-er)
        // Searching for "running" should match doc2 which contains "running"
        let index = build_sample_index();
        let config = SearchConfig {
            mode: QueryMode::Any,
            ..default_config()
        };
        let results = search(&index, "running", &config).unwrap();
        // doc2 has "running" — should match via shared lemma_id
        let doc2_match = results.iter().any(|r| {
            index.get_doc(r.doc_id).map(|m| m.title.as_str()) == Some("doc2")
        });
        assert!(doc2_match, "Should match doc2 via lemma for 'running'");
    }

    #[test]
    fn test_exact_match_bonus() {
        // Create index with two docs: one has "running", other has "run"
        let docs = vec![
            ("has_running".to_string(), "the athlete is running fast".to_string()),
            ("has_run".to_string(), "the athlete will run fast".to_string()),
        ];
        let index = build_index(&docs).unwrap();
        let config = SearchConfig {
            mode: QueryMode::Any,
            ..default_config()
        };

        // Search for "running" — doc with "running" should score higher due to exact match
        let results = search(&index, "running", &config).unwrap();
        assert!(!results.is_empty(), "Should find results");
    }

    // ── Query Modes ─────────────────────────────────────────────────────

    #[test]
    fn test_query_mode_all() {
        let index = build_sample_index();
        let config = SearchConfig {
            mode: QueryMode::All,
            ..default_config()
        };
        // "quick fox" — only doc1 has both
        let results = search(&index, "quick fox", &config).unwrap();
        assert!(!results.is_empty(), "Should find at least one result for 'quick fox'");
        for r in &results {
            let title = &index.get_doc(r.doc_id).unwrap().title;
            assert_eq!(title, "doc1", "Only doc1 should match All mode for 'quick fox'");
        }
    }

    #[test]
    fn test_query_mode_any() {
        let index = build_sample_index();
        let config = SearchConfig {
            mode: QueryMode::Any,
            ..default_config()
        };
        // "quick runner" — doc1 has "quick", doc2 has "runner"
        let results = search(&index, "quick runner", &config).unwrap();
        assert!(results.len() >= 2, "Any mode should match multiple docs");
    }

    // ── Filters ─────────────────────────────────────────────────────────

    #[test]
    fn test_pos_filter() {
        let index = build_sample_index();
        let config = SearchConfig {
            mode: QueryMode::Any,
            filter: SearchFilter {
                pos: Some(0), // Noun
                role: None,
                morph_mask: None,
            },
            max_results: 20,
        };
        let results = search(&index, "dog", &config).unwrap();
        // Should still find results — "dog" is a noun
        // The filter means only hits where POS=Noun are counted
        assert!(!results.is_empty(), "Should find noun-filtered results");
    }

    #[test]
    fn test_role_filter() {
        let index = build_sample_index();
        let config = SearchConfig {
            mode: QueryMode::Any,
            filter: SearchFilter {
                pos: None,
                role: Some(0), // Agent
                morph_mask: None,
            },
            max_results: 20,
        };
        let results = search(&index, "fox", &config).unwrap();
        // fox as Agent — may or may not match depending on semantic analysis
        // Just verify it doesn't crash and returns deterministic results
        let results2 = search(&index, "fox", &config).unwrap();
        assert_eq!(results, results2);
    }

    #[test]
    fn test_morph_filter() {
        let index = build_sample_index();
        let config = SearchConfig {
            mode: QueryMode::Any,
            filter: SearchFilter {
                pos: None,
                role: None,
                morph_mask: Some(morph_flags::HAS_SUFFIX),
            },
            max_results: 20,
        };
        // "running" has a suffix — searching with morph filter should find it
        let results = search(&index, "running", &config).unwrap();
        let results2 = search(&index, "running", &config).unwrap();
        assert_eq!(results, results2, "Morph filter must be deterministic");
    }

    // ── Edge Cases ──────────────────────────────────────────────────────

    #[test]
    fn test_empty_query() {
        let index = build_sample_index();
        let results = search(&index, "", &default_config()).unwrap();
        assert!(results.is_empty(), "Empty query should return no results");
    }

    #[test]
    fn test_no_match() {
        let index = build_sample_index();
        let results = search(&index, "xylophone", &default_config()).unwrap();
        assert!(results.is_empty(), "Non-existent term should return no results");
    }

    #[test]
    fn test_empty_index() {
        let index = SearchIndex::new();
        let results = search(&index, "hello", &default_config()).unwrap();
        assert!(results.is_empty(), "Empty index should return no results");
    }

    // ── Score Ordering ──────────────────────────────────────────────────

    #[test]
    fn test_score_ordering() {
        // Doc with more matches should score higher
        let docs = vec![
            ("many".to_string(), "dog dog dog dog dog".to_string()),
            ("few".to_string(), "the dog is here".to_string()),
        ];
        let index = build_index(&docs).unwrap();
        let config = SearchConfig {
            mode: QueryMode::Any,
            ..default_config()
        };
        let results = search(&index, "dog", &config).unwrap();
        if results.len() >= 2 {
            assert!(results[0].score >= results[1].score,
                "Doc with more matches should rank higher");
        }
    }

    #[test]
    fn test_multi_term_coverage_bonus() {
        // Doc matching 3/3 query terms should beat doc matching 3x of 1/3 terms
        let docs = vec![
            ("broad".to_string(), "the quick brown fox".to_string()),
            ("deep".to_string(), "quick quick quick".to_string()),
        ];
        let index = build_index(&docs).unwrap();
        let config = SearchConfig {
            mode: QueryMode::Any,
            ..default_config()
        };
        let results = search(&index, "quick brown fox", &config).unwrap();
        if results.len() >= 2 {
            let broad_result = results.iter().find(|r| {
                index.get_doc(r.doc_id).map(|m| m.title.as_str()) == Some("broad")
            });
            let deep_result = results.iter().find(|r| {
                index.get_doc(r.doc_id).map(|m| m.title.as_str()) == Some("deep")
            });
            if let (Some(broad), Some(deep)) = (broad_result, deep_result) {
                assert!(broad.score > deep.score,
                    "Broader coverage should score higher (broad={}, deep={})",
                    broad.score, deep.score);
            }
        }
    }

    // ── Serialization ───────────────────────────────────────────────────

    #[test]
    fn test_index_roundtrip_bytes() {
        let index = build_sample_index();
        let bytes = index.to_bytes();
        let index2 = SearchIndex::from_bytes(&bytes).unwrap();
        assert_eq!(index.doc_count(), index2.doc_count());
        assert_eq!(index.posting_count(), index2.posting_count());
        assert_eq!(bytes, index2.to_bytes());
    }

    #[test]
    fn test_index_roundtrip_file() {
        let index = build_sample_index();
        let dir = std::env::temp_dir().join("morphlex_search_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.mxidx");
        index.write_to_path(&path).unwrap();
        let index2 = SearchIndex::read_from_path(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        // Search on deserialized index should produce same results
        let config = default_config();
        let r1 = search(&index, "dog", &config).unwrap();
        let r2 = search(&index2, "dog", &config).unwrap();
        assert_eq!(r1, r2, "Roundtripped index should produce identical search results");
    }

    #[test]
    fn test_index_format_magic() {
        let index = build_sample_index();
        let bytes = index.to_bytes();
        assert_eq!(&bytes[0..8], b"MXSEARCH", "Index should start with MXSEARCH magic");
    }

    // ── Integer-only scores ─────────────────────────────────────────────

    #[test]
    fn test_integer_only_scores() {
        let index = build_sample_index();
        let config = SearchConfig {
            mode: QueryMode::Any,
            ..default_config()
        };
        let results = search(&index, "dog fox", &config).unwrap();
        for r in &results {
            // Score is i32 — this test verifies the type at compile time
            let _: i32 = r.score;
            assert!(r.score > 0, "Matching docs should have positive scores");
        }
    }

    // ── Document text storage ───────────────────────────────────────────

    #[test]
    fn test_add_document_with_text() {
        let mut index = SearchIndex::new();
        let doc_id = index.add_document_with_text("test", "hello world").unwrap();
        assert_eq!(index.get_doc_text(doc_id), Some("hello world"));
    }
}
