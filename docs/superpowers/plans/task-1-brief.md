## Task 1: Create the shared regression library

**Files:**
- Create: `System/apps/scripts/jasterish_regression.py`
- Test: `System/apps/tests/test_regression_lib.py`

**Interfaces:**
- Produces:
  - `class Case` — represents one regression case directory.
  - `class Result` — `status: str`, `arch: str`, `duration: float`, `detail: str`.
  - `discover_cases(root: Path) -> list[Case]`
  - `run_case(case: Case, arch: str, update: bool) -> Result`
  - `compare_output(actual: str, golden_path: Path, mode: str) -> tuple[bool, str]`
  - `report(results: list[Result], json_mode: bool) -> int`

- [ ] **Step 1: Write the failing unit test for case discovery**

```python
# System/apps/tests/test_regression_lib.py
from pathlib import Path
import tempfile

from scripts.jasterish_regression import discover_cases


def test_discover_cases_finds_test_toml():
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        case_dir = root / "print-literal"
        case_dir.mkdir()
        (case_dir / "test.toml").write_text('name = "print-literal"\nkind = "compiler"\n')
        (case_dir / "main.jstr").write_text('print 42')
        cases = discover_cases(root)
        assert len(cases) == 1
        assert cases[0].name == "print-literal"
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd System/apps && .venv/bin/python -m pytest tests/test_regression_lib.py::test_discover_cases_finds_test_toml -v`

Expected: `ModuleNotFoundError` or `ImportError` for `scripts.jasterish_regression`.

- [ ] **Step 3: Implement minimal case discovery**

```python
# System/apps/scripts/jasterish_regression.py
from __future__ import annotations

import tomllib
from dataclasses import dataclass
from pathlib import Path


@dataclass
class Case:
    root: Path
    name: str
    kind: str
    archs: list[str]
    timeout: int
    compare: str

    @property
    def source_path(self) -> Path:
        return self.root / "main.jstr"


def discover_cases(root: Path) -> list[Case]:
    cases: list[Case] = []
    if not root.exists():
        return cases
    for entry in sorted(root.iterdir()):
        if not entry.is_dir():
            continue
        manifest = entry / "test.toml"
        if not manifest.exists():
            continue
        data = tomllib.loads(manifest.read_text())
        cases.append(
            Case(
                root=entry,
                name=data.get("name", entry.name),
                kind=data["kind"],
                archs=data.get("archs", ["x86_64"]),
                timeout=data.get("timeout", 30),
                compare=data.get("compare", "exact"),
            )
        )
    return cases
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd System/apps && .venv/bin/python -m pytest tests/test_regression_lib.py::test_discover_cases_finds_test_toml -v`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd System/apps
git add scripts/jasterish_regression.py tests/test_regression_lib.py
git commit -m "feat(regression): add shared case discovery library"
```

---
