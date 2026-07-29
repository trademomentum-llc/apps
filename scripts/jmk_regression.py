#!/usr/bin/env python3
from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent / "apps" / "scripts"))

from jasterish_regression import discover_cases, report, run_cases, run_kernel_case


def main(kernel_dir: Path | None = None) -> int:
    parser = argparse.ArgumentParser(description="Jasterish Micro-Kernel regression runner")
    parser.add_argument("corpus", type=Path, help="Path to regression corpus directory")
    parser.add_argument("--arch", action="append", help="Architecture to test (repeatable)")
    parser.add_argument("--update", action="store_true", help="Update golden files")
    parser.add_argument("--json", action="store_true", help="Emit JSON report")
    args = parser.parse_args()

    if kernel_dir is None:
        kernel_dir = Path(__file__).resolve().parent.parent
    cases = [c for c in discover_cases(args.corpus) if c.kind == "kernel"]
    results = run_cases(cases, args.arch, args.update, lambda case, arch, update: run_kernel_case(case, arch, update, kernel_dir=kernel_dir))
    return report(results, args.json)


if __name__ == "__main__":
    raise SystemExit(main())
