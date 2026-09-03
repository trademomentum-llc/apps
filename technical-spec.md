# System/apps — Technical Spec


> **IMMUTABLE BASELINE** — Do not rewrite this document.
> Policy: `Structure/spec-immutability/` · Enforce: `python3 Structure/spec-immutability/scripts/check_specs.py`
> Changes after seal: **amend only** under `## Amendments` (append). Never replace the body above `<!-- SPEC-BASELINE-END -->`.

Per-app stacks as present in subfolders. Root specs only describe the apps workspace role.

<!-- SPEC-BASELINE-END -->

## Amendments

_No amendments yet. Append new entries below this line only. Do not edit the baseline above the marker._

### 2026-09-02: Command-Execution Controls

- `scripts/jasterish_regression.py` supports only `aarch64` and `x86_64`,
  accepts compiler overrides only when they resolve to an executable beneath
  the repository `target/` directory, and launches fixed case-local paths.
- `scripts/jasterish_orchestrator.py` confines corpora to their configured
  regression roots and passes a fixed `.` corpus argument from that directory.
- `scripts/generate_release_provenance.py` permits only named Git/OpenSSL
  operations, requires an absolute external signing-key path, and never places
  that path in an OpenSSL command.
- All subprocess APIs use argument arrays with `shell=False`.
- `.github/workflows/codeql-adv.yml` pins Checkout and CodeQL actions to
  signature-verified full commit SHAs.
