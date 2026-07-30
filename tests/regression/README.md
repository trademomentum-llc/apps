# Jasterish Compiler Regression Tests

This directory contains regression cases for the JStar compiler and the self-hosting compiler test.

## Running the suite

From `System/apps`:

```bash
make regression
```

To update golden files after intentional output changes (requires a host that can execute the target ELFs):

```bash
make regression-update
```

To run only the compiler/self-host suite directly:

```bash
python3 scripts/jstar_regression.py tests/regression
```

To run the combined compiler + kernel suite:

```bash
# from System/apps
python3 scripts/jasterish_orchestrator.py

# from the Sovereign root
python3 System/apps/scripts/jasterish_orchestrator.py
```

## Case layout

Each case is a directory containing:

- `main.jstr` — the Jasterish source program.
- `test.toml` — case metadata (`name`, `kind`, `archs`, `timeout`, `compare`).
- `expected.<arch>` — expected stdout for architecture `<arch>`.
- `expected.<arch>.exit` — optional expected exit code.
- `skip.<arch>` — optional skip marker; if present the case is skipped for that architecture.

## Host limitations

The compiler emits Linux ELFs. On non-Linux hosts (e.g., macOS) the runner cannot execute the generated binaries and reports `SKIP` with a message like `cannot execute x86_64 ELF on this host`. This is expected behavior — the compile step is still exercised, and the cases will validate end-to-end on a Linux runner.

The self-hosting case is additionally skipped by `tests/regression/self-host/skip.x86_64` until its `expected.x86_64` golden SHA256 is generated on an x86_64 Linux host with:

```bash
cd System/apps
JASTERISH_COMPILER=target/debug/morphlex python3 scripts/jstar_regression.py tests/regression --update --arch x86_64
```

After generating the golden file, remove the `skip.x86_64` marker to enable the case.
