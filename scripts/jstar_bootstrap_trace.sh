#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
COMPILER_SRC="${JSTAR_COMPILER_SRC:-${ROOT_DIR}/jstar/compiler.jstr}"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/home/llc/.cache/jstar-target}"
OUT_DIR="${JSTAR_BOOTSTRAP_OUT_DIR:-/home/llc/exports/control_node/runtime/jstar_bootstrap}"

log() {
  printf '[jstar-bootstrap-trace] %s\n' "$1"
}

if [ ! -f "${COMPILER_SRC}" ]; then
  echo "compiler source not found at ${COMPILER_SRC}" >&2
  exit 1
fi

mkdir -p "${CARGO_TARGET_DIR}"
mkdir -p "${OUT_DIR}"

log "root=${ROOT_DIR}"
log "compiler=${COMPILER_SRC}"
log "out=${OUT_DIR}"

cd "${ROOT_DIR}"

log "stage1: rust bootstrap -> jstar1"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR}" \
  cargo run --quiet -- jstar compile --input "${COMPILER_SRC}" --output "${OUT_DIR}/jstar1" --raw
chmod +x "${OUT_DIR}/jstar1"

log "stage2: jstar1 -> jstar2"
"${OUT_DIR}/jstar1" < "${COMPILER_SRC}" > "${OUT_DIR}/jstar2"
chmod +x "${OUT_DIR}/jstar2"

log "stage3: jstar2 -> jstar3"
"${OUT_DIR}/jstar2" < "${COMPILER_SRC}" > "${OUT_DIR}/jstar3"
chmod +x "${OUT_DIR}/jstar3"

sha256sum "${OUT_DIR}/jstar1" "${OUT_DIR}/jstar2" "${OUT_DIR}/jstar3" > "${OUT_DIR}/sha256.txt"
wc -c "${OUT_DIR}/jstar1" "${OUT_DIR}/jstar2" "${OUT_DIR}/jstar3" > "${OUT_DIR}/sizes.txt"
cmp -l "${OUT_DIR}/jstar2" "${OUT_DIR}/jstar3" > "${OUT_DIR}/jstar2_vs_jstar3.cmp" || true

JSTAR_BOOTSTRAP_OUT_DIR="${OUT_DIR}" python3 - <<'PY'
from pathlib import Path
import json
import os
out = Path(os.environ['JSTAR_BOOTSTRAP_OUT_DIR'])
b2 = (out / 'jstar2').read_bytes()
b3 = (out / 'jstar3').read_bytes()
first = None
for i, (a, b) in enumerate(zip(b2, b3)):
    if a != b:
        first = i
        break
summary = {
    'jstar2_size': len(b2),
    'jstar3_size': len(b3),
    'size_delta': len(b2) - len(b3),
    'prefix_equal_bytes': first if first is not None else min(len(b2), len(b3)),
    'first_diff_offset': first,
    'first_diff_jstar2': b2[first] if first is not None else None,
    'first_diff_jstar3': b3[first] if first is not None else None,
}
(out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary))
PY

log "trace complete"
