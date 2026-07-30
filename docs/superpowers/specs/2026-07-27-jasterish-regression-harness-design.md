# Jasterish Regression Harness Design

**Date:** 2026-07-27  
**Scope:** JStar compiler (`morphlex jstar`), Jasterish Micro-Kernel (`jmk`), and self-hosting verification.  
**Status:** Implemented.

---

## 1. Goal

Provide a single, discoverable regression harness that verifies the Jasterish toolchain does not break existing behavior when the compiler, kernel source, or build system changes.

The harness must cover:

1. **Compiler regression** — sample JStar programs compile and produce the expected stdout/exit code on x86_64 and aarch64.
2. **Kernel regression** — the Jasterish Micro-Kernel builds and boots under QEMU on x86_64 and aarch64, producing expected serial output.
3. **Self-hosting regression** — `compiler.jstr` compiles itself and produces a byte-identical binary across two generations.

---

## 2. Architecture

The harness is a combination of a dedicated Python regression runner and Makefile targets.

```
System/
├── apps/
│   ├── tests/regression/              # compiler regression corpus
│   │   ├── print-literal/
│   │   │   ├── main.jstr
│   │   │   ├── test.toml
│   │   │   ├── expected.x86_64
│   │   │   └── expected.aarch64
│   │   └── ...
│   └── scripts/
│       ├── jasterish_regression.py    # shared regression library
│       ├── jstar_regression.py        # compiler runner
│       └── jasterish_orchestrator.py  # top-level orchestrator (optional)
│
├── engine/nnos/neurodios/jasterish-microkernel/
│   ├── tests/regression/              # kernel regression corpus
│   │   ├── boot-to-shell/
│   │   │   ├── main.jstr              # init overlay
│   │   │   ├── test.toml
│   │   │   ├── expected.x86_64
│   │   │   └── expected.aarch64
│   │   └── ...
│   └── scripts/
│       └── jmk_regression.py          # kernel runner (uses shared library)
```

### Components

- **`jstar_regression.py`** — discovers compiler cases, compiles each for requested architectures, runs the resulting ELF, and compares stdout/stderr/exit code against golden files.
- **`jmk_regression.py`** — discovers kernel cases, stages the init overlay, builds `jmk.bin`, runs it in QEMU, captures serial output, and compares against golden files.
- **Top-level orchestrator** — runs compiler, kernel, and self-host suites and emits a combined report.
- **Makefile targets** — `make regression` and `make regression-update` in each project.

---

## 3. Test Case Layout

Each regression case is a directory containing:

| File | Purpose |
|------|---------|
| `main.jstr` | Source program (compiler case) or init process overlay (kernel case). |
| `test.toml` | Metadata: name, kind, architectures, timeout, comparison mode. |
| `expected.<arch>` | Golden output for architecture `<arch>`. |
| `expected.<arch>.exit` | Optional golden exit code (default `0`). |
| `skip.<arch>` | Optional marker file: if present, skip that architecture. |

### `test.toml` schema

```toml
name = "print-literal"          # human-readable case name
kind = "compiler"               # compiler | kernel | self-host
archs = ["x86_64", "aarch64"]   # architectures to test
timeout = 30                    # seconds
compare = "exact"               # exact | contains | regex

[compiler]
flags = ["--raw"]               # extra compiler flags (compiler cases only)

[kernel]
init_source = "main.jstr"       # source to build as init process (kernel cases only)

[self-host]
reference = "System/apps/jstar/compiler.jstr"
```

### Comparison modes

- **`exact`** — stdout/serial must match the golden file byte-for-byte after normalizing trailing newlines.
- **`contains`** — every non-empty line in the golden file must appear somewhere in the output.
- **`regex`** — every non-empty line in the golden file is a regex that must match somewhere in the output.

---

## 4. Runner Behavior

The runner executes in four phases.

### Phase 1 — Discover

Recursively walk the regression tree and collect every directory containing `test.toml`. Skip a case for an architecture if:

- the architecture is absent from `archs`, or
- a `skip.<arch>` file exists, or
- the golden file `expected.<arch>` is missing.

### Phase 2 — Build / Compile

For each retained case and architecture:

**Compiler case:**
```bash
morphlex jstar compile --target <arch> --input main.jstr --output <tmp>/<case>.elf [flags...]
```

**Kernel case:**
1. Stage `main.jstr` into the kernel build tree as the init process source.
2. Run `make ARCH=<arch> clean build` in the microkernel directory.

**Self-host case:**
1. Compile `compiler.jstr` with the Rust compiler to produce `stage0.elf`.
2. `./stage0.elf < compiler.jstr > stage1.elf`
3. `./stage1.elf < compiler.jstr > stage2.elf`
4. Verify `cmp stage1.elf stage2.elf` succeeds.

### Phase 3 — Run & Capture

**Compiler case:** run the compiled ELF, capture stdout, stderr, and exit code.

**Kernel case:** run QEMU headlessly with a timeout:
```bash
qemu-system-<arch> -machine <machine> -cpu <cpu> -m 512 \
  -serial stdio -no-reboot -no-shutdown -display none \
  -kernel jmk.bin
```

**Self-host case:** record the sha256 of `stage1.elf`.

### Phase 4 — Compare & Report

Compare captured output against `expected.<arch>` using the configured mode. Emit a line-oriented report:

```
PASS  compiler/print-literal        x86_64   0.12s
PASS  compiler/print-literal        aarch64  0.15s
FAIL  kernel/boot-to-shell          aarch64  0.28s
      missing golden marker: "JMK>"
PASS  self-host/compiler            x86_64   4.20s
```

Exit codes:

- `0` — all cases passed.
- `1` — one or more cases failed or timed out.

---

## 5. Makefile Integration

### Compiler project

`System/apps/` is a Rust/cargo project; a thin `Makefile` is added for the regression entry points.

```makefile
# System/apps/Makefile
REGRESSION_DIR := tests/regression

regression:
	@python3 scripts/jstar_regression.py $(REGRESSION_DIR)

regression-update:
	@python3 scripts/jstar_regression.py $(REGRESSION_DIR) --update
```

### Kernel project

```makefile
# System/engine/nnos/neurodios/jasterish-microkernel/Makefile
REGRESSION_DIR := tests/regression

regression:
	@python3 scripts/jmk_regression.py $(REGRESSION_DIR)

regression-update:
	@python3 scripts/jmk_regression.py $(REGRESSION_DIR) --update
```

### Top-level orchestrator (optional)

```bash
python3 System/apps/scripts/jasterish_orchestrator.py \
  --compiler-corpus tests/regression \
  --kernel-corpus ../engine/nnos/neurodios/jasterish-microkernel/tests/regression
```

---

## 6. Self-Hosting Verification

Self-hosting is treated as a special compiler regression case.

Steps:

1. Build the reference compiler with the Rust toolchain:
   ```bash
   morphlex jstar compile --target x86_64 --input compiler.jstr --output stage0.elf
   ```
2. Use `stage0.elf` to compile `compiler.jstr`:
   ```bash
   chmod +x stage0.elf
   ./stage0.elf < compiler.jstr > stage1.elf
   ```
3. Use `stage1.elf` to compile `compiler.jstr` again:
   ```bash
   chmod +x stage1.elf
   ./stage1.elf < compiler.jstr > stage2.elf
   ```
4. Assert `stage1.elf` and `stage2.elf` are byte-identical.
5. Store the sha256 of `stage1.elf` in `expected.x86_64` to detect silent codegen drift.

---

## 7. Kernel Regression Details

Kernel cases reuse the existing Makefile and QEMU invocation patterns.

For each case:

1. The runner copies `main.jstr` into the kernel source tree as the init process (or appends it to the build list, depending on how the kernel loads user programs).
2. The runner invokes `make ARCH=<arch> clean build`.
3. The runner launches QEMU with `-display none`, captures serial output, and enforces `timeout`.
4. QEMU is killed if it hangs or exceeds the timeout.
5. The captured serial log is compared against `expected.<arch>`.

The same `compare` modes apply, so a case can require an exact boot log or simply check that `"JMK>"` appears.

---

## 8. Error Handling & Reporting

- **Build failure:** reported as `FAIL` with the compiler/Make stderr.
- **Timeout:** reported as `FAIL (timeout)`.
- **Missing golden file:** the architecture is skipped for that case.
- **Output mismatch:** the runner prints a unified diff or the first missing fragment.
- **Final summary:** total pass / fail / skip / timeout counts.
- **`--update` flag:** refreshes golden files from current output. The runner writes each `expected.<arch>` and optionally `expected.<arch>.exit`.
- **`--json` flag:** emits machine-readable results for CI.

---

## 9. Success Criteria

After implementation:

1. `cd System/apps && make regression` runs all compiler regression cases and exits 0 on a clean tree.
2. `cd System/engine/nnos/neurodios/jasterish-microkernel && make regression` runs all kernel regression cases and exits 0 on a clean tree.
3. A deliberate breaking change to the compiler or kernel causes at least one regression case to fail.
4. `make regression-update` regenerates golden files safely and deterministically.
5. The harness runs on both x86_64 and aarch64 hosts (where tooling is available).

---

## 10. Out of Scope

- Performance benchmarking (only correctness/output comparison).
- Fuzzing or property-based generation (may be added later).
- CI integration beyond a machine-readable `--json` report.
- Replacing existing unit tests in `System/apps/src/jstar/mod.rs` or the AArch64 smoke script.

---

## 11. Open Questions

1. Should the kernel init overlay be copied into the tree or passed to the Makefile via an environment variable?
2. Should the top-level orchestrator be implemented in this phase or deferred until both project-level runners exist?
3. Should we normalize QEMU timestamps or memory addresses in kernel golden files, or keep them architecture-specific and stable enough?
