# Jasterish Regression Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Python regression harness with Makefile integration that verifies the JStar compiler, Jasterish Micro-Kernel, and self-hosted compiler produce expected outputs on x86_64 and aarch64.

**Architecture:** A shared Python library (`jasterish_regression.py`) provides discovery, execution, comparison, and reporting. Two thin project runners (`jstar_regression.py` and `jmk_regression.py`) configure the library for compiler and kernel cases. Makefile targets invoke the runners, and a small initial corpus demonstrates the three case kinds.

**Tech Stack:** Python 3.11+ (stdlib only), TOML, Make, QEMU, morphlex/cargo, shell.

## Global Constraints

- Use only the Python standard library — no new pip dependencies.
- Golden files are per-architecture: `expected.x86_64`, `expected.aarch64`.
- Comparison modes: `exact`, `contains`, `regex`.
- Runner exit code: `0` on all pass, `1` on any fail/timeout.
- `--update` refreshes golden files from current output.
- `--json` emits machine-readable results.
- Kernel cases reuse the existing `Makefile` and QEMU patterns in `System/engine/nnos/neurodios/jasterish-microkernel/`.
- Compiler cases use `${JASTERISH_COMPILER:-morphlex} jstar compile --target <arch> --input <src> --output <elf>`.

---

## File Structure

| File | Responsibility |
|------|----------------|
| `System/apps/scripts/jasterish_regression.py` | Shared library: case discovery, compilation/run orchestration, output comparison, reporting. |
| `System/apps/scripts/jstar_regression.py` | Compiler runner: configures shared library for compiler cases. |
| `System/apps/scripts/jmk_regression.py` | Kernel runner: configures shared library for kernel cases. |
| `System/apps/Makefile` | Thin Makefile adding `regression` and `regression-update` targets for the compiler project. |
| `System/apps/tests/regression/` | Compiler regression corpus. |
| `System/engine/nnos/neurodios/jasterish-microkernel/scripts/jmk_regression.py` | Symlink or wrapper to the kernel runner (keeps the runner near the kernel). |
| `System/engine/nnos/neurodios/jasterish-microkernel/Makefile` | Adds `regression` and `regression-update` targets. |
| `System/engine/nnos/neurodios/jasterish-microkernel/tests/regression/` | Kernel regression corpus. |

---

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

Run: `cd System/apps && python -m pytest tests/test_regression_lib.py::test_discover_cases_finds_test_toml -v`

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

Run: `cd System/apps && python -m pytest tests/test_regression_lib.py::test_discover_cases_finds_test_toml -v`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd System/apps
git add scripts/jasterish_regression.py tests/test_regression_lib.py
git commit -m "feat(regression): add shared case discovery library"
```

---

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

## Task 3: Implement compiler case execution

**Files:**
- Modify: `System/apps/scripts/jasterish_regression.py`
- Create: `System/apps/tests/regression/print-literal/main.jstr`
- Create: `System/apps/tests/regression/print-literal/test.toml`
- Create: `System/apps/tests/regression/print-literal/expected.x86_64`
- Create: `System/apps/tests/regression/print-literal/expected.aarch64`

**Interfaces:**
- Consumes: `Case`, `compare_output`.
- Produces: `run_compiler_case(case: Case, arch: str, update: bool) -> Result`, `Result` dataclass.

- [ ] **Step 1: Define the Result dataclass and add compiler runner function**

```python
# System/apps/scripts/jasterish_regression.py
from dataclasses import dataclass, field
import subprocess
import time


@dataclass
class Result:
    name: str
    arch: str
    status: str  # PASS | FAIL | SKIP | TIMEOUT
    duration: float
    detail: str = ""


def run_compiler_case(case: Case, arch: str, update: bool) -> Result:
    import os

    compiler = os.environ.get("JASTERISH_COMPILER", "morphlex")
    elf_path = case.root / f"{case.name}.{arch}.elf"
    build_cmd = [
        compiler, "jstar", "compile",
        "--target", arch,
        "--input", str(case.source_path),
        "--output", str(elf_path),
    ]

    t0 = time.monotonic()
    try:
        build = subprocess.run(
            build_cmd,
            capture_output=True,
            text=True,
            timeout=case.timeout,
        )
    except subprocess.TimeoutExpired:
        return Result(case.name, arch, "TIMEOUT", time.monotonic() - t0, "compile timed out")

    if build.returncode != 0:
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, f"compile failed:\n{build.stderr}")

    try:
        run = subprocess.run(
            [str(elf_path)],
            capture_output=True,
            text=True,
            timeout=case.timeout,
        )
    except subprocess.TimeoutExpired:
        return Result(case.name, arch, "TIMEOUT", time.monotonic() - t0, "run timed out")

    actual = run.stdout
    exit_code = run.returncode
    golden_path = case.root / f"expected.{arch}"

    if update:
        golden_path.write_text(actual)
        exit_golden = case.root / f"expected.{arch}.exit"
        exit_golden.write_text(str(exit_code))
        return Result(case.name, arch, "UPDATED", time.monotonic() - t0, f"wrote {golden_path}")

    if not golden_path.exists():
        return Result(case.name, arch, "SKIP", time.monotonic() - t0, f"missing {golden_path}")

    ok, detail = compare_output(actual, golden_path, case.compare)
    if not ok:
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, detail)

    exit_golden = case.root / f"expected.{arch}.exit"
    if exit_golden.exists():
        expected_exit = int(exit_golden.read_text().strip())
        if exit_code != expected_exit:
            return Result(
                case.name,
                arch,
                "FAIL",
                time.monotonic() - t0,
                f"exit code mismatch: expected {expected_exit}, got {exit_code}",
            )

    return Result(case.name, arch, "PASS", time.monotonic() - t0, "")
```

- [ ] **Step 2: Create the first compiler regression case**

```jasterish
# System/apps/tests/regression/print-literal/main.jstr
print 42
```

```toml
# System/apps/tests/regression/print-literal/test.toml
name = "print-literal"
kind = "compiler"
archs = ["x86_64", "aarch64"]
timeout = 30
compare = "exact"
```

- [ ] **Step 3: Generate golden files manually**

Run the program through the compiler for each architecture to produce output, or write the expected output by inspection:

```bash
cd System/apps
export JASTERISH_COMPILER=${JASTERISH_COMPILER:-morphlex}
$JASTERISH_COMPILER jstar compile --target x86_64 --input tests/regression/print-literal/main.jstr --output /tmp/print-literal.x86_64.elf
chmod +x /tmp/print-literal.x86_64.elf
/tmp/print-literal.x86_64.elf > tests/regression/print-literal/expected.x86_64

$JASTERISH_COMPILER jstar compile --target aarch64 --input tests/regression/print-literal/main.jstr --output /tmp/print-literal.aarch64.elf
chmod +x /tmp/print-literal.aarch64.elf
/tmp/print-literal.aarch64.elf > tests/regression/print-literal/expected.aarch64
```

- [ ] **Step 4: Add a unit test for compiler case execution**

```python
# System/apps/tests/test_regression_lib.py
from pathlib import Path
import tempfile

from scripts.jasterish_regression import Case, run_compiler_case


def test_run_compiler_case_skips_without_golden():
    with tempfile.TemporaryDirectory() as tmp:
        case_dir = Path(tmp) / "print-literal"
        case_dir.mkdir()
        (case_dir / "main.jstr").write_text("print 42")
        case = Case(root=case_dir, name="print-literal", kind="compiler", archs=["x86_64"], timeout=30, compare="exact")
        result = run_compiler_case(case, "x86_64", update=False)
        assert result.status == "SKIP"
```

- [ ] **Step 5: Run tests**

Run: `cd System/apps && python -m pytest tests/test_regression_lib.py -v`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
cd System/apps
git add scripts/jasterish_regression.py tests/test_regression_lib.py tests/regression/print-literal/
git commit -m "feat(regression): add compiler case execution and first sample"
```

---

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

## Task 5: Implement kernel case execution

**Files:**
- Modify: `System/apps/scripts/jasterish_regression.py`
- Create: `System/apps/scripts/jmk_regression.py`
- Create: `System/engine/nnos/neurodios/jasterish-microkernel/scripts/jmk_regression.py`
- Modify: `System/engine/nnos/neurodios/jasterish-microkernel/Makefile`
- Create: `System/engine/nnos/neurodios/jasterish-microkernel/tests/regression/boot-to-shell/test.toml`
- Create: `System/engine/nnos/neurodios/jasterish-microkernel/tests/regression/boot-to-shell/expected.x86_64`
- Create: `System/engine/nnos/neurodios/jasterish-microkernel/tests/regression/boot-to-shell/expected.aarch64`

**Interfaces:**
- Consumes: `Case`, `compare_output`, `Result`.
- Produces: `run_kernel_case(case: Case, arch: str, update: bool) -> Result`.

- [ ] **Step 1: Implement kernel runner function**

```python
# System/apps/scripts/jasterish_regression.py
import os
import shutil


def run_kernel_case(case: Case, arch: str, update: bool, kernel_dir: Path | None = None) -> Result:
    if kernel_dir is None:
        # Default when called from the shared library location (System/apps/scripts/)
        kernel_dir = Path(__file__).resolve().parent.parent.parent / "engine" / "nnos" / "neurodios" / "jasterish-microkernel"
    build_dir = kernel_dir
    t0 = time.monotonic()

    # Stage init overlay if provided
    init_source = case.root / "main.jstr"
    if init_source.exists():
        target_init = build_dir / "init_user.jstr"
        shutil.copy(init_source, target_init)

    try:
        build = subprocess.run(
            ["make", f"ARCH={arch}", "clean", "build"],
            cwd=build_dir,
            capture_output=True,
            text=True,
            timeout=case.timeout,
        )
    except subprocess.TimeoutExpired:
        return Result(case.name, arch, "TIMEOUT", time.monotonic() - t0, "kernel build timed out")

    if build.returncode != 0:
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, f"kernel build failed:\n{build.stderr}")

    qemu = f"qemu-system-{arch}"
    machine = "virt" if arch == "aarch64" else "q35"
    cpu = "cortex-a72" if arch == "aarch64" else "qemu64"
    kernel_bin = build_dir / "jmk.bin"
    log_file = case.root / f"actual.{arch}.log"

    qemu_cmd = [
        qemu,
        "-machine", machine,
        "-cpu", cpu,
        "-m", "512",
        "-serial", "stdio",
        "-no-reboot",
        "-no-shutdown",
        "-display", "none",
        "-kernel", str(kernel_bin),
    ]

    try:
        with log_file.open("w") as log:
            proc = subprocess.Popen(qemu_cmd, stdout=log, stderr=subprocess.STDOUT, text=True)
            try:
                proc.wait(timeout=case.timeout)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
                return Result(case.name, arch, "TIMEOUT", time.monotonic() - t0, f"QEMU timed out after {case.timeout}s")
    except Exception as exc:
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, f"QEMU launch failed: {exc}")

    actual = log_file.read_text()
    golden_path = case.root / f"expected.{arch}"

    if update:
        golden_path.write_text(actual)
        return Result(case.name, arch, "UPDATED", time.monotonic() - t0, f"wrote {golden_path}")

    if not golden_path.exists():
        return Result(case.name, arch, "SKIP", time.monotonic() - t0, f"missing {golden_path}")

    ok, detail = compare_output(actual, golden_path, case.compare)
    if not ok:
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, detail)

    return Result(case.name, arch, "PASS", time.monotonic() - t0, "")
```

- [ ] **Step 2: Create the kernel runner CLI**

```python
# System/apps/scripts/jmk_regression.py
#!/usr/bin/env python3
from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent / "apps" / "scripts"))

from jasterish_regression import discover_cases, report, run_cases, run_kernel_case


def main() -> int:
    parser = argparse.ArgumentParser(description="Jasterish Micro-Kernel regression runner")
    parser.add_argument("corpus", type=Path, help="Path to regression corpus directory")
    parser.add_argument("--arch", action="append", help="Architecture to test (repeatable)")
    parser.add_argument("--update", action="store_true", help="Update golden files")
    parser.add_argument("--json", action="store_true", help="Emit JSON report")
    args = parser.parse_args()

    kernel_dir = Path(__file__).resolve().parent.parent
    cases = [c for c in discover_cases(args.corpus) if c.kind == "kernel"]
    results = run_cases(cases, args.arch, args.update, lambda case, arch, update: run_kernel_case(case, arch, update, kernel_dir=kernel_dir))
    return report(results, args.json)


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 3: Create a kernel-tree wrapper for the runner**

```python
# System/engine/nnos/neurodios/jasterish-microkernel/scripts/jmk_regression.py
#!/usr/bin/env python3
import sys
from pathlib import Path

# The real runner and shared library live in System/apps/scripts/
APPS_SCRIPTS = Path(__file__).resolve().parent.parent.parent.parent.parent.parent / "apps" / "scripts"
sys.path.insert(0, str(APPS_SCRIPTS))

from jmk_regression import main  # noqa: E402

if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 4: Add kernel Makefile targets**

```makefile
# System/engine/nnos/neurodios/jasterish-microkernel/Makefile
REGRESSION_DIR := tests/regression

regression:
	@python3 scripts/jmk_regression.py $(REGRESSION_DIR)

regression-update:
	@python3 scripts/jmk_regression.py $(REGRESSION_DIR) --update
```

- [ ] **Step 5: Create the first kernel regression case**

```toml
# System/engine/nnos/neurodios/jasterish-microkernel/tests/regression/boot-to-shell/test.toml
name = "boot-to-shell"
kind = "kernel"
archs = ["x86_64", "aarch64"]
timeout = 30
compare = "contains"
```

```
# System/engine/nnos/neurodios/jasterish-microkernel/tests/regression/boot-to-shell/expected.x86_64
BOOT
JMK>
```

```
# System/engine/nnos/neurodios/jasterish-microkernel/tests/regression/boot-to-shell/expected.aarch64
BOOT
JMK>
```

- [ ] **Step 6: Run the kernel regression suite**

Run: `cd System/engine/nnos/neurodios/jasterish-microkernel && make regression`

Expected: `PASS boot-to-shell x86_64 ...` and/or `PASS boot-to-shell aarch64 ...` (if QEMU and toolchain are available).

- [ ] **Step 7: Commit**

```bash
cd System/engine/nnos/neurodios/jasterish-microkernel
git add scripts/jmk_regression.py Makefile tests/regression/boot-to-shell/
cd ../../../../../..
cd System/apps
git add scripts/jmk_regression.py scripts/jasterish_regression.py
git commit -m "feat(regression): add kernel runner and boot-to-shell case"
```

---

## Task 6: Implement self-hosting regression

**Files:**
- Modify: `System/apps/scripts/jasterish_regression.py`
- Create: `System/apps/tests/regression/self-host/test.toml`
- Create: `System/apps/tests/regression/self-host/expected.x86_64`
- Modify: `System/apps/scripts/jstar_regression.py`

**Interfaces:**
- Consumes: `Case`, `Result`.
- Produces: `run_self_host_case(case: Case, arch: str, update: bool) -> Result`.

- [ ] **Step 1: Implement self-host runner function**

```python
# System/apps/scripts/jasterish_regression.py
import hashlib
import os


def run_self_host_case(case: Case, arch: str, update: bool) -> Result:
    t0 = time.monotonic()
    reference = case.root / "main.jstr"
    if not reference.exists():
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, "missing compiler.jstr reference")

    work = case.root / f"work.{arch}"
    work.mkdir(exist_ok=True)
    stage0 = work / "stage0.elf"
    stage1 = work / "stage1.elf"
    stage2 = work / "stage2.elf"

    # Stage 0: reference compiler from Rust toolchain
    compiler = os.environ.get("JASTERISH_COMPILER", "morphlex")
    try:
        build = subprocess.run(
            [compiler, "jstar", "compile", "--target", arch, "--input", str(reference), "--output", str(stage0)],
            capture_output=True,
            text=True,
            timeout=case.timeout,
        )
    except subprocess.TimeoutExpired:
        return Result(case.name, arch, "TIMEOUT", time.monotonic() - t0, "stage0 compile timed out")
    if build.returncode != 0:
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, f"stage0 compile failed:\n{build.stderr}")

    # Stage 1: compiler compiled by stage0
    for stage_in, stage_out in [(stage0, stage1), (stage1, stage2)]:
        try:
            run = subprocess.run(
                [str(stage_in)],
                input=reference.read_bytes(),
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=case.timeout,
            )
        except subprocess.TimeoutExpired:
            return Result(case.name, arch, "TIMEOUT", time.monotonic() - t0, f"{stage_out.name} generation timed out")
        if run.returncode != 0:
            return Result(case.name, arch, "FAIL", time.monotonic() - t0, f"{stage_out.name} generation failed:\n{run.stderr.decode('utf-8', errors='replace')}")
        stage_out.write_bytes(run.stdout)
        os.chmod(stage_out, 0o755)

    # Byte-identical check
    s1 = stage1.read_bytes()
    s2 = stage2.read_bytes()
    if s1 != s2:
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, "stage1 and stage2 are not byte-identical")

    digest = hashlib.sha256(s1).hexdigest()
    golden_path = case.root / f"expected.{arch}"

    if update:
        golden_path.write_text(digest + "\n")
        return Result(case.name, arch, "UPDATED", time.monotonic() - t0, f"wrote sha256 {digest}")

    if not golden_path.exists():
        return Result(case.name, arch, "SKIP", time.monotonic() - t0, f"missing {golden_path}")

    expected = golden_path.read_text().strip()
    if digest != expected:
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, f"sha256 mismatch: expected {expected}, got {digest}")

    return Result(case.name, arch, "PASS", time.monotonic() - t0, "")
```

- [ ] **Step 2: Create the self-host regression case**

```toml
# System/apps/tests/regression/self-host/test.toml
name = "self-host"
kind = "self-host"
archs = ["x86_64"]
timeout = 120
compare = "exact"
```

```bash
cd System/apps
ln -s ../../../jstar/compiler.jstr tests/regression/self-host/main.jstr
```

- [ ] **Step 3: Update compiler runner to dispatch self-host cases**

```python
# System/apps/scripts/jstar_regression.py
def main() -> int:
    parser = argparse.ArgumentParser(description="JStar compiler regression runner")
    parser.add_argument("corpus", type=Path, help="Path to regression corpus directory")
    parser.add_argument("--arch", action="append", help="Architecture to test (repeatable; default: all in case)")
    parser.add_argument("--update", action="store_true", help="Update golden files from current output")
    parser.add_argument("--json", action="store_true", help="Emit JSON report")
    args = parser.parse_args()

    results: list[Result] = []
    for case in discover_cases(args.corpus):
        if case.kind not in ("compiler", "self-host"):
            continue
        for arch in args.arch or case.archs:
            if arch not in case.archs:
                continue
            if (case.root / f"skip.{arch}").exists():
                results.append(Result(case.name, arch, "SKIP", 0.0, "skip marker present"))
                continue
            if case.kind == "compiler":
                results.append(run_compiler_case(case, arch, args.update))
            elif case.kind == "self-host":
                results.append(run_self_host_case(case, arch, args.update))
    return report(results, args.json)
```

- [ ] **Step 4: Generate the initial self-host golden file**

Run: `cd System/apps && python3 scripts/jstar_regression.py tests/regression --update --arch x86_64`

Expected: `UPDATED self-host x86_64 ...` and the sha256 is written to `tests/regression/self-host/expected.x86_64`.

- [ ] **Step 5: Run the full compiler regression suite**

Run: `cd System/apps && make regression`

Expected: all compiler and self-host cases PASS.

- [ ] **Step 6: Commit**

```bash
cd System/apps
git add scripts/jasterish_regression.py scripts/jstar_regression.py tests/regression/self-host/
git commit -m "feat(regression): add self-hosting verification case"
```

---

## Task 7: Add top-level orchestrator

**Files:**
- Create: `System/apps/scripts/jasterish_orchestrator.py`

**Interfaces:**
- Consumes: `jstar_regression.py`, `jmk_regression.py`.
- Produces: combined report across compiler, kernel, and self-host suites.

- [ ] **Step 1: Create the top-level orchestrator**

```python
# System/apps/scripts/jasterish_orchestrator.py
#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def run_suite(
    script: Path,
    corpus: Path,
    archs: list[str] | None,
    update: bool,
    json_mode: bool,
    results: list[dict],
) -> int:
    cmd = [sys.executable, str(script), str(corpus)]
    if update:
        cmd.append("--update")
    if json_mode:
        cmd.append("--json")
    if archs:
        for arch in archs:
            cmd.extend(["--arch", arch])

    if json_mode:
        result = subprocess.run(cmd, capture_output=True, text=True)
        try:
            results.extend(json.loads(result.stdout))
        except json.JSONDecodeError:
            print(result.stdout, end="")
            print(result.stderr, file=sys.stderr, end="")
        return result.returncode

    result = subprocess.run(cmd)
    return result.returncode


def main() -> int:
    parser = argparse.ArgumentParser(description="Jasterish combined regression harness")
    parser.add_argument("--compiler-corpus", type=Path, default=ROOT / "tests" / "regression")
    parser.add_argument("--kernel-corpus", type=Path, default=ROOT.parent / "engine" / "nnos" / "neurodios" / "jasterish-microkernel" / "tests" / "regression")
    parser.add_argument("--arch", action="append")
    parser.add_argument("--update", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    compiler_script = ROOT / "scripts" / "jstar_regression.py"
    kernel_script = ROOT.parent / "engine" / "nnos" / "neurodios" / "jasterish-microkernel" / "scripts" / "jmk_regression.py"

    results: list[dict] = []
    code = 0
    code |= run_suite(compiler_script, args.compiler_corpus, args.arch, args.update, args.json, results)
    code |= run_suite(kernel_script, args.kernel_corpus, args.arch, args.update, args.json, results)

    if args.json:
        print(json.dumps(results, indent=2))

    return code


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 2: Run the top-level orchestrator**

Run: `cd /Users/nnos/Projects/Sovereign/System/apps && python3 scripts/jasterish_orchestrator.py`

Expected: compiler and kernel suites both run; combined exit code reflects any failures.

- [ ] **Step 3: Commit**

```bash
cd /Users/nnos/Projects/Sovereign/System/apps
git add scripts/jasterish_orchestrator.py
git commit -m "feat(regression): add top-level combined orchestrator"
```

---

## Task 8: Add a few more compiler regression cases

**Files:**
- Create: `System/apps/tests/regression/arithmetic/`
- Create: `System/apps/tests/regression/control-flow/`
- Create: `System/apps/tests/regression/functions/`

**Interfaces:**
- Consumes: existing compiler runner.
- Produces: additional golden files.

- [ ] **Step 1: Create arithmetic case**

```jasterish
# System/apps/tests/regression/arithmetic/main.jstr
add 3 5
multiply it 2
return it
```

```toml
# System/apps/tests/regression/arithmetic/test.toml
name = "arithmetic"
kind = "compiler"
archs = ["x86_64", "aarch64"]
timeout = 30
compare = "exact"
```

Generate golden files by running the program and capturing stdout/exit code.

- [ ] **Step 2: Create control-flow case**

```jasterish
# System/apps/tests/regression/control-flow/main.jstr
a counter
store 0 into counter
while compare counter 3
add counter 1
store it into counter
end
return counter
```

```toml
# System/apps/tests/regression/control-flow/test.toml
name = "control-flow"
kind = "compiler"
archs = ["x86_64", "aarch64"]
timeout = 30
compare = "exact"
```

- [ ] **Step 3: Create function case**

```jasterish
# System/apps/tests/regression/functions/main.jstr
define answer
return 42
end
call answer
return it
```

```toml
# System/apps/tests/regression/functions/test.toml
name = "functions"
kind = "compiler"
archs = ["x86_64", "aarch64"]
timeout = 30
compare = "exact"
```

- [ ] **Step 4: Update golden files**

Run: `cd System/apps && python3 scripts/jstar_regression.py tests/regression --update`

- [ ] **Step 5: Run the compiler suite**

Run: `cd System/apps && make regression`

Expected: all cases PASS.

- [ ] **Step 6: Commit**

```bash
cd System/apps
git add tests/regression/arithmetic tests/regression/control-flow tests/regression/functions
git commit -m "feat(regression): expand compiler corpus with arithmetic, control-flow, and functions"
```

---

## Task 9: Verify and document

**Files:**
- Modify: `System/apps/docs/superpowers/specs/2026-07-27-jasterish-regression-harness-design.md`
- Modify: `System/engine/nnos/neurodios/jasterish-microkernel/README.md` (optional)
- Modify: `System/apps/README.md` (optional)

- [ ] **Step 1: Run the full combined regression suite**

Run: `cd /Users/nnos/Projects/Sovereign/System/apps && python3 scripts/jasterish_orchestrator.py`

Expected: exit 0 with all cases PASS.

- [ ] **Step 2: Update the design doc to mark status as implemented**

Change the status line in `System/apps/docs/superpowers/specs/2026-07-27-jasterish-regression-harness-design.md` from:

```markdown
**Status:** Design — pending implementation plan.
```

to:

```markdown
**Status:** Implemented.
```

- [ ] **Step 3: Add a short README section about regression testing**

Add to `System/apps/README.md` or create `System/apps/tests/regression/README.md`:

```markdown
## Regression tests

Run compiler and self-host regression:

```bash
make regression
```

Update golden files after intentional changes:

```bash
make regression-update
```
```

- [ ] **Step 4: Final commit**

```bash
cd /Users/nnos/Projects/Sovereign
git add System/apps/docs/superpowers/specs/2026-07-27-jasterish-regression-harness-design.md
git add System/apps/tests/regression/README.md || true
git commit -m "docs(regression): mark harness as implemented and add usage notes"
```

---

## Self-Review

### Spec coverage

| Spec section | Plan task |
|--------------|-----------|
| Shared Python library | Task 1, 2 |
| Compiler regression | Task 3, 4, 8 |
| Kernel regression | Task 5 |
| Self-hosting regression | Task 6 |
| Makefile integration | Task 4, 5 |
| Top-level orchestrator | Task 7 |
| `--update` and `--json` | Task 1, 4, 5, 6, 7 |
| Per-arch golden files | Task 3, 5, 6, 8 |

### Placeholder scan

- No TBD/TODO/"implement later".
- Every step includes concrete code or commands.
- File paths are exact.
- Function names are consistent across tasks.

### Type consistency

- `Case`, `Result`, `discover_cases`, `compare_output`, `run_compiler_case`, `run_kernel_case`, `run_self_host_case`, `report`, `run_cases` are used consistently.
- `Result.status` values are `PASS`, `FAIL`, `SKIP`, `TIMEOUT`, `UPDATED` everywhere.

### Open questions from spec

1. Kernel init overlay is staged by copying `main.jstr` to `init_user.jstr` in the kernel build directory.
2. Top-level orchestrator is implemented in Task 7.
3. Kernel golden files use `contains` mode initially to avoid unstable addresses/timestamps.
