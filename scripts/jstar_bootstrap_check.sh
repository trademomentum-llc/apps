#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
COMPILER_SRC="${JSTAR_COMPILER_SRC:-${ROOT_DIR}/jstar/compiler.jstr}"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${ROOT_DIR}/target/jstar-bootstrap}"
RUN_FIXPOINT="${RUN_FIXPOINT:-0}"
BASELINE_SCRIPT="${SCRIPT_DIR}/verify_jstar_canonical_baseline.sh"
LOG_PREFIX="[jstar-bootstrap]"

errors=()

log() {
  printf '%s %s\n' "${LOG_PREFIX}" "$1"
}

check_file() {
  if [ ! -f "$1" ]; then
    errors+=("$2")
  fi
}

check_executable() {
  if [ ! -x "$1" ]; then
    errors+=("$2")
  fi
}

check_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    errors+=("$2")
  fi
}

log "repo_root=${ROOT_DIR}"
log "compiler_src=${COMPILER_SRC}"
log "cargo_target_dir=${CARGO_TARGET_DIR}"

check_file "${COMPILER_SRC}" \
  "compiler source not found at ${COMPILER_SRC}"
check_executable "${BASELINE_SCRIPT}" \
  "canonical baseline verifier not executable at ${BASELINE_SCRIPT}"

if [ "${RUN_FIXPOINT}" = "1" ]; then
  check_command cargo "cargo is required for optional regression tests"
  if [ "$(uname -s)" != "Linux" ]; then
    errors+=("Linux is required for optional live self-host diagnostics; current host is $(uname -s)")
  fi
fi

mkdir -p "${CARGO_TARGET_DIR}"

if [ "${#errors[@]}" -gt 0 ]; then
  log "bootstrap prerequisites are not satisfied:"
  for err in "${errors[@]}"; do
    printf '  - %s\n' "${err}"
  done
  find "${ROOT_DIR}" -maxdepth 4 -name '*.jstr' | sort | sed 's/^/  candidate: /' || true
  exit 1
fi

log "verifying canonical JStar self-host baseline"
"${BASELINE_SCRIPT}"

if [ "${RUN_FIXPOINT}" != "1" ]; then
  log "preflight passed; canonical invariant is jstar4 == jstar5"
  log "live rebuild diagnostics require RUN_FIXPOINT=1 and an explicit JSTAR_BOOTSTRAP_OUT_DIR"
  exit 0
fi

log "running portable codegen regression suite"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR}" cargo test jstar::codegen -- --nocapture

log "canonical fixpoint already verified by ${BASELINE_SCRIPT}"
log "use ALLOW_FORENSIC_BOOTSTRAP_REBUILD=1 scripts/jstar_bootstrap_trace.sh for a quarantined live ladder"
