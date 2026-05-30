# NNOS on TP-HCF - TODO

## Immediate Actions

### T-Diagram Stabilization

#### [DONE] Root Cause Identified and Fixed (2026-05-30)

**Issue:** 1-byte codegen divergence between jstar2 and jstar3 at offset 0x1c8b.

**Root cause:** `string_data_len` was used before declared in compiler.jstr.
- Line 909: `store data_len into string_data_len` (use)
- Line 1090: `global string_data_len` (declaration)

The self-hosted compiler's single-pass lookup failed, reading garbage from
`var_offset[-1]`. This produced offset 0 in jstar2 and offset 103 in jstar3.

**Fix:** Moved `global string_data_len` to line 35 (before first use).

**Verification:**
- jstar2 == jstar3 (70,925 bytes, byte-identical)
- jstar3 == jstar4 (stable fixpoint at generation 2)
- `test_t_diagram_fixpoint` passes without workaround

---

## NNOS Ports (Ready for Commit)

### Validation Module
- [ ] Commit validation port
- [ ] Add tests for TP-HCF integration

### Morph Module
- [ ] Commit morph port
- [ ] Integrate with Morphlex pipeline

### Morphlex Pipeline
- [ ] Commit full Morphlex with LLM
- [ ] Document training data requirements

### LDS Sensor Fusion
- [ ] Integrate into Morphlex pipeline
- [ ] Test with real sensor data

## Audio Processing Pipeline

### Berktay Pre-Distortion
- [ ] Port nnos_berktay_predistortion
- [ ] Integrate with audio pipeline
- [ ] Test with ultrasonic transducers

### Neural Analyzer
- [ ] Port nnos_neuro_analyzer
- [ ] Connect to MorphlexLLM
- [ ] Test pattern recognition

## System Integration

### Daemon Coordination
- [ ] Connect ConvergenceManager to LLM training
- [ ] Add SITREP reporting for training jobs
- [ ] Implement AAR for training completion

### Security Hardening
- [ ] Enable GuardianAgent for LLM outputs
- [ ] Add threat detection for model poisoning
- [ ] Implement secure model checkpointing

## Documentation

- [ ] Update CUSTOM_LLM_PLAN.md with current status
- [ ] Add T-Diagram debugging guide
- [ ] Document daemon setup and monitoring

---

## Status Legend

- [DONE] Complete
- [!] In Progress / Partial
- [X] Not Started / Blocked
- [~] In Review

---

**Priority:** T-Diagram stabilization → NNOS ports → Audio pipeline → System integration
