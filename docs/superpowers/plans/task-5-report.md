# Task 5 Report: Kernel Regression Harness

## What was implemented

- Added `run_kernel_case(case, arch, update, kernel_dir=None)` to `System/apps/scripts/jasterish_regression.py`.
  - Stages `case.root/main.jstr` as `init_user.jstr` in the kernel build directory when present.
  - Runs `make ARCH=<arch> clean build` in the kernel tree, using a sanitized environment (no inherited `MAKEFLAGS`/`MAKELEVEL`) so the harness can be invoked from `make` without recursive-make side effects.
  - Runs `qemu-system-<arch>` with the machine/CPU settings from the brief (`q35`/`qemu64` for x86_64, `virt`/`cortex-a72` for aarch64).
  - Captures serial output to `actual.<arch>.log`, compares against `expected.<arch>` using the existing `compare_output` helper, and supports `--update`.

- Created `System/apps/scripts/jmk_regression.py`, the CLI entry point for kernel regression cases.
  - It discovers kernel cases, filters by `--arch`, and delegates to `run_kernel_case`.
  - The CLI accepts `--update` and `--json` flags and reuses `discover_cases`, `run_cases`, and `report` from the shared library.
  - `main()` accepts an optional `kernel_dir` argument so the wrapper can pass the correct kernel tree path; relying on `Path(__file__)` inside the imported module would resolve to the apps script location, not the kernel tree.

- Created `System/engine/nnos/neurodios/jasterish-microkernel/scripts/jmk_regression.py` as a thin wrapper that adds the apps scripts directory to `sys.path` and calls the shared CLI with the local kernel directory.

- Added `regression` and `regression-update` Makefile targets to `System/engine/nnos/neurodios/jasterish-microkernel/Makefile`.

- Created the first kernel regression case:
  - `System/engine/nnos/neurodios/jasterish-microkernel/tests/regression/boot-to-shell/test.toml`
  - `System/engine/nnos/neurodios/jasterish-microkernel/tests/regression/boot-to-shell/expected.x86_64`
  - `System/engine/nnos/neurodios/jasterish-microkernel/tests/regression/boot-to-shell/expected.aarch64`

No existing interfaces from earlier tasks (`Case`, `discover_cases`, `compare_output`, `Result`, `report`, `run_cases`) were changed.

## Commands run and results

Syntax check:

```bash
python3 -m py_compile \
  System/apps/scripts/jmk_regression.py \
  System/engine/nnos/neurodios/jasterish-microkernel/scripts/jmk_regression.py \
  System/apps/scripts/jasterish_regression.py
```

Result: all three files compile without error.

Regression run:

```bash
cd System/engine/nnos/neurodios/jasterish-microkernel
make regression
```

Output:

```text
TIMEOUT  boot-to-shell                  x86_64   31.174s
         QEMU timed out after 30s
TIMEOUT  boot-to-shell                  aarch64  30.887s
         QEMU timed out after 30s

Total: 2  Pass: 0  Fail: 2  Skip: 0  Updated: 0
make: *** [regression] Error 1
```

The kernel build step succeeded for both architectures (`jmk.bin` and `jmk.elf` were produced). QEMU launched and the serial log was captured, but the JMK kernel does not halt after booting to the shell prompt; it stays in the interactive shell loop, so the harness reaches its 30-second timeout before QEMU exits.

Observed captured output:
- `actual.x86_64.log` was empty after the timeout.
- `actual.aarch64.log` contained the expected `BOOT` and `JMK>` fragments, plus scheduler/process initialization messages, and ended while the counter demo was running.

## Commits made

- Kernel tree: `3198cc3` — `feat(regression): add kernel runner and boot-to-shell case`
- Apps tree: `f074f04` — `feat(regression): add kernel runner and boot-to-shell case`

Only the task-relevant files were staged:
- `Makefile`, `scripts/jmk_regression.py`, and `tests/regression/boot-to-shell/` in the kernel tree.
- `scripts/jmk_regression.py` and `scripts/jasterish_regression.py` in the apps tree.

## Concerns

1. **Kernel does not exit after boot.** The harness follows the brief and waits for QEMU to terminate. Because the current JMK image boots into an interactive shell and never halts, both cases time out. To make the cases pass, the kernel image needs either a self-test mode that shuts down after emitting the expected output, or the harness needs to be extended to kill QEMU once the golden fragments have been observed.

2. **x86_64 serial output is empty.** While aarch64 produced the expected serial log, the x86_64 run produced no serial output at all under the same `-serial stdio` QEMU flags. This suggests a separate x86_64 serial/boot issue in the kernel itself, not in the harness.

3. **Brief CLI snippet and `__file__`.** The brief's `jmk_regression.py` snippet defines `main()` with `kernel_dir = Path(__file__).resolve().parent.parent`. If that function is imported by the wrapper, `__file__` resolves to the apps script path and the kernel build runs in the wrong directory (`make: *** No rule to make target 'clean'. Stop.`). Keeping `kernel_dir` as an explicit argument keeps the wrapper working.

4. **Makefile environment.** The brief's `run_kernel_case` snippet does not strip inherited make variables. Because the harness is invoked from `make regression`, inherited `MAKEFLAGS`/`MAKELEVEL` caused the sub-make to misbehave until they were removed from the subprocess environment.
