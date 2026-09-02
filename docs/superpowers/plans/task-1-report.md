# Task 1 Report: Shared Regression Library

## What was implemented

Created the shared Python library for the Jasterish regression harness under `System/apps/`.

Files added:
- `System/apps/scripts/jasterish_regression.py` — the regression library.
- `System/apps/tests/test_regression_lib.py` — unit test for case discovery.

Library interfaces implemented (per the brief):
- `class Case` with fields `root: Path`, `name: str`, `kind: str`, `archs: list[str]`, `timeout: int`, `compare: str`, plus property `source_path: Path`.
- `discover_cases(root: Path) -> list[Case]` — scans `root` for subdirectories containing `test.toml`, parses them with `tomllib`, and returns sorted `Case` objects. Defaults match the brief: `archs=["x86_64"]`, `timeout=30`, `compare="exact"`, and `name` falls back to the directory name.

The implementation uses only the Python standard library (`dataclasses`, `pathlib`, `tomllib`).

## Test command run and results

Pytest was not available in the system Python, so a project-local virtual environment was created at `System/apps/.venv` and pytest was installed there.

Command:
```bash
cd System/apps && .venv/bin/python -m pytest tests/test_regression_lib.py -v
```

Output:
```
============================= test session starts ==============================
platform darwin -- Python 3.14.6, pytest-9.1.1, pluggy-1.6.0 -- /Users/nnos/Projects/Sovereign/System/apps/.venv/bin/python
cachedir: .pytest_cache
rootdir: /Users/nnos/Projects/Sovereign/System/apps
collecting ... collected 1 item

tests/test_regression_lib.py::test_discover_cases_finds_test_toml PASSED [100%]

============================== 1 passed in 0.03s ===============================
```

## Commits made

- `9e3f188` — `feat(regression): add shared case discovery library`
  - Added `scripts/jasterish_regression.py`
  - Added `tests/test_regression_lib.py`

## Concerns

- The repository has many unrelated uncommitted modifications. The commit above staged only the two new regression-harness files, so it does not include those pre-existing changes.
- The brief lists additional interfaces (`Result`, `run_case`, `compare_output`, `report`) under the "Produces" section, but Task 1's steps only implement `Case` and `discover_cases`. Those other interfaces are expected in later tasks.

## Verification of corrected test command

Verified that `System/apps/.venv` exists and pytest is installed there, and that the corrected test command passes:

```bash
cd System/apps && .venv/bin/python -m pytest tests/test_regression_lib.py -v
```

Output:

```
============================= test session starts ==============================
platform darwin -- Python 3.14.6, pytest-9.1.1, pluggy-1.6.0 -- /Users/nnos/Projects/Sovereign/System/apps/.venv/bin/python
cachedir: .pytest_cache
rootdir: /Users/nnos/Projects/Sovereign/System/apps
collecting ... collected 1 item

tests/test_regression_lib.py::test_discover_cases_finds_test_toml PASSED [100%]

============================== 1 passed in 0.03s ===============================
```
