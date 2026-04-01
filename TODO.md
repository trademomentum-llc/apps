# NNOS on TP-HCF - TODO

## Immediate Actions

### T-Diagram Stabilization
- [ ] Debug jstar3.bin SIGSEGV crash
  - jstar2_raw.bin: 1.8MB, functional ✅
  - jstar3.bin: 68KB, crashes ❌
  - Size difference indicates missing code/data sections
  
- [ ] Compare jstar2 vs jstar3 codegen
  - Use `objdump -d` to disassemble both
  - Identify missing instructions in jstar3
  
- [ ] Complete compiler.jstr Phases 2-6
  - Phase 2: Fix parse warnings for type modifiers
  - Phase 3: Consider adding type checking pass
  - Phase 4: Consider adding IR lowering pass
  - Phase 5: Complete all instruction implementations
  - Phase 6: Verify ELF linking correctness

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

- ✅ Complete
- ⚠️ In Progress / Partial
- ❌ Not Started / Blocked
- 🔄 In Review

---

**Priority:** T-Diagram stabilization → NNOS ports → Audio pipeline → System integration
