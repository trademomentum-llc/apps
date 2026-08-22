#!/usr/bin/env bash
set -euo pipefail

# Close the NNOS release-provenance loop from a local, user-controlled shell.
#
# This script intentionally performs the security-sensitive pieces in a narrow,
# auditable sequence:
#   1. Commit only the quarantined debug_logs/ deletions, using the explicit
#      rr-commit-guard bypass required for that protected path.
#   2. Import the release signing private key into macOS Keychain as a
#      non-extractable identity.
#   3. Regenerate signed release provenance.
#   4. Verify deployment security, manifest signature/SBOM/attestation, JStar
#      fixpoint evidence, and the fail-closed release gate.
#   5. Commit only release/ provenance artifacts.
#
# Required current-state assumptions:
#   - Quarantined debug artifacts already live under:
#       /private/tmp/nnos-quarantine-2026-08-22/apps-nonproduction-artifacts
#   - The release signing private key already exists outside the repository:
#       /private/tmp/nnos-release-trustroot-2026-08-22/release-manifest.key.pem
#   - The immutable daemon image digest below has already been built locally.
#
# Optional environment overrides:
#   APP_REPO=/path/to/apps
#   KEY_DIR=/private/tmp/...
#   KEYCHAIN=/Users/nnos/Library/Keychains/login.keychain-db
#   IMAGE_REF=localhost/nnos-daemon:release-...
#   IMAGE_DIGEST=sha256:...
#   DELETE_TEMP_KEY_AFTER_IMPORT=1
#   FORCE_KEYCHAIN_IMPORT=1
#   SKIP_RELEASE_COMMIT=1

APP_REPO="${APP_REPO:-/Users/nnos/Projects/Sovereign/System/apps}"
KEY_DIR="${KEY_DIR:-/private/tmp/nnos-release-trustroot-2026-08-22}"
KEY_FILE="${KEY_FILE:-${KEY_DIR}/release-manifest.key.pem}"
CERT_FILE="${CERT_FILE:-${KEY_DIR}/release-manifest.cert.pem}"
P12_FILE="${P12_FILE:-${KEY_DIR}/release-manifest.keychain.p12}"
P12_PASS_FILE="${P12_PASS_FILE:-${KEY_DIR}/release-manifest.keychain.p12.pass}"
KEYCHAIN="${KEYCHAIN:-/Users/nnos/Library/Keychains/login.keychain-db}"
IDENTITY_LABEL="${IDENTITY_LABEL:-NNOS Release Manifest 2026-08-22}"
IMAGE_REF="${IMAGE_REF:-localhost/nnos-daemon:release-20260822-0639bcd659c4}"
IMAGE_DIGEST="${IMAGE_DIGEST:-sha256:03122d8cfaede9ec6cd8e4dbca680e6ad5db09c4cc5eb0180de89de704e48a3c}"
DELETE_TEMP_KEY_AFTER_IMPORT="${DELETE_TEMP_KEY_AFTER_IMPORT:-0}"
FORCE_KEYCHAIN_IMPORT="${FORCE_KEYCHAIN_IMPORT:-0}"
SKIP_RELEASE_COMMIT="${SKIP_RELEASE_COMMIT:-0}"

die() {
  printf 'ERROR: %s\n' "$*" >&2
  exit 1
}

note() {
  printf '\n==> %s\n' "$*"
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

assert_repo() {
  [[ -d "$APP_REPO/.git" ]] || die "APP_REPO is not a git repository: $APP_REPO"
  cd "$APP_REPO"
}

assert_only_staged_debug_logs() {
  local staged_names staged_status
  staged_names="$(git diff --cached --name-only)"
  [[ -n "$staged_names" ]] || return 0

  if printf '%s\n' "$staged_names" | grep -v '^debug_logs/' >/dev/null; then
    printf '%s\n' "$staged_names" >&2
    die "refusing to continue: staged changes outside debug_logs/ are present"
  fi

  staged_status="$(git diff --cached --name-status -- debug_logs)"
  if [[ -n "$staged_status" ]] && ! printf '%s\n' "$staged_status" | awk '$1 != "D" { bad=1 } END { exit bad }'; then
    printf '%s\n' "$staged_status" >&2
    die "refusing to continue: debug_logs/ staged changes are not all deletions"
  fi
}

stage_debug_log_deletions_if_needed() {
  if ! git diff --cached --quiet -- debug_logs; then
    printf 'debug_logs/ deletions are already staged; skipping git add for the missing directory.\n'
    return 0
  fi

  if ! git diff --name-only --diff-filter=D -- debug_logs | grep -q .; then
    printf 'No unstaged tracked debug_logs/ deletions found.\n'
    return 0
  fi

  # Stage only tracked updates/deletions. Do not stage untracked debug files.
  if ! git add -u -- debug_logs 2>/dev/null; then
    # When the entire directory is gone, some Git versions reject the directory
    # pathspec. Fall back to the exact deleted tracked paths reported by Git.
    git diff --name-only -z --diff-filter=D -- debug_logs | xargs -0 git add -u --
  fi
}

commit_quarantined_debug_deletions() {
  note "Committing quarantined debug_logs/ deletions"
  assert_only_staged_debug_logs
  stage_debug_log_deletions_if_needed
  assert_only_staged_debug_logs

  if git diff --cached --quiet -- debug_logs; then
    printf 'No staged debug_logs/ deletion commit needed.\n'
    return 0
  fi

  RR_COMMIT_GUARD_BYPASS=1 git commit \
    -m "fix(security): remove quarantined debug artifacts" \
    -m "The debug_logs artifacts were moved to /private/tmp/nnos-quarantine-2026-08-22/apps-nonproduction-artifacts with hash evidence before deleting the tracked production paths. This keeps non-production crash/debug artifacts out of the release tree while preserving quarantine evidence."
}

import_release_key_to_keychain() {
  note "Importing release signing identity into macOS Keychain"

  [[ -f "$KEY_FILE" ]] || die "missing release signing private key: $KEY_FILE"
  [[ -f "$KEYCHAIN" ]] || die "missing target keychain: $KEYCHAIN"

  local key_mode
  key_mode="$(stat -f '%Lp' "$KEY_FILE")"
  [[ "$key_mode" == "600" ]] || die "private key permissions must be 0600, found ${key_mode}: $KEY_FILE"

  if [[ "$FORCE_KEYCHAIN_IMPORT" != "1" ]] && security find-certificate -c "$IDENTITY_LABEL" "$KEYCHAIN" >/dev/null 2>&1; then
    printf 'Keychain certificate already present for "%s"; set FORCE_KEYCHAIN_IMPORT=1 to import again.\n' "$IDENTITY_LABEL"
    return 0
  fi

  umask 077
  openssl req -new -x509 \
    -key "$KEY_FILE" \
    -out "$CERT_FILE" \
    -days 3650 \
    -subj "/CN=${IDENTITY_LABEL}/O=Trade Momentum LLC/OU=NNOS"

  if [[ ! -f "$P12_PASS_FILE" ]]; then
    openssl rand -base64 48 >"$P12_PASS_FILE"
    chmod 0600 "$P12_PASS_FILE"
  fi

  # The PKCS#12 is created only as a local import wrapper. OpenSSL 3 defaults
  # can produce PBES2/AES/SHA256 bundles that some macOS security builds reject
  # with a misleading "passphrase incorrect" error, so use the legacy-compatible
  # envelope for import only. The imported private key is marked non-extractable.
  openssl pkcs12 -export \
    -legacy \
    -inkey "$KEY_FILE" \
    -in "$CERT_FILE" \
    -name "$IDENTITY_LABEL" \
    -out "$P12_FILE" \
    -passout "file:${P12_PASS_FILE}"

  if ! security import "$P12_FILE" \
    -k "$KEYCHAIN" \
    -f pkcs12 \
    -P "$(cat "$P12_PASS_FILE")" \
    -x \
    -T /usr/bin/openssl; then
    printf 'PKCS#12 import failed; falling back to direct PEM private key and certificate import.\n' >&2
    security import "$KEY_FILE" \
      -k "$KEYCHAIN" \
      -t priv \
      -f openssl \
      -x \
      -T /usr/bin/openssl
    security import "$CERT_FILE" \
      -k "$KEYCHAIN" \
      -t cert \
      -f openssl
  fi

  security find-certificate -c "$IDENTITY_LABEL" "$KEYCHAIN" >/dev/null
  rm -f "$P12_FILE" "$P12_PASS_FILE"
  printf 'Imported non-extractable Keychain identity: %s\n' "$IDENTITY_LABEL"
}

regenerate_release_provenance() {
  note "Regenerating signed release provenance"
  [[ -f "$KEY_FILE" ]] || die "release generator still requires KEY_FILE for signing: $KEY_FILE"

  scripts/generate_release_provenance.py \
    --signing-key "$KEY_FILE" \
    --image-ref "$IMAGE_REF" \
    --image-digest "$IMAGE_DIGEST"
}

verify_release() {
  note "Verifying deployment security, provenance, JStar fixpoint, and release gate"

  python3 scripts/verify_deployment_security.py

  python3 scripts/verify_release_provenance.py \
    --manifest release/provenance/manifest.sha256 \
    --signature release/provenance/manifest.sha256.sig \
    --public-key release/provenance/release-manifest.pub.pem \
    --sbom release/provenance/sbom.spdx.json \
    --attestation release/provenance/build.attestation.intoto.jsonl \
    --reproducible release/provenance/reproducible-verification.json

  scripts/verify_jstar_canonical_baseline.sh
}

commit_release_artifacts() {
  note "Committing release provenance artifacts"
  git add -A release

  if git diff --cached --quiet -- release; then
    printf 'No staged release/ provenance commit needed.\n'
    return 0
  fi

  git commit \
    -m "build(release): add signed provenance bundle" \
    -m "Adds the signed release manifest, SPDX SBOM, build attestation, reproducible verification evidence, node identity manifest, firewall manifest, and public release verification key."
}

final_gate() {
  note "Running fail-closed release gate"
  scripts/release_provenance_gate.sh
}

maybe_delete_temp_key() {
  if [[ "$DELETE_TEMP_KEY_AFTER_IMPORT" != "1" ]]; then
    printf '\nTemporary private key retained for current OpenSSL-based signing bridge: %s\n' "$KEY_FILE"
    printf 'Set DELETE_TEMP_KEY_AFTER_IMPORT=1 to remove it after a successful run.\n'
    return 0
  fi

  note "Deleting temporary private key after successful Keychain import and release signing"
  [[ "$KEY_FILE" == /private/tmp/nnos-release-trustroot-2026-08-22/release-manifest.key.pem ]] || \
    die "refusing to delete unexpected KEY_FILE path: $KEY_FILE"
  rm -f "$KEY_FILE"
  printf 'Deleted temporary private key: %s\n' "$KEY_FILE"
}

main() {
  require_cmd git
  require_cmd openssl
  require_cmd security
  require_cmd python3

  assert_repo

  commit_quarantined_debug_deletions
  import_release_key_to_keychain
  regenerate_release_provenance
  verify_release

  if [[ "$SKIP_RELEASE_COMMIT" == "1" ]]; then
    printf '\nSKIP_RELEASE_COMMIT=1 set; release/ artifacts remain uncommitted.\n'
  else
    commit_release_artifacts
  fi

  final_gate
  maybe_delete_temp_key

  note "Complete"
  git status --short
}

main "$@"
