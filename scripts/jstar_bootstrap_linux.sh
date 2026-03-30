#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
CHECK_SCRIPT="${SCRIPT_DIR}/jstar_bootstrap_check.sh"
TRACE_SCRIPT="${SCRIPT_DIR}/jstar_bootstrap_trace.sh"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/home/llc/.cache/jstar-target}"
RUN_FIXPOINT="${RUN_FIXPOINT:-0}"
RUN_TRACE="${RUN_TRACE:-1}"

log() {
  printf '[jstar-bootstrap-linux] %s\n' "$1"
}

CANONICAL_COMPILER_DIR="${ROOT_DIR}/jstar"
CANONICAL_COMPILER_SRC="${CANONICAL_COMPILER_DIR}/compiler.jstr"
SOURCE_COMPILER_SRC="${JSTAR_COMPILER_SRC:-${CANONICAL_COMPILER_SRC}}"

STAGED_LINK=0
CREATED_COMPILER_DIR=0

cleanup() {
  if [ "${STAGED_LINK}" -eq 1 ] && [ -L "${CANONICAL_COMPILER_SRC}" ]; then
    rm -f "${CANONICAL_COMPILER_SRC}"
  fi

  if [ "${CREATED_COMPILER_DIR}" -eq 1 ] && [ -d "${CANONICAL_COMPILER_DIR}" ]; then
    rmdir "${CANONICAL_COMPILER_DIR}" 2>/dev/null || true
  fi
}

trap cleanup EXIT

if [ "$(uname -s)" != "Linux" ]; then
  log "Linux is required; current host is $(uname -s)"
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  log "cargo is required"
  exit 1
fi

if [ ! -f "${SOURCE_COMPILER_SRC}" ]; then
  log "compiler source not found at ${SOURCE_COMPILER_SRC}"
  exit 1
fi

if [ ! -e "${CANONICAL_COMPILER_SRC}" ]; then
  if [ ! -d "${CANONICAL_COMPILER_DIR}" ]; then
    mkdir -p "${CANONICAL_COMPILER_DIR}"
    CREATED_COMPILER_DIR=1
  fi

  if [ "${SOURCE_COMPILER_SRC}" != "${CANONICAL_COMPILER_SRC}" ]; then
    ln -s "${SOURCE_COMPILER_SRC}" "${CANONICAL_COMPILER_SRC}"
    STAGED_LINK=1
    log "staged compiler source at ${CANONICAL_COMPILER_SRC}"
  fi
fi

log "running preflight checks"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR}" \
  JSTAR_COMPILER_SRC="${SOURCE_COMPILER_SRC}" \
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
  CARGO_TARGET_DIR="${CARGO_TARGET_DIR}" \
    JSTAR_COMPILER_SRC="${SOURCE_COMPILER_SRC}" \
    "${TRACE_SCRIPT}"
fi

log "bootstrap ladder completed"
