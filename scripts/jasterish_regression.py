from __future__ import annotations

import errno
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import time
import tomllib
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent


@dataclass(frozen=True)
class ArchitectureConfig:
    compiler_target: str
    qemu_candidates: tuple[Path, ...]
    machine: str
    cpu: str


ARCHITECTURES = {
    "aarch64": ArchitectureConfig(
        compiler_target="aarch64",
        qemu_candidates=(
            Path("/opt/homebrew/bin/qemu-system-aarch64"),
            Path("/usr/local/bin/qemu-system-aarch64"),
            Path("/usr/bin/qemu-system-aarch64"),
        ),
        machine="virt",
        cpu="cortex-a72",
    ),
    "x86_64": ArchitectureConfig(
        compiler_target="x86_64",
        qemu_candidates=(
            Path("/opt/homebrew/bin/qemu-system-x86_64"),
            Path("/usr/local/bin/qemu-system-x86_64"),
            Path("/usr/bin/qemu-system-x86_64"),
        ),
        machine="q35",
        cpu="qemu64",
    ),
}
SUPPORTED_ARCHITECTURE_NAMES = tuple(sorted(ARCHITECTURES))
TRUSTED_COMPILER_PATHS = (
    REPO_ROOT / "target" / "debug" / "morphlex",
    REPO_ROOT / "target" / "release" / "morphlex",
)
TRUSTED_MAKE_PATHS = (
    Path("/usr/bin/make"),
    Path("/opt/homebrew/bin/make"),
    Path("/usr/local/bin/make"),
)


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


def _architecture_config(arch: str) -> ArchitectureConfig:
    try:
        return ARCHITECTURES[arch]
    except KeyError as exc:
        allowed = ", ".join(SUPPORTED_ARCHITECTURE_NAMES)
        raise ValueError(f"unsupported architecture {arch!r}; expected one of: {allowed}") from exc


def _is_executable_file(path: Path) -> bool:
    return path.is_file() and os.access(path, os.X_OK)


def _resolve_trusted_executable(candidates: tuple[Path, ...], label: str) -> Path:
    for candidate in candidates:
        if _is_executable_file(candidate):
            return candidate
    checked = ", ".join(str(candidate) for candidate in candidates)
    raise FileNotFoundError(f"no trusted {label} executable found; checked: {checked}")


def _available_compilers() -> tuple[Path, ...]:
    target_root = (REPO_ROOT / "target").resolve()
    available: list[Path] = []
    for candidate in TRUSTED_COMPILER_PATHS:
        try:
            resolved = candidate.resolve(strict=True)
        except OSError:
            continue
        if resolved.is_relative_to(target_root) and _is_executable_file(resolved):
            available.append(resolved)
    return tuple(available)


def _resolve_compiler() -> Path:
    available = _available_compilers()
    override = os.environ.get("JASTERISH_COMPILER", "").strip()
    if override:
        try:
            requested = Path(override).expanduser().resolve(strict=True)
        except OSError as exc:
            raise ValueError(f"JASTERISH_COMPILER does not resolve to an executable file: {override!r}") from exc
        for trusted in available:
            if requested == trusted:
                return trusted
        allowed = ", ".join(str(path) for path in TRUSTED_COMPILER_PATHS)
        raise ValueError(
            "JASTERISH_COMPILER must select a repository-built compiler; "
            f"expected one of: {allowed}"
        )
    if available:
        return available[0]
    checked = ", ".join(str(path) for path in TRUSTED_COMPILER_PATHS)
    raise FileNotFoundError(f"no repository-built morphlex compiler found; checked: {checked}")


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
    try:
        config = _architecture_config(arch)
    except ValueError as exc:
        return Result(case.name, arch, "FAIL", 0.0, f"command validation failed: {exc}")

    elf_name = f"actual.{config.compiler_target}.elf"
    elf_path = case.root / elf_name
    golden_path = case.root / f"expected.{config.compiler_target}"

    if not update and not golden_path.exists():
        return Result(case.name, arch, "SKIP", 0.0, f"missing {golden_path}")

    try:
        compiler = _resolve_compiler()
    except (OSError, ValueError) as exc:
        return Result(case.name, arch, "FAIL", 0.0, f"command validation failed: {exc}")

    build_cmd = [
        str(compiler), "jstar", "compile",
        "--target", config.compiler_target,
        "--input", "main.jstr",
        "--output", elf_name,
    ]

    t0 = time.monotonic()
    try:
        build = subprocess.run(
            build_cmd,
            cwd=case.root,
            capture_output=True,
            text=True,
            timeout=case.timeout,
            shell=False,
        )
    except subprocess.TimeoutExpired:
        return Result(case.name, arch, "TIMEOUT", time.monotonic() - t0, "compile timed out")
    except OSError as exc:
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, f"compile launch failed: {exc}")

    if build.returncode != 0:
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, f"compile failed:\n{build.stderr}")

    try:
        run = subprocess.run(
            [f"./{elf_name}"],
            cwd=case.root,
            capture_output=True,
            text=True,
            timeout=case.timeout,
            shell=False,
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
    try:
        config = _architecture_config(arch)
        make = _resolve_trusted_executable(TRUSTED_MAKE_PATHS, "make")
        qemu = _resolve_trusted_executable(config.qemu_candidates, "QEMU")
    except (OSError, ValueError) as exc:
        return Result(case.name, arch, "FAIL", 0.0, f"command validation failed: {exc}")

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
            [str(make), f"ARCH={config.compiler_target}", "clean", "build"],
            cwd=build_dir,
            capture_output=True,
            text=True,
            timeout=case.timeout,
            env=build_env,
            shell=False,
        )
    except subprocess.TimeoutExpired:
        return Result(case.name, arch, "TIMEOUT", time.monotonic() - t0, "kernel build timed out")

    if build.returncode != 0:
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, f"kernel build failed:\n{build.stderr}")

    kernel_bin = build_dir / "jmk.bin"
    log_file = case.root / f"actual.{config.compiler_target}.log"

    qemu_cmd = [
        str(qemu),
        "-machine", config.machine,
        "-cpu", config.cpu,
        "-m", "512",
        "-serial", "stdio",
        "-no-reboot",
        "-no-shutdown",
        "-display", "none",
        "-kernel", str(kernel_bin),
    ]

    try:
        with log_file.open("w") as log:
            proc = subprocess.Popen(
                qemu_cmd,
                stdout=log,
                stderr=subprocess.STDOUT,
                text=True,
                shell=False,
            )
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
    golden_path = case.root / f"expected.{config.compiler_target}"

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
    try:
        config = _architecture_config(arch)
        compiler = _resolve_compiler()
    except (OSError, ValueError) as exc:
        return Result(case.name, arch, "FAIL", 0.0, f"command validation failed: {exc}")

    reference = case.source_path
    if not reference.exists():
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, "missing compiler.jstr reference")

    work_name = f"work.{config.compiler_target}"
    work = case.root / work_name
    work.mkdir(exist_ok=True)
    stage0_name = f"{work_name}/stage0.elf"
    stage1_name = f"{work_name}/stage1.elf"
    stage2_name = f"{work_name}/stage2.elf"
    stage0 = case.root / stage0_name
    stage1 = case.root / stage1_name
    stage2 = case.root / stage2_name

    # Stage 0: reference compiler from Rust toolchain
    build_cmd = [
        str(compiler), "jstar", "compile",
        "--target", config.compiler_target,
        "--input", "main.jstr",
        "--output", stage0_name,
    ]
    try:
        build = subprocess.run(
            build_cmd,
            cwd=case.root,
            capture_output=True,
            text=True,
            timeout=case.timeout,
            shell=False,
        )
    except subprocess.TimeoutExpired:
        return Result(case.name, arch, "TIMEOUT", time.monotonic() - t0, "stage0 compile timed out")
    except OSError as exc:
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, f"stage0 compile launch failed: {exc}")
    if build.returncode != 0:
        return Result(case.name, arch, "FAIL", time.monotonic() - t0, f"stage0 compile failed:\n{build.stderr}")

    # Stage 1/2: compiler compiled by stage0
    for stage_in_name, stage_in, stage_out in (
        (stage0_name, stage0, stage1),
        (stage1_name, stage1, stage2),
    ):
        try:
            run = subprocess.run(
                [f"./{stage_in_name}"],
                cwd=case.root,
                input=reference.read_bytes(),
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=case.timeout,
                shell=False,
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
        # The generated compiler must be executable, but only by its owner.
        os.chmod(stage_out, stat.S_IRUSR | stat.S_IWUSR | stat.S_IXUSR)

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
