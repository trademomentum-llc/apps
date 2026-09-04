#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
COMPILER_SRC="${JSTAR_COMPILER_SRC:-${ROOT_DIR}/jstar/compiler.jstr}"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${ROOT_DIR}/target/jstar-bootstrap}"
OUT_DIR="${JSTAR_BOOTSTRAP_OUT_DIR:-}"

log() {
  printf '[jstar-bootstrap-trace] %s\n' "$1"
}

fail() {
  printf '[jstar-bootstrap-trace] ERROR: %s\n' "$1" >&2
  exit 1
}

run_stage() {
  set +e
  "$@"
  local rc=$?
  set -e
  return "$rc"
}

[ "${ALLOW_FORENSIC_BOOTSTRAP_REBUILD:-0}" = "1" ] || \
  fail "refusing live bootstrap rebuild without ALLOW_FORENSIC_BOOTSTRAP_REBUILD=1"
[ -n "${OUT_DIR}" ] || \
  fail "JSTAR_BOOTSTRAP_OUT_DIR must point to an explicit quarantine/output directory"
[ -f "${COMPILER_SRC}" ] || fail "compiler source not found at ${COMPILER_SRC}"

case "${OUT_DIR}" in
  "${ROOT_DIR}"/*)
    fail "refusing to write bootstrap trace inside the release repository: ${OUT_DIR}"
    ;;
  /home/llc/*)
    fail "refusing legacy hardcoded /home/llc bootstrap path: ${OUT_DIR}"
    ;;
esac

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
  printf '{"stage1_rc": %s, "stage2_rc": null, "stage3_rc": null, "stage4_rc": null, "stage5_rc": null}\n' \
    "${stage1_rc}" > "${OUT_DIR}/summary.json"
  fail "stage1 failed rc=${stage1_rc}"
fi
chmod +x "${OUT_DIR}/jstar1"

prev="${OUT_DIR}/jstar1"
stage2_rc=127
stage3_rc=127
stage4_rc=127
stage5_rc=127

for stage in 2 3 4 5; do
  out="${OUT_DIR}/jstar${stage}"
  log "stage${stage}: $(basename "${prev}") -> jstar${stage}"
  set +e
  "${prev}" < "${COMPILER_SRC}" > "${out}" 2> "${out}.stderr"
  rc=$?
  set -e
  chmod +x "${out}" 2>/dev/null || true
  case "${stage}" in
    2) stage2_rc=${rc} ;;
    3) stage3_rc=${rc} ;;
    4) stage4_rc=${rc} ;;
    5) stage5_rc=${rc} ;;
  esac
  log "stage${stage} rc=${rc}"
  if [ "${rc}" -ne 0 ]; then
    break
  fi
  prev="${out}"
done

sha_files=()
for f in "${OUT_DIR}"/jstar1 "${OUT_DIR}"/jstar2 "${OUT_DIR}"/jstar3 "${OUT_DIR}"/jstar4 "${OUT_DIR}"/jstar5; do
  if [ -f "${f}" ]; then
    sha_files+=("${f}")
  fi
done
if [ "${#sha_files[@]}" -gt 0 ]; then
  sha256sum "${sha_files[@]}" > "${OUT_DIR}/sha256.txt"
  wc -c "${sha_files[@]}" > "${OUT_DIR}/sizes.txt"
fi
if [ -f "${OUT_DIR}/jstar4" ] && [ -f "${OUT_DIR}/jstar5" ]; then
  cmp -l "${OUT_DIR}/jstar4" "${OUT_DIR}/jstar5" > "${OUT_DIR}/jstar4_vs_jstar5.cmp" || true
fi

JSTAR_BOOTSTRAP_OUT_DIR="${OUT_DIR}" \
STAGE1_RC="${stage1_rc}" \
STAGE2_RC="${stage2_rc}" \
STAGE3_RC="${stage3_rc}" \
STAGE4_RC="${stage4_rc}" \
STAGE5_RC="${stage5_rc}" \
python3 - <<'PY'
from pathlib import Path
import hashlib
import json
import os

out = Path(os.environ["JSTAR_BOOTSTRAP_OUT_DIR"])
summary = {
    "stage1_rc": int(os.environ["STAGE1_RC"]),
    "stage2_rc": int(os.environ["STAGE2_RC"]),
    "stage3_rc": int(os.environ["STAGE3_RC"]),
    "stage4_rc": int(os.environ["STAGE4_RC"]),
    "stage5_rc": int(os.environ["STAGE5_RC"]),
    "artifacts": {},
    "canonical_fixpoint": False,
}

for stage in range(1, 6):
    path = out / f"jstar{stage}"
    if path.exists():
        data = path.read_bytes()
        summary["artifacts"][f"jstar{stage}"] = {
            "size_bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
        }

j4 = summary["artifacts"].get("jstar4", {}).get("sha256")
j5 = summary["artifacts"].get("jstar5", {}).get("sha256")
summary["canonical_fixpoint"] = bool(j4 and j5 and j4 == j5)
(out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
print(json.dumps(summary))
PY

if [ "${stage4_rc}" -eq 0 ] && [ "${stage5_rc}" -eq 0 ] && cmp -s "${OUT_DIR}/jstar4" "${OUT_DIR}/jstar5"; then
  log "trace complete: jstar4 == jstar5"
else
  fail "trace complete with noncanonical or incomplete fixpoint; keep output quarantined"
fi
