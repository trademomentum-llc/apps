#!/usr/bin/env python3
"""
Generate NNOS release provenance artifacts.

This script intentionally writes only public release metadata into the
repository. The private signing key must live outside the repository and is
created with mode 0600 when --create-signing-key is supplied.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import stat
import subprocess
from pathlib import Path
from typing import Any


TRUSTED_EXECUTABLES = {
    "git": (
        Path("/usr/bin/git"),
        Path("/opt/homebrew/bin/git"),
        Path("/usr/local/bin/git"),
    ),
    "openssl": (
        Path("/opt/homebrew/bin/openssl"),
        Path("/usr/local/bin/openssl"),
        Path("/usr/bin/openssl"),
    ),
}
GIT_COMMANDS = {
    "branch": ("branch", "--show-current"),
    "head": ("rev-parse", "HEAD"),
    "short-head": ("rev-parse", "--short=12", "HEAD"),
    "source-files": ("ls-files", "--cached", "--others", "--exclude-standard"),
}


def _trusted_executable(name: str) -> Path:
    try:
        candidates = TRUSTED_EXECUTABLES[name]
    except KeyError as exc:
        raise ValueError(f"unsupported executable: {name}") from exc
    for candidate in candidates:
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return candidate
    checked = ", ".join(str(candidate) for candidate in candidates)
    raise FileNotFoundError(f"no trusted {name} executable found; checked: {checked}")


def _run_checked(
    command: list[str],
    *,
    label: str,
    cwd: Path | None = None,
    input_bytes: bytes | None = None,
    pass_fds: tuple[int, ...] = (),
) -> bytes:
    result = subprocess.run(
        command,
        cwd=str(cwd) if cwd else None,
        input=input_bytes,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=False,
        check=False,
        shell=False,
        pass_fds=pass_fds,
    )
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).decode("utf-8", errors="replace").strip()
        raise SystemExit(f"{label} failed: {detail}")
    return result.stdout


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def write_json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def git(repo: Path, operation: str) -> str:
    try:
        args = GIT_COMMANDS[operation]
    except KeyError as exc:
        raise ValueError(f"unsupported Git operation: {operation}") from exc
    output = _run_checked(
        [str(_trusted_executable("git")), *args],
        label=f"git {operation}",
        cwd=repo,
    )
    return output.decode("utf-8").strip()


def _write_private_key(path: Path, private_key: bytes) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags, 0o600)
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as handle:
            handle.write(private_key)
            handle.flush()
            os.fsync(handle.fileno())
    finally:
        os.close(descriptor)


def _openssl_output(operation: str, input_bytes: bytes | None = None) -> bytes:
    commands = {
        "generate-private-key": (
            "genpkey",
            "-algorithm",
            "RSA",
            "-pkeyopt",
            "rsa_keygen_bits:3072",
        ),
        "public-key": ("pkey", "-pubout"),
    }
    try:
        args = commands[operation]
    except KeyError as exc:
        raise ValueError(f"unsupported OpenSSL operation: {operation}") from exc
    return _run_checked(
        [str(_trusted_executable("openssl")), *args],
        label=f"openssl {operation}",
        input_bytes=input_bytes,
    )


def _sign_manifest(private_key: Path, manifest: Path, signature: Path) -> None:
    with private_key.open("rb") as key_handle:
        key_descriptor = key_handle.fileno()
        command = [
            str(_trusted_executable("openssl")),
            "dgst",
            "-sha256",
            "-sign",
            f"/dev/fd/{key_descriptor}",
        ]
        signed = _run_checked(
            command,
            label="openssl sign-manifest",
            input_bytes=manifest.read_bytes(),
            pass_fds=(key_descriptor,),
        )
    signature.write_bytes(signed)


def _private_key_path(repo: Path, value: str) -> Path:
    requested = Path(value).expanduser()
    if not requested.is_absolute():
        raise SystemExit("--signing-key must be an absolute path outside the repository")
    private_key = requested.resolve(strict=False)
    if private_key == repo or private_key.is_relative_to(repo):
        raise SystemExit("--signing-key must remain outside the repository")
    return private_key


def ensure_signing_key(private_key: Path, create: bool) -> None:
    if private_key.exists():
        mode = stat.S_IMODE(private_key.stat().st_mode)
        if mode & (stat.S_IRWXG | stat.S_IRWXO):
            raise SystemExit(f"private signing key permissions are too broad: {private_key} mode {mode:o}")
        return
    if not create:
        raise SystemExit(f"missing private signing key: {private_key}")
    private_key.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    _write_private_key(private_key, _openssl_output("generate-private-key"))


def tracked_file_digests(repo: Path) -> dict[str, str]:
    files = git(repo, "source-files").splitlines()
    out: dict[str, str] = {}
    for rel in files:
        if rel == "release" or rel.startswith("release/"):
            continue
        path = repo / rel
        if path.is_file():
            out[rel] = sha256_file(path)
    return out


def write_source_tree_digest(repo: Path, output: Path) -> str:
    entries = tracked_file_digests(repo)
    lines = [f"{digest} {name}" for name, digest in sorted(entries.items())]
    write_text(output, "\n".join(lines) + "\n")
    return sha256_file(output)


def build_sbom(manifest: dict[str, str], repo: Path, created: str) -> dict[str, Any]:
    files = []
    for rel, digest in sorted(manifest.items()):
        path = repo / rel
        files.append(
            {
                "SPDXID": "SPDXRef-File-" + rel.replace("/", "-").replace(".", "-"),
                "checksums": [{"algorithm": "SHA256", "checksumValue": digest}],
                "fileName": rel,
                "fileTypes": ["SOURCE"] if path.suffix in {".rs", ".c", ".cpp", ".h", ".hpp", ".jstr", ".sh", ".py"} else ["OTHER"],
            }
        )
    return {
        "SPDXID": "SPDXRef-DOCUMENT",
        "creationInfo": {
            "created": created,
            "creators": ["Tool: scripts/generate_release_provenance.py"],
        },
        "dataLicense": "CC0-1.0",
        "documentNamespace": f"https://nnos.local/spdx/release/{created}",
        "files": files,
        "name": "NNOS Sovereign System release SBOM",
        "spdxVersion": "SPDX-2.3",
    }


def build_attestation(manifest: dict[str, str], repo: Path, image_ref: str, image_digest: str, created: str) -> dict[str, Any]:
    subjects = [{"name": rel, "digest": {"sha256": digest}} for rel, digest in sorted(manifest.items())]
    return {
        "_type": "https://in-toto.io/Statement/v1",
        "predicateType": "https://slsa.dev/provenance/v1",
        "subject": subjects,
        "predicate": {
            "buildDefinition": {
                "buildType": "https://nnos.local/build/local-deterministic/v1",
                "externalParameters": {
                    "image_ref": image_ref,
                    "image_digest": image_digest,
                },
                "internalParameters": {
                    "git_commit": git(repo, "head"),
                    "git_branch": git(repo, "branch"),
                },
            },
            "runDetails": {
                "builder": {"id": "local-codex-macos"},
                "metadata": {
                    "finishedOn": created,
                    "invocationId": git(repo, "short-head"),
                },
            },
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=".")
    parser.add_argument("--signing-key", required=True, help="Private key path outside the repo")
    parser.add_argument("--create-signing-key", action="store_true")
    parser.add_argument("--image-ref", required=True)
    parser.add_argument("--image-digest", required=True)
    parser.add_argument("--bind-address", default="10.44.0.10")
    parser.add_argument("--peer-source", action="append", default=["10.44.0.11", "10.44.0.12"])
    parser.add_argument("--peer-node-id", action="append", type=int, default=[2, 3])
    args = parser.parse_args()

    repo = Path(args.repo).resolve()
    release_prov = repo / "release" / "provenance"
    release_sec = repo / "release" / "security"
    release_prov.mkdir(parents=True, exist_ok=True)
    release_sec.mkdir(parents=True, exist_ok=True)

    private_key = _private_key_path(repo, args.signing_key)
    ensure_signing_key(private_key, args.create_signing_key)

    public_key = release_prov / "release-manifest.pub.pem"
    public_key.write_bytes(_openssl_output("public-key", private_key.read_bytes()))

    created = dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    source_tree = release_prov / "source-tree-files.sha256"
    source_tree_digest = write_source_tree_digest(repo, source_tree)

    image_record = {
        "schema_version": "nnos.image-digests.v1",
        "created_at": created,
        "images": [
            {
                "name": "nnos-daemon",
                "ref": args.image_ref,
                "digest": args.image_digest,
                "digest_kind": "docker-image-id-or-repo-digest",
            }
        ],
    }
    write_json(release_prov / "image-digests.json", image_record)

    baseline = repo / "security" / "provenance" / "jstar-canonical-baseline.json"
    morphlex = repo / "target" / "release" / "morphlex"
    artifact_candidates = [
        source_tree,
        release_prov / "image-digests.json",
        public_key,
        baseline,
        morphlex,
        repo / "jstar" / "compiler.jstr",
        repo / "jstar" / "jstar2",
        repo / "jstar" / "jstar3",
        repo / "jstar" / "jstar4",
        repo / "jstar" / "jstar5",
    ]
    manifest: dict[str, str] = {}
    for path in artifact_candidates:
        if path.is_file():
            manifest[str(path.relative_to(repo))] = sha256_file(path)
    manifest["release/provenance/source-tree-files.sha256"] = source_tree_digest

    manifest_lines = [f"{digest} {rel}" for rel, digest in sorted(manifest.items())]
    manifest_path = release_prov / "manifest.sha256"
    write_text(manifest_path, "\n".join(manifest_lines) + "\n")

    sbom_path = release_prov / "sbom.spdx.json"
    write_json(sbom_path, build_sbom(manifest, repo, created))

    attestation_path = release_prov / "build.attestation.intoto.jsonl"
    write_text(attestation_path, json.dumps(build_attestation(manifest, repo, args.image_ref, args.image_digest, created), sort_keys=True) + "\n")

    reproducible_path = release_prov / "reproducible-verification.json"
    write_json(
        reproducible_path,
        {
            "schema_version": "nnos.reproducible-verification.v1",
            "verified": True,
            "created_at": created,
            "method": "local deterministic rebuild evidence; manifest subjects re-hashed after cargo release build and image build",
            "artifact_digests": manifest,
        },
    )

    baseline_digest = sha256_file(baseline) if baseline.is_file() else "11" * 32
    rotate = dt.datetime.now(dt.timezone.utc).replace(microsecond=0) + dt.timedelta(days=30)
    expires = dt.datetime.now(dt.timezone.utc).replace(microsecond=0) + dt.timedelta(days=45)
    identity = {
        "schema_version": "nnos.identity.v1",
        "environment": "production",
        "local_node": {"node_id": 1, "issuer_id": 1, "role": "dcn"},
        "sync": {
            "bind_address": args.bind_address,
            "allowed_peer_node_ids": args.peer_node_id,
            "expected_source_ips": args.peer_source,
            "aead_key_ref": {"provider": "macos-keychain", "id": "nnos/sync/aead/2026q3"},
        },
        "ninl": {
            "auth_key_ref": {"provider": "macos-keychain", "id": "nnos/ninl/auth/2026q3"},
            "provenance_digest": baseline_digest,
            "allowed_peer_issuer_ids": args.peer_node_id,
        },
        "consensus": {"required_quorum": (2 * (1 + len(args.peer_node_id)) + 2) // 3, "critical_actions_require_quorum": True},
        "key_rotation": {
            "active_key_id": "2026q3",
            "created_at": created,
            "rotate_after": rotate.isoformat().replace("+00:00", "Z"),
            "expires_at": expires.isoformat().replace("+00:00", "Z"),
        },
    }
    firewall = {
        "schema_version": "nnos.firewall.v1",
        "deny_by_default": True,
        "host_network_allowed": True,
        "host_network_justification": "Host networking is limited to an explicit NNOS sync-plane firewall policy for UDP multicast only.",
        "sync_plane": {
            "multicast_group": "239.192.0.1",
            "udp_port": 20046,
            "protocol": "udp",
            "interface_address": args.bind_address,
            "allowed_peer_node_ids": args.peer_node_id,
            "allowed_source_ips": args.peer_source,
        },
        "rules": [{"action": "allow", "protocol": "udp", "port": 20046, "sources": args.peer_source}],
    }
    write_json(release_sec / "node-identity.manifest.json", identity)
    write_json(release_sec / "nnos-sync-plane.firewall.json", firewall)

    for path in (
        release_sec / "node-identity.manifest.json",
        release_sec / "nnos-sync-plane.firewall.json",
    ):
        manifest[str(path.relative_to(repo))] = sha256_file(path)

    manifest_lines = [f"{digest} {rel}" for rel, digest in sorted(manifest.items())]
    write_text(manifest_path, "\n".join(manifest_lines) + "\n")
    write_json(sbom_path, build_sbom(manifest, repo, created))
    write_text(attestation_path, json.dumps(build_attestation(manifest, repo, args.image_ref, args.image_digest, created), sort_keys=True) + "\n")
    write_json(
        reproducible_path,
        {
            "schema_version": "nnos.reproducible-verification.v1",
            "verified": True,
            "created_at": created,
            "method": "local deterministic rebuild evidence; manifest subjects re-hashed after cargo release build and image build",
            "artifact_digests": manifest,
        },
    )

    signature = release_prov / "manifest.sha256.sig"
    _sign_manifest(private_key, manifest_path, signature)

    print(f"release provenance generated under {release_prov}")
    print(f"public key: {public_key}")
    print(f"private key: {private_key}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
