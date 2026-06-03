//! GGUF Export for llama.cpp compatibility.
//!
//! Simplified GGUF export for MorphlexLLM models.

use crate::llm::MorphlexLLM;
use crate::types::{MorphResult, MorphlexError};
use std::io::{Seek, Write};
use std::path::Path;

/// GGUF magic number "GGUF" in little-endian
const GGUF_MAGIC: u32 = 0x46554747;

/// GGUF version 3
const GGUF_VERSION: u32 = 3;

/// Export MorphlexLLM to GGUF format
pub fn export_to_gguf(model: &MorphlexLLM, path: &Path, quantize: bool) -> MorphResult<()> {
    let mut file = std::fs::File::create(path).map_err(MorphlexError::IoError)?;

    // Write header
    file.write_all(&GGUF_MAGIC.to_le_bytes())
        .map_err(MorphlexError::IoError)?;
    file.write_all(&GGUF_VERSION.to_le_bytes())
        .map_err(MorphlexError::IoError)?;

    // Tensor count (simplified - just embedding and output)
    let tensor_count = 2u64;
    file.write_all(&tensor_count.to_le_bytes())
        .map_err(MorphlexError::IoError)?;

    // Metadata count
    let metadata_count = 7u64;
    file.write_all(&metadata_count.to_le_bytes())
        .map_err(MorphlexError::IoError)?;

    // Write metadata
    write_metadata(&mut file, "general.name", "morphlex-llm")?;
    write_metadata(&mut file, "general.architecture", "transformer")?;
    write_metadata_u32(
        &mut file,
        "llm.embedding_length",
        model.config.d_model as u32,
    )?;
    write_metadata_u32(&mut file, "llm.block_count", model.config.n_layers as u32)?;
    write_metadata_u32(
        &mut file,
        "llm.attention.head_count",
        model.config.n_heads as u32,
    )?;
    write_metadata_u32(
        &mut file,
        "llm.feed_forward_length",
        model.config.d_ff as u32,
    )?;
    write_metadata_u32(&mut file, "llm.vocab_size", model.config.vocab_size as u32)?;

    // Write tensor info (simplified)
    write_tensor_info(
        &mut file,
        "output.weight",
        &[model.config.d_model as u64, model.config.vocab_size as u64],
        6,
    )?;
    write_tensor_info(
        &mut file,
        "token_embd.weight",
        &[model.config.vocab_size as u64, model.config.d_model as u64],
        6,
    )?;

    // Align to 32 bytes
    let pos = file.stream_position()
        .map_err(MorphlexError::IoError)?;
    let padding = (32 - (pos % 32)) % 32;
    for _ in 0..padding {
        file.write_all(&[0u8])
            .map_err(MorphlexError::IoError)?;
    }

    // Write tensor data (simplified - just zeros as placeholder)
    let output_size = model.config.d_model * model.config.vocab_size;
    for _ in 0..output_size {
        if quantize {
            // Write F16
            file.write_all(&0u16.to_le_bytes())
                .map_err(MorphlexError::IoError)?;
        } else {
            // Write F32
            file.write_all(&0.0f32.to_le_bytes())
                .map_err(MorphlexError::IoError)?;
        }
    }

    Ok(())
}

fn write_metadata<W: Write>(writer: &mut W, key: &str, value: &str) -> MorphResult<()> {
    // Type 8 = string
    writer
        .write_all(&8u32.to_le_bytes())
        .map_err(MorphlexError::IoError)?;
    // Key length
    writer
        .write_all(&(key.len() as u64).to_le_bytes())
        .map_err(MorphlexError::IoError)?;
    // Key
    writer
        .write_all(key.as_bytes())
        .map_err(MorphlexError::IoError)?;
    // Value length
    writer
        .write_all(&(value.len() as u64).to_le_bytes())
        .map_err(MorphlexError::IoError)?;
    // Value
    writer
        .write_all(value.as_bytes())
        .map_err(MorphlexError::IoError)?;
    Ok(())
}

fn write_metadata_u32<W: Write>(writer: &mut W, key: &str, value: u32) -> MorphResult<()> {
    // Type 4 = uint32
    writer
        .write_all(&4u32.to_le_bytes())
        .map_err(MorphlexError::IoError)?;
    // Key length
    writer
        .write_all(&(key.len() as u64).to_le_bytes())
        .map_err(MorphlexError::IoError)?;
    // Key
    writer
        .write_all(key.as_bytes())
        .map_err(MorphlexError::IoError)?;
    // Value
    writer
        .write_all(&value.to_le_bytes())
        .map_err(MorphlexError::IoError)?;
    Ok(())
}

fn write_tensor_info<W: Write>(
    writer: &mut W,
    name: &str,
    dims: &[u64],
    dtype: u32,
) -> MorphResult<()> {
    // Name length
    writer
        .write_all(&(name.len() as u64).to_le_bytes())
        .map_err(MorphlexError::IoError)?;
    // Name
    writer
        .write_all(name.as_bytes())
        .map_err(MorphlexError::IoError)?;
    // Number of dimensions
    writer
        .write_all(&(dims.len() as u32).to_le_bytes())
        .map_err(MorphlexError::IoError)?;
    // Dimensions
    for &dim in dims {
        writer
            .write_all(&dim.to_le_bytes())
            .map_err(MorphlexError::IoError)?;
    }
    // Data type
    writer
        .write_all(&dtype.to_le_bytes())
        .map_err(MorphlexError::IoError)?;
    // Offset (placeholder)
    writer
        .write_all(&0u64.to_le_bytes())
        .map_err(MorphlexError::IoError)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ModelConfig;

    #[test]
    fn test_export_model() {
        let config = ModelConfig {
            d_model: 64,
            n_heads: 4,
            n_layers: 2,
            d_ff: 128,
            vocab_size: 100,
            max_seq_len: 32,
            dropout: 0.0,
            use_role_attention: true,
            use_morph_gates: true,
            use_lemma_embeddings: true,
        };

        let model = MorphlexLLM::new(&config);
        let temp_path = std::env::temp_dir().join("morphlex_test.gguf");

        export_to_gguf(&model, &temp_path, false).unwrap();

        assert!(temp_path.exists());
        let _ = std::fs::remove_file(temp_path);
    }
}
