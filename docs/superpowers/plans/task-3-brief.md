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
