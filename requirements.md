# System/apps — Requirements


> **IMMUTABLE BASELINE** — Do not rewrite this document.
> Policy: `Structure/spec-immutability/` · Enforce: `python3 Structure/spec-immutability/scripts/check_specs.py`
> Changes after seal: **amend only** under `## Amendments` (append). Never replace the body above `<!-- SPEC-BASELINE-END -->`.

## Objective
Application collection workspace under the System tree for sovereign apps that are not NeuroDiOS kernel/core.

<!-- SPEC-BASELINE-END -->

## Amendments

_No amendments yet. Append new entries below this line only. Do not edit the baseline above the marker._

### 2026-09-02: Command-Execution Boundaries

1. Regression and provenance utilities shall reject unsupported executable,
   architecture, script, corpus, and signing-key selections before process
   creation.
2. Caller-selected corpus and private-key paths shall not be placed in child
   process command arrays.
3. Private signing keys shall remain outside the repository.
4. Third-party GitHub Actions shall be pinned to verified full commit SHAs.
