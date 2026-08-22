# Sovereign/Morphlex red-team threat matrix

## Scope

This matrix covers the current `apps`, `engine`, `NeuroDiOS`, and `tables` scan context. It separates implemented gates from remaining test coverage.

## Immediate system threat vectors

| Vector | Current surface | Current control | Required next control |
|---|---|---|---|
| Auth-level vulnerability | NINL UDP dispatch, multicast sync, RR mission roles | NINL authenticated envelope added in NeuroDiOS reference C++; multicast key/interface/firewall/node-ID/peer/source/replay/rate checks added in engine dirty tree; release identity manifest validator added | Apply OS/HSM/KMS secret provisioning and rotation in deployment |
| Cross-session leak | Agent/chat/RAG patterns, Promptfoo `cross-session-leak` | Promptfoo test coverage configured | Add state-isolation tests for any persistent agent memory/session store |
| Context compliance attack | Prompt-injected history or retrieved content | Promptfoo `cca`, `indirect-prompt-injection`, `special-token-injection` configured | Enforce instruction hierarchy in runtime; mark retrieved/user content untrusted |
| Divergent repetition | Model extraction through repetition | Promptfoo `divergent-repetition` configured | Rate/length caps and refusal policy in model gateway |
| Hijacking capabilities | Tool/agent objective takeover | Promptfoo `hijacking`, `excessive-agency`, `goal-misalignment` configured | Per-action authorization policy and tool allowlists |
| Indirect prompt injection | RAG/web/tool content | Promptfoo `indirect-prompt-injection`, `data-exfil`, `rag-*` configured | Content provenance labels and tool-output quarantine |
| MCP exploitation | Model Context Protocol tool exposure | Promptfoo `mcp` configured | MCP allowlist, scoped tokens, audit log, deny unknown tool schemas |
| DPI / deep packet inspection bypass | Encoded or obfuscated payloads | Promptfoo `ascii-smuggling`, `special-token-injection`, `wordplay` configured | Canonicalization before moderation and security policy checks |
| PII leak | JSH env dump, logs, RAG/session data | JSH `env` disabled by default; Promptfoo `pii:*` configured | Secret/PII detector in logs, prompts, and retrieved chunks |
| Prompt extraction | System/developer prompt disclosure | Promptfoo `prompt-extraction`, `system-prompt-override` configured | Gateway-level system prompt redaction and refusal checks |
| RAG exfiltration | Retrieved docs or source attribution | Promptfoo `rag-document-exfiltration`, `rag-poisoning`, `rag-source-attribution` configured | Source ACLs, chunk-level provenance, retrieval-time RBAC |
| RBAC bypass | RR roles/capabilities, NINL issuers, multicast issuers | NINL issuer allowlist and multicast issuer allowlist added; Promptfoo `rbac`, `bfla`, `bola` configured | Central policy engine for RR missions, JSH, MCP, and registry actions |
| Denial of service | Reasoning loops, packet floods, large malformed DBs | DB entry/lemma/vector bounds added; multicast packet-rate detection added; Promptfoo `reasoning-dos` configured | Per-peer NINL packet-rate ceilings and model token/time budgets |
| Shell injection | JSH/native execution and tool surfaces | JSH native execution and file IO disabled by default; Promptfoo `shell-injection` configured | OS sandbox/jail for explicit unsafe execution |
| SSRF | Agent/resource fetch tools | Promptfoo `ssrf` configured | URL allowlists, private-network denylist, DNS rebinding checks |
| System prompt override/hijacking | Prompt/RAG/tool context | Promptfoo `system-prompt-override`, `prompt-extraction`, `tool-discovery` configured | Runtime instruction hierarchy enforcement and telemetry |
| Host-network exposure | Quadlets using host network for sync plane | Release firewall manifest validator requires explicit UDP/20046 source allowlist and deny-by-default policy | Apply and verify platform firewall rules on each node |

## Promptfoo plugin coverage configured

The repo-local Promptfoo config in `security/promptfoo/promptfooconfig.yaml` includes:

- Bias: `bias:age`, `bias:disability`, `bias:gender`, `bias:race`
- Dataset/trust: `aegis`, `beavertails`, `cyberseceval`, `donotanswer`, `harmbench`, `pliny`, `toxic-chat`, `unsafebench`, `vlguard`, `xstest`
- Compliance/legal: `contracts`, `coppa`, `ferpa`
- Harmful-content families: `harmful:*`, including malicious code via `harmful:cybercrime:malicious-code`
- Brand/alignment: `competitors`, `excessive-agency`, `goal-misalignment`, `hallucination`, `imitation`, `off-topic`, `overreliance`, `unverifiable-claims`
- Security/access control: `mcp`, `rbac`, `bfla`, `bola`, `ssrf`, `shell-injection`, `sql-injection`, `prompt-extraction`, `system-prompt-override`, `tool-discovery`, `cross-session-leak`, `data-exfil`, `debug-access`, `indirect-prompt-injection`, `rag-*`, `reasoning-dos`

## Tamper-looking artifacts

| Artifact | Evidence | Status | Remediation |
|---|---|---|---|
| `jstar_bootstrap_out/jstar3` | zero-byte artifact with empty-file SHA-256 in `sha256.txt` | Quarantined under `jstar_bootstrap_out/quarantine/2026-08-22-invalid-bootstrap/` | Regenerate from clean tree, compare deterministic hashes, sign manifest, produce SBOM and build attestation |
| `debug_logs/` | tracked crash/debug logs and ELF artifacts | Release gate now rejects release trees containing `debug_logs` | Move logs to ephemeral CI artifact storage and scrub paths/config before sharing |
| Promptfoo dev dependency tree | `npm audit` reports 5 high-severity dev findings through transitive Promptfoo deps | Accepted only as dev-tool risk; no forced downgrade applied | Track upstream Promptfoo dependency update or isolate Promptfoo in CI/container |
