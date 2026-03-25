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

run_stage() {
  set +e
  "$@"
  local rc=$?
  set -e
  return "$rc"
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
CARGO_TARGET_DIR="${CARGO_TARGET_DIR}" run_stage \
  cargo run --quiet -- jstar compile --input "${COMPILER_SRC}" --output "${OUT_DIR}/jstar1" --raw
stage1_rc=$?
if [ "${stage1_rc}" -ne 0 ]; then
  log "stage1 failed rc=${stage1_rc}"
  printf '{"stage1_rc": %s, "stage2_rc": null, "stage3_rc": null}\n' "${stage1_rc}" > "${OUT_DIR}/summary.json"
  exit "${stage1_rc}"
fi
chmod +x "${OUT_DIR}/jstar1"

log "stage2: jstar1 -> jstar2"
set +e
"${OUT_DIR}/jstar1" < "${COMPILER_SRC}" > "${OUT_DIR}/jstar2" 2> "${OUT_DIR}/jstar2.stderr"
stage2_rc=$?
set -e
if [ -f "${OUT_DIR}/jstar2" ]; then
  chmod +x "${OUT_DIR}/jstar2"
fi
log "stage2 rc=${stage2_rc}"

log "stage3: jstar2 -> jstar3"
stage3_rc=127
if [ "${stage2_rc}" -eq 0 ]; then
  set +e
  "${OUT_DIR}/jstar2" < "${COMPILER_SRC}" > "${OUT_DIR}/jstar3" 2> "${OUT_DIR}/jstar3.stderr"
  stage3_rc=$?
  set -e
  if [ -f "${OUT_DIR}/jstar3" ]; then
    chmod +x "${OUT_DIR}/jstar3"
  fi
  log "stage3 rc=${stage3_rc}"
else
  log "skipping stage3 because stage2 failed"
fi

sha_files=()
for f in "${OUT_DIR}/jstar1" "${OUT_DIR}/jstar2" "${OUT_DIR}/jstar3"; do
  if [ -f "${f}" ]; then
    sha_files+=("${f}")
  fi
done
if [ "${#sha_files[@]}" -gt 0 ]; then
  sha256sum "${sha_files[@]}" > "${OUT_DIR}/sha256.txt"
  wc -c "${sha_files[@]}" > "${OUT_DIR}/sizes.txt"
fi
if [ -f "${OUT_DIR}/jstar2" ] && [ -f "${OUT_DIR}/jstar3" ]; then
  cmp -l "${OUT_DIR}/jstar2" "${OUT_DIR}/jstar3" > "${OUT_DIR}/jstar2_vs_jstar3.cmp" || true
fi

# Intent -> Mapping -> Machine-Executable -> Execution smoke path
SAMPLE_SRC="${OUT_DIR}/intent_sample.jstr"
cat > "${SAMPLE_SRC}" <<'EOF'
add 20 22
return it
EOF

sample_stage1_compile_rc=127
sample_stage1_exec_rc=127
sample_stage2_compile_rc=127
sample_stage2_exec_rc=127

if [ "${stage1_rc}" -eq 0 ]; then
  set +e
  "${OUT_DIR}/jstar1" < "${SAMPLE_SRC}" > "${OUT_DIR}/intent_stage1.elf" 2> "${OUT_DIR}/intent_stage1.stderr"
  sample_stage1_compile_rc=$?
  set -e
  if [ "${sample_stage1_compile_rc}" -eq 0 ] && [ -f "${OUT_DIR}/intent_stage1.elf" ]; then
    chmod +x "${OUT_DIR}/intent_stage1.elf"
    set +e
    "${OUT_DIR}/intent_stage1.elf" > /dev/null 2>&1
    sample_stage1_exec_rc=$?
    set -e
  fi
fi

if [ "${stage2_rc}" -eq 0 ] && [ -f "${OUT_DIR}/jstar2" ]; then
  set +e
  "${OUT_DIR}/jstar2" < "${SAMPLE_SRC}" > "${OUT_DIR}/intent_stage2.elf" 2> "${OUT_DIR}/intent_stage2.stderr"
  sample_stage2_compile_rc=$?
  set -e
  if [ "${sample_stage2_compile_rc}" -eq 0 ] && [ -f "${OUT_DIR}/intent_stage2.elf" ]; then
    chmod +x "${OUT_DIR}/intent_stage2.elf"
    set +e
    "${OUT_DIR}/intent_stage2.elf" > /dev/null 2>&1
    sample_stage2_exec_rc=$?
    set -e
  fi
fi

JSTAR_BOOTSTRAP_OUT_DIR="${OUT_DIR}" STAGE1_RC="${stage1_rc}" STAGE2_RC="${stage2_rc}" STAGE3_RC="${stage3_rc}" SAMPLE_STAGE1_COMPILE_RC="${sample_stage1_compile_rc}" SAMPLE_STAGE1_EXEC_RC="${sample_stage1_exec_rc}" SAMPLE_STAGE2_COMPILE_RC="${sample_stage2_compile_rc}" SAMPLE_STAGE2_EXEC_RC="${sample_stage2_exec_rc}" python3 - <<'PY'
from pathlib import Path
import json
import os
out = Path(os.environ['JSTAR_BOOTSTRAP_OUT_DIR'])
stage1_rc = int(os.environ['STAGE1_RC'])
stage2_rc = int(os.environ['STAGE2_RC'])
stage3_rc = int(os.environ['STAGE3_RC'])
sample_stage1_compile_rc = int(os.environ['SAMPLE_STAGE1_COMPILE_RC'])
sample_stage1_exec_rc = int(os.environ['SAMPLE_STAGE1_EXEC_RC'])
sample_stage2_compile_rc = int(os.environ['SAMPLE_STAGE2_COMPILE_RC'])
sample_stage2_exec_rc = int(os.environ['SAMPLE_STAGE2_EXEC_RC'])
b2 = (out / 'jstar2').read_bytes() if (out / 'jstar2').exists() else b''
b3 = (out / 'jstar3').read_bytes() if (out / 'jstar3').exists() else b''
first = None
if b2 and b3:
    for i, (a, b) in enumerate(zip(b2, b3)):
        if a != b:
            first = i
            break
summary = {
    'stage1_rc': stage1_rc,
    'stage2_rc': stage2_rc,
    'stage3_rc': stage3_rc,
    'intent_sample': {
        'stage1_compile_rc': sample_stage1_compile_rc,
        'stage1_exec_rc': sample_stage1_exec_rc,
        'stage2_compile_rc': sample_stage2_compile_rc,
        'stage2_exec_rc': sample_stage2_exec_rc,
    },
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
