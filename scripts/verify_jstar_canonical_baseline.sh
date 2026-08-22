#!/usr/bin/env bash
set -euo pipefail

fail() {
    printf 'JSTAR CANONICAL BASELINE FAIL: %s\n' "$*" >&2
    exit 1
}

git rev-parse --is-inside-work-tree >/dev/null 2>&1 || fail "not inside a git work tree"
repo_root="$(git rev-parse --show-toplevel)"
manifest="${1:-${repo_root}/security/provenance/jstar-canonical-baseline.json}"

[ -s "$manifest" ] || fail "missing baseline manifest: $manifest"

python3 - "$manifest" "$repo_root" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

manifest_path = Path(sys.argv[1])
repo_root = Path(sys.argv[2])
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()

errors: list[str] = []
artifacts = manifest.get("artifacts", {})

for name, record in artifacts.items():
    rel = record["path"]
    path = repo_root / rel
    if not path.is_file():
        errors.append(f"{name}: missing {rel}")
        continue
    size = path.stat().st_size
    if size != int(record["size_bytes"]):
        errors.append(f"{name}: size mismatch for {rel}: expected {record['size_bytes']}, got {size}")
    actual = digest(path)
    if actual != record["sha256"]:
        errors.append(f"{name}: sha256 mismatch for {rel}: expected {record['sha256']}, got {actual}")

j4 = artifacts.get("jstar4", {}).get("sha256")
j5 = artifacts.get("jstar5", {}).get("sha256")
if not j4 or not j5 or j4 != j5:
    errors.append("accepted fixpoint invariant failed: jstar4_sha256 != jstar5_sha256")

if errors:
    for error in errors:
        print(error, file=sys.stderr)
    sys.exit(1)

print(f"JSTAR CANONICAL BASELINE OK: {manifest_path}")
print(f"canonical_fixpoint_sha256={j4}")
PY
