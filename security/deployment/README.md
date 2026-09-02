# NNOS deployment security manifests

Production releases must provide two non-secret manifests:

- `release/security/node-identity.manifest.json`
- `release/security/nnos-sync-plane.firewall.json`

The manifests contain identity, peer, key-reference, rotation, provenance, and
firewall policy metadata. They must not contain raw private keys, raw symmetric
keys, API keys, environment assignments, or PEM private key blocks.

Validate locally:

```sh
python3 scripts/verify_deployment_security.py \
  --identity release/security/node-identity.manifest.json \
  --firewall release/security/nnos-sync-plane.firewall.json
```

Run the built-in validator regression checks:

```sh
python3 scripts/verify_deployment_security.py --self-test
```

Render non-secret runtime variables:

```sh
python3 scripts/render_nnos_runtime_env.py \
  --identity release/security/node-identity.manifest.json
```

The render command intentionally prints only non-secret variables and key
reference comments. It does not print `NNOS_SYNC_KEY` or `NNOS_NINL_AUTH_KEY`.

## Key references

Allowed `provider` values are:

- `macos-keychain`
- `linux-keyring`
- `systemd-credential`
- `hsm`
- `kms`
- `tmpfs-sealed-file`

The validator intentionally checks references only. It does not create, store,
or print secrets.

## Operational environment mapping

The validator enforces the same non-secret values required by the daemons:

| Manifest field | Runtime variable |
|---|---|
| `local_node.node_id` | `NNOS_SYNC_NODE_ID`, `NNOS_NINL_ISSUER_ID` |
| `sync.bind_address` | `NNOS_SYNC_IFADDR`, `NNOS_NINL_BIND_IP` |
| `sync.allowed_peer_node_ids` | `NNOS_SYNC_ALLOWED_PEERS` |
| `sync.expected_source_ips` | `NNOS_SYNC_ALLOWED_SOURCES` |
| `ninl.allowed_peer_issuer_ids` | `NNOS_NINL_ALLOWED_PEERS` |
| `ninl.provenance_digest` | `NNOS_NINL_PROVENANCE_DIGEST` |

Secret runtime variables such as `NNOS_SYNC_KEY` and `NNOS_NINL_AUTH_KEY` must
be injected from the referenced protected storage at deployment time.
