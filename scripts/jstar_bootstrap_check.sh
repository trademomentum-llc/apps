#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
COMPILER_SRC="${JSTAR_COMPILER_SRC:-${ROOT_DIR}/jstar/compiler.jstr}"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/home/llc/.cache/jstar-target}"
RUN_FIXPOINT="${RUN_FIXPOINT:-0}"

errors=()

log() {
  printf '[jstar-bootstrap] %s\n' "$1"
}

check() {
  if ! eval "$1"; then
    errors+=("$2")
  fi
}

log "repo_root=${ROOT_DIR}"
log "compiler_src=${COMPILER_SRC}"
log "cargo_target_dir=${CARGO_TARGET_DIR}"

check "command -v cargo >/dev/null 2>&1" "cargo is required"
check "[[ \"$(uname -s)\" == \"Linux\" ]]" \
  "Linux is required for the self-host fixpoint run; current host is $(uname -s)"
check "[[ -f \"${COMPILER_SRC}\" ]]" \
  "compiler source not found at ${COMPILER_SRC}"

mkdir -p "${CARGO_TARGET_DIR}"

if [ "${#errors[@]}" -gt 0 ]; then
  log "bootstrap prerequisites are not satisfied:"
  for err in "${errors[@]}"; do
    printf '  - %s\n' "${err}"
  done
  find "${ROOT_DIR}" -maxdepth 4 -name '*.jstr' | sort | sed 's/^/  candidate: /' || true
  exit 1
fi

log "running portable codegen regression suite"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR}" cargo test jstar::codegen -- --nocapture

if [ "${RUN_FIXPOINT}" != "1" ]; then
  log "preflight passed"
  log "re-run with RUN_FIXPOINT=1 for fixpoint test"
  exit 0
fi

log "running Linux self-host fixpoint test"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR}" cargo test test_t_diagram_fixpoint -- --ignored --nocapture

