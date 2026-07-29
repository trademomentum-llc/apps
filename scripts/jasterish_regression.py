from __future__ import annotations

import json
import os
import re
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


def run_compiler_case(case: Case, arch: str, update: bool) -> Result:
    compiler = os.environ.get("JASTERISH_COMPILER", "morphlex")
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


def run_self_host_case(case: Case, arch: str, update: bool) -> Result:
    """Placeholder for the self-hosting compiler regression runner (Task 5)."""
    raise NotImplementedError("run_self_host_case is not implemented yet")


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
