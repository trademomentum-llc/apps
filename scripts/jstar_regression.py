#!/usr/bin/env python3
from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from jasterish_regression import (
    discover_cases,
    report,
    Result,
    run_compiler_case,
    run_self_host_case,
)


def main() -> int:
    parser = argparse.ArgumentParser(description="JStar compiler regression runner")
    parser.add_argument("corpus", type=Path, help="Path to regression corpus directory")
    parser.add_argument("--arch", action="append", help="Architecture to test (repeatable; default: all in case)")
    parser.add_argument("--update", action="store_true", help="Update golden files from current output")
    parser.add_argument("--json", action="store_true", help="Emit JSON report")
    args = parser.parse_args()

    results: list[Result] = []
    for case in discover_cases(args.corpus):
        if case.kind not in ("compiler", "self-host"):
            continue
        for arch in args.arch or case.archs:
            if arch not in case.archs:
                continue
            if (case.root / f"skip.{arch}").exists():
                results.append(Result(case.name, arch, "SKIP", 0.0, "skip marker present"))
                continue
            if case.kind == "compiler":
                results.append(run_compiler_case(case, arch, args.update))
            elif case.kind == "self-host":
                results.append(run_self_host_case(case, arch, args.update))
    return report(results, args.json)


if __name__ == "__main__":
    raise SystemExit(main())
