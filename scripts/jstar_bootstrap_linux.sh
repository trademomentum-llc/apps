#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
CHECK_SCRIPT="${SCRIPT_DIR}/jstar_bootstrap_check.sh"
TRACE_SCRIPT="${SCRIPT_DIR}/jstar_bootstrap_trace.sh"

COMPILER_SRC="${JSTAR_COMPILER_SRC:-${ROOT_DIR}/jstar/compiler.jstr}"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/home/llc/.cache/jstar-target}"
RUN_FIXPOINT="${RUN_FIXPOINT:-0}"
RUN_TRACE="${RUN_TRACE:-1}"

log() {
  printf '[jstar-bootstrap-linux] %s\n' "$1"
}

if [ "$(uname -s)" != "Linux" ]; then
  log "Linux is required; current host is $(uname -s)"
  exit 1
fi

if [ ! -f "${COMPILER_SRC}" ]; then
  log "compiler source not found at ${COMPILER_SRC}"
  exit 1
fi

mkdir -p "${CARGO_TARGET_DIR}"

log "running preflight checks"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR}" \
  JSTAR_COMPILER_SRC="${COMPILER_SRC}" \
  RUN_FIXPOINT=0 \
  "${CHECK_SCRIPT}"

log "running Linux self-host smoke tests"
cd "${ROOT_DIR}"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR}" cargo test test_selfhost_arithmetic -- --nocapture
CARGO_TARGET_DIR="${CARGO_TARGET_DIR}" cargo test test_selfhost_variable -- --nocapture
CARGO_TARGET_DIR="${CARGO_TARGET_DIR}" cargo test test_selfhost_if_else -- --nocapture

if [ "${RUN_FIXPOINT}" = "1" ]; then
  log "running fixpoint test"
  CARGO_TARGET_DIR="${CARGO_TARGET_DIR}" cargo test test_t_diagram_fixpoint -- --ignored --nocapture
else
  log "skipping fixpoint test (RUN_FIXPOINT=${RUN_FIXPOINT})"
fi

if [ "${RUN_TRACE}" = "1" ]; then
  log "running stage trace"
  CARGO_TARGET_DIR="${CARGO_TARGET_DIR}" JSTAR_COMPILER_SRC="${COMPILER_SRC}" "${TRACE_SCRIPT}"
fi

log "bootstrap ladder completed"

