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
