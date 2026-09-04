# System/apps — Design Spec


> **IMMUTABLE BASELINE** — Do not rewrite this document.
> Policy: `Structure/spec-immutability/` · Enforce: `python3 Structure/spec-immutability/scripts/check_specs.py`
> Changes after seal: **amend only** under `## Amendments` (append). Never replace the body above `<!-- SPEC-BASELINE-END -->`.

Apps monorepo/workspace design: isolate app packages, share System primitives where needed.

<!-- SPEC-BASELINE-END -->

## Amendments

_No amendments yet. Append new entries below this line only. Do not edit the baseline above the marker._

### 2026-09-02: Command-Execution Boundary Design

Process-launch choices are represented by immutable allowlist mappings. The
Jasterish harness selects fixed architecture, QEMU, Make, Python, and compiler
values, validates corpus containment, and uses fixed case-local filenames. The
provenance generator selects fixed Git and OpenSSL operations and transfers
private-key content through standard input or inherited file descriptors.
