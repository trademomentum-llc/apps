# NNOS Quarantine Manifest — 2026-08-22

Confidential / dual-use security record.

## Scope

This manifest records non-production or non-canonical artifacts removed from
active release/build paths during the 2026-08-22 hardening pass.

## Quarantine locations

| Location | Contents | Reason |
|---|---|---|
| `/private/tmp/nnos-quarantine-2026-08-22/jstar-noncanonical-c/` | `jstar_c.c`, `jstar_canonical.c`, and `jstar_macos` copies from `apps`, `NeuroDiOS/compiler`, `NeuroDiOS/jstar`, `NeuroDiOS/external/apps`, and `NeuroDiOS/external/engine/nnos` | Native/C and Mac helper bootstrap artifacts are not the canonical fully self-hosted baseline unless separately attested. |
| `/private/tmp/nnos-quarantine-2026-08-22/apps-nonproduction-artifacts/` | `debug_logs/` and invalid `jstar_bootstrap_out/` artifacts | Non-production diagnostics and stale invalid bootstrap outputs must not ship. |
| `/private/tmp/nnos-quarantine-2026-08-22/apps-generated-cache/` | `scripts/__pycache__/`, `scripts__pycache__-postverify/`, and `tests__pycache__/` | Generated cache artifacts; no release provenance value. |
| `/private/tmp/nnos-quarantine-2026-08-22/submodule-anomalies/external-engine-local-preinit/` | 4.8 GB pre-existing local `NeuroDiOS/external/engine` directory | The superproject recorded `external/engine` as an uninitialized submodule while a non-submodule local directory occupied that path. |

## Hash records

- `/private/tmp/nnos-quarantine-2026-08-22/jstar-noncanonical-c/sha256.txt`
- `/private/tmp/nnos-quarantine-2026-08-22/apps-nonproduction-artifacts.sha256`
- `/private/tmp/nnos-quarantine-2026-08-22/submodule-anomalies/external-engine-local-preinit.sha256`

## Active canonical baseline

The active JStar baseline remains:

- `jstar/compiler.jstr`
- `jstar/jstar2`
- `jstar/jstar3`
- `jstar/jstar4`
- `jstar/jstar5`

The release invariant is `jstar/jstar4 == jstar/jstar5` with SHA-256:

```text
d510be40bea44ece8442e66289e39a4f5a89822307316ed80ca84ad969187dc1
```

Production release remains blocked until the tree is clean and the required
signed manifest, SBOM, build attestation, reproducible verification, node
identity manifest, and firewall manifest are present.
