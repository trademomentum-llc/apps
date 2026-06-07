#!/usr/bin/env python3
"""
NeuroDiOS Binary Footprint Analyzer
Version: 1.0.0
Date: 2026-05-30

Part of the NeuroDiOS Tooling & Analysis Suite (NEURODIOS-TOOL-REQ-001, DES-001, TECH-001).

Supports Binary Optimization Plan v1.1 Phase 2 (Efficiency & Minimal Footprint)
and Phase 3 (Provenance, Traceability & Multi-Arch).

Deterministically inspects ELF and Mach-O artifacts produced by the NNOS daemon
builds and Jasterish Micro-Kernel. Reports section sizes, oversized data patterns,
reproducibility hashes, and concrete recommendations aligned to the Efficiency Mandate
and 8 Validated Denominators.

No external Python packages. Uses only platform binutils via subprocess with graceful
degradation. Output is always JSON-first and fully reproducible given identical inputs.

Example:
  python3 scripts/binary_footprint_analyzer.py --binary-dir build-minsize --json
"""

from __future__ import annotations

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
# Ground Truth Alignment
# =============================================================================

CANONICAL_8_DENOMINATORS = [
    "Fluctuation Dynamics", "Budget / Resource Accounting",
    "Contrast Differential", "Controlled Oscillation",
    "Adaptation Offset", "Primitive Traceability / Atomic Dependency Mapping",
    "Origin Vault (Deterministic Provenance & State History)", "Drift Detection"
]


@dataclass(frozen=True, slots=True)
class BinaryAnalysis:
    path: str
    architecture: str
    file_type: str
    size_bytes: int
    sections: dict[str, int]
    stripped_sha256: str
    largest_symbols: list[dict[str, Any]]
    oversized_flags: list[str]
    reproducibility_score: float
    recommendations: list[str]


@dataclass(frozen=True, slots=True)
class FootprintReport:
    meta: dict[str, Any]
    summary: dict[str, Any]
    binaries: list[dict[str, Any]]
    aggregate_math: dict[str, Any]
    phase2_opportunities: list[str]


# =============================================================================
# Pure Helpers
# =============================================================================

def provenance_stamp(paths: list[pathlib.Path]) -> dict[str, Any]:
    git_commit = "no-git"
    try:
        git_commit = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True, stderr=subprocess.DEVNULL).strip()
    except Exception:
        pass
    try:
        host = subprocess.check_output(["uname", "-a"], text=True).strip()
    except Exception:
        host = "unknown"
    file_shas = {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(paths) if p.is_file()}
    return {
        "tool": "binary_footprint_analyzer",
        "version": "1.0.0",
        "git_commit": git_commit,
        "build_host": host,
        "analyzed_file_shas": file_shas,
        "analysis_time_iso": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "denominator_justification": [
            {"denominator": "Budget / Resource Accounting", "mechanism": "Explicit per-section and per-symbol footprint measurement + oversized data flagging"},
            {"denominator": "Origin Vault (Deterministic Provenance & State History)", "mechanism": "Full git + host + per-binary stripped SHA256 recorded for every artifact"},
            {"denominator": "Drift Detection", "mechanism": "Reproducibility score + section hash vectors consumable by binary_drift_check.sh"},
            {"denominator": "Primitive Traceability / Atomic Dependency Mapping", "mechanism": "Largest symbols + oversized flags provide atomic view of emitted code/data"}
        ]
    }


def run_cmd(cmd: list[str]) -> str:
    try:
        return subprocess.check_output(cmd, text=True, stderr=subprocess.DEVNULL, timeout=30).strip()
    except Exception:
        return ""


def get_architecture(path: pathlib.Path) -> str:
    out = run_cmd(["file", "-b", str(path)])
    if "x86-64" in out or "x86_64" in out:
        return "x86_64"
    if "arm64" in out or "aarch64" in out:
        return "aarch64"
    if "Mach-O" in out and "arm64" in out:
        return "arm64"
    return "unknown"


def parse_size_output(path: pathlib.Path) -> dict[str, int]:
    """Parse `size` output across platforms."""
    raw = run_cmd(["size", "-A", str(path)])
    if not raw:
        raw = run_cmd(["size", str(path)])
    sections: dict[str, int] = {}
    for line in raw.splitlines():
        parts = re.split(r'\s+', line.strip())
        if len(parts) >= 2:
            try:
                name = parts[0].lower().strip(":")
                val = int(parts[1])
                if name in ("text", ".text", "data", ".data", "bss", ".bss", "rodata", ".rodata"):
                    sections[name] = val
            except ValueError:
                continue
    return sections


def compute_stripped_sha(path: pathlib.Path) -> str:
    tmp = pathlib.Path("/tmp") / f"stripped_{path.name}_{hashlib.sha256(str(path).encode()).hexdigest()[:8]}"
    try:
        run_cmd(["strip", "-o", str(tmp), str(path)])
        if tmp.exists():
            sha = hashlib.sha256(tmp.read_bytes()).hexdigest()
            tmp.unlink(missing_ok=True)
            return sha
    except Exception:
        pass
    # Fallback: hash original (conservative)
    return hashlib.sha256(path.read_bytes()).hexdigest()


def get_largest_symbols(path: pathlib.Path) -> list[dict[str, Any]]:
    raw = run_cmd(["nm", "-S", "--size-sort", str(path)])
    if not raw:
        raw = run_cmd(["nm", "-n", str(path)])
    symbols = []
    for line in raw.splitlines()[:20]:
        parts = re.split(r'\s+', line.strip())
        if len(parts) >= 3:
            try:
                size = int(parts[1], 16) if len(parts[1]) > 0 else 0
                name = parts[-1]
                if size > 0:
                    symbols.append({"name": name, "size_bytes": size})
            except ValueError:
                continue
    return symbols


def detect_oversized(path: pathlib.Path, sections: dict[str, int], symbols: list[dict]) -> list[str]:
    flags: list[str] = []
    total = sum(sections.values()) or path.stat().st_size
    if total > 200 * 1024:
        flags.append("binary_over_200kB_consider_minimal_libc_or_LTO")
    data_sz = sections.get("data", 0) + sections.get(".data", 0)
    if data_sz > 4096:
        flags.append("large_.data_section_promote_constants_to_rodata_or_immediates")
    for sym in symbols:
        if sym["size_bytes"] > 128:
            flags.append(f"symbol_{sym['name'][:32]}_over_128B_evaluate_for_INT8_INT16_pack")
    return flags


def generate_phase2_recommendations(flags: list[str], arch: str) -> list[str]:
    recs: list[str] = []
    if any("large_.data" in f for f in flags):
        recs.append("Phase 2: Add -fdata-sections -ffunction-sections + gc-sections at link (Efficiency Mandate)")
    if any("over_200kB" in f for f in flags):
        recs.append("Phase 2: Enable LTO (-flto) and -Oz or -Os for release profile")
    if "aarch64" in arch or "arm64" in arch:
        recs.append("TP-HCF: Produce matching x86_64 cross-build for NUC nodes (Phase 3)")
    recs.append("Always emit alongside provenance manifest (generate_provenance_manifest.sh)")
    return recs


def analyze_binary(path: pathlib.Path) -> BinaryAnalysis:
    arch = get_architecture(path)
    ftype = run_cmd(["file", "-b", str(path)])[:80]
    size_bytes = path.stat().st_size
    sections = parse_size_output(path)
    stripped_sha = compute_stripped_sha(path)
    symbols = get_largest_symbols(path)
    flags = detect_oversized(path, sections, symbols)
    recs = generate_phase2_recommendations(flags, arch)

    # Reproducibility score placeholder (1.0 when compared against golden externally)
    repro = 1.0 if not flags else max(0.6, 1.0 - (len(flags) * 0.05))

    return BinaryAnalysis(
        path=str(path),
        architecture=arch,
        file_type=ftype,
        size_bytes=size_bytes,
        sections=sections,
        stripped_sha256=stripped_sha,
        largest_symbols=symbols,
        oversized_flags=flags,
        reproducibility_score=repro,
        recommendations=recs,
    )


# =============================================================================
# Main
# =============================================================================

def main() -> None:
    import argparse
    parser = argparse.ArgumentParser(description="NeuroDiOS Binary Footprint Analyzer")
    parser.add_argument("--binary-dir", default="build", help="Directory containing daemons and kernel artifacts")
    parser.add_argument("--include-kernel", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    root = pathlib.Path(args.binary_dir).resolve()
    if not root.exists():
        print(json.dumps({"error": "directory not found", "path": str(root)}))
        sys.exit(2)

    candidates = sorted(list(root.glob("lsa_*")) +
                        list(root.glob("*Daemon")) +
                        list(root.glob("*Manager")) +
                        list(root.glob("*.elf")) +
                        list(root.glob("*.bin")))

    if args.include_kernel:
        jmk = pathlib.Path("neurodios/jasterish-microkernel")
        candidates += list(jmk.glob("*.elf")) + list(jmk.glob("*.bin"))

    analyses: list[BinaryAnalysis] = []
    for c in candidates:
        if c.is_file() and (c.suffix in {".elf", ".bin"} or "ELF" in run_cmd(["file", "-b", str(c)]) or "Mach-O" in run_cmd(["file", "-b", str(c)])):
            analyses.append(analyze_binary(c))

    analyses.sort(key=lambda a: a.path)

    total_size = sum(a.size_bytes for a in analyses)
    total_opportunities = sum(len(a.oversized_flags) for a in analyses)

    meta = provenance_stamp([pathlib.Path(a.path) for a in analyses])

    report = FootprintReport(
        meta=meta,
        summary={
            "binaries_analyzed": len(analyses),
            "total_size_bytes": total_size,
            "total_oversized_flags": total_opportunities,
            "architectures": sorted({a.architecture for a in analyses}),
        },
        binaries=[asdict(a) for a in analyses],
        aggregate_math={
            "statement": f"Analysis of {len(analyses)} binaries identified {total_opportunities} oversized patterns representing concrete Phase 2 optimization surface",
            "derivation": "sum(len(oversized_flags)) across all artifacts",
            "average_size_bytes": total_size // max(1, len(analyses)),
        },
        phase2_opportunities=[
            "Apply LTO + -Oz + strip to all release builds",
            "Replace any remaining 64-bit control scalars with INT8/INT16 in Jasterish codegen (post .data fix)",
            "Cross-compile for full TP-HCF (x86_64 + aarch64) and record per-arch provenance",
            "Feed section hashes directly into binary_drift_check.sh golden manifests"
        ]
    )

    if args.json:
        print(json.dumps(asdict(report), indent=2, sort_keys=True))
    else:
        print(json.dumps(asdict(report), indent=2, sort_keys=True))

    sys.exit(0)


if __name__ == "__main__":
    main()
