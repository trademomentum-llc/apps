from __future__ import annotations

import errno
import hashlib
import json
import os
import re
import shutil
import subprocess
import time
import tomllib
from collections.abc import Callable
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


@dataclass
class Result:
    name: str
    arch: str
    status: str  # PASS | FAIL | SKIP | TIMEOUT | UPDATED
    duration: float
    detail: str = ""


def discover_cases(root: Path) -> list[Case]:
    cases: list[Case] = []
    if not root.exists():
        return cases
    for manifest in sorted(root.rglob("test.toml")):
        entry = manifest.parent
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


def _resolve_compiler() -> str:
    env = os.environ.get("JASTERISH_COMPILER", "").strip()
    if env:
        return env
    local = Path(__file__).resolve().parent.parent / "target" / "debug" / "morphlex"
    if local.exists():
        return str(local)
    found = shutil.which("morphlex")
    if found:
        return found
    return "morphlex"


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
            try:
                matched = re.search(line, actual)
            except re.error as exc:
                return False, f"invalid regex in golden file: {line!r} ({exc})"
            if not matched:
                missing.append(line)
        if not missing:
            return True, ""
        return False, f"unmatched regexes: {missing}"

    return False, f"unknown compare mode: {mode}"


def run_compiler_case(case: Case, arch: str, update: bool) -> Result:
    compiler = _resolve_compiler()
    elf_path = case.root / f"{case.name}.{arch}.elf"
    golden_path = case.root / f"expected.{arch}"

    if not update and not golden_path.exists():
        return Result(case.name, arch, "SKIP", 0.0, f"missing {golden_path}")

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
    except OSError as exc:
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, f"compile launch failed: {exc}")

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
    except OSError as exc:
        if exc.errno == errno.ENOEXEC:
            return Result(case.name, arch, "SKIP", time.monotonic() - t0, f"cannot execute {arch} ELF on this host: {exc}")
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, f"run launch failed: {exc}")

    actual = run.stdout
    exit_code = run.returncode

    if update:
        golden_path.write_text(actual)
        exit_golden = case.root / f"expected.{arch}.exit"
        exit_golden.write_text(str(exit_code))
        return Result(case.name, arch, "UPDATED", time.monotonic() - t0, f"wrote {golden_path}")

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

    # Avoid inheriting MAKEFLAGS/MAKELEVEL when the harness is invoked from make;
    # otherwise the kernel build subprocess may behave as a recursive make.
    build_env = os.environ.copy()
    build_env.pop("MAKEFLAGS", None)
    build_env.pop("MAKELEVEL", None)

    try:
        build = subprocess.run(
            ["make", f"ARCH={arch}", "clean", "build"],
            cwd=build_dir,
            capture_output=True,
            text=True,
            timeout=case.timeout,
            env=build_env,
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
                # Kernels that boot to an interactive shell will not exit on their own.
                # Capture whatever output we got and evaluate it against the golden file.
                timed_out = True
            else:
                timed_out = False
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
        status = "FAIL" if not timed_out else "TIMEOUT"
        return Result(case.name, arch, status, time.monotonic() - t0, detail)

    if timed_out:
        return Result(case.name, arch, "PASS", time.monotonic() - t0, "(QEMU timed out, but output matched)")
    return Result(case.name, arch, "PASS", time.monotonic() - t0, "")


def run_self_host_case(case: Case, arch: str, update: bool) -> Result:
    t0 = time.monotonic()
    reference = case.source_path
    if not reference.exists():
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, "missing compiler.jstr reference")

    work = case.root / f"work.{arch}"
    work.mkdir(exist_ok=True)
    stage0 = work / "stage0.elf"
    stage1 = work / "stage1.elf"
    stage2 = work / "stage2.elf"

    # Stage 0: reference compiler from Rust toolchain
    compiler = _resolve_compiler()
    build_cmd = [
        compiler, "jstar", "compile",
        "--target", arch,
        "--input", str(reference),
        "--output", str(stage0),
    ]
    try:
        build = subprocess.run(
            build_cmd,
            capture_output=True,
            text=True,
            timeout=case.timeout,
        )
    except subprocess.TimeoutExpired:
        return Result(case.name, arch, "TIMEOUT", time.monotonic() - t0, "stage0 compile timed out")
    except OSError as exc:
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, f"stage0 compile launch failed: {exc}")
    if build.returncode != 0:
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, f"stage0 compile failed:\n{build.stderr}")

    # Stage 1/2: compiler compiled by stage0
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
        except OSError as exc:
            if exc.errno == errno.ENOEXEC:
                return Result(case.name, arch, "SKIP", time.monotonic() - t0, f"cannot execute {arch} ELF on this host: {exc}")
            return Result(case.name, arch, "FAIL", time.monotonic() - t0, f"{stage_out.name} generation launch failed: {exc}")
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
