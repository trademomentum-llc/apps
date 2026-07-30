# Self-hosting regression case

This case verifies that the JStar compiler can compile itself and that the
resulting binary is byte-identical when it compiles itself again.

## Generating the golden file

The host that owns this repository (macOS) cannot run the Linux x86_64 ELFs
produced by the compiler, so `expected.x86_64` is not present here.

Generate the golden hash on an x86_64 Linux runner with the compiler toolchain
installed:

```bash
python3 scripts/jstar_regression.py tests/regression --update --arch x86_64
```

This writes `tests/regression/self-host/expected.x86_64` containing the SHA-256
digest of the self-hosted ELF. Commit that file (and remove `skip.x86_64` if the
runner is intended to validate the case).

## Skipping on non-Linux hosts

`skip.x86_64` is present so the regression runner skips this case on hosts that
cannot execute the produced ELF. The runner treats any `skip.<arch>` marker in a
case directory as a skip directive for that architecture.
