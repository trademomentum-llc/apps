# Confidential Defensive Engineering Specification

## 1. Deployment Identity and Firewall Policy

### 1.1 Scope

This specification governs deploy-time identity, key references, rotation
metadata, and NNOS synchronization-plane firewall policy for release gating.

### 1.2 Requirements

| ID | Requirement | Status |
|---|---|---|
| DEPLOY-SEC-001 | Production releases MUST include `release/security/node-identity.manifest.json`. | Required |
| DEPLOY-SEC-002 | Production releases MUST include `release/security/nnos-sync-plane.firewall.json`. | Required |
| DEPLOY-SEC-003 | Identity manifests MUST NOT contain inline private keys, raw AEAD keys, raw HMAC keys, API keys, PEM private blocks, or environment key assignments. | Required |
| DEPLOY-SEC-004 | Key material MUST be referenced by OS-protected or externally protected storage only: macOS Keychain, Linux keyring, systemd credentials, HSM, KMS, or sealed tmpfs file. | Required |
| DEPLOY-SEC-005 | Rotation metadata MUST include active key ID, creation time, rotation deadline, and expiration time; expired keys MUST fail release validation. | Required |
| DEPLOY-SEC-006 | NINL provenance digest MUST be pinned in the manifest and match the deployed artifact provenance digest. | Required |
| DEPLOY-SEC-007 | Sync-plane firewall policy MUST deny by default, reject wildcard sources, and explicitly allow only UDP `20046` for configured peer source IPs. | Required |
| DEPLOY-SEC-008 | Host networking is prohibited for production unless the firewall manifest explicitly authorizes it with justification and concrete rules. | Required |
| DEPLOY-SEC-009 | Identity peer IDs, expected source IPs, and firewall allowed sources MUST cross-match. | Required |
| DEPLOY-SEC-010 | Release provenance gate MUST run the deployment-security validator before reporting pass. | Required |

### 1.3 Threat Model

The primary adversarial assumptions are:

- A release artifact may be stale, replaced, truncated, or generated from a
  different tree.
- A node may start with a valid binary but broad network exposure.
- A leaked environment file may expose raw symmetric key material.
- A peer may spoof source, replay old packets, or induce fail-open startup.

### 1.4 Enforcement Boundary

The validator proves manifest shape, key-reference discipline, rotation
metadata, firewall specificity, and cross-manifest consistency. It does not
install secrets into Keychain/HSM/KMS and does not apply firewall rules to the
host. Those operations remain explicit deployment actions.

