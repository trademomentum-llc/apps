#!/usr/bin/env python3
"""
Validate NNOS deployment identity and firewall manifests without printing or
requiring secret material.
"""

from __future__ import annotations

import argparse
import datetime as dt
import ipaddress
import json
import re
import sys
import tempfile
from pathlib import Path
from typing import Any


IDENTITY_SCHEMA = "nnos.identity.v1"
FIREWALL_SCHEMA = "nnos.firewall.v1"
SYNC_GROUP = "239.192.0.1"
SYNC_PORT = 20046
ALLOWED_KEY_REF_PROVIDERS = {
    "macos-keychain",
    "linux-keyring",
    "systemd-credential",
    "hsm",
    "kms",
    "tmpfs-sealed-file",
}

SECRET_HEX_RE = re.compile(r"^[0-9a-fA-F]{64,}$")
API_KEY_RE = re.compile(r"(?i)(sk-[a-z0-9_-]{16,}|api[_-]?key\s*=|secret\s*=|token\s*=)")
PRIVATE_KEY_RE = re.compile(r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----")


class ValidationError(Exception):
    pass


def load_json(path: Path) -> dict[str, Any]:
    try:
        with path.open("r", encoding="utf-8") as handle:
            value = json.load(handle)
    except FileNotFoundError as exc:
        raise ValidationError(f"missing manifest: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ValidationError(f"invalid JSON in {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise ValidationError(f"{path} must contain a JSON object")
    return value


def parse_time(value: Any, field: str) -> dt.datetime:
    if not isinstance(value, str) or not value:
        raise ValidationError(f"{field} must be an RFC3339 timestamp")
    normalized = value[:-1] + "+00:00" if value.endswith("Z") else value
    try:
        parsed = dt.datetime.fromisoformat(normalized)
    except ValueError as exc:
        raise ValidationError(f"{field} must be an RFC3339 timestamp") from exc
    if parsed.tzinfo is None:
        raise ValidationError(f"{field} must include a timezone")
    return parsed.astimezone(dt.timezone.utc)


def require_bool(value: Any, field: str) -> bool:
    if not isinstance(value, bool):
        raise ValidationError(f"{field} must be boolean")
    return value


def require_string(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ValidationError(f"{field} must be a non-empty string")
    return value


def require_uint32(value: Any, field: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0 or value > 0xFFFFFFFF:
        raise ValidationError(f"{field} must be an integer in 1..UINT32_MAX")
    return value


def require_ipv4(value: Any, field: str) -> str:
    raw = require_string(value, field)
    try:
        parsed = ipaddress.ip_address(raw)
    except ValueError as exc:
        raise ValidationError(f"{field} must be an IPv4 address") from exc
    if parsed.version != 4 or parsed.is_unspecified:
        raise ValidationError(f"{field} must be an explicit non-0.0.0.0 IPv4 address")
    return str(parsed)


def require_ipv4_list(value: Any, field: str) -> list[str]:
    if not isinstance(value, list) or not value:
        raise ValidationError(f"{field} must be a non-empty list of IPv4 addresses")
    seen: set[str] = set()
    out: list[str] = []
    for index, item in enumerate(value):
        parsed = require_ipv4(item, f"{field}[{index}]")
        if parsed in seen:
            raise ValidationError(f"{field} contains duplicate IPv4 address: {parsed}")
        seen.add(parsed)
        out.append(parsed)
    return out


def require_uint32_list(value: Any, field: str) -> list[int]:
    if not isinstance(value, list) or not value:
        raise ValidationError(f"{field} must be a non-empty list of node IDs")
    seen: set[int] = set()
    out: list[int] = []
    for index, item in enumerate(value):
        parsed = require_uint32(item, f"{field}[{index}]")
        if parsed in seen:
            raise ValidationError(f"{field} contains duplicate node ID: {parsed}")
        seen.add(parsed)
        out.append(parsed)
    return out


def is_digest_field(path: str) -> bool:
    lowered = path.lower()
    return any(word in lowered for word in ("digest", "hash", "fingerprint", "signature"))


def check_no_inline_secrets(value: Any, path: str = "$") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            check_no_inline_secrets(child, f"{path}.{key}")
        return
    if isinstance(value, list):
        for index, child in enumerate(value):
            check_no_inline_secrets(child, f"{path}[{index}]")
        return
    if not isinstance(value, str):
        return

    if PRIVATE_KEY_RE.search(value):
        raise ValidationError(f"{path} contains inline private key material")
    if API_KEY_RE.search(value):
        raise ValidationError(f"{path} contains inline API key, secret, or token material")
    if SECRET_HEX_RE.match(value) and not is_digest_field(path):
        raise ValidationError(f"{path} looks like inline raw key material")


def validate_key_ref(value: Any, field: str) -> dict[str, str]:
    if not isinstance(value, dict):
        raise ValidationError(f"{field} must be an object")
    provider = require_string(value.get("provider"), f"{field}.provider")
    if provider not in ALLOWED_KEY_REF_PROVIDERS:
        allowed = ", ".join(sorted(ALLOWED_KEY_REF_PROVIDERS))
        raise ValidationError(f"{field}.provider must be one of: {allowed}")
    key_id = require_string(value.get("id"), f"{field}.id")
    forbidden = {"value", "secret", "key", "material", "private_key", "raw"}
    present = forbidden.intersection(value)
    if present:
        raise ValidationError(f"{field} must not contain inline key fields: {sorted(present)}")
    return {"provider": provider, "id": key_id}


def validate_rotation(value: Any, now: dt.datetime) -> None:
    if not isinstance(value, dict):
        raise ValidationError("key_rotation must be an object")
    require_string(value.get("active_key_id"), "key_rotation.active_key_id")
    created = parse_time(value.get("created_at"), "key_rotation.created_at")
    rotate_after = parse_time(value.get("rotate_after"), "key_rotation.rotate_after")
    expires = parse_time(value.get("expires_at"), "key_rotation.expires_at")
    if not (created < rotate_after <= expires):
        raise ValidationError("key_rotation must satisfy created_at < rotate_after <= expires_at")
    if expires <= now:
        raise ValidationError("key_rotation.expires_at is not in the future")


def validate_digest(value: Any, field: str) -> str:
    digest = require_string(value, field)
    if not re.fullmatch(r"[0-9a-fA-F]{64}", digest):
        raise ValidationError(f"{field} must be a 64-character hex digest")
    if int(digest, 16) == 0:
        raise ValidationError(f"{field} must not be all zero")
    return digest.lower()


def quorum_for(total_nodes: int) -> int:
    return (2 * total_nodes + 2) // 3


def validate_identity(manifest: dict[str, Any], now: dt.datetime) -> dict[str, Any]:
    check_no_inline_secrets(manifest)
    if manifest.get("schema_version") != IDENTITY_SCHEMA:
        raise ValidationError(f"identity schema_version must be {IDENTITY_SCHEMA}")

    local_node = manifest.get("local_node")
    if not isinstance(local_node, dict):
        raise ValidationError("local_node must be an object")
    node_id = require_uint32(local_node.get("node_id"), "local_node.node_id")
    issuer_id = require_uint32(local_node.get("issuer_id"), "local_node.issuer_id")
    if issuer_id != node_id:
        raise ValidationError("local_node.issuer_id must equal local_node.node_id")

    sync = manifest.get("sync")
    if not isinstance(sync, dict):
        raise ValidationError("sync must be an object")
    bind_address = require_ipv4(sync.get("bind_address"), "sync.bind_address")
    allowed_peers = require_uint32_list(sync.get("allowed_peer_node_ids"), "sync.allowed_peer_node_ids")
    if node_id in allowed_peers:
        raise ValidationError("sync.allowed_peer_node_ids must not include local_node.node_id")
    allowed_sources = require_ipv4_list(sync.get("expected_source_ips"), "sync.expected_source_ips")
    validate_key_ref(sync.get("aead_key_ref"), "sync.aead_key_ref")

    ninl = manifest.get("ninl")
    if not isinstance(ninl, dict):
        raise ValidationError("ninl must be an object")
    validate_key_ref(ninl.get("auth_key_ref"), "ninl.auth_key_ref")
    validate_digest(ninl.get("provenance_digest"), "ninl.provenance_digest")
    ninl_allowed = require_uint32_list(ninl.get("allowed_peer_issuer_ids"), "ninl.allowed_peer_issuer_ids")
    if node_id in ninl_allowed:
        raise ValidationError("ninl.allowed_peer_issuer_ids must not include local_node.node_id")

    consensus = manifest.get("consensus")
    if not isinstance(consensus, dict):
        raise ValidationError("consensus must be an object")
    total_nodes = 1 + len(allowed_peers)
    expected_quorum = quorum_for(total_nodes)
    declared_quorum = require_uint32(consensus.get("required_quorum"), "consensus.required_quorum")
    if declared_quorum != expected_quorum:
        raise ValidationError(
            f"consensus.required_quorum must be {expected_quorum} for {total_nodes} configured nodes"
        )
    if require_bool(consensus.get("critical_actions_require_quorum"), "consensus.critical_actions_require_quorum") is not True:
        raise ValidationError("consensus.critical_actions_require_quorum must be true")

    validate_rotation(manifest.get("key_rotation"), now)

    return {
        "node_id": node_id,
        "allowed_peers": set(allowed_peers),
        "allowed_sources": set(allowed_sources),
        "bind_address": bind_address,
    }


def reject_broad_source(value: str, field: str) -> None:
    lowered = value.lower()
    if lowered in {"*", "any", "all", "0.0.0.0", "0.0.0.0/0", "::/0"}:
        raise ValidationError(f"{field} must not allow wildcard or global sources")


def validate_rule_sources(value: Any, field: str) -> list[str]:
    if not isinstance(value, list) or not value:
        raise ValidationError(f"{field} must be a non-empty list")
    sources: list[str] = []
    for index, item in enumerate(value):
        raw = require_string(item, f"{field}[{index}]")
        reject_broad_source(raw, f"{field}[{index}]")
        sources.append(require_ipv4(raw, f"{field}[{index}]"))
    return sources


def validate_firewall(manifest: dict[str, Any], identity: dict[str, Any]) -> None:
    check_no_inline_secrets(manifest)
    if manifest.get("schema_version") != FIREWALL_SCHEMA:
        raise ValidationError(f"firewall schema_version must be {FIREWALL_SCHEMA}")
    if require_bool(manifest.get("deny_by_default"), "deny_by_default") is not True:
        raise ValidationError("deny_by_default must be true")

    host_network_allowed = require_bool(manifest.get("host_network_allowed"), "host_network_allowed")
    if host_network_allowed:
        justification = require_string(manifest.get("host_network_justification"), "host_network_justification")
        if len(justification) < 24:
            raise ValidationError("host_network_justification must be concrete")

    sync_plane = manifest.get("sync_plane")
    if not isinstance(sync_plane, dict):
        raise ValidationError("sync_plane must be an object")
    if sync_plane.get("multicast_group") != SYNC_GROUP:
        raise ValidationError(f"sync_plane.multicast_group must be {SYNC_GROUP}")
    if sync_plane.get("udp_port") != SYNC_PORT:
        raise ValidationError(f"sync_plane.udp_port must be {SYNC_PORT}")
    if sync_plane.get("protocol") != "udp":
        raise ValidationError("sync_plane.protocol must be udp")
    interface_address = require_ipv4(sync_plane.get("interface_address"), "sync_plane.interface_address")
    if interface_address != identity["bind_address"]:
        raise ValidationError("firewall interface_address must match identity sync.bind_address")
    allowed_sources = set(require_ipv4_list(sync_plane.get("allowed_source_ips"), "sync_plane.allowed_source_ips"))
    if allowed_sources != identity["allowed_sources"]:
        raise ValidationError("firewall allowed_source_ips must match identity sync.expected_source_ips")

    if "allowed_peer_node_ids" in sync_plane:
        peer_ids = set(require_uint32_list(sync_plane.get("allowed_peer_node_ids"), "sync_plane.allowed_peer_node_ids"))
        if peer_ids != identity["allowed_peers"]:
            raise ValidationError("firewall allowed_peer_node_ids must match identity sync.allowed_peer_node_ids")

    rules = manifest.get("rules")
    if not isinstance(rules, list) or not rules:
        raise ValidationError("rules must be a non-empty list")

    allow_sources: set[str] = set()
    for index, rule in enumerate(rules):
        if not isinstance(rule, dict):
            raise ValidationError(f"rules[{index}] must be an object")
        action = require_string(rule.get("action"), f"rules[{index}].action")
        protocol = require_string(rule.get("protocol"), f"rules[{index}].protocol")
        port = rule.get("port")
        if protocol != "udp" or port != SYNC_PORT:
            raise ValidationError(f"rules[{index}] must target udp/{SYNC_PORT}")
        sources = validate_rule_sources(rule.get("sources"), f"rules[{index}].sources")
        if action == "allow":
            allow_sources.update(sources)
        elif action != "deny":
            raise ValidationError(f"rules[{index}].action must be allow or deny")

    if allow_sources != identity["allowed_sources"]:
        raise ValidationError("allow rules must exactly cover identity expected source IPs")


def validate(identity_path: Path, firewall_path: Path) -> None:
    now = dt.datetime.now(dt.timezone.utc)
    identity = validate_identity(load_json(identity_path), now)
    validate_firewall(load_json(firewall_path), identity)


def valid_example(now: dt.datetime) -> tuple[dict[str, Any], dict[str, Any]]:
    rotate = now + dt.timedelta(days=30)
    expires = now + dt.timedelta(days=45)
    identity = {
        "schema_version": IDENTITY_SCHEMA,
        "environment": "production",
        "local_node": {"node_id": 1, "issuer_id": 1, "role": "dcn"},
        "sync": {
            "bind_address": "10.44.0.10",
            "allowed_peer_node_ids": [2, 3],
            "expected_source_ips": ["10.44.0.11", "10.44.0.12"],
            "aead_key_ref": {"provider": "macos-keychain", "id": "nnos/sync/aead/2026q3"},
        },
        "ninl": {
            "auth_key_ref": {"provider": "macos-keychain", "id": "nnos/ninl/auth/2026q3"},
            "provenance_digest": "11" * 32,
            "allowed_peer_issuer_ids": [2, 3],
        },
        "consensus": {
            "required_quorum": 2,
            "critical_actions_require_quorum": True,
        },
        "key_rotation": {
            "active_key_id": "2026q3",
            "created_at": now.isoformat(),
            "rotate_after": rotate.isoformat(),
            "expires_at": expires.isoformat(),
        },
    }
    firewall = {
        "schema_version": FIREWALL_SCHEMA,
        "deny_by_default": True,
        "host_network_allowed": True,
        "host_network_justification": "Host networking is limited to an explicit NNOS sync-plane firewall policy.",
        "sync_plane": {
            "multicast_group": SYNC_GROUP,
            "udp_port": SYNC_PORT,
            "protocol": "udp",
            "interface_address": "10.44.0.10",
            "allowed_peer_node_ids": [2, 3],
            "allowed_source_ips": ["10.44.0.11", "10.44.0.12"],
        },
        "rules": [
            {"action": "allow", "protocol": "udp", "port": SYNC_PORT, "sources": ["10.44.0.11", "10.44.0.12"]},
        ],
    }
    return identity, firewall


def self_test() -> None:
    now = dt.datetime.now(dt.timezone.utc)
    identity, firewall = valid_example(now)
    with tempfile.TemporaryDirectory(prefix="nnos-deploy-security-") as temp_dir:
        temp = Path(temp_dir)
        identity_path = temp / "identity.json"
        firewall_path = temp / "firewall.json"
        identity_path.write_text(json.dumps(identity), encoding="utf-8")
        firewall_path.write_text(json.dumps(firewall), encoding="utf-8")
        validate(identity_path, firewall_path)

        bad_identity = json.loads(json.dumps(identity))
        bad_identity["sync"]["aead_key_ref"]["value"] = "aa" * 32
        identity_path.write_text(json.dumps(bad_identity), encoding="utf-8")
        try:
            validate(identity_path, firewall_path)
        except ValidationError:
            pass
        else:
            raise ValidationError("self-test failed to reject inline key material")

        identity_path.write_text(json.dumps(identity), encoding="utf-8")
        bad_firewall = json.loads(json.dumps(firewall))
        bad_firewall["rules"][0]["sources"] = ["0.0.0.0/0"]
        firewall_path.write_text(json.dumps(bad_firewall), encoding="utf-8")
        try:
            validate(identity_path, firewall_path)
        except ValidationError:
            pass
        else:
            raise ValidationError("self-test failed to reject broad firewall source")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--identity", default="release/security/node-identity.manifest.json")
    parser.add_argument("--firewall", default="release/security/nnos-sync-plane.firewall.json")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    try:
        if args.self_test:
            self_test()
            print("DEPLOYMENT SECURITY SELF-TEST PASS")
            return 0
        validate(Path(args.identity), Path(args.firewall))
        print("DEPLOYMENT SECURITY PASS: identity, key references, rotation, and firewall policy validated.")
        return 0
    except ValidationError as exc:
        print(f"DEPLOYMENT SECURITY FAIL: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())

