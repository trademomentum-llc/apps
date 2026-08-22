#!/usr/bin/env python3
"""
Fail-closed release provenance verifier.

This validates that release provenance files are not merely present, but are
cryptographically bound to a trusted release public key and internally
consistent enough to block placeholder artifacts.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any


SHA256_RE = re.compile(r"^[0-9a-fA-F]{64}$")


class ProvenanceError(Exception):
    pass


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise ProvenanceError(f"missing file: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ProvenanceError(f"invalid JSON in {path}: {exc}") from exc


def parse_manifest(path: Path) -> dict[str, str]:
    digests: dict[str, str] = {}
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError as exc:
        raise ProvenanceError(f"missing file: {path}") from exc

    for line_no, raw in enumerate(lines, start=1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) != 2:
            raise ProvenanceError(f"{path}:{line_no} must be '<sha256> <path>'")
        digest, artifact = parts
        if not SHA256_RE.fullmatch(digest):
            raise ProvenanceError(f"{path}:{line_no} has invalid sha256 digest")
        if artifact.startswith("/") or ".." in Path(artifact).parts:
            raise ProvenanceError(f"{path}:{line_no} artifact path must be relative and contained")
        digests[artifact] = digest.lower()

    if not digests:
        raise ProvenanceError(f"{path} contains no artifact digests")
    return digests


def verify_manifest_signature(manifest: Path, signature: Path, public_key: Path) -> None:
    for path in (manifest, signature, public_key):
        if not path.is_file() or path.stat().st_size == 0:
            raise ProvenanceError(f"missing or empty signature input: {path}")

    result = subprocess.run(
        [
            "openssl",
            "dgst",
            "-sha256",
            "-verify",
            str(public_key),
            "-signature",
            str(signature),
            str(manifest),
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()
        raise ProvenanceError(f"manifest signature verification failed: {detail}")


def validate_sbom(sbom_path: Path, manifest_digests: dict[str, str]) -> None:
    sbom = load_json(sbom_path)
    if not isinstance(sbom, dict):
        raise ProvenanceError("SBOM must be a JSON object")
    if "spdxVersion" not in sbom:
        raise ProvenanceError("SBOM must declare spdxVersion")

    files = sbom.get("files", [])
    if files is None:
        files = []
    if not isinstance(files, list):
        raise ProvenanceError("SBOM files must be a list when present")

    for entry in files:
        if not isinstance(entry, dict):
            raise ProvenanceError("SBOM file entries must be objects")
        name = str(entry.get("fileName") or entry.get("name") or "").lstrip("./")
        checksums = entry.get("checksums", [])
        if not name or not isinstance(checksums, list):
            continue
        sha = None
        for checksum in checksums:
            if not isinstance(checksum, dict):
                continue
            if str(checksum.get("algorithm", "")).upper() == "SHA256":
                value = str(checksum.get("checksumValue", "")).lower()
                if SHA256_RE.fullmatch(value):
                    sha = value
        if sha and name in manifest_digests and manifest_digests[name] != sha:
            raise ProvenanceError(f"SBOM digest mismatch for {name}")


def validate_attestation(attestation_path: Path, manifest_digests: dict[str, str]) -> None:
    seen_subject = False
    try:
        lines = attestation_path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError as exc:
        raise ProvenanceError(f"missing file: {attestation_path}") from exc

    for line_no, raw in enumerate(lines, start=1):
        line = raw.strip()
        if not line:
            continue
        try:
            statement = json.loads(line)
        except json.JSONDecodeError as exc:
            raise ProvenanceError(f"{attestation_path}:{line_no} invalid JSONL: {exc}") from exc
        if not isinstance(statement, dict):
            raise ProvenanceError(f"{attestation_path}:{line_no} must be a JSON object")
        if not statement.get("predicateType"):
            raise ProvenanceError(f"{attestation_path}:{line_no} missing predicateType")
        subjects = statement.get("subject")
        if not isinstance(subjects, list) or not subjects:
            raise ProvenanceError(f"{attestation_path}:{line_no} missing subject list")
        for subject in subjects:
            if not isinstance(subject, dict):
                raise ProvenanceError(f"{attestation_path}:{line_no} subject must be object")
            name = str(subject.get("name", "")).lstrip("./")
            digest = subject.get("digest")
            if not name or not isinstance(digest, dict):
                raise ProvenanceError(f"{attestation_path}:{line_no} subject missing name/digest")
            sha = str(digest.get("sha256", "")).lower()
            if not SHA256_RE.fullmatch(sha):
                raise ProvenanceError(f"{attestation_path}:{line_no} subject has invalid sha256")
            if name in manifest_digests and manifest_digests[name] != sha:
                raise ProvenanceError(f"attestation digest mismatch for {name}")
            seen_subject = True

    if not seen_subject:
        raise ProvenanceError("attestation contains no subjects")


def validate_reproducible(path: Path, manifest_digests: dict[str, str]) -> None:
    data = load_json(path)
    if not isinstance(data, dict):
        raise ProvenanceError("reproducible verification must be a JSON object")
    if data.get("verified") is not True:
        raise ProvenanceError("reproducible verification must set verified=true")

    artifacts = data.get("artifact_digests")
    if not isinstance(artifacts, dict) or not artifacts:
        raise ProvenanceError("reproducible verification must include artifact_digests")

    for name, digest in artifacts.items():
        artifact = str(name).lstrip("./")
        sha = str(digest).lower()
        if not SHA256_RE.fullmatch(sha):
            raise ProvenanceError(f"reproducible digest for {artifact} is invalid")
        if artifact not in manifest_digests:
            raise ProvenanceError(f"reproducible artifact missing from manifest: {artifact}")
        if manifest_digests[artifact] != sha:
            raise ProvenanceError(f"reproducible digest mismatch for {artifact}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--signature", required=True)
    parser.add_argument("--public-key", required=True)
    parser.add_argument("--sbom", required=True)
    parser.add_argument("--attestation", required=True)
    parser.add_argument("--reproducible", required=True)
    args = parser.parse_args()

    try:
        manifest = Path(args.manifest)
        digests = parse_manifest(manifest)
        verify_manifest_signature(manifest, Path(args.signature), Path(args.public_key))
        validate_sbom(Path(args.sbom), digests)
        validate_attestation(Path(args.attestation), digests)
        validate_reproducible(Path(args.reproducible), digests)
    except ProvenanceError as exc:
        print(f"RELEASE PROVENANCE FAIL: {exc}", file=sys.stderr)
        return 1

    print("RELEASE PROVENANCE PASS: manifest signature, SBOM, attestation, and reproducible digests verified.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
