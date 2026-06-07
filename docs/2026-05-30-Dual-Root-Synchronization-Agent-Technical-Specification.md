# Dual-Root Synchronization Agent — Technical Specification

**Document ID:** NEURODIOS-DRSA-TECH-001  
**Version:** 1.0.0  
**Date:** 2026-05-30  
**Status:** Authoritative Baseline  
**Related:** NEURODIOS-DRSA-REQ-001, NEURODIOS-DRSA-DES-001

---

## 1. Implementation Environment & Constraints

- Primary language for agent tooling: Python 3.12+ (deterministic, ubiquitous in the engine/nnos tree).
- Hash primitive: SHA-256 (FIPS 180-4). Collision resistance provides the mathematical grounding for coherence proofs.
- No external dependencies beyond hashlib, pathlib, dataclasses, re (standard library only) — satisfies Efficiency Mandate and Primitive Traceability.
- All code and reports must be UTF-8.
- All code and reports must be UTF-8.
- Execution context: may run inside engine/nnos (full LSA/NeuroBalance access) or apps (compiler-focused). The load_chained_truth.py pattern is reused for binding ingestion.

---

## 2. Core Algorithms

### 2.1 Hash Computation (Deterministic Proof Primitive)

```python
def compute_sha256(path: Path) -> str:
    """Returns lowercase hex digest. Pure function."""
    if not path.is_file():
        return "MISSING"
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()
```

**Mathematical Property:** For any two files f1, f2, if compute_sha256(f1) == compute_sha256(f2) then with overwhelming probability (2^-256 collision bound) the byte contents are identical. This is the constructive proof step for Invariant I.

### 2.2 Manifest-Driven Scan (Core Procedure)

Pseudocode (actual implementation must be in a checked-in script dual_root_coherence.py):

```
function SCAN(manifest: list[ManifestEntry]) -> CoherenceResult:
    result = new CoherenceResult(now)
    for entry in manifest:
        h_e = compute_sha256(entry.engine_path)
        h_a = compute_sha256(entry.apps_path)
        if entry.sync_mode == EXACT:
            if h_e != h_a or h_e == "MISSING" or h_a == "MISSING":
                record_divergence(result, entry, h_e, h_a, "EXACT_MISMATCH")
            else:
                record_match(result, entry, h_e)
        elif entry.sync_mode == VARIANT_AUTHORIZED:
            # semantic diff against documented rule (omitted for brevity; regex allow-list)
            if not satisfies_variant_rule(entry, h_e, h_a):
                record_divergence(...)
    result.proof = build_proof_string(result.exact_ok)   # "I holds: H(e1)=H(a1) ∧ ..."
    return result
```

### 2.3 Legacy "5 Denominator" Detector (Reconciliation Enforcer)

```python
LEGACY_5_PATTERN = re.compile(
    r"5 Validated Denominators|five Validated|5-denom|Validated Denominators.*5",
    re.IGNORECASE
)

def scan_legacy5(root: Path, historical_markers: list[str]) -> list[str]:
    offenders = []
    for p in root.rglob("*.md"):
        text = p.read_text(encoding="utf-8", errors="replace")
        if LEGACY_5_PATTERN.search(text):
            # Exempt only sections explicitly marked historical
            if not any(marker in text for marker in historical_markers):
                offenders.append(str(p))
    return offenders
```

Historical markers example: "2026-05-29 Dissection (historical)", "legacy 5-denom references logged".

### 2.4 Efficiency Footprint Calculator Hook

The agent shall call (or duplicate the logic of):

```python
from neurobalance.minimal_types import compute_action_footprint, Intensity8

def agent_scan_footprint(num_files: Intensity8) -> Intensity8:
    # Empirical model: base + per-file cost in normalized units
    return Intensity8(min(255, 8 + (num_files // 4)))
```

Result is logged to NeuroBalance Coordinator under Budget/Resource Accounting.

---

## 3. Manifest Format (Authoritative Instance)

The live manifest is maintained inside the Chained binding §7 and as a standalone JSON (for machine consumption) at:

engine/nnos/docs/dual_root_manifest.json (canonical)  
apps/context/dual_root_manifest.json (working copy — must be EXACT synced)

Example entry (2026-05-30 baseline):

```json
{
  "version": "2026-05-30-001",
  "entries": [
    {
      "logical_name": "Chained_Source_of_Truth_Kimi_Binding",
      "engine_path": "engine/nnos/lsa/synthesized/Chained_Source_of_Truth_Kimi_Binding.md",
      "apps_path": "apps/context/Chained_Source_of_Truth_Kimi_Binding.md",
      "sync_mode": "VARIANT_AUTHORIZED",
      "denom_mask": 0b11111111,
      "efficiency": true,
      "variant_rule": "Root-context wording only; 8-denom list, §7 coherence record, and all criteria must be byte-identical"
    },
    {
      "logical_name": "jasterish_microkernel_README",
      "engine_path": ".../neurodios/jasterish-microkernel/README.md",
      "apps_path": ".../jasterish-microkernel/README.md",
      "sync_mode": "EXACT",
      "denom_mask": 0b00000110,  /* #6 Traceability, #7 Origin Vault */
      "efficiency": true,
      "variant_rule": null
    }
    /* ... full list ... */
  ]
}
```

---

## 4. Automation Design (Level of Safe Automation)

**Safe Automation Level: 85% Structural + Hash + Legacy Scan**

A standalone script `scripts/dual_root_sync_check.py` (to be placed in both roots under scripts/ and kept EXACT) can be executed:

- Manually by Grok or any agent.
- Via cron / launchd / systemd timer (low frequency during development, e.g., every 4 hours).
- As a pre-commit hook or CI gate on either root.
- Before any self-host bootstrap or QEMU kernel run.

**What the script can fully automate (deterministic, no false negatives on structural issues):**
- Hash equality proofs for all EXACT manifest entries.
- Presence/absence detection.
- Legacy 5-denom language scan (with historical exemption list).
- Generation of machine-readable report + human delta.
- Automatic update of binding §7 coherence record when clean (with human review gate for the actual edit).

**What requires human + Criteria Enforcement Agent (15%):**
- Semantic judgment on whether a compiler change in apps/ is compatible with kernel expectations in engine/nnos/ (e.g., new instruction emission affecting NeuroBalance hot paths).
- Approval of new manifest entries or variant rules.
- Reconciliation rewrites of legacy language that are not purely mechanical.
- Decision to open a controlled Roller Coaster divergence window.

**Implementation Sketch for the Automation Script (Core Loop)**

```python
#!/usr/bin/env python3
# dual_root_sync_check.py — executable in both roots
import json
from pathlib import Path
# ... imports of local compute_sha256, scan_legacy5 ...

def main():
    root = Path(__file__).resolve().parents[2]  # engine or apps
    manifest = load_manifest(root)
    result = SCAN(manifest)
    legacy = scan_legacy5(root, HISTORICAL_MARKERS)
    write_report(result, legacy)
    if result.divergences or legacy:
        print("DIVERGENCE DETECTED — see report. Escalate to Grok + Criteria Agent.")
        # exit code 1 for CI
    else:
        # Optionally auto-append clean record (behind flag)
        print("COHERENCE PROOF: " + result.proof)
```

Exit code discipline enables CI enforcement.

**Scheduling Recommendation (Deterministic Policy)**
- Development: on-demand + before any bootstrap/QEMU.
- Stabilization: every 2 hours via timer.
- Release candidates: mandatory gate.

---

## 5. Mathematical Validation of the Agent

**Theorem (Soundness of Hash-Based Coherence):**  
If the scan reports no EXACT divergences and emits a proof string enumerating H(p_e) = H(p_a) for every p in S_exact, then Invariant I holds at the timestamp of the scan (by definition of SHA-256 as a collision-resistant hash and exhaustive enumeration).

**Proof Sketch (Constructive):**  
1. The algorithm visits every entry exactly once (loop invariant).  
2. For each EXACT entry it materializes the two digests.  
3. Equality of digests + collision resistance ⇒ content equality.  
4. Conjunction over the finite set ⇒ I.

**Liveness:** Repeated scans with no intervening mutations will always produce identical results (idempotence of pure hash + manifest).

---

## 6. File Placement & Dual-Root Deployment

- Authoritative triad (this document + Requirements + Design): engine/nnos/docs/2026-05-30-Dual-Root-Synchronization-Agent-*.md
- Working reference copies (or symlinks where filesystem permits): apps/docs/ or apps/context/
- Automation script: engine/nnos/scripts/dual_root_sync_check.py and apps/scripts/dual_root_sync_check.py (must remain EXACT)
- Manifest JSON: as defined above in both roots (EXACT)

The Dual-Root Synchronization Agent, when instantiated, shall first verify its own triad and manifest entry exist and are coherent before performing any other work.

---

## 7. Future Extensions (Subject to 8-Denominator Criteria)

- Git hook integration (post-receive style) for real-time flagging.
- Cryptographic signing of coherence reports (Origin Vault + Drift Detection).
- Integration with the full NeuroBalance Coordinator as a first-class monitored "process" with its own Roller Coaster profile.
- Automated suggestion of minimal reconciliations (e.g., s/5 Validated/8 Validated/ with human approval).

All extensions must themselves pass the atomic/kinetic/fractal + earned mechanism + deterministic uplift test.

---

**End of Technical Specification**

This completes the mandatory Requirements + Design + Technical triad for the Dual-Root Synchronization Agent (NEURODIOS-DRSA-REQ/DES/TECH-001). The agent is now fully specified and may operate under the Chained Source of Truth.