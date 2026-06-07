# Dual-Root Synchronization Agent — Requirements Specification

**Document ID:** NEURODIOS-DRSA-REQ-001  
**Version:** 1.0.0  
**Date:** 2026-05-30  
**Status:** Authoritative Baseline  
**Maintainer:** Grok (Dual-Root Synchronization Agent instantiation)  
**Roots:** engine/nnos (canonical) + apps (working)

---

## 1. Purpose

The Dual-Root Synchronization Agent is the guardian of cross-root coherence for the NeuroDiOS sovereign project. It ensures that the two primary development roots — engine/nnos (kernel, LSA synthesis, denominators, NeuroBalance, Roller Coaster, micro-kernel sources) and apps (Jasterish compiler self-hosting, toolchain, bootstrap sources, micro-kernel sources) — remain perfectly consistent with respect to the Chained Source of Truth, the 8 Validated Denominators, the Efficiency Mandate, and all binding decisions.

This agent is one of the five specialized background sub-agents activated 2026-05-30.

---

## 2. Functional Requirements

FR-1. **Artifact Manifest Maintenance**  
Maintain a deterministic, versioned manifest of all cross-root synchronized artifacts (bindings, microkernel sources, key specification triads, coordinator implementations, minimal type layers). The manifest shall be stored in both roots and kept byte-coherent for the canonical entries.

FR-2. **Coherence Scanning**  
On activation or on explicit trigger, perform a full or incremental scan comparing corresponding artifacts across roots using SHA-256 (or stronger) for exact-match items and content-aware semantic comparison for authorized variant copies (e.g., Minimal_Context working variants).

FR-3. **Divergence Flagging**  
Any detected divergence (hash mismatch on exact items, semantic drift on variant items, presence/absence asymmetry, legacy terminology in non-historical sections) shall be logged with precise evidence (file paths, hashes, line excerpts, mapping to affected denominators) and reported into the Chained Source of Truth.

FR-4. **Denominator & Efficiency Enforcement**  
Every change proposal, new artifact, or compiler/kernel decision originating in either root must be explicitly mapped (in prose and/or structured comment) to one or more of the 8 Validated Denominators plus the Efficiency Mandate (smallest-safe-integer discipline + Compute Footprint tracking). The agent shall verify such mapping exists before accepting the change into the binding.

FR-5. **Binding Propagation**  
When the canonical Chained_Source_of_Truth_Kimi_Binding.md or Minimal_Context_Kimi_Binding.md is updated in engine/nnos/lsa/synthesized/, propagate the authoritative content (with root-appropriate wording adjustments only) to apps/context/ and verify post-propagation hashes.

FR-6. **Micro-Kernel & Compiler Interface Validation**  
Verify that any evolution in apps/ Jasterish compiler (codegen, data emission, self-host ladder) produces binaries whose properties (determinism, section layout, provenance) satisfy the expectations of the Jasterish Micro-Kernel sources and NeuroBalance governor in engine/nnos/. Flag any incompatibility that would violate Origin Vault (#7), Drift Detection (#8), or Primitive Traceability (#6).

FR-7. **Legacy Reconciliation Tracking**  
Maintain a live ledger (inside the binding §7 coherence record and a dedicated Fragmented/Historical section) of all files containing pre-8-denominator language. Track progress of reconciliation as a measurable, deterministic work item.

FR-8. **Triad Compliance**  
For any new functional module, subsystem, or major artifact introduced in either root, verify that the full Requirements + Design Specification + Technical Specification triad exists and is cross-referenced from the binding before the artifact is accepted as canonical.

FR-9. **Report Generation**  
Produce machine-readable and human-readable synchronization status reports (including mathematical proofs of coherence where hashes match) after every scan.

---

## 3. Non-Functional Requirements

NFR-1. **Determinism**  
All outputs, reports, and decisions of the agent shall be fully deterministic given the same input state of the two roots. No randomness, no external network calls during core scan.

NFR-2. **Grounding in 8 Denominators + Efficiency**  
Every action, flag, or recommendation emitted by the agent shall be explicitly traceable to at least one of:
1. Fluctuation Dynamics
2. Budget / Resource Accounting (incl. Compute Footprint)
3. Contrast Differential
4. Controlled Oscillation
5. Adaptation Offset
6. Primitive Traceability / Atomic Dependency Mapping
7. Origin Vault (Deterministic Provenance & State History)
8. Drift Detection
plus the Efficiency Mandate (minimal safe integer types, token/context minimization).

NFR-3. **Minimal Footprint**  
The agent's own implementation and reports shall themselves obey the Efficiency Mandate. Prefer INT8/INT16 representations; produce Minimal_Context variants of its own outputs when appropriate.

NFR-4. **Auditability / Origin Vault**  
Every synchronization decision and propagation shall append an immutable entry (timestamp, agent identity, before/after hashes, denominator mappings) to the Origin Vault mechanism (or equivalent provenance manifest).

NFR-5. **Safety**  
The agent shall never perform destructive edits without explicit prior authorization recorded in the binding. All edits use search_replace after read_file (or equivalent deterministic read).

---

## 4. Constraints & Invariants

C-1. Canonical master of the Chained Source of Truth binding always resides at engine/nnos/lsa/synthesized/Chained_Source_of_Truth_Kimi_Binding.md. Apps/context/ holds only the authorized working copy variant.

C-2. The 8 Validated Denominators (enumerated in §2 NFR-2) are the sole physics layer. No new base denominator may be introduced without passing the full atomic + kinetic + fractal + earned-mechanism + real-world deterministic uplift criteria documented in the Denominators Dissection.

C-3. Jasterish compiler outputs (apps) and kernel expectations (engine/nnos) must remain compatible; any change in one root that would break the other must be accompanied by coordinated updates in both and recorded in the binding.

C-4. The agent itself is subject to the same triad requirement and must not operate without its own authoritative Requirements, Design Specification, and Technical Specification documents.

---

## 5. Mathematical Coherence Invariant (Proof Obligation)

Let S_exact be the subset of the manifest requiring byte identity.  
Let H(p) = SHA-256( content of path p ).

**Invariant I (Cross-Root Coherence):**  
For all (p_e, p_a) in S_exact : H(p_e) = H(p_a)

**Proof of Correct Scan:**  
A scan that reports "no divergence on S_exact" and supplies the matching hash pairs constitutes a constructive proof (by exhaustive enumeration of the finite manifest at scan time) that I holds at that instant. Subsequent mutations are only accepted if they preserve I (i.e., both sides are updated atomically or via verified propagation).

For variant items (Minimal_Context copies), a weaker but still deterministic relation R_variant (documented per item) must hold; the agent records the exact R_variant used.

---

## 6. Interfaces to Other Agents & Binding Layer

- Receives triggers and context from Live Context Maintenance Agent.
- Feeds divergence flags and reconciliation proposals to Criteria Enforcement / Reviewer Agent for 8-denominator validation.
- Supplies raw scan data to Parallel Deep Analysis Agent for deeper binary or LSA artifact study.
- Consumes tooling generated by Tooling / Script Generation Agent (e.g., automated manifest differs).
- All outputs are written exclusively into the Chained Source of Truth binding documents (or explicitly linked triad docs) so that Grok remains the persistent memory layer.

---

**End of Requirements Specification**

This document, together with its Design Specification and Technical Specification counterparts, constitutes the complete authoritative baseline for the Dual-Root Synchronization Agent per NeuroDiOS governing rules.