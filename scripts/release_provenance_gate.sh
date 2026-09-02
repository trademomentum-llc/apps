#!/usr/bin/env bash
set -euo pipefail

fail() {
    printf 'RELEASE GATE FAIL: %s\n' "$*" >&2
    exit 1
}

git rev-parse --is-inside-work-tree >/dev/null 2>&1 || fail "not inside a git work tree"
repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

status="$(git status --short)"
if [ -n "$status" ]; then
    printf '%s\n' "$status" >&2
    fail "git status --short is non-empty"
fi

for path in debug_logs jstar_bootstrap_out wandb; do
    if [ -e "$path" ]; then
        fail "release tree contains non-production artifact path: $path"
    fi
done

for path in jstar/jstar_c jstar/jstar_canonical jstar/jstar_c.c jstar/jstar_canonical.c; do
    if [ -e "$path" ]; then
        fail "release tree contains non-canonical JStar bootstrap helper: $path"
    fi
done

scripts/verify_jstar_canonical_baseline.sh \
    || fail "canonical JStar self-host baseline validation failed"

if [ -f .gitmodules ]; then
    submodule_status="$(git submodule status --recursive)"
    if printf '%s\n' "$submodule_status" | grep -Eq '^[+-U]'; then
        printf '%s\n' "$submodule_status" >&2
        fail "one or more submodules are uninitialized, unpinned, or conflicted"
    fi
fi

required_files=(
    release/provenance/manifest.sha256
    release/provenance/manifest.sha256.sig
    release/provenance/release-manifest.pub.pem
    release/provenance/sbom.spdx.json
    release/provenance/build.attestation.intoto.jsonl
    release/provenance/reproducible-verification.json
    release/security/node-identity.manifest.json
    release/security/nnos-sync-plane.firewall.json
)

for file in "${required_files[@]}"; do
    [ -s "$file" ] || fail "missing required release file: $file"
done

quadlet_scan_roots=(
    ../engine/nnos/quadlets
    ../NeuroDiOS/services/quadlets
    ../NeuroDiOS/external/engine/nnos/quadlets
)

if grep -R --line-number -E '^Image=.*:latest([[:space:]]|$)' \
    "${quadlet_scan_roots[@]}" 2>/dev/null; then
    fail "quadlet images must be digest-pinned; :latest is prohibited"
fi

if grep -R --line-number 'sha256:0000000000000000000000000000000000000000000000000000000000000000' \
    "${quadlet_scan_roots[@]}" 2>/dev/null; then
    fail "quadlet image digest placeholder must be replaced with a signed image digest"
fi

if grep -R --line-number -E '^[[:space:]]*PublishPort=20046:20046/udp' \
    ../NeuroDiOS/services/quadlets 2>/dev/null; then
    fail "UDP 20046 must not be published by generic pods; only the verified sync unit may use the sync plane"
fi

python3 scripts/verify_deployment_security.py \
    --identity release/security/node-identity.manifest.json \
    --firewall release/security/nnos-sync-plane.firewall.json \
    || fail "deployment identity/firewall validation failed"

python3 scripts/verify_release_provenance.py \
    --manifest release/provenance/manifest.sha256 \
    --signature release/provenance/manifest.sha256.sig \
    --public-key release/provenance/release-manifest.pub.pem \
    --sbom release/provenance/sbom.spdx.json \
    --attestation release/provenance/build.attestation.intoto.jsonl \
    --reproducible release/provenance/reproducible-verification.json \
    || fail "release provenance cryptographic validation failed"

printf 'RELEASE GATE PASS: clean tree, pinned submodules, signed manifest, SBOM, build attestation, reproducible verification, identity, and firewall policy present.\n'
