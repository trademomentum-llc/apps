#!/usr/bin/env python3
"""
NeuroDiOS Efficiency Mandate Auditor
Version: 1.0.0
Date: 2026-05-30

Part of the NeuroDiOS Tooling & Analysis Suite (NEURODIOS-TOOL-REQ-001, DES-001, TECH-001).

Purpose:
Deterministically scan source for violations of the Efficiency Mandate
(smallest safe integer / fixed-point types only in hot paths and control logic).
Primary target: remediation of the critical violation in neurobalance_coordinator.py
identified in Chained_Source_of_Truth_Kimi_Binding.md §8 (2026-05-30 baseline audit).

Outputs only machine-readable JSON (plus optional human text) with:
- Exact line-level findings
- Bit-level mathematical savings proofs (integer arithmetic)
- Explicit mapping to the 8 Validated Denominators
- Safe remediation suggestions and --apply-safe mode

Strictly deterministic: sorted traversal, no external state, no floats in proof paths.

Invocation (from engine/nnos/):
  python3 scripts/efficiency_mandate_auditor.py --target neurobalance/neurobalance_coordinator.py --json

See the three Tooling Suite specifications for full contract, justification, and evolution path.
"""

from __future__ import annotations

import ast
import datetime
import hashlib
import json
import pathlib
import re
import subprocess
import sys
from dataclasses import dataclass, asdict
from typing import Any

# =============================================================================
# Canonical Ground Truth (never drift from Chained binding)
# =============================================================================

CANONICAL_8_DENOMINATORS: list[str] = [
    "Fluctuation Dynamics",
    "Budget / Resource Accounting",
    "Contrast Differential",
    "Controlled Oscillation",
    "Adaptation Offset",
    "Primitive Traceability / Atomic Dependency Mapping",
    "Origin Vault (Deterministic Provenance & State History)",
    "Drift Detection",
]

# Hot paths identified by 2026-05-30 Criteria Enforcement audit (Chained §8)
DEFAULT_HOT_PATHS: list[str] = [
    "assess_and_offset",
    "generate_uplift_opportunities",
    "recommend",
    "update_denominators",
    "apply_offset",
    "NeuroBalanceCoordinator",
]

# Scaling derivation (recorded in every suggestion for deterministic reproduction)
INTENSITY_SCALE_FACTOR = 255  # 0.0-1.0 domain -> Intensity8 [0, 255]
Q7_9_SCALE = 512              # Q7.9 fixed point


@dataclass(frozen=True, slots=True)
class Violation:
    file: str
    line: int
    function: str
    literal: float | str
    context: str
    suggested_replacement: str
    old_bits: int
    new_bits: int
    estimated_instances_per_cycle: int
    denominators: list[str]
    proof: dict[str, Any]


@dataclass(frozen=True, slots=True)
class AuditReport:
    meta: dict[str, Any]
    summary: dict[str, Any]
    findings: list[dict[str, Any]]
    math_proofs: list[dict[str, Any]]
    recommendations: list[str]
    remediation_diff: str | None


# =============================================================================
# Pure Deterministic Helpers (no side effects)
# =============================================================================

def provenance_stamp(target_paths: list[pathlib.Path]) -> dict[str, Any]:
    """Record full Origin Vault provenance. Deterministic ordering."""
    git_commit = "no-git"
    try:
        git_commit = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], text=True, stderr=subprocess.DEVNULL
        ).strip()
    except Exception:
        pass

    try:
        host = subprocess.check_output(["uname", "-a"], text=True).strip()
    except Exception:
        host = "unknown-host"

    file_shas: dict[str, str] = {}
    for p in sorted(target_paths):
        if p.is_file():
            file_shas[str(p)] = hashlib.sha256(p.read_bytes()).hexdigest()

    return {
        "tool": "efficiency_mandate_auditor",
        "version": "1.0.0",
        "git_commit": git_commit,
        "build_host": host,
        "analyzed_file_shas": file_shas,
        "analysis_time_iso": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "denominator_justification": [
            {
                "denominator": "Budget / Resource Accounting",
                "mechanism": "Compute Footprint sub-budget enforcement via smallest-safe-type detection",
                "uplift": "Direct remediation of live governor violation; enables all downstream minimal-footprint guarantees"
            },
            {
                "denominator": "Primitive Traceability / Atomic Dependency Mapping",
                "mechanism": "Every finding mapped to exact source lines and 8-denom relationships",
                "uplift": "Zero ambiguity in remediation; supports Origin Vault"
            },
            {
                "denominator": "Drift Detection",
                "mechanism": "Reproducible JSON output + embedded source hashes",
                "uplift": "Any future change to hot-path types is automatically visible"
            }
        ]
    }


def compute_bit_savings(old_bits: int, new_bits: int, count: int) -> dict[str, Any]:
    """Pure integer arithmetic proof. No floats in derivation."""
    if old_bits <= new_bits or count <= 0:
        return {"statement": "no_savings", "derivation": "0"}
    saved_bits = count * (old_bits - new_bits)
    return {
        "statement": f"Replacing {count} {old_bits}-bit scalars with {new_bits}-bit yields {saved_bits} bits saved per evaluation cycle",
        "derivation": f"{count} * ({old_bits} - {new_bits}) = {saved_bits}",
        "bits_saved_per_cycle": saved_bits,
        "bytes_saved_per_cycle": saved_bits // 8,
        "percentage_reduction": round(((old_bits - new_bits) / old_bits) * 100, 2),
    }


def scale_to_intensity8(value: float) -> int:
    """Deterministic scaling. Records derivation in suggestions."""
    scaled = int(round(value * INTENSITY_SCALE_FACTOR))
    return max(0, min(255, scaled))


def scale_to_q7_9(value: float) -> int:
    scaled = int(round(value * Q7_9_SCALE))
    return max(-32768, min(32767, scaled))


# =============================================================================
# Python Float Violation Detector (AST, deterministic)
# =============================================================================

class FloatViolationVisitor(ast.NodeVisitor):
    def __init__(self, hot_paths: list[str], filepath: str) -> None:
        self.hot_paths = hot_paths
        self.filepath = filepath
        self.current_function: str = "<module>"
        self.violations: list[Violation] = []

    def visit_FunctionDef(self, node: ast.FunctionDef) -> None:
        previous = self.current_function
        self.current_function = node.name
        self.generic_visit(node)
        self.current_function = previous

    def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef) -> None:
        previous = self.current_function
        self.current_function = node.name
        self.generic_visit(node)
        self.current_function = previous

    def visit_ClassDef(self, node: ast.ClassDef) -> None:
        previous = self.current_function
        self.current_function = node.name
        self.generic_visit(node)
        self.current_function = previous

    def _is_hot(self) -> bool:
        return any(h.lower() in self.current_function.lower() for h in self.hot_paths)

    def visit_Constant(self, node: ast.Constant) -> None:
        if isinstance(node.value, float) and self._is_hot():
            suggested = f"Intensity8({scale_to_intensity8(node.value)})  # 0.0-1.0 -> [0,255] deterministic; see Efficiency Mandate"
            self.violations.append(
                Violation(
                    file=self.filepath,
                    line=node.lineno,
                    function=self.current_function,
                    literal=node.value,
                    context=self._get_context(node.lineno),
                    suggested_replacement=suggested,
                    old_bits=64,
                    new_bits=8,
                    estimated_instances_per_cycle=1,
                    denominators=[
                        "Budget / Resource Accounting",
                        "Drift Detection",
                    ],
                    proof=compute_bit_savings(64, 8, 1),
                )
            )
        self.generic_visit(node)

    def _get_context(self, lineno: int) -> str:
        # Best-effort context from original source (read once at report time)
        try:
            with open(self.filepath, "r", encoding="utf-8") as f:
                lines = f.readlines()
            start = max(0, lineno - 2)
            end = min(len(lines), lineno + 1)
            return "".join(lines[start:end]).strip()
        except Exception:
            return "<context unavailable>"


def scan_python_file(path: pathlib.Path, hot_paths: list[str]) -> list[Violation]:
    try:
        source = path.read_text(encoding="utf-8")
        tree = ast.parse(source, filename=str(path))
    except Exception as e:
        print(f"WARNING: Could not parse {path}: {e}", file=sys.stderr)
        return []

    visitor = FloatViolationVisitor(hot_paths, str(path))
    visitor.visit(tree)
    return visitor.violations


# =============================================================================
# C/C++ Conservative Detector (regex, no external parser)
# =============================================================================

CPP_FLOAT_LITERAL_RE = re.compile(
    r'\b(\d+\.\d+(?:[eE][+-]?\d+)?)[fF]?\b'
)

def scan_cpp_file(path: pathlib.Path, hot_paths: list[str]) -> list[Violation]:
    violations: list[Violation] = []
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except Exception:
        return violations

    lines = text.splitlines()
    in_function = False
    current_func = "<global>"

    # Extremely conservative function context tracking
    for i, line in enumerate(lines, 1):
        func_match = re.search(r'(?:^|\s)([a-zA-Z_][a-zA-Z0-9_]*)\s*\([^)]*\)\s*\{', line)
        if func_match:
            current_func = func_match.group(1)
            in_function = True

        if '}' in line and in_function:
            in_function = False
            current_func = "<global>"

        if in_function and any(h.lower() in current_func.lower() for h in hot_paths):
            for m in CPP_FLOAT_LITERAL_RE.finditer(line):
                val_str = m.group(1)
                try:
                    val = float(val_str)
                except ValueError:
                    continue
                suggested = f"/* Intensity8({scale_to_intensity8(val)}) */ /* scaled from {val_str} */"
                violations.append(
                    Violation(
                        file=str(path),
                        line=i,
                        function=current_func,
                        literal=val_str,
                        context=line.strip(),
                        suggested_replacement=suggested,
                        old_bits=64,
                        new_bits=8,
                        estimated_instances_per_cycle=1,
                        denominators=["Budget / Resource Accounting"],
                        proof=compute_bit_savings(64, 8, 1),
                    )
                )
    return violations


# =============================================================================
# Report Assembly & Safe Apply
# =============================================================================

def build_report(violations: list[Violation], targets: list[pathlib.Path], hot_paths: list[str]) -> AuditReport:
    meta = provenance_stamp(targets)
    total_bits = 0
    for v in violations:
        total_bits += v.proof.get("bits_saved_per_cycle", 0)

    findings = [asdict(v) for v in sorted(violations, key=lambda x: (x.file, x.line))]

    math_proofs = []
    if violations:
        aggregate = compute_bit_savings(64, 8, len(violations))
        math_proofs.append(aggregate)
        math_proofs.append({
            "statement": "All proofs use integer arithmetic after the single documented scaling derivation (v * 255). No floating-point in savings path.",
            "derivation": "See individual findings.proof + scale_to_intensity8 definition"
        })

    recommendations = [
        "Convert all hot-path scalars to Intensity8 / Delta8 / FixedQ7_9 from neurobalance/minimal_types.py",
        "Invoke compute_action_footprint() on every MinimalOffsetAction selection",
        "Update module docstring to reference exclusively the 8 Validated Denominators",
        "Re-audit after changes using --strict to gate future regressions"
    ]

    remediation_diff: str | None = None
    if violations:
        # Very conservative unified diff sketch (human must review)
        lines = ["# Suggested remediation (review before application)"]
        for v in violations[:5]:  # limit for safety
            lines.append(f"# {v.file}:{v.line} in {v.function}")
            lines.append(f"#   old: {v.literal}")
            lines.append(f"#   new: {v.suggested_replacement}")
        remediation_diff = "\n".join(lines)

    summary = {
        "total_violations": len(violations),
        "total_potential_bits_saved_per_cycle": total_bits,
        "files_affected": len({v.file for v in violations}),
        "hot_paths_scanned": hot_paths,
        "efficiency_mandate_status": "VIOLATIONS_DETECTED" if violations else "COMPLIANT",
    }

    return AuditReport(
        meta=meta,
        summary=summary,
        findings=findings,
        math_proofs=math_proofs,
        recommendations=recommendations,
        remediation_diff=remediation_diff,
    )


def write_safe_backup(target: pathlib.Path) -> pathlib.Path:
    ts = datetime.datetime.utcnow().strftime("%Y%m%d-%H%M%S")
    backup = target.with_suffix(target.suffix + f".bak-{ts}")
    backup.write_bytes(target.read_bytes())
    return backup


def apply_safe_remediation(report: AuditReport, targets: list[pathlib.Path]) -> None:
    # Currently advisory only (full rewrite engine deferred per Technical Spec limitations)
    # Records the intent and the exact diff in an audit log for Origin Vault.
    log_path = pathlib.Path("scripts/auditor_audit.log")
    log_path.parent.mkdir(exist_ok=True)
    entry = {
        "timestamp": datetime.datetime.utcnow().isoformat() + "Z",
        "action": "apply-safe (advisory)",
        "targets": [str(t) for t in targets],
        "report_meta": report.meta,
        "remediation_note": "Full automated rewrite not performed in v1.0. See Technical Spec §8.",
    }
    with log_path.open("a", encoding="utf-8") as f:
        f.write(json.dumps(entry) + "\n")
    print(f"SAFETY: Remediation intent logged to {log_path}. No source files were modified.", file=sys.stderr)


# =============================================================================
# CLI
# =============================================================================

def main() -> None:
    import argparse

    parser = argparse.ArgumentParser(
        description="NeuroDiOS Efficiency Mandate Auditor (Tooling Suite v1.0)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="See docs/2026-05-30-NeuroDiOS-Tooling-Suite-*.md for full Requirements, Design, Technical Specification."
    )
    parser.add_argument("--target", default="neurobalance", help="File or directory to scan")
    parser.add_argument("--hot-paths", nargs="*", default=DEFAULT_HOT_PATHS, help="Override hot path function/class names")
    parser.add_argument("--json", action="store_true", help="Emit machine-readable JSON only (stdout)")
    parser.add_argument("--apply-safe", action="store_true", help="Log remediation intent (no auto-mutation in v1)")
    parser.add_argument("--strict", action="store_true", help="Exit 1 if any violations found")
    args = parser.parse_args()

    root = pathlib.Path(args.target).resolve()
    if not root.exists():
        print(f"ERROR: target not found: {root}", file=sys.stderr)
        sys.exit(2)

    targets: list[pathlib.Path] = []
    if root.is_file():
        targets = [root]
    else:
        targets = sorted([p for p in root.rglob("*.py") if p.is_file()] +
                         [p for p in root.rglob("*.c") if p.is_file()] +
                         [p for p in root.rglob("*.cpp") if p.is_file()] +
                         [p for p in root.rglob("*.h") if p.is_file()] +
                         [p for p in root.rglob("*.hpp") if p.is_file()])

    all_violations: list[Violation] = []
    for t in targets:
        if t.suffix == ".py":
            all_violations.extend(scan_python_file(t, args.hot_paths))
        elif t.suffix in {".c", ".cpp", ".h", ".hpp"}:
            all_violations.extend(scan_cpp_file(t, args.hot_paths))

    report = build_report(all_violations, targets, args.hot_paths)

    if args.json:
        print(json.dumps(asdict(report), indent=2, sort_keys=True))
    else:
        print(json.dumps(asdict(report), indent=2, sort_keys=True))

    if args.apply_safe:
        apply_safe_remediation(report, targets)

    if args.strict and report.summary["total_violations"] > 0:
        sys.exit(1)

    sys.exit(0)


if __name__ == "__main__":
    main()
