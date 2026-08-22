# Task 3 Report: Compiler Case Execution

## What was implemented

- Added the `Result` dataclass to `scripts/jasterish_regression.py` with fields `name`, `arch`, `status`, `duration`, and optional `detail`.
- Implemented `run_compiler_case(case: Case, arch: str, update: bool) -> Result` in the same file. The function:
  - Resolves the compiler from `$JASTERISH_COMPILER`, defaulting to `morphlex`.
  - Compiles `case.source_path` to an architecture-specific ELF.
  - Runs the ELF, captures stdout and exit code.
  - In `update` mode, writes the captured output and exit code to golden files.
  - In normal mode, skips if the golden file is missing, then compares stdout against the golden file and optionally checks the exit code golden file.
  - Returns `Result` statuses: `PASS`, `FAIL`, `SKIP`, `TIMEOUT`, or `UPDATED`.
- One minor robustness adjustment: the missing-golden check is performed before invoking the compiler when not in update mode. This lets the harness skip cleanly without requiring a working compiler for cases that have no golden yet, and it makes the unit test pass in environments where `morphlex` is not installed.
- Created the first compiler regression sample:
  - `tests/regression/print-literal/main.jstr` — `print 42`
  - `tests/regression/print-literal/test.toml` — compiler case for `x86_64` and `aarch64`
  - `tests/regression/print-literal/expected.x86_64` — `42\n`
  - `tests/regression/print-literal/expected.aarch64` — `42\n`
- Added `test_run_compiler_case_skips_without_golden` to `tests/test_regression_lib.py`.

## Test command run and results

```bash
cd /Users/nnos/Projects/Sovereign/System/apps && .venv/bin/python -m pytest tests/test_regression_lib.py -v
```

Output:

```
tests/test_regression_lib.py::test_discover_cases_finds_test_toml PASSED
tests/test_regression_lib.py::test_compare_exact_pass PASSED
tests/test_regression_lib.py::test_compare_exact_fail PASSED
tests/test_regression_lib.py::test_compare_contains_pass PASSED
tests/test_regression_lib.py::test_compare_regex_pass PASSED
tests/test_regression_lib.py::test_run_compiler_case_skips_without_golden PASSED

============================== 6 passed in 0.04s ===============================
```

## Commits made

```
890cec8 feat(regression): add compiler case execution and first sample
```

## Concerns

- `morphlex` is not present in this environment and `$JASTERISH_COMPILER` is unset. Because the golden-missing check now happens before compilation, the regression-library unit test passes without invoking the compiler. A full end-to-end compile/run/compare test of the `print-literal` case still requires a working `morphlex` installation, which matches the task brief's stated assumption.
- Golden files were created by inspection (`42\n`) because the compiler could not be run here.
