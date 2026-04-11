# NNOS on TP-HCF - TODO

## Immediate Actions

### T-Diagram Stabilization

#### [DONE] Root Cause Identified (2026-04-01 16:00)

**jstar3 crash cause:** Missing .data section (1.7MB)!

- jstar2 (Rust): 1.8MB with .text + .data
- jstar3 (jstar2 output): 68KB with .text ONLY

**Missing in compiler.jstr Phase 5:**
1. String literal emission to `datasec`
2. Global variable data emission
3. `data_len` increment logic

#### [IN PROGRESS] Fix In Progress

- [ ] Add string literal data emission (~100 lines)
  - Copy string bytes from input to datasec at data_len
  - Increment data_len by string length
  - Patch token offsets to point to datasec
  
- [ ] Add global variable data emission (~100 lines)
  - Zero-initialize globals in datasec at data_len
  - Increment data_len by variable size
  - Track global_vreg → datasec offset mapping
  
- [ ] Test with simple program first
  ```
  return 42
  ```
  
- [ ] Test with string literal
  ```
  print "hello"
  return 0
  ```
  
- [ ] Full self-host test
  ```bash
  cargo run -- jstar compile --input jstar/compiler.jstr --output jstar2.bin
  ./jstar2.bin < jstar/compiler.jstr > jstar3.bin
  cmp jstar2.bin jstar3.bin  # Should match!
  ```

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
