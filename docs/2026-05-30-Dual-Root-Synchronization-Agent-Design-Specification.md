# Dual-Root Synchronization Agent — Design Specification

**Document ID:** NEURODIOS-DRSA-DES-001  
**Version:** 1.0.0  
**Date:** 2026-05-30  
**Status:** Authoritative Baseline  
**Related:** NEURODIOS-DRSA-REQ-001 (Requirements)

---

## 1. Architectural Overview

The Dual-Root Synchronization Agent implements a deterministic observer-controller pattern specialized for two-root coherence. It mirrors the neurobiological function of the corpus callosum: continuous, high-fidelity transfer and integration of state between two "hemispheres" (engine/nnos kernel/LSA domain and apps compiler/toolchain domain) while enforcing a single shared "physics" (the 8 Validated Denominators + Efficiency Mandate).

Core design principles (directly derived from the 8 denominators):
- Origin Vault & Primitive Traceability → every comparison is hashed and logged with full provenance.
- Drift Detection → primary detection mechanism.
- Budget / Efficiency Mandate → minimal footprint in both implementation and reports.
- Controlled Oscillation & Contrast Differential → the agent permits deliberate divergence during active development on one root provided it is bounded, time-limited, and reconciled before crystallization (Roller Coaster safe cycle).

---

## 2. Major Components

### 2.1 Artifact Manifest (Central Data Structure)

A versioned, deterministic registry of every cross-root artifact.

Fields per entry:
- logical_name: string (e.g., "Chained_Source_of_Truth_Kimi_Binding")
- engine_path: absolute path under engine/nnos/
- apps_path: absolute path under apps/
- sync_mode: enum { EXACT, VARIANT_AUTHORIZED, REFERENCE_ONLY }
- denom_mapping: list of 1..8 integers (which denominators this artifact primarily serves)
- last_verified: ISO-8601 timestamp + SHA-256 at verification time
- variant_rule (only for VARIANT): prose + optional predicate (e.g., "minor root-context wording only; core 8-denom enumeration and §7 coherence record must be identical")

Manifest is itself an artifact in the manifest (self-referential, Origin Vault property).

Current initial manifest (2026-05-30) includes at minimum:
- Both Chained and both Minimal binding files (VARIANT for Minimals, EXACT intent for full Chained with documented tolerance)
- All .jstr files under the two jasterish-microkernel/ trees (EXACT)
- neurobalance_coordinator.py + minimal_types.py (EXACT)
- The three dated 2026-05-29/30 Efficiency / Binary-Opt / Denominators docs (REFERENCE or VARIANT)
- Compiler self-host triad (apps/docs/) and kernel port triads (engine/docs/) — cross-referenced

### 2.2 Coherence Checker

Two-phase engine:
1. **Structural Phase** (fast, deterministic): directory walk restricted to manifest paths + presence/absence check.
2. **Hash Phase**: for every EXACT entry compute H_engine and H_apps; equality test is the mathematical proof step.
3. **Semantic Phase** (for VARIANT): line-by-line or section-by-section diff with an allow-list of permitted differences (root-scoped wording). Any unpermitted difference is treated as drift.

### 2.3 Denominator & Efficiency Mapper / Verifier

A lightweight deterministic parser that extracts explicit mapping statements from prose, code comments, or structured metadata.  
Rule: a proposal or change is "grounded" only if it contains at least one sentence of the form "This satisfies Denominators {2,6,7} + Efficiency Mandate via smallest-safe INT8 usage in the emitted .data layout (Origin Vault + Budget)."

The mapper produces a coverage vector (8-bit mask + efficiency flag). Coverage < 100% on a critical path triggers Criteria Enforcement escalation.

### 2.4 Propagation Controller

- Canonical → Working Copy flow only (never reverse).
- For EXACT items: byte-copy + immediate post-copy hash verification.
- For VARIANT items: content transformation (template substitution for root name) + verification against the documented variant_rule.
- All propagations append an Origin Vault entry.

### 2.5 Report Generator

Produces two artifacts per scan:
- Human: markdown delta report with evidence excerpts and denominator mapping table.
- Machine: JSON with {timestamp, manifest_version, exact_matches, divergences: [{path_pair, h_engine, h_apps, reason, denom_impact}], proof: "I holds for S_exact at T"}.

---

## 3. Operational Flows

### Primary Scan-and-Reconcile Flow (Deterministic)

1. Load current manifest (from binding or dedicated manifest file).
2. Execute Coherence Checker.
3. For each divergence:
   a. Classify (structural, hash, semantic, missing triad, legacy-5 language).
   b. Map impact to the 8 denominators.
   c. If safety-critical (kernel/compiler interface or binding layer), halt and escalate to Criteria + Live Context agents.
   d. Otherwise, record in binding §7 and (if authorized) queue for propagation.
4. If clean or reconciled, emit report containing the mathematical coherence proof (hash equality statements).
5. Update binding coherence record with new hashes and timestamp.
6. Propagate any binding updates to the other root.

### Trigger Conditions
- Explicit invocation by Grok or Live Context Maintenance Agent.
- Any write to a manifest-listed file in either root (via future git hook or watcher).
- Before any Jasterish compiler self-host test or kernel QEMU boot attempt.
- On schedule (see Automation section in Technical Spec).

### Roller Coaster Integration (Controlled Oscillation)

The agent explicitly supports bounded, intentional divergence during "ascent" (active development on one root, e.g., compiler .data emission work in apps) provided:
- A time-box and success criterion are recorded.
- NeuroBalance Coordinator (via Compute Footprint) is consulted.
- Crystallization (merge/reconcile) phase is mandatory and enforced by the agent before the next "relief" (stable release or binding snapshot).

This directly implements denominators 3, 4, and 5 while protecting 2 (Budget) and 7 (Origin Vault).

---

## 4. Data Structures (Pseudocode / Deterministic)

```python
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
import hashlib

class SyncMode(Enum):
    EXACT = 0
    VARIANT_AUTHORIZED = 1
    REFERENCE_ONLY = 2

@dataclass(frozen=True)
class ManifestEntry:
    logical_name: str
    engine_path: Path
    apps_path: Path
    sync_mode: SyncMode
    denom_mask: int          # bitmask of 8 denominators
    efficiency: bool
    variant_rule: str | None

@dataclass
class CoherenceResult:
    timestamp: str
    exact_ok: list[tuple[ManifestEntry, str, str]]  # entry, h_e, h_a
    divergences: list[dict]
    proof: str   # "I holds for S_exact at T" or explanation
```

All structures are immutable where possible; updates produce new versions with provenance.

---

## 5. Integration with NeuroBalance & Efficiency Mandate

The agent registers its own Compute Footprint (token and CPU cost of a full scan) with the NeuroBalance Coordinator.  
High-cost scans may be throttled or replaced by incremental hash checks during Roller Coaster ascent phases.

All reports preferentially use INT8/INT16 encodings for counts and masks.

---

## 6. Safety & Failure Modes

- Missing manifest entry for a newly created cross-root artifact → automatic escalation (violates Primitive Traceability #6 and Origin Vault #7).
- Unauthorized reverse propagation (apps → engine canonical) → hard reject.
- Legacy "5 Denominator" language introduced in new files → immediate flag + mandatory rewrite before acceptance.

---

**End of Design Specification**

Paired with the Requirements (NEURODIOS-DRSA-REQ-001) and the forthcoming Technical Specification, this document completes the mandated triad for the Dual-Root Synchronization Agent.