# Task 2 Report: Implement Output Comparison

## What was implemented

Added the `compare_output()` function to `System/apps/scripts/jasterish_regression.py`,
along with the helper `_normalize_trailing_newlines()`. The function supports the
three comparison modes required by the regression harness:

- `exact` — normalizes trailing newlines and compares the full output.
- `contains` — verifies every non-empty line in the golden file appears somewhere
  in the actual output.
- `regex` — verifies every non-empty line in the golden file matches somewhere in
  the actual output using `re.search()`.

If the golden file is missing or the comparison mode is unknown, the function
returns a failing result with a descriptive message.

Added four unit tests to `System/apps/tests/test_regression_lib.py` covering:

- `test_compare_exact_pass`
- `test_compare_exact_fail`
- `test_compare_contains_pass`
- `test_compare_regex_pass`

The existing `test_discover_cases_finds_test_toml` test from Task 1 remains
passing.

## Test command run and results

```bash
cd System/apps && .venv/bin/python -m pytest tests/test_regression_lib.py -v
```

Output:

```
============================= test session starts ==============================
platform darwin -- Python 3.14.6, pytest-9.1.1, pluggy-1.6.0 -- /Users/nnos/Projects/Sovereign/System/apps/.venv/bin/python
cachedir: .pytest_cache
rootdir: /Users/nnos/Projects/Sovereign/System/apps
collecting ... collected 5 items

tests/test_regression_lib.py::test_discover_cases_finds_test_toml PASSED [ 20%]
tests/test_regression_lib.py::test_compare_exact_pass PASSED             [ 40%]
tests/test_regression_lib.py::test_compare_exact_fail PASSED             [ 60%]
tests/test_regression_lib.py::test_compare_contains_pass PASSED          [ 80%]
tests/test_regression_lib.py::test_compare_regex_pass PASSED             [100%]

============================== 5 passed in 0.04s ===============================
```

## Commits made

```
[docs/jasterish-updates 2262c2a] feat(regression): implement exact/contains/regex comparison
 2 files changed, 71 insertions(+), 1 deletion(-)
```

## Concerns

None. The brief was complete and the implementation matches the specified
interfaces and behavior exactly.
