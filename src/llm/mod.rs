//! MorphlexLLM - Custom language model using morphlex vectors.
//!
//! This module implements a transformer-based language model that leverages
//! morphlex's 12-byte integer token vectors for efficient, deterministic NLP.
//!
//! # Architecture
//!
//! ```text
//! TokenVector (12 bytes) → Embedding Projection → Transformer Encoder → Output Head
//! ```

pub mod gguf;
pub mod training;

pub use gguf::*;
pub use training::*;

use crate::types::{MorphResult, MorphlexError, TokenVector};
use serde::{Deserialize, Serialize};

// ============================================================================
// Configuration
// ============================================================================

/// Model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    // Model dimensions
    pub d_model: usize,
    pub n_heads: usize,
    pub n_layers: usize,
    pub d_ff: usize,

    // Vocabulary
    pub vocab_size: usize,
    pub max_seq_len: usize,

    // Dropout
    pub dropout: f32,

    // Features
    pub use_role_attention: bool,
    pub use_morph_gates: bool,
    pub use_lemma_embeddings: bool,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            d_model: 512,
            n_heads: 8,
            n_layers: 6,
            d_ff: 2048,
            vocab_size: 50000,
            max_seq_len: 512,
            dropout: 0.1,
            use_role_attention: true,
            use_morph_gates: true,
            use_lemma_embeddings: true,
        }
    }
}

impl ModelConfig {
    /// Create a small model config
    pub fn small() -> Self {
        Self {
            d_model: 512,
            n_heads: 8,
            n_layers: 6,
            d_ff: 2048,
            ..Default::default()
        }
    }

    /// Create a medium model config
    pub fn medium() -> Self {
        Self {
            d_model: 1024,
            n_heads: 16,
            n_layers: 12,
            d_ff: 4096,
            ..Default::default()
        }
    }

    /// Create a large model config
    pub fn large() -> Self {
        Self {
            d_model: 2048,
            n_heads: 16,
            n_layers: 24,
            d_ff: 8192,
            ..Default::default()
        }
    }

    /// Get estimated parameter count
    pub fn param_count(&self) -> usize {
        // Embedding: vocab * d_model
        let emb = self.vocab_size * self.d_model;
        // Lemma embedding: vocab * d_model
        let lemma_emb = if self.use_lemma_embeddings {
            self.vocab_size * self.d_model
        } else {
            0
        };
        // Transformer layers: 4 * d_model^2 per layer (attention + FFN)
        let transformer =
            self.n_layers * (4 * self.d_model * self.d_model + 2 * self.d_model * self.d_ff);
        // Output projection: d_model * vocab
        let output = self.d_model * self.vocab_size;

        emb + lemma_emb + transformer + output
    }
}

// ============================================================================
// Token Representation
// ============================================================================

/// Enriched token with embedding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedToken {
    /// Original morphlex vector
    pub vector: TokenVector,
    /// Learned embedding
    pub embedding: Vec<f32>,
    /// Sequence position
    pub position: usize,
    /// Attention mask (false = padding)
    pub attention_mask: bool,
}

impl EnrichedToken {
    /// Create from token vector
    pub fn from_vector(vector: TokenVector, d_model: usize) -> Self {
        Self {
            embedding: vec![0.0; d_model],
            position: 0,
            attention_mask: true,
            vector,
        }
    }

    /// Get POS embedding index
    pub fn pos_index(&self) -> usize {
        self.vector.pos as usize
    }

    /// Get semantic role index
    pub fn role_index(&self) -> usize {
        self.vector.role as usize
    }

    /// Get morphological flags
    pub fn morph_flags(&self) -> u16 {
        self.vector.morph as u16
    }
}

// ============================================================================
// Embedding Layer
// ============================================================================

/// Token embedding projector
/// Projects 12-byte morphlex vectors to d_model dimensional embeddings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingProjector {
    /// Token embedding matrix [vocab_size x d_model]
    pub token_embeddings: Vec<Vec<f32>>,
    /// Lemma embedding matrix [vocab_size x d_model]
    pub lemma_embeddings: Option<Vec<Vec<f32>>>,
    /// POS embedding matrix [10 x d_model]
    pub pos_embeddings: Vec<Vec<f32>>,
    /// Role embedding matrix [9 x d_model]
    pub role_embeddings: Vec<Vec<f32>>,
    /// Position embedding matrix [max_seq_len x d_model]
    pub position_embeddings: Vec<Vec<f32>>,
    /// Layer normalization weights
    pub layer_norm_weight: Vec<f32>,
    /// Layer normalization bias
    pub layer_norm_bias: Vec<f32>,
}

impl EmbeddingProjector {
    /// Create new embedding projector
    pub fn new(config: &ModelConfig) -> Self {
        Self {
            token_embeddings: Self::init_matrix(config.vocab_size, config.d_model),
            lemma_embeddings: if config.use_lemma_embeddings {
                Some(Self::init_matrix(config.vocab_size, config.d_model))
            } else {
                None
            },
            pos_embeddings: Self::init_matrix(10, config.d_model),
            role_embeddings: Self::init_matrix(9, config.d_model),
            position_embeddings: Self::init_matrix(config.max_seq_len, config.d_model),
            layer_norm_weight: vec![1.0; config.d_model],
            layer_norm_bias: vec![0.0; config.d_model],
        }
    }

    /// Initialize matrix with small random values
    fn init_matrix(rows: usize, cols: usize) -> Vec<Vec<f32>> {
        (0..rows)
            .map(|_| (0..cols).map(|_| 0.01).collect())
            .collect()
    }

    /// Project token vector to embedding
    pub fn project(&self, token: &EnrichedToken) -> Vec<f32> {
        let d_model = self.layer_norm_weight.len();
        let mut embedding = vec![0.0; d_model];

        // Add token embedding (by ID hash)
        let token_idx = (token.vector.id.abs() as usize) % self.token_embeddings.len();
        for (i, &val) in self.token_embeddings[token_idx].iter().enumerate() {
            embedding[i] += val;
        }

        // Add lemma embedding
        if let Some(ref lemma_emb) = self.lemma_embeddings {
            let lemma_idx = (token.vector.lemma_id.abs() as usize) % lemma_emb.len();
            for (i, &val) in lemma_emb[lemma_idx].iter().enumerate() {
                embedding[i] += val * 0.5; // Weighted contribution
            }
        }

        // Add POS embedding
        let pos_idx = token.pos_index().min(9);
        for (i, &val) in self.pos_embeddings[pos_idx].iter().enumerate() {
            embedding[i] += val * 0.3;
        }

        // Add role embedding
        let role_idx = token.role_index().min(8);
        for (i, &val) in self.role_embeddings[role_idx].iter().enumerate() {
            embedding[i] += val * 0.3;
        }

        // Add position embedding
        let pos_idx = token.position.min(self.position_embeddings.len() - 1);
        for (i, &val) in self.position_embeddings[pos_idx].iter().enumerate() {
            embedding[i] += val;
        }

        // Layer normalization
        self.layer_normalize(&mut embedding);

        embedding
    }

    /// Layer normalization
    fn layer_normalize(&self, x: &mut [f32]) {
        let mean = x.iter().sum::<f32>() / x.len() as f32;
        let variance = x.iter().map(|&v| (v - mean).powi(2)).sum::<f32>() / x.len() as f32;
        let std = (variance + 1e-6).sqrt();

        for (i, val) in x.iter_mut().enumerate() {
            *val = self.layer_norm_weight[i] * (*val - mean) / std + self.layer_norm_bias[i];
        }
    }

    /// Project a sequence of tokens
    pub fn project_sequence(&self, tokens: &[EnrichedToken]) -> Vec<Vec<f32>> {
        tokens.iter().map(|t| self.project(t)).collect()
    }
}

// ============================================================================
// Attention Mechanism
// ============================================================================

/// Multi-head attention with role-aware bias
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiHeadAttention {
    /// Query projection matrices [n_heads x d_model x d_k]
    pub w_q: Vec<Vec<Vec<f32>>>,
    /// Key projection matrices [n_heads x d_model x d_k]
    pub w_k: Vec<Vec<Vec<f32>>>,
    /// Value projection matrices [n_heads x d_model x d_v]
    pub w_v: Vec<Vec<Vec<f32>>>,
    /// Output projection [n_heads * d_v x d_model]
    pub w_o: Vec<Vec<f32>>,
    /// Role attention bias [n_heads x 9 x 9]
    pub role_bias: Option<Vec<Vec<Vec<f32>>>>,
    /// Number of heads
    pub n_heads: usize,
    /// Dimension per head
    pub d_k: usize,
    pub d_v: usize,
}

impl MultiHeadAttention {
    /// Create new multi-head attention
    pub fn new(d_model: usize, n_heads: usize, use_role_bias: bool) -> Self {
        let d_k = d_model / n_heads;
        let d_v = d_model / n_heads;

        Self {
            w_q: Self::init_attention_matrix(n_heads, d_model, d_k),
            w_k: Self::init_attention_matrix(n_heads, d_model, d_k),
            w_v: Self::init_attention_matrix(n_heads, d_model, d_v),
            w_o: Self::init_matrix(n_heads * d_v, d_model),
            role_bias: if use_role_bias {
                Some(vec![vec![vec![0.0; 9]; 9]; n_heads])
            } else {
                None
            },
            n_heads,
            d_k,
            d_v,
        }
    }

    fn init_attention_matrix(heads: usize, rows: usize, cols: usize) -> Vec<Vec<Vec<f32>>> {
        (0..heads)
            .map(|_| {
                (0..rows)
                    .map(|_| (0..cols).map(|_| 0.01).collect())
                    .collect()
            })
            .collect()
    }

    fn init_matrix(rows: usize, cols: usize) -> Vec<Vec<f32>> {
        (0..rows)
            .map(|_| (0..cols).map(|_| 0.01).collect())
            .collect()
    }

    /// Forward pass through attention
    pub fn forward(&self, x: &[Vec<f32>], mask: Option<&[bool]>) -> Vec<Vec<f32>> {
        let batch_size = x.len();
        let d_model = x[0].len();

        // Project Q, K, V
        let q = self.project_qkv(x, &self.w_q);
        let k = self.project_qkv(x, &self.w_k);
        let v = self.project_qkv(x, &self.w_v);

        // Scaled dot-product attention
        let scale = 1.0 / (self.d_k as f32).sqrt();
        let mut attention_scores = vec![vec![0.0; batch_size]; batch_size];

        for i in 0..batch_size {
            for j in 0..batch_size {
                if let Some(m) = mask {
                    if !m[j] {
                        attention_scores[i][j] = -1e9;
                        continue;
                    }
                }
                let score: f32 = q[i].iter().zip(k[j].iter()).map(|(a, b)| a * b).sum();
                attention_scores[i][j] = score * scale;
            }
        }

        // Softmax
        let attention_weights = self.softmax_2d(&attention_scores);

        // Apply attention to values
        let mut context = vec![vec![0.0; self.d_v * self.n_heads]; batch_size];
        for i in 0..batch_size {
            for j in 0..batch_size {
                for (d, &val) in v[j].iter().enumerate() {
                    context[i][d] += attention_weights[i][j] * val;
                }
            }
        }

        // Output projection
        self.project_output(&context)
    }

    fn project_qkv(&self, x: &[Vec<f32>], w: &Vec<Vec<Vec<f32>>>) -> Vec<Vec<f32>> {
        let batch_size = x.len();
        let mut result = vec![vec![0.0; self.d_k * self.n_heads]; batch_size];

        for (b, input) in x.iter().enumerate() {
            for (h, w_h) in w.iter().enumerate() {
                for (i, row) in w_h.iter().enumerate() {
                    if i < self.d_k {
                        let sum: f32 = input.iter().zip(row.iter()).map(|(a, b)| a * b).sum();
                        result[b][h * self.d_k + i] = sum;
                    }
                }
            }
        }

        result
    }

    fn project_output(&self, x: &[Vec<f32>]) -> Vec<Vec<f32>> {
        let batch_size = x.len();
        let d_model = self.w_o[0].len();
        let mut result = vec![vec![0.0; d_model]; batch_size];

        for (b, input) in x.iter().enumerate() {
            for (i, row) in self.w_o.iter().enumerate() {
                let sum: f32 = input.iter().zip(row.iter()).map(|(a, b)| a * b).sum();
                result[b][i] = sum;
            }
        }

        result
    }

    fn softmax_2d(&self, x: &[Vec<f32>]) -> Vec<Vec<f32>> {
        x.iter()
            .map(|row| {
                let max_val = row.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let exp: Vec<f32> = row.iter().map(|&v| (v - max_val).exp()).collect();
                let sum: f32 = exp.iter().sum();
                exp.iter().map(|&v| v / sum).collect()
            })
            .collect()
    }
}

// ============================================================================
// Feed-Forward Network
// ============================================================================

/// Position-wise feed-forward network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedForward {
    /// First linear layer [d_model x d_ff]
    pub w1: Vec<Vec<f32>>,
    /// Second linear layer [d_ff x d_model]
    pub w2: Vec<Vec<f32>>,
    /// Layer norm weight
    pub ln_weight: Vec<f32>,
    /// Layer norm bias
    pub ln_bias: Vec<f32>,
}

impl FeedForward {
    /// Create new feed-forward network
    pub fn new(d_model: usize, d_ff: usize) -> Self {
        Self {
            w1: (0..d_model)
                .map(|_| (0..d_ff).map(|_| 0.01).collect())
                .collect(),
            w2: (0..d_ff)
                .map(|_| (0..d_model).map(|_| 0.01).collect())
                .collect(),
            ln_weight: vec![1.0; d_model],
            ln_bias: vec![0.0; d_model],
        }
    }

    /// Forward pass
    pub fn forward(&self, x: &[f32]) -> Vec<f32> {
        // First linear + ReLU
        let mut hidden: Vec<f32> = self
            .w1
            .iter()
            .map(|row| {
                let sum: f32 = x.iter().zip(row.iter()).map(|(a, b)| a * b).sum();
                sum.max(0.0) // ReLU
            })
            .collect();

        // Second linear
        let output: Vec<f32> = self
            .w2
            .iter()
            .map(|row| hidden.iter().zip(row.iter()).map(|(a, b)| a * b).sum())
            .collect();

        output
    }
}

// ============================================================================
// Transformer Encoder Layer
// ============================================================================

/// Single transformer encoder layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformerLayer {
    /// Multi-head attention
    pub attention: MultiHeadAttention,
    /// Feed-forward network
    pub feed_forward: FeedForward,
    /// Layer norm 1 (attention)
    pub ln1_weight: Vec<f32>,
    pub ln1_bias: Vec<f32>,
    /// Layer norm 2 (FFN)
    pub ln2_weight: Vec<f32>,
    pub ln2_bias: Vec<f32>,
}

impl TransformerLayer {
    /// Create new transformer layer
    pub fn new(d_model: usize, n_heads: usize, d_ff: usize, use_role_bias: bool) -> Self {
        Self {
            attention: MultiHeadAttention::new(d_model, n_heads, use_role_bias),
            feed_forward: FeedForward::new(d_model, d_ff),
            ln1_weight: vec![1.0; d_model],
            ln1_bias: vec![0.0; d_model],
            ln2_weight: vec![1.0; d_model],
            ln2_bias: vec![0.0; d_model],
        }
    }

    /// Forward pass through layer
    pub fn forward(&self, x: &[Vec<f32>], mask: Option<&[bool]>) -> Vec<Vec<f32>> {
        let d_model = x[0].len();

        // Attention with residual
        let attn_out = self.attention.forward(x, mask);
        let mut x = self.add_norm(x, &attn_out, &self.ln1_weight, &self.ln1_bias);

        // FFN with residual
        let ffn_out: Vec<Vec<f32>> = x.iter().map(|v| self.feed_forward.forward(v)).collect();
        x = self.add_norm(&x, &ffn_out, &self.ln2_weight, &self.ln2_bias);

        x
    }

    fn add_norm(
        &self,
        x: &[Vec<f32>],
        delta: &[Vec<f32>],
        weight: &[f32],
        bias: &[f32],
    ) -> Vec<Vec<f32>> {
        x.iter()
            .zip(delta.iter())
            .map(|(orig, d)| {
                orig.iter()
                    .zip(d.iter())
                    .zip(weight.iter())
                    .zip(bias.iter())
                    .map(|(((o, d), w), b)| (o + d) * w + b)
                    .collect()
            })
            .collect()
    }
}

// ============================================================================
// Full Model
// ============================================================================

/// Complete MorphlexLLM model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorphlexLLM {
    /// Model configuration
    pub config: ModelConfig,
    /// Embedding projector
    pub embeddings: EmbeddingProjector,
    /// Transformer layers
    pub layers: Vec<TransformerLayer>,
    /// Output projection [d_model x vocab_size]
    pub output_projection: Vec<Vec<f32>>,
    /// Lemma prediction head
    pub lemma_head: Option<Vec<Vec<f32>>>,
    /// POS prediction head
    pub pos_head: Option<Vec<Vec<f32>>>,
    /// Role prediction head
    pub role_head: Option<Vec<Vec<f32>>>,
}

impl MorphlexLLM {
    /// Create new model from config
    pub fn new(config: &ModelConfig) -> Self {
        let mut layers = Vec::with_capacity(config.n_layers);
        for _ in 0..config.n_layers {
            layers.push(TransformerLayer::new(
                config.d_model,
                config.n_heads,
                config.d_ff,
                config.use_role_attention,
            ));
        }

        Self {
            config: config.clone(),
            embeddings: EmbeddingProjector::new(config),
            layers,
            output_projection: (0..config.d_model)
                .map(|_| (0..config.vocab_size).map(|_| 0.01).collect())
                .collect(),
            lemma_head: if config.use_lemma_embeddings {
                Some(
                    (0..config.d_model)
                        .map(|_| (0..config.vocab_size).map(|_| 0.01).collect())
                        .collect(),
                )
            } else {
                None
            },
            pos_head: Some(
                (0..config.d_model)
                    .map(|_| (0..10).map(|_| 0.01).collect())
                    .collect(),
            ),
            role_head: Some(
                (0..config.d_model)
                    .map(|_| (0..9).map(|_| 0.01).collect())
                    .collect(),
            ),
        }
    }

    /// Forward pass through model
    pub fn forward(&self, tokens: &[EnrichedToken]) -> Vec<Vec<f32>> {
        // Embed tokens
        let mut x = self.embeddings.project_sequence(tokens);

        // Pass through transformer layers
        let mask: Vec<bool> = tokens.iter().map(|t| t.attention_mask).collect();
        for layer in &self.layers {
            x = layer.forward(&x, Some(&mask));
        }

        x
    }

    /// Get next token logits
    pub fn predict(&self, tokens: &[EnrichedToken]) -> Vec<f32> {
        let hidden = self.forward(tokens);

        // Use last token's hidden state
        let last_hidden = hidden.last().unwrap();

        // Project to vocabulary
        let mut logits = vec![0.0; self.config.vocab_size];
        for (i, row) in self.output_projection.iter().enumerate() {
            for (j, &w) in row.iter().enumerate() {
                logits[j] += last_hidden[i] * w;
            }
        }

        logits
    }

    /// Get auxiliary task predictions
    pub fn predict_aux(&self, tokens: &[EnrichedToken]) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let hidden = self.forward(tokens);
        let last_hidden = hidden.last().unwrap();

        // Lemma logits
        let lemma_logits = self
            .lemma_head
            .as_ref()
            .map(|head| {
                let mut logits = vec![0.0; self.config.vocab_size];
                for (i, row) in head.iter().enumerate() {
                    for (j, &w) in row.iter().enumerate() {
                        logits[j] += last_hidden[i] * w;
                    }
                }
                logits
            })
            .unwrap_or_else(|| vec![0.0; self.config.vocab_size]);

        // POS logits
        let pos_logits = self
            .pos_head
            .as_ref()
            .map(|head| {
                let mut logits = vec![0.0; 10];
                for (i, row) in head.iter().enumerate() {
                    for (j, &w) in row.iter().enumerate() {
                        logits[j] += last_hidden[i] * w;
                    }
                }
                logits
            })
            .unwrap_or_else(|| vec![0.0; 10]);

        // Role logits
        let role_logits = self
            .role_head
            .as_ref()
            .map(|head| {
                let mut logits = vec![0.0; 9];
                for (i, row) in head.iter().enumerate() {
                    for (j, &w) in row.iter().enumerate() {
                        logits[j] += last_hidden[i] * w;
                    }
                }
                logits
            })
            .unwrap_or_else(|| vec![0.0; 9]);

        (lemma_logits, pos_logits, role_logits)
    }

    /// Get estimated parameter count
    pub fn param_count(&self) -> usize {
        self.config.param_count()
    }

    /// Save model to file
    pub fn save(&self, path: &std::path::Path) -> MorphResult<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| MorphlexError::DatabaseError(e.to_string()))?;
        std::fs::write(path, json).map_err(|e| MorphlexError::IoError(e))?;
        Ok(())
    }

    /// Load model from file
    pub fn load(path: &std::path::Path) -> MorphResult<Self> {
        let json = std::fs::read_to_string(path).map_err(|e| MorphlexError::IoError(e))?;
        let model =
            serde_json::from_str(&json).map_err(|e| MorphlexError::DatabaseError(e.to_string()))?;
        Ok(model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TokenVector;

    #[test]
    fn test_model_creation() {
        let config = ModelConfig::small();
        let model = MorphlexLLM::new(&config);

        assert_eq!(model.config.d_model, 512);
        assert_eq!(model.config.n_layers, 6);
        assert_eq!(model.layers.len(), 6);
    }

    #[test]
    fn test_forward_pass() {
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

        // Create test tokens
        let tokens = vec![
            EnrichedToken::from_vector(
                TokenVector {
                    id: 1,
                    lemma_id: 1,
                    pos: 0,
                    role: 0,
                    morph: 0,
                },
                config.d_model,
            ),
            EnrichedToken::from_vector(
                TokenVector {
                    id: 2,
                    lemma_id: 2,
                    pos: 1,
                    role: 1,
                    morph: 0,
                },
                config.d_model,
            ),
        ];

        let output = model.forward(&tokens);

        assert_eq!(output.len(), 2);
        assert_eq!(output[0].len(), config.d_model);
    }

    #[test]
    fn test_param_count() {
        let config = ModelConfig::small();
        let model = MorphlexLLM::new(&config);

        let params = model.param_count();
        println!("Small model parameters: {}", params);
        assert!(params > 0);
    }
}
