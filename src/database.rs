//! Database -- Phases 6, 7, 8 of the morphlex pipeline.
//!
//! Phase 6: Write integer-packed token vectors into a flat binary format.
//! Phase 7: Compact to exact read-only size.
//! Phase 8: PQC encrypt (ML-KEM-1024 + AES-256-GCM) and sign (ML-DSA-65).
//!
//! Cryptographic standards:
//!   FIPS 203 -- ML-KEM-1024 (key encapsulation, quantum-resistant)
//!   FIPS 204 -- ML-DSA-65 (digital signatures, quantum-resistant)
//!   FIPS 197 + SP 800-38D -- AES-256-GCM (symmetric encryption)
//!
//! Binary format -- flat, memory-mappable, no indirection:
//!
//!   [Header: 24 bytes]
//!     magic:       [u8; 8]  -- "MORPHLEX"
//!     version:     u32      -- format version
//!     entry_count: u64      -- number of entries
//!     flags:       u32      -- bit flags
//!
//!   [Lemma Table: variable]
//!     For each entry:
//!       lemma_len: u16
//!       lemma:     [u8; lemma_len]
//!
//!   [Vector Table: entry_count * 12 bytes]
//!     For each entry:
//!       TokenVector as 12 packed bytes (id, lemma_id, pos, role, morph)
//!
//! Encrypted output format:
//!   [ML-KEM-1024 ciphertext][AES-256-GCM nonce: 12B][AES-256-GCM ciphertext]
//!   + separate .sig file with ML-DSA-65 signature

use std::fs;
use std::path::Path;

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, AeadCore, Nonce};

use ml_kem::kem::{Decapsulate, Encapsulate, Kem};
use ml_kem::MlKem1024;

use ml_dsa::KeyGen;

use crate::types::*;

const MAGIC: &[u8; 8] = b"MORPHLEX";
const FORMAT_VERSION: u32 = 3; // v3: PQC encryption
const HEADER_SIZE: usize = 24;

/// ML-KEM-1024 ciphertext size in bytes.
const MLKEM1024_CT_SIZE: usize = 1568;

// ---- Phase 6: Write ----

/// Phase 6: Write token vectors and their lemmas to a binary database.
pub fn write_database(
    vectors: &[TokenVector],
    lemmas: &[String],
    path: &Path,
) -> MorphResult<()> {
    assert_eq!(vectors.len(), lemmas.len());

    let mut buf = Vec::new();

    // Header
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    buf.extend_from_slice(&(vectors.len() as u64).to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes()); // flags

    // Lemma table
    for lemma in lemmas {
        let bytes = lemma.as_bytes();
        buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(bytes);
    }

    // Vector table -- pure 12-byte packed entries, contiguous
    for tv in vectors {
        buf.extend_from_slice(&tv.to_bytes());
    }

    fs::write(path, &buf).map_err(MorphlexError::IoError)?;
    Ok(())
}

// ---- Phase 7: Compact ----

/// Phase 7: Compact the database to exact read-only size.
pub fn compact(path: &Path) -> MorphResult<u64> {
    let data = fs::read(path).map_err(MorphlexError::IoError)?;
    let exact_size = calculate_exact_size(&data)?;

    if exact_size < data.len() {
        fs::write(path, &data[..exact_size]).map_err(MorphlexError::IoError)?;
    }

    // Set read-only
    let mut perms = fs::metadata(path)
        .map_err(MorphlexError::IoError)?
        .permissions();
    perms.set_readonly(true);
    fs::set_permissions(path, perms).map_err(MorphlexError::IoError)?;

    Ok(exact_size as u64)
}

/// Calculate the exact byte size of a valid database.
fn calculate_exact_size(data: &[u8]) -> MorphResult<usize> {
    if data.len() < HEADER_SIZE {
        return Err(MorphlexError::DatabaseError("File too small".to_string()));
    }
    if &data[0..8] != MAGIC {
        return Err(MorphlexError::DatabaseError("Invalid magic".to_string()));
    }

    let entry_count = u64::from_le_bytes(data[12..20].try_into().unwrap()) as usize;

    // Walk the lemma table
    let mut offset = HEADER_SIZE;
    for _ in 0..entry_count {
        if offset + 2 > data.len() {
            return Err(MorphlexError::DatabaseError("Truncated lemma".to_string()));
        }
        let lemma_len = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2 + lemma_len;
    }

    // Vector table: entry_count * 12 bytes
    offset += entry_count * TOKEN_VECTOR_SIZE;

    Ok(offset)
}

// ---- Phase 8: PQC Encrypt + Sign ----

/// Key bundle returned from encryption.
/// Contains everything needed to decrypt the database.
pub struct PqcKeyBundle {
    /// ML-KEM-1024 decapsulation key (private key) -- serialized bytes
    pub decapsulation_key: Vec<u8>,
    /// ML-DSA-65 signing key -- serialized bytes
    pub signing_key: Vec<u8>,
    /// ML-DSA-65 verifying key -- serialized bytes
    pub verifying_key: Vec<u8>,
}

/// Phase 8: PQC encrypt the compacted database.
///
/// 1. Generate ML-KEM-1024 keypair (FIPS 203)
/// 2. Encapsulate a shared secret with the encapsulation key
/// 3. Use the shared secret as AES-256-GCM key to encrypt the database
/// 4. Sign the ciphertext with ML-DSA-65 (FIPS 204)
/// 5. Write: [kem_ciphertext | nonce | aes_ciphertext]
/// 6. Write signature to .sig file
///
/// Returns the key bundle (decapsulation key + signing/verifying keys).
pub fn encrypt(path: &Path, output_path: &Path) -> MorphResult<PqcKeyBundle> {
    // Remove read-only for reading
    let mut perms = fs::metadata(path)
        .map_err(MorphlexError::IoError)?
        .permissions();
    perms.set_readonly(false);
    fs::set_permissions(path, perms).map_err(MorphlexError::IoError)?;

    let plaintext = fs::read(path).map_err(MorphlexError::IoError)?;

    // Step 1: ML-KEM-1024 keypair (FIPS 203)
    let (dk, ek) = MlKem1024::generate_keypair();

    // Step 2: Encapsulate -- produces shared secret + KEM ciphertext
    let (kem_ct, shared_secret) = ek.encapsulate();

    // Step 3: Use shared secret as AES-256-GCM key
    // ML-KEM shared secret is 32 bytes -- exactly AES-256 key size
    let aes_key: [u8; 32] = shared_secret.into();
    let cipher = Aes256Gcm::new_from_slice(&aes_key)
        .map_err(|e| MorphlexError::EncryptionError(e.to_string()))?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let aes_ciphertext = cipher
        .encrypt(&nonce, plaintext.as_ref())
        .map_err(|e| MorphlexError::EncryptionError(e.to_string()))?;

    // Step 4: Sign with ML-DSA-65 (FIPS 204)
    let mut rng = getrandom::rand_core::UnwrapErr(getrandom::SysRng);
    let dsa_keypair = ml_dsa::MlDsa65::key_gen(&mut rng);

    // Build the encrypted payload: [kem_ct | nonce | aes_ciphertext]
    let kem_ct_bytes: &[u8] = kem_ct.as_ref();
    let mut encrypted_payload = Vec::with_capacity(
        kem_ct_bytes.len() + 12 + aes_ciphertext.len(),
    );
    encrypted_payload.extend_from_slice(kem_ct_bytes);
    encrypted_payload.extend_from_slice(nonce.as_ref());
    encrypted_payload.extend_from_slice(&aes_ciphertext);

    // Sign the encrypted payload
    use ml_dsa::signature::Signer;
    let signature = dsa_keypair.signing_key().sign(&encrypted_payload);

    // Step 5: Write encrypted database
    fs::write(output_path, &encrypted_payload).map_err(MorphlexError::IoError)?;

    // Step 6: Write signature to .sig file
    let sig_path = output_path.with_extension("sig");
    let sig_encoded = signature.encode();
    let sig_slice: &[u8] = sig_encoded.as_ref();
    fs::write(&sig_path, sig_slice).map_err(MorphlexError::IoError)?;

    // Set both files to read-only
    for p in &[output_path, &sig_path] {
        let mut perms = fs::metadata(p)
            .map_err(MorphlexError::IoError)?
            .permissions();
        perms.set_readonly(true);
        fs::set_permissions(p, perms).map_err(MorphlexError::IoError)?;
    }

    // Serialize keys for the bundle
    // DK: use seed (64 bytes) for compact storage
    use ml_kem::kem::KeyExport;
    let dk_seed = dk.to_bytes();
    let dk_bytes: Vec<u8> = AsRef::<[u8]>::as_ref(&dk_seed).to_vec();

    // DSA: use seed from keypair, encoded form for verifying key
    let sk_seed = dsa_keypair.to_seed();
    let sk_bytes: Vec<u8> = AsRef::<[u8]>::as_ref(&sk_seed).to_vec();
    let vk_encoded = dsa_keypair.verifying_key().encode();
    let vk_bytes: Vec<u8> = AsRef::<[u8]>::as_ref(&vk_encoded).to_vec();

    Ok(PqcKeyBundle {
        decapsulation_key: dk_bytes,
        signing_key: sk_bytes,
        verifying_key: vk_bytes,
    })
}

/// Decrypt a PQC-encrypted database.
///
/// 1. Verify ML-DSA-65 signature
/// 2. Extract ML-KEM-1024 ciphertext
/// 3. Decapsulate to recover shared secret
/// 4. Decrypt AES-256-GCM with the shared secret
pub fn decrypt(
    path: &Path,
    dk_bytes: &[u8],
    vk_bytes: Option<&[u8]>,
) -> MorphResult<Vec<u8>> {
    let data = fs::read(path).map_err(MorphlexError::IoError)?;

    // Verify signature if verifying key provided
    if let Some(vk_raw) = vk_bytes {
        let sig_path = path.with_extension("sig");
        if sig_path.exists() {
            let sig_bytes = fs::read(&sig_path).map_err(MorphlexError::IoError)?;

            // Reconstruct verifying key from encoded bytes
            let vk_encoded = ml_dsa::EncodedVerifyingKey::<ml_dsa::MlDsa65>::try_from(vk_raw)
                .map_err(|_| MorphlexError::EncryptionError("Invalid verifying key size".to_string()))?;
            let vk = ml_dsa::VerifyingKey::<ml_dsa::MlDsa65>::decode(&vk_encoded);

            // Reconstruct signature from encoded bytes
            let sig_encoded = ml_dsa::EncodedSignature::<ml_dsa::MlDsa65>::try_from(sig_bytes.as_slice())
                .map_err(|_| MorphlexError::EncryptionError("Invalid signature size".to_string()))?;
            let sig = ml_dsa::Signature::<ml_dsa::MlDsa65>::decode(&sig_encoded)
                .ok_or_else(|| MorphlexError::EncryptionError("Invalid signature data".to_string()))?;

            use ml_dsa::signature::Verifier;
            vk.verify(&data, &sig)
                .map_err(|e| MorphlexError::EncryptionError(format!("Signature verification failed: {}", e)))?;
        }
    }

    if data.len() < MLKEM1024_CT_SIZE + 12 {
        return Err(MorphlexError::EncryptionError(
            "File too small for KEM ciphertext + nonce".to_string(),
        ));
    }

    // Split: [kem_ct | nonce | aes_ciphertext]
    let (kem_ct_bytes, rest) = data.split_at(MLKEM1024_CT_SIZE);
    let (nonce_bytes, aes_ciphertext) = rest.split_at(12);

    // Decapsulate ML-KEM-1024
    // Reconstruct DK from seed bytes
    if dk_bytes.len() != 64 {
        return Err(MorphlexError::EncryptionError(
            format!("Decapsulation key must be 64 bytes (seed), got {}", dk_bytes.len()),
        ));
    }
    let dk_seed = ml_kem::Seed::try_from(dk_bytes)
        .map_err(|_| MorphlexError::EncryptionError("Invalid seed size".to_string()))?;
    let dk = ml_kem::DecapsulationKey::<ml_kem::MlKem1024>::from(dk_seed);

    let kem_ct = ml_kem::Ciphertext::<ml_kem::MlKem1024>::try_from(kem_ct_bytes)
        .map_err(|_| MorphlexError::EncryptionError("Invalid KEM ciphertext".to_string()))?;

    let shared_secret = dk.decapsulate(&kem_ct);
    let aes_key: [u8; 32] = shared_secret.into();

    // Decrypt AES-256-GCM
    let nonce = Nonce::from_slice(nonce_bytes);
    let cipher = Aes256Gcm::new_from_slice(&aes_key)
        .map_err(|e| MorphlexError::EncryptionError(e.to_string()))?;

    cipher
        .decrypt(nonce, aes_ciphertext)
        .map_err(|e| MorphlexError::EncryptionError(e.to_string()))
}

// ---- Read ----

/// Parse a raw database into (lemmas, vectors).
pub fn read_database(data: &[u8]) -> MorphResult<(Vec<String>, Vec<TokenVector>)> {
    if data.len() < HEADER_SIZE {
        return Err(MorphlexError::DatabaseError("File too small".to_string()));
    }
    if &data[0..8] != MAGIC {
        return Err(MorphlexError::DatabaseError("Invalid magic".to_string()));
    }

    let entry_count = u64::from_le_bytes(data[12..20].try_into().unwrap()) as usize;

    // Read lemma table
    let mut lemmas = Vec::with_capacity(entry_count);
    let mut offset = HEADER_SIZE;
    for _ in 0..entry_count {
        let lemma_len = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2;
        let lemma = String::from_utf8(data[offset..offset + lemma_len].to_vec())
            .map_err(|e| MorphlexError::DatabaseError(e.to_string()))?;
        offset += lemma_len;
        lemmas.push(lemma);
    }

    // Read vector table
    let mut vectors = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        let buf: [u8; TOKEN_VECTOR_SIZE] = data[offset..offset + TOKEN_VECTOR_SIZE]
            .try_into()
            .map_err(|_| MorphlexError::DatabaseError("Truncated vector".to_string()))?;
        vectors.push(TokenVector::from_bytes(&buf));
        offset += TOKEN_VECTOR_SIZE;
    }

    Ok((lemmas, vectors))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;
    use std::io::Write;

    fn make_test_data() -> (Vec<TokenVector>, Vec<String>) {
        let vectors = vec![
            TokenVector {
                id: 42,
                lemma_id: 42,
                pos: 0,
                role: 0,
                morph: 0,
            },
            TokenVector {
                id: 99,
                lemma_id: 99,
                pos: 1,
                role: 1,
                morph: morph_flags::HAS_SUFFIX,
            },
        ];
        let lemmas = vec!["hello".to_string(), "world".to_string()];
        (vectors, lemmas)
    }

    #[test]
    fn test_write_and_read_roundtrip() {
        let path = std::env::temp_dir().join("morphlex_test_v3_db");
        let _ = fs::remove_file(&path);

        let (vectors, lemmas) = make_test_data();
        write_database(&vectors, &lemmas, &path).unwrap();

        let data = fs::read(&path).unwrap();
        let (recovered_lemmas, recovered_vectors) = read_database(&data).unwrap();

        assert_eq!(recovered_lemmas, lemmas);
        assert_eq!(recovered_vectors, vectors);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_compact_shrinks() {
        let path = std::env::temp_dir().join("morphlex_test_v3_compact");
        let _ = fs::remove_file(&path);

        let (vectors, lemmas) = make_test_data();
        write_database(&vectors, &lemmas, &path).unwrap();

        // Append junk
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(&[0u8; 1024]).unwrap();
        drop(file);

        let size_before = fs::metadata(&path).unwrap().len();
        let size_after = compact(&path).unwrap();
        assert!(size_after < size_before);

        // Cleanup
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(false);
        fs::set_permissions(&path, perms).unwrap();
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_pqc_encrypt_decrypt_roundtrip() {
        let plain_path = std::env::temp_dir().join("morphlex_test_v3_pqc_plain");
        let enc_path = std::env::temp_dir().join("morphlex_test_v3_pqc_cipher");
        let sig_path = enc_path.with_extension("sig");
        let _ = fs::remove_file(&plain_path);
        let _ = fs::remove_file(&enc_path);
        let _ = fs::remove_file(&sig_path);

        let (vectors, lemmas) = make_test_data();
        write_database(&vectors, &lemmas, &plain_path).unwrap();
        compact(&plain_path).unwrap();

        // Encrypt with PQC
        let bundle = encrypt(&plain_path, &enc_path).unwrap();

        // Decrypt with PQC (with signature verification)
        // Remove read-only for cleanup later
        let mut perms = fs::metadata(&enc_path).unwrap().permissions();
        perms.set_readonly(false);
        fs::set_permissions(&enc_path, perms).unwrap();

        let mut perms = fs::metadata(&sig_path).unwrap().permissions();
        perms.set_readonly(false);
        fs::set_permissions(&sig_path, perms).unwrap();

        let decrypted = decrypt(
            &enc_path,
            &bundle.decapsulation_key,
            Some(&bundle.verifying_key),
        )
        .unwrap();

        let (recovered_lemmas, recovered_vectors) = read_database(&decrypted).unwrap();

        assert_eq!(recovered_lemmas, lemmas);
        assert_eq!(recovered_vectors, vectors);

        // Cleanup
        let _ = fs::remove_file(&plain_path);
        let _ = fs::remove_file(&enc_path);
        let _ = fs::remove_file(&sig_path);
    }

    #[test]
    fn test_tampered_ciphertext_fails_signature() {
        let plain_path = std::env::temp_dir().join("morphlex_test_v3_tamper_plain");
        let enc_path = std::env::temp_dir().join("morphlex_test_v3_tamper_cipher");
        let sig_path = enc_path.with_extension("sig");
        let _ = fs::remove_file(&plain_path);
        let _ = fs::remove_file(&enc_path);
        let _ = fs::remove_file(&sig_path);

        let (vectors, lemmas) = make_test_data();
        write_database(&vectors, &lemmas, &plain_path).unwrap();
        compact(&plain_path).unwrap();

        let bundle = encrypt(&plain_path, &enc_path).unwrap();

        // Tamper with the encrypted file
        let mut perms = fs::metadata(&enc_path).unwrap().permissions();
        perms.set_readonly(false);
        fs::set_permissions(&enc_path, perms).unwrap();

        let mut data = fs::read(&enc_path).unwrap();
        if let Some(byte) = data.last_mut() {
            *byte ^= 0xFF; // flip last byte
        }
        fs::write(&enc_path, &data).unwrap();

        let mut perms = fs::metadata(&sig_path).unwrap().permissions();
        perms.set_readonly(false);
        fs::set_permissions(&sig_path, perms).unwrap();

        // Decrypt should fail signature verification
        let result = decrypt(
            &enc_path,
            &bundle.decapsulation_key,
            Some(&bundle.verifying_key),
        );
        assert!(result.is_err());

        // Cleanup
        let _ = fs::remove_file(&plain_path);
        let _ = fs::remove_file(&enc_path);
        let _ = fs::remove_file(&sig_path);
    }

    #[test]
    fn test_entry_size_is_12_bytes() {
        let path = std::env::temp_dir().join("morphlex_test_v3_size");
        let _ = fs::remove_file(&path);

        let (vectors, lemmas) = make_test_data();
        write_database(&vectors, &lemmas, &path).unwrap();

        let data = fs::read(&path).unwrap();
        let exact = calculate_exact_size(&data).unwrap();

        // Header(24) + lemma_table(2+5 + 2+5 = 14) + vectors(2*12 = 24) = 62
        assert_eq!(exact, 62);

        let _ = fs::remove_file(&path);
    }
}
