#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def run_suite(
    script: Path,
    corpus: Path,
    archs: list[str] | None,
    update: bool,
    json_mode: bool,
    results: list[dict],
) -> int:
    cmd = [sys.executable, str(script), str(corpus)]
    if update:
        cmd.append("--update")
    if json_mode:
        cmd.append("--json")
    if archs:
        for arch in archs:
            cmd.extend(["--arch", arch])

    if json_mode:
        result = subprocess.run(cmd, capture_output=True, text=True)
        try:
            results.extend(json.loads(result.stdout))
        except json.JSONDecodeError:
            # If a runner emitted non-JSON (e.g., an early traceback), preserve it.
            print(result.stdout, end="")
            print(result.stderr, file=sys.stderr, end="")
        return result.returncode

    result = subprocess.run(cmd)
    return result.returncode


def main() -> int:
    parser = argparse.ArgumentParser(description="Jasterish combined regression harness")
    parser.add_argument("--compiler-corpus", type=Path, default=ROOT / "tests" / "regression")
    parser.add_argument("--kernel-corpus", type=Path, default=ROOT.parent / "engine" / "nnos" / "neurodios" / "jasterish-microkernel" / "tests" / "regression")
    parser.add_argument("--arch", action="append")
    parser.add_argument("--update", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    compiler_script = ROOT / "scripts" / "jstar_regression.py"
    kernel_script = ROOT.parent / "engine" / "nnos" / "neurodios" / "jasterish-microkernel" / "scripts" / "jmk_regression.py"

    results: list[dict] = []
    code = 0
    code |= run_suite(compiler_script, args.compiler_corpus, args.arch, args.update, args.json, results)
    code |= run_suite(kernel_script, args.kernel_corpus, args.arch, args.update, args.json, results)

    if args.json:
        print(json.dumps(results, indent=2))

    return code


if __name__ == "__main__":
    raise SystemExit(main())
