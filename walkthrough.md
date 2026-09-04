# Security Audit: CodeQL Command-Execution Boundaries

Confidentiality: Internal engineering review

## Status

| Item | Result |
|---|---|
| Initial findings | 6 critical CodeQL command-line findings |
| Locally remediated | 6 |
| Security tests | 13 passed |
| Semgrep | 0 findings across 151 Python rules |
| Gitleaks current tree | 0 findings |
| Remote CodeQL closure | Pending publication and GitHub Actions run |

The scanner skill referenced by the security-planning procedure was not
installed. The repository was therefore checked with installed Semgrep 1.175.0,
Gitleaks 8.30.1, standard-library security tests, Python compilation, and
Actionlint. GitHub CodeQL remains the authoritative alert-closure mechanism.

## Vulnerability Report

| ID | GitHub alert | File | Original line | Severity | Status | Remediation |
|---|---:|---|---:|---|---|---|
| CS-EXEC-001 | 171 | `scripts/generate_release_provenance.py` | 24 | Critical | Fixed locally | Replaced arbitrary command lists with allowlisted Git/OpenSSL operations; private-key paths no longer enter OpenSSL arguments. |
| CS-EXEC-002 | 172 | `scripts/jasterish_orchestrator.py` | 31 | Critical | Fixed locally | JSON-mode launch now contains only a validated interpreter, confined internal script, fixed flags, allowlisted architecture, and fixed corpus argument. |
| CS-EXEC-003 | 173 | `scripts/jasterish_orchestrator.py` | 40 | Critical | Fixed locally | Normal-mode launch uses the same validated command construction and confined working directory. |
| CS-EXEC-004 | 174 | `scripts/jasterish_regression.py` | 131 | Critical | Fixed locally | Compiler selection is confined to executable repository build outputs; architecture and case-local arguments are selected from fixed values. |
| CS-EXEC-005 | 175 | `scripts/jasterish_regression.py` | 240 | Critical | Fixed locally | QEMU executable, machine, and CPU are selected by a two-entry architecture allowlist. |
| CS-EXEC-006 | 176 | `scripts/jasterish_regression.py` | 296 | Critical | Fixed locally | Self-host bootstrap uses the same trusted compiler and fixed case-local input/output arguments. |

## Verification Evidence

- `python3 -m unittest -v tests.test_command_execution_boundaries`: 13 passed.
- Python compilation: passed for all modified Python files and the new test.
- Compiler integration: compilation completed for four x86-64 corpus cases;
  execution was skipped with the expected `ENOEXEC` result on the ARM host.
- Semgrep: 151 rules, 6 targets, 0 findings.
- Gitleaks current-tree scan: 0 findings.
- Actionlint: advanced CodeQL workflow passed local syntax validation.
- Checkout v7 commit `3d3c42e5aac5ba805825da76410c181273ba90b1`
  and CodeQL v4 commit `cdf488f595d80d6e07e03d4674febd5ab45fa938`
  were verified by GitHub before pinning.

## Residual Risk

1. GitHub alerts remain open until the branch is published and CodeQL analyzes
   the new commit.
2. No local CodeQL CLI is installed, so exact query closure cannot be asserted
   from local tools alone.
3. Gitleaks reported eight historical `generic-api-key` matches in four removed
   dataset files across two old commits. The matches are absent from the current
   tree and were not exposed during this audit; they require separate content
   classification before any destructive history rewrite.
