## Task 2: Implement output comparison

**Files:**
- Modify: `System/apps/scripts/jasterish_regression.py`
- Test: `System/apps/tests/test_regression_lib.py`

**Interfaces:**
- Consumes: nothing new.
- Produces: `compare_output(actual: str, golden_path: Path, mode: str) -> tuple[bool, str]`

- [ ] **Step 1: Write failing tests for comparison modes**

```python
# System/apps/tests/test_regression_lib.py
from pathlib import Path
import tempfile

from scripts.jasterish_regression import compare_output


def test_compare_exact_pass():
    with tempfile.TemporaryDirectory() as tmp:
        golden = Path(tmp) / "expected.x86_64"
        golden.write_text("hello\n")
        ok, detail = compare_output("hello\n", golden, "exact")
        assert ok is True
        assert detail == ""


def test_compare_exact_fail():
    with tempfile.TemporaryDirectory() as tmp:
        golden = Path(tmp) / "expected.x86_64"
        golden.write_text("hello\n")
        ok, detail = compare_output("world\n", golden, "exact")
        assert ok is False
        assert "mismatch" in detail.lower()


def test_compare_contains_pass():
    with tempfile.TemporaryDirectory() as tmp:
        golden = Path(tmp) / "expected.x86_64"
        golden.write_text("BOOT\nJMK>\n")
        ok, detail = compare_output("BOOT\nJMK> prompt\n", golden, "contains")
        assert ok is True


def test_compare_regex_pass():
    with tempfile.TemporaryDirectory() as tmp:
        golden = Path(tmp) / "expected.x86_64"
        golden.write_text(r"^JMK>\s*$")
        ok, detail = compare_output("JMK> \n", golden, "regex")
        assert ok is True
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd System/apps && python -m pytest tests/test_regression_lib.py -v`

Expected: `AttributeError` or `ImportError` for `compare_output`.

- [ ] **Step 3: Implement comparison functions**

```python
# System/apps/scripts/jasterish_regression.py
import re


def _normalize_trailing_newlines(text: str) -> str:
    return text.rstrip("\n") + "\n" if text else text


def compare_output(actual: str, golden_path: Path, mode: str) -> tuple[bool, str]:
    if not golden_path.exists():
        return False, f"missing golden file: {golden_path}"

    golden = golden_path.read_text()

    if mode == "exact":
        if _normalize_trailing_newlines(actual) == _normalize_trailing_newlines(golden):
            return True, ""
        return False, f"output mismatch:\n--- expected ---\n{golden}\n--- actual ---\n{actual}"

    if mode == "contains":
        missing = [line for line in golden.splitlines() if line and line not in actual]
        if not missing:
            return True, ""
        return False, f"missing fragments: {missing}"

    if mode == "regex":
        missing = []
        for line in golden.splitlines():
            if not line:
                continue
            if not re.search(line, actual):
                missing.append(line)
        if not missing:
            return True, ""
        return False, f"unmatched regexes: {missing}"

    return False, f"unknown compare mode: {mode}"
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd System/apps && python -m pytest tests/test_regression_lib.py -v`

Expected: all four comparison tests PASS.

- [ ] **Step 5: Commit**

```bash
cd System/apps
git add scripts/jasterish_regression.py tests/test_regression_lib.py
git commit -m "feat(regression): implement exact/contains/regex comparison"
```

---
