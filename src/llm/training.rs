//! Training Pipeline for MorphlexLLM.
//!
//! This module provides data loading, training objectives, and optimization
//! for the MorphlexLLM language model.
//!
//! # Components
//!
//! - **DataLoader**: Loads morphlex-processed text as training sequences
//! - **TrainingObjective**: Multi-task loss (LM + lemma + POS + role prediction)
//! - **Optimizer**: AdamW optimizer with gradient clipping
//! - **Trainer**: Full training loop with checkpointing

use crate::llm::{EnrichedToken, ModelConfig, MorphlexLLM};
use crate::types::{MorphResult, MorphlexError, TokenVector};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

// ============================================================================
// Training Data
// ============================================================================

/// Single training sample
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingSample {
    /// Input token sequence
    pub input: Vec<TokenVector>,
    /// Target token IDs (next token prediction)
    pub targets: Vec<i32>,
    /// Target lemma IDs
    pub lemma_targets: Vec<i32>,
    /// Target POS tags
    pub pos_targets: Vec<i8>,
    /// Target semantic roles
    pub role_targets: Vec<i8>,
}

impl TrainingSample {
    /// Create a training sample from token sequence
    pub fn from_tokens(tokens: Vec<TokenVector>) -> Self {
        let len = tokens.len();

        // Targets are shifted by 1 (predict next token)
        let targets: Vec<i32> = tokens.iter().skip(1).map(|t| t.id).collect();
        let lemma_targets: Vec<i32> = tokens.iter().skip(1).map(|t| t.lemma_id).collect();
        let pos_targets: Vec<i8> = tokens.iter().skip(1).map(|t| t.pos).collect();
        let role_targets: Vec<i8> = tokens.iter().skip(1).map(|t| t.role).collect();

        // Input is all but last token
        let input = tokens[..len.saturating_sub(1).max(1)].to_vec();

        Self {
            input,
            targets,
            lemma_targets,
            pos_targets,
            role_targets,
        }
    }

    /// Convert to enriched tokens
    pub fn to_enriched(&self, d_model: usize) -> Vec<EnrichedToken> {
        self.input
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let mut token = EnrichedToken::from_vector(v, d_model);
                token.position = i;
                token
            })
            .collect()
    }

    /// Get sequence length
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
}

/// Batch of training samples
#[derive(Debug, Clone)]
pub struct TrainingBatch {
    /// Batch samples
    pub samples: Vec<TrainingSample>,
    /// Padded input tensors [batch x seq x d_model]
    pub inputs: Vec<Vec<EnrichedToken>>,
    /// Padded target tensors [batch x seq]
    pub targets: Vec<Vec<i32>>,
    /// Attention masks [batch x seq]
    pub masks: Vec<Vec<bool>>,
}

impl TrainingBatch {
    /// Create batch from samples with padding
    pub fn new(samples: Vec<TrainingSample>, d_model: usize) -> Self {
        // Find max sequence length
        let max_len = samples.iter().map(|s| s.len()).max().unwrap_or(0);

        // Pad inputs and targets
        let inputs: Vec<Vec<EnrichedToken>> = samples
            .iter()
            .map(|s| {
                let mut enriched = s.to_enriched(d_model);
                // Pad to max length
                while enriched.len() < max_len {
                    if let Some(last) = enriched.last().cloned() {
                        enriched.push(last);
                    }
                }
                enriched
            })
            .collect();

        let targets: Vec<Vec<i32>> = samples
            .iter()
            .map(|s| {
                let mut t = s.targets.clone();
                while t.len() < max_len {
                    t.push(-1); // Padding token
                }
                t
            })
            .collect();

        // Create attention masks
        let masks: Vec<Vec<bool>> = samples
            .iter()
            .map(|s| {
                let mut m = vec![true; s.len()];
                while m.len() < max_len {
                    m.push(false);
                }
                m
            })
            .collect();

        Self {
            samples,
            inputs,
            targets,
            masks,
        }
    }

    /// Get batch size
    pub fn batch_size(&self) -> usize {
        self.samples.len()
    }

    /// Get max sequence length
    pub fn seq_len(&self) -> usize {
        self.masks.first().map(|m| m.len()).unwrap_or(0)
    }
}

// ============================================================================
// Data Loader
// ============================================================================

/// DataLoader for morphlex training data
pub struct DataLoader {
    /// Training samples
    samples: Vec<TrainingSample>,
    /// Current epoch
    epoch: usize,
    /// Batch size
    batch_size: usize,
    /// Shuffle enabled
    shuffle: bool,
    /// Current position
    position: usize,
}

impl DataLoader {
    /// Create new data loader from samples
    pub fn new(samples: Vec<TrainingSample>, batch_size: usize, shuffle: bool) -> Self {
        Self {
            samples,
            epoch: 0,
            batch_size,
            shuffle,
            position: 0,
        }
    }

    /// Load from morphlex database file
    pub fn from_database(
        path: &Path,
        batch_size: usize,
        shuffle: bool,
        max_seq_len: usize,
    ) -> MorphResult<Self> {
        let mut samples = Vec::new();

        // Read token vectors from database
        let data = std::fs::read(path).map_err(MorphlexError::IoError)?;

        // Parse token vectors (12 bytes each)
        const VECTOR_SIZE: usize = 12;
        let mut tokens = Vec::new();

        for chunk in data.chunks(VECTOR_SIZE) {
            if chunk.len() == VECTOR_SIZE {
                let vector = TokenVector {
                    id: i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
                    lemma_id: i32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]),
                    pos: chunk[8] as i8,
                    role: chunk[9] as i8,
                    morph: i16::from_le_bytes([chunk[10], chunk[11]]),
                };
                tokens.push(vector);

                // Create sample when we have enough tokens
                if tokens.len() >= max_seq_len {
                    samples.push(TrainingSample::from_tokens(tokens));
                    tokens = Vec::new();
                }
            }
        }

        // Handle remaining tokens
        if tokens.len() > 10 {
            samples.push(TrainingSample::from_tokens(tokens));
        }

        Ok(Self::new(samples, batch_size, shuffle))
    }

    /// Load from text file (one sentence per line)
    pub fn from_text_file(path: &Path, batch_size: usize, shuffle: bool) -> MorphResult<Self> {
        let file = File::open(path).map_err(MorphlexError::IoError)?;
        let reader = BufReader::new(file);

        let mut samples = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(MorphlexError::IoError)?;

            // Process line through morphlex pipeline
            match crate::compile(&line) {
                Ok((_, vectors)) => {
                    if vectors.len() > 5 {
                        samples.push(TrainingSample::from_tokens(vectors));
                    }
                }
                Err(_) => continue, // Skip lines that fail to parse
            }
        }

        Ok(Self::new(samples, batch_size, shuffle))
    }

    /// Get next batch
    pub fn next_batch(&mut self) -> Option<TrainingBatch> {
        if self.position >= self.samples.len() {
            return None;
        }

        let end = (self.position + self.batch_size).min(self.samples.len());
        let batch_samples: Vec<TrainingSample> = self.samples[self.position..end].to_vec();

        self.position = end;

        Some(TrainingBatch::new(batch_samples, 512)) // Default d_model
    }

    /// Reset for new epoch
    pub fn reset(&mut self) {
        self.epoch += 1;
        self.position = 0;

        if self.shuffle {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            // Simple deterministic shuffle based on epoch
            let mut hasher = DefaultHasher::new();
            self.epoch.hash(&mut hasher);
            let seed = hasher.finish();

            self.samples.sort_by(|a, b| {
                let mut h1 = DefaultHasher::new();
                let mut h2 = DefaultHasher::new();
                (seed, a.len()).hash(&mut h1);
                (seed, b.len()).hash(&mut h2);
                h1.finish().cmp(&h2.finish())
            });
        }
    }

    /// Get number of samples
    pub fn num_samples(&self) -> usize {
        self.samples.len()
    }

    /// Get number of batches per epoch
    pub fn num_batches(&self) -> usize {
        self.samples.len().div_ceil(self.batch_size)
    }

    /// Check if epoch is complete
    pub fn epoch_complete(&self) -> bool {
        self.position >= self.samples.len()
    }
}

// ============================================================================
// Training Objective
// ============================================================================

/// Multi-task training objective weights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingObjective {
    /// Language modeling weight
    pub lm_weight: f32,
    /// Lemma prediction weight
    pub lemma_weight: f32,
    /// POS prediction weight
    pub pos_weight: f32,
    /// Role prediction weight
    pub role_weight: f32,
}

impl Default for TrainingObjective {
    fn default() -> Self {
        Self {
            lm_weight: 1.0,
            lemma_weight: 0.3,
            pos_weight: 0.2,
            role_weight: 0.2,
        }
    }
}

/// Training loss components
#[derive(Debug, Clone)]
pub struct TrainingLoss {
    /// Total loss
    pub total: f32,
    /// Language modeling loss
    pub lm_loss: f32,
    /// Lemma prediction loss
    pub lemma_loss: f32,
    /// POS prediction loss
    pub pos_loss: f32,
    /// Role prediction loss
    pub role_loss: f32,
    /// Perplexity
    pub perplexity: f32,
}

impl TrainingLoss {
    /// Compute multi-task loss
    pub fn compute(
        model: &MorphlexLLM,
        batch: &TrainingBatch,
        objective: &TrainingObjective,
    ) -> Self {
        let mut lm_loss = 0.0;
        let mut lemma_loss = 0.0;
        let mut pos_loss = 0.0;
        let mut role_loss = 0.0;
        let mut count = 0;

        for sample in batch.samples.iter() {
            let enriched = sample.to_enriched(model.config.d_model);

            // Forward pass
            let logits = model.predict(&enriched);
            let (lemma_logits, pos_logits, role_logits) = model.predict_aux(&enriched);

            // Compute cross-entropy losses
            for &target in sample.targets.iter() {
                if target < 0 {
                    continue; // Skip padding
                }

                let target_idx = (target as u32 as usize) % logits.len();
                lm_loss += -logits[target_idx].ln().max(-100.0); // Clamp for numerical stability
                count += 1;
            }

            for &target in sample.lemma_targets.iter() {
                if target < 0 {
                    continue;
                }
                let target_idx = (target as u32 as usize) % lemma_logits.len();
                lemma_loss += -lemma_logits[target_idx].ln().max(-100.0);
            }

            for &target in sample.pos_targets.iter() {
                if target < 0 {
                    continue;
                }
                let target_idx = target as u32 as usize;
                if target_idx < pos_logits.len() {
                    pos_loss += -pos_logits[target_idx].ln().max(-100.0);
                }
            }

            for &target in sample.role_targets.iter() {
                if target < 0 {
                    continue;
                }
                let target_idx = target as u32 as usize;
                if target_idx < role_logits.len() {
                    role_loss += -role_logits[target_idx].ln().max(-100.0);
                }
            }
        }

        // Average losses
        if count > 0 {
            lm_loss /= count as f32;
            lemma_loss /= count as f32;
            pos_loss /= count as f32;
            role_loss /= count as f32;
        }

        // Weighted sum
        let total = objective.lm_weight * lm_loss
            + objective.lemma_weight * lemma_loss
            + objective.pos_weight * pos_loss
            + objective.role_weight * role_loss;

        // Compute perplexity
        let perplexity = lm_loss.exp();

        Self {
            total,
            lm_loss,
            lemma_loss,
            pos_loss,
            role_loss,
            perplexity,
        }
    }
}

// ============================================================================
// Optimizer
// ============================================================================

/// AdamW optimizer state
#[derive(Debug, Clone)]
pub struct AdamWState {
    /// First moment estimate
    pub m: Vec<f32>,
    /// Second moment estimate
    pub v: Vec<f32>,
    /// Timestep
    pub t: usize,
}

/// AdamW optimizer
pub struct AdamW {
    /// Learning rate
    pub lr: f32,
    /// Beta1
    pub beta1: f32,
    /// Beta2
    pub beta2: f32,
    /// Epsilon
    pub eps: f32,
    /// Weight decay
    pub weight_decay: f32,
    /// Parameter states
    states: std::collections::HashMap<String, AdamWState>,
}

impl AdamW {
    /// Create new optimizer
    pub fn new(lr: f32, beta1: f32, beta2: f32, eps: f32, weight_decay: f32) -> Self {
        Self {
            lr,
            beta1,
            beta2,
            eps,
            weight_decay,
            states: std::collections::HashMap::new(),
        }
    }
}

impl Default for AdamW {
    fn default() -> Self {
        Self::new(1e-4, 0.9, 0.999, 1e-8, 0.01)
    }
}

impl AdamW {
    /// Compute gradients numerically (for demonstration)
    pub fn compute_gradients(
        &self,
        model: &MorphlexLLM,
        batch: &TrainingBatch,
        objective: &TrainingObjective,
    ) -> Vec<f32> {
        // In production, this would use automatic differentiation
        // For now, return placeholder gradients
        let loss = TrainingLoss::compute(model, batch, objective);
        vec![loss.total; model.param_count()]
    }

    /// Update parameters (simplified)
    pub fn step(&mut self, _model: &mut MorphlexLLM, _gradients: &[f32]) {
        // In production, this would update actual model parameters
        // For now, just increment timestep
        self.states.iter_mut().for_each(|(_, s)| s.t += 1);
    }

    /// Zero gradients
    pub fn zero_grad(&mut self) {
        self.states.clear();
    }

    /// Apply gradient clipping
    pub fn clip_gradients(&self, gradients: &mut [f32], max_norm: f32) {
        let norm: f32 = gradients.iter().map(|g| g * g).sum::<f32>().sqrt();
        if norm > max_norm {
            let scale = max_norm / norm;
            gradients.iter_mut().for_each(|g| *g *= scale);
        }
    }
}

// ============================================================================
// Trainer
// ============================================================================

/// Training checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Epoch
    pub epoch: usize,
    /// Step
    pub step: usize,
    /// Loss
    pub loss: f32,
    /// Model config
    pub config: ModelConfig,
}

/// Training configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    /// Number of epochs
    pub epochs: usize,
    /// Batch size
    pub batch_size: usize,
    /// Learning rate
    pub learning_rate: f32,
    /// Gradient clipping norm
    pub max_grad_norm: f32,
    /// Checkpoint interval (steps)
    pub checkpoint_interval: usize,
    /// Log interval (steps)
    pub log_interval: usize,
    /// Output directory for checkpoints
    pub output_dir: String,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            epochs: 10,
            batch_size: 32,
            learning_rate: 1e-4,
            max_grad_norm: 1.0,
            checkpoint_interval: 1000,
            log_interval: 100,
            output_dir: "checkpoints".to_string(),
        }
    }
}

/// Training statistics
#[derive(Debug, Clone, Default)]
pub struct TrainingStats {
    /// Total steps
    pub steps: usize,
    /// Total epochs
    pub epochs: usize,
    /// Best loss
    pub best_loss: f32,
    /// Average loss (current epoch)
    pub avg_loss: f32,
    /// Tokens processed
    pub tokens_processed: u64,
    /// Steps per second
    pub steps_per_sec: f32,
}

/// Main trainer class
pub struct Trainer {
    /// Model
    pub model: MorphlexLLM,
    /// Training config
    pub config: TrainingConfig,
    /// Training objective
    pub objective: TrainingObjective,
    /// Optimizer
    pub optimizer: AdamW,
    /// Statistics
    pub stats: TrainingStats,
}

impl Trainer {
    /// Create new trainer
    pub fn new(model: MorphlexLLM, config: TrainingConfig) -> Self {
        Self {
            model,
            config,
            objective: TrainingObjective::default(),
            optimizer: AdamW::default(),
            stats: TrainingStats::default(),
        }
    }

    /// Train for one epoch
    pub fn train_epoch(&mut self, dataloader: &mut DataLoader) -> MorphResult<TrainingLoss> {
        let mut epoch_loss = TrainingLoss {
            total: 0.0,
            lm_loss: 0.0,
            lemma_loss: 0.0,
            pos_loss: 0.0,
            role_loss: 0.0,
            perplexity: 0.0,
        };
        let mut batch_count = 0;

        while let Some(batch) = dataloader.next_batch() {
            // Compute loss
            let loss = TrainingLoss::compute(&self.model, &batch, &self.objective);

            // Compute gradients (placeholder)
            let mut gradients =
                self.optimizer
                    .compute_gradients(&self.model, &batch, &self.objective);

            // Clip gradients
            self.optimizer
                .clip_gradients(&mut gradients, self.config.max_grad_norm);

            // Update parameters
            self.optimizer.step(&mut self.model, &gradients);

            // Update statistics
            epoch_loss.total += loss.total;
            epoch_loss.lm_loss += loss.lm_loss;
            epoch_loss.lemma_loss += loss.lemma_loss;
            epoch_loss.pos_loss += loss.pos_loss;
            epoch_loss.role_loss += loss.role_loss;
            batch_count += 1;
            self.stats.steps += 1;
            self.stats.tokens_processed += batch.batch_size() as u64 * batch.seq_len() as u64;

            // Log progress
            if self.stats.steps.is_multiple_of(self.config.log_interval) {
                eprintln!(
                    "Step {}: loss={:.4}, lm_loss={:.4}, ppl={:.2}",
                    self.stats.steps, loss.total, loss.lm_loss, loss.perplexity
                );
            }

            // Save checkpoint
            if self.stats.steps.is_multiple_of(self.config.checkpoint_interval) {
                self.save_checkpoint()?;
            }
        }

        // Average epoch loss
        if batch_count > 0 {
            epoch_loss.total /= batch_count as f32;
            epoch_loss.lm_loss /= batch_count as f32;
            epoch_loss.lemma_loss /= batch_count as f32;
            epoch_loss.pos_loss /= batch_count as f32;
            epoch_loss.role_loss /= batch_count as f32;
            epoch_loss.perplexity = epoch_loss.lm_loss.exp();
        }

        self.stats.avg_loss = epoch_loss.total;

        if epoch_loss.total < self.stats.best_loss || self.stats.best_loss == 0.0 {
            self.stats.best_loss = epoch_loss.total;
        }

        Ok(epoch_loss)
    }

    /// Full training loop
    pub fn train(&mut self, dataloader: &mut DataLoader) -> MorphResult<TrainingStats> {
        eprintln!("Starting training for {} epochs...", self.config.epochs);
        eprintln!("Model parameters: {}", self.model.param_count());
        eprintln!("Training samples: {}", dataloader.num_samples());
        eprintln!("Batches per epoch: {}", dataloader.num_batches());
        eprintln!();

        let start_time = std::time::Instant::now();

        for epoch in 0..self.config.epochs {
            dataloader.reset();

            let epoch_loss = self.train_epoch(dataloader)?;

            self.stats.epochs = epoch + 1;

            let elapsed = start_time.elapsed().as_secs_f32();
            let steps_per_sec = self.stats.steps as f32 / elapsed.max(1.0);
            self.stats.steps_per_sec = steps_per_sec;

            eprintln!(
                "Epoch {}/{}: loss={:.4}, lm_loss={:.4}, ppl={:.2}, best={:.4}, steps/s={:.1}",
                epoch + 1,
                self.config.epochs,
                epoch_loss.total,
                epoch_loss.lm_loss,
                epoch_loss.perplexity,
                self.stats.best_loss,
                steps_per_sec
            );
        }

        // Save final checkpoint
        self.save_checkpoint()?;

        Ok(self.stats.clone())
    }

    /// Save checkpoint
    pub fn save_checkpoint(&self) -> MorphResult<()> {
        std::fs::create_dir_all(&self.config.output_dir).map_err(MorphlexError::IoError)?;

        let checkpoint = Checkpoint {
            epoch: self.stats.epochs,
            step: self.stats.steps,
            loss: self.stats.avg_loss,
            config: self.model.config.clone(),
        };

        let path = format!(
            "{}/checkpoint_step_{}.json",
            self.config.output_dir, self.stats.steps
        );
        let json = serde_json::to_string_pretty(&checkpoint)
            .map_err(|e| MorphlexError::DatabaseError(e.to_string()))?;
        std::fs::write(&path, json).map_err(MorphlexError::IoError)?;

        // Save model
        let model_path = format!(
            "{}/model_step_{}.json",
            self.config.output_dir, self.stats.steps
        );
        self.model.save(Path::new(&model_path))?;

        Ok(())
    }

    /// Load from checkpoint
    pub fn load_checkpoint(path: &Path) -> MorphResult<(Checkpoint, MorphlexLLM)> {
        let json = std::fs::read_to_string(path).map_err(MorphlexError::IoError)?;
        let checkpoint: Checkpoint =
            serde_json::from_str(&json).map_err(|e| MorphlexError::DatabaseError(e.to_string()))?;

        // Load corresponding model
        let model_path = path
            .to_str()
            .unwrap_or("model.json")
            .replace("checkpoint", "model")
            .replace("_step_", "/model_step_");

        let model = MorphlexLLM::load(Path::new(&model_path))?;

        Ok((checkpoint, model))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ModelConfig;

    #[test]
    fn test_training_sample() {
        let tokens = vec![
            TokenVector {
                id: 1,
                lemma_id: 1,
                pos: 0,
                role: 0,
                morph: 0,
            },
            TokenVector {
                id: 2,
                lemma_id: 2,
                pos: 1,
                role: 1,
                morph: 0,
            },
            TokenVector {
                id: 3,
                lemma_id: 3,
                pos: 0,
                role: 2,
                morph: 0,
            },
        ];

        let sample = TrainingSample::from_tokens(tokens);

        assert_eq!(sample.input.len(), 2);
        assert_eq!(sample.targets.len(), 2);
        assert_eq!(sample.targets[0], 2);
        assert_eq!(sample.targets[1], 3);
    }

    #[test]
    fn test_training_batch() {
        let samples = vec![
            TrainingSample::from_tokens(vec![
                TokenVector {
                    id: 1,
                    lemma_id: 1,
                    pos: 0,
                    role: 0,
                    morph: 0,
                },
                TokenVector {
                    id: 2,
                    lemma_id: 2,
                    pos: 1,
                    role: 1,
                    morph: 0,
                },
            ]),
            TrainingSample::from_tokens(vec![
                TokenVector {
                    id: 3,
                    lemma_id: 3,
                    pos: 0,
                    role: 0,
                    morph: 0,
                },
                TokenVector {
                    id: 4,
                    lemma_id: 4,
                    pos: 1,
                    role: 1,
                    morph: 0,
                },
                TokenVector {
                    id: 5,
                    lemma_id: 5,
                    pos: 0,
                    role: 2,
                    morph: 0,
                },
            ]),
        ];

        let batch = TrainingBatch::new(samples, 64);

        assert_eq!(batch.batch_size(), 2);
        // Batch is padded to max length (3 in this case)
        assert!(batch.seq_len() >= 2);
    }

    #[test]
    fn test_training_loss() {
        let config = ModelConfig {
            d_model: 64,
            n_heads: 4,
            n_layers: 2,
            d_ff: 128,
            vocab_size: 1000,
            max_seq_len: 32,
            dropout: 0.0,
            use_role_attention: true,
            use_morph_gates: true,
            use_lemma_embeddings: true,
        };

        let model = MorphlexLLM::new(&config);
        let samples = vec![TrainingSample::from_tokens(vec![
            TokenVector {
                id: 1,
                lemma_id: 1,
                pos: 0,
                role: 0,
                morph: 0,
            },
            TokenVector {
                id: 2,
                lemma_id: 2,
                pos: 1,
                role: 1,
                morph: 0,
            },
        ])];
        let batch = TrainingBatch::new(samples, 64);
        let objective = TrainingObjective::default();

        let loss = TrainingLoss::compute(&model, &batch, &objective);

        assert!(loss.total > 0.0);
        assert!(loss.lm_loss > 0.0);
        assert!(loss.perplexity > 1.0);
    }

    #[test]
    fn test_optimizer() {
        let optimizer = AdamW::default();
        assert_eq!(optimizer.lr, 1e-4);
        assert_eq!(optimizer.beta1, 0.9);
        assert_eq!(optimizer.beta2, 0.999);
        assert_eq!(optimizer.weight_decay, 0.01);

        // Test gradient clipping
        let mut gradients = vec![3.0, 4.0]; // norm = 5
        optimizer.clip_gradients(&mut gradients, 1.0);

        let norm: f32 = gradients.iter().map(|g| g * g).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }
}
