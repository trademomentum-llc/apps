## Task 4: Add reporting and the compiler runner CLI

**Files:**
- Modify: `System/apps/scripts/jasterish_regression.py`
- Create: `System/apps/scripts/jstar_regression.py`
- Create: `System/apps/Makefile`

**Interfaces:**
- Consumes: `discover_cases`, `run_compiler_case`, `Result`.
- Produces: `report(results: list[Result], json_mode: bool) -> int`, `main()` in `jstar_regression.py`.

- [ ] **Step 1: Add report function and dispatch helper**

```python
# System/apps/scripts/jasterish_regression.py
import json
import sys
from collections.abc import Callable


def report(results: list[Result], json_mode: bool) -> int:
    if json_mode:
        print(json.dumps([{"name": r.name, "arch": r.arch, "status": r.status, "duration": r.duration, "detail": r.detail} for r in results], indent=2))
    else:
        for r in results:
            print(f"{r.status:<8} {r.name:<30} {r.arch:<8} {r.duration:.3f}s")
            if r.detail:
                for line in r.detail.splitlines():
                    print(f"         {line}")
        total = len(results)
        passed = sum(1 for r in results if r.status == "PASS")
        failed = sum(1 for r in results if r.status in ("FAIL", "TIMEOUT"))
        skipped = sum(1 for r in results if r.status == "SKIP")
        updated = sum(1 for r in results if r.status == "UPDATED")
        print(f"\nTotal: {total}  Pass: {passed}  Fail: {failed}  Skip: {skipped}  Updated: {updated}")

    return 0 if all(r.status in ("PASS", "SKIP", "UPDATED") for r in results) else 1


def run_cases(cases: list[Case], archs: list[str] | None, update: bool, runner: Callable[[Case, str, bool], Result]) -> list[Result]:
    results: list[Result] = []
    for case in cases:
        for arch in archs or case.archs:
            if arch not in case.archs:
                continue
            skip_marker = case.root / f"skip.{arch}"
            if skip_marker.exists():
                results.append(Result(case.name, arch, "SKIP", 0.0, f"skip marker present"))
                continue
            results.append(runner(case, arch, update))
    return results
```

- [ ] **Step 2: Create the compiler runner CLI**

```python
# System/apps/scripts/jstar_regression.py
#!/usr/bin/env python3
from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from jasterish_regression import (
    Case,
    Result,
    discover_cases,
    report,
    run_cases,
    run_compiler_case,
    run_self_host_case,
)


def main() -> int:
    parser = argparse.ArgumentParser(description="JStar compiler regression runner")
    parser.add_argument("corpus", type=Path, help="Path to regression corpus directory")
    parser.add_argument("--arch", action="append", help="Architecture to test (repeatable; default: all in case)")
    parser.add_argument("--update", action="store_true", help="Update golden files from current output")
    parser.add_argument("--json", action="store_true", help="Emit JSON report")
    args = parser.parse_args()

    cases = [c for c in discover_cases(args.corpus) if c.kind == "compiler"]
    results = run_cases(cases, args.arch, args.update, run_compiler_case)
    return report(results, args.json)


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 3: Create the compiler project Makefile**

```makefile
# System/apps/Makefile
REGRESSION_DIR := tests/regression

regression:
	@python3 scripts/jstar_regression.py $(REGRESSION_DIR)

regression-update:
	@python3 scripts/jstar_regression.py $(REGRESSION_DIR) --update
```

- [ ] **Step 4: Run the compiler regression suite**

Run: `cd System/apps && make regression`

Expected: `PASS print-literal x86_64 ...` and `PASS print-literal aarch64 ...` (if tooling is available).

- [ ] **Step 5: Commit**

```bash
cd System/apps
git add scripts/jstar_regression.py scripts/jasterish_regression.py Makefile
git commit -m "feat(regression): add compiler runner CLI and Makefile targets"
```

---
