#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
KERNEL_ROOT = ROOT.parent / "engine" / "nnos" / "neurodios" / "jasterish-microkernel"
ARCHITECTURES = {"aarch64": "aarch64", "x86_64": "x86_64"}
SUPPORTED_ARCHITECTURE_NAMES = tuple(sorted(ARCHITECTURES))


@dataclass(frozen=True)
class Suite:
    name: str
    project_root: Path
    script: Path
    corpus_root: Path


COMPILER_SUITE = Suite(
    name="compiler",
    project_root=ROOT,
    script=ROOT / "scripts" / "jstar_regression.py",
    corpus_root=ROOT / "tests" / "regression",
)
KERNEL_SUITE = Suite(
    name="kernel",
    project_root=KERNEL_ROOT,
    script=KERNEL_ROOT / "scripts" / "jmk_regression.py",
    corpus_root=KERNEL_ROOT / "tests" / "regression",
)


def _validated_architectures(archs: list[str] | None) -> list[str]:
    if not archs:
        return []
    validated: list[str] = []
    for arch in archs:
        try:
            validated.append(ARCHITECTURES[arch])
        except KeyError as exc:
            allowed = ", ".join(SUPPORTED_ARCHITECTURE_NAMES)
            raise ValueError(f"unsupported architecture {arch!r}; expected one of: {allowed}") from exc
    return validated


def _confined_directory(requested: Path, allowed_root: Path, label: str) -> Path:
    try:
        root = allowed_root.expanduser().resolve(strict=True)
        resolved = requested.expanduser().resolve(strict=True)
    except OSError as exc:
        raise ValueError(f"{label} corpus does not resolve to an existing directory: {requested}") from exc
    if not root.is_dir():
        raise ValueError(f"configured {label} corpus root is not a directory: {root}")
    if not resolved.is_dir() or not resolved.is_relative_to(root):
        raise ValueError(f"{label} corpus must be a directory within {root}: {resolved}")
    return resolved


def _internal_script(suite: Suite) -> Path:
    try:
        project_root = suite.project_root.resolve(strict=True)
        script = suite.script.resolve(strict=True)
    except OSError as exc:
        raise ValueError(f"{suite.name} regression script is missing: {suite.script}") from exc
    if not script.is_file() or not script.is_relative_to(project_root):
        raise ValueError(f"{suite.name} regression script is not a file: {script}")
    return script


def _python_executable() -> Path:
    try:
        executable = Path(sys.executable).resolve(strict=True)
    except OSError as exc:
        raise ValueError("the active Python interpreter cannot be resolved") from exc
    if not executable.is_file() or not os.access(executable, os.X_OK):
        raise ValueError(f"the active Python interpreter is not executable: {executable}")
    return executable


def run_suite(
    suite: Suite,
    corpus: Path,
    archs: list[str] | None,
    update: bool,
    json_mode: bool,
    results: list[dict],
) -> int:
    script = _internal_script(suite)
    safe_corpus = _confined_directory(corpus, suite.corpus_root, suite.name)
    safe_archs = _validated_architectures(archs)
    cmd = [str(_python_executable()), str(script)]
    if update:
        cmd.append("--update")
    if json_mode:
        cmd.append("--json")
    if safe_archs:
        for arch in safe_archs:
            cmd.extend(["--arch", arch])
    # Keep caller-controlled corpus paths out of the command line. The child
    # receives a fixed positional value and runs from the validated directory.
    cmd.extend(["--", "."])

    if json_mode:
        result = subprocess.run(
            cmd,
            cwd=safe_corpus,
            capture_output=True,
            text=True,
            shell=False,
        )
        try:
            results.extend(json.loads(result.stdout))
        except json.JSONDecodeError:
            # If a runner emitted non-JSON (e.g., an early traceback), preserve it.
            print(result.stdout, end="")
            print(result.stderr, file=sys.stderr, end="")
        return result.returncode

    result = subprocess.run(cmd, cwd=safe_corpus, shell=False)
    return result.returncode


def main() -> int:
    parser = argparse.ArgumentParser(description="Jasterish combined regression harness")
    parser.add_argument("--compiler-corpus", type=Path, default=COMPILER_SUITE.corpus_root)
    parser.add_argument("--kernel-corpus", type=Path, default=KERNEL_SUITE.corpus_root)
    parser.add_argument("--arch", action="append", choices=SUPPORTED_ARCHITECTURE_NAMES)
    parser.add_argument("--update", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    results: list[dict] = []
    code = 0
    try:
        code |= run_suite(COMPILER_SUITE, args.compiler_corpus, args.arch, args.update, args.json, results)
        code |= run_suite(KERNEL_SUITE, args.kernel_corpus, args.arch, args.update, args.json, results)
    except ValueError as exc:
        parser.error(str(exc))

    if args.json:
        print(json.dumps(results, indent=2))

    return code


if __name__ == "__main__":
    raise SystemExit(main())
