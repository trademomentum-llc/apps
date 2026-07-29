#!/usr/bin/env python3
from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from jasterish_regression import (
    discover_cases,
    report,
    run_cases,
    run_compiler_case,
)


def main() -> int:
    parser = argparse.ArgumentParser(description="JStar compiler regression runner")
    parser.add_argument("corpus", type=Path, help="Path to regression corpus directory")
    parser.add_argument("--arch", action="append", help="Architecture to test (repeatable; default: all in case)")
    parser.add_argument("--update", action="store_true", help="Update golden files from current output")
    parser.add_argument("--json", action="store_true", help="Emit JSON report")
    args = parser.parse_args()

    cases = [c for c in discover_cases(args.corpus) if c.kind == "compiler"]
    results = run_cases(cases, args.arch, args.update, run_compiler_case)
    return report(results, args.json)


if __name__ == "__main__":
    raise SystemExit(main())
