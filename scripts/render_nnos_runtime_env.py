#!/usr/bin/env python3
"""
Render non-secret NNOS runtime environment values from a validated identity
manifest. Secret keys are intentionally not printed.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from verify_deployment_security import ValidationError, validate


def load_manifest(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def csv(values: list[int] | list[str]) -> str:
    return ",".join(str(value) for value in values)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--identity", default="release/security/node-identity.manifest.json")
    parser.add_argument("--firewall", required=True)
    args = parser.parse_args()

    identity_path = Path(args.identity)
    firewall_path = Path(args.firewall)
    try:
        validate(identity_path, firewall_path)
    except ValidationError as exc:
        print(f"refusing to render runtime env: {exc}", file=sys.stderr)
        return 1

    manifest = load_manifest(identity_path)
    local_node = manifest["local_node"]
    sync = manifest["sync"]
    ninl = manifest["ninl"]

    print("# Non-secret NNOS runtime environment generated from validated identity and firewall manifests.")
    print("# Inject NNOS_SYNC_KEY and NNOS_NINL_AUTH_KEY from protected storage at deploy time.")
    print(f"# sync.aead_key_ref={sync['aead_key_ref']['provider']}:{sync['aead_key_ref']['id']}")
    print("# ninl.auth_key_ref=[redacted]")
    print(f"NNOS_SYNC_NODE_ID={local_node['node_id']}")
    print(f"NNOS_SYNC_IFADDR={sync['bind_address']}")
    print(f"NNOS_SYNC_ALLOWED_PEERS={csv(sync['allowed_peer_node_ids'])}")
    print(f"NNOS_SYNC_ALLOWED_SOURCES={csv(sync['expected_source_ips'])}")
    print("NNOS_SYNC_FIREWALL_CONFIRMED=1")
    print(f"NNOS_NINL_ISSUER_ID={local_node['issuer_id']}")
    print(f"NNOS_NINL_BIND_IP={sync['bind_address']}")
    print(f"NNOS_NINL_ALLOWED_PEERS={csv(ninl['allowed_peer_issuer_ids'])}")
    print(f"NNOS_NINL_PROVENANCE_DIGEST={ninl['provenance_digest']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
