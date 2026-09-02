# Promptfoo red-team harness

This directory contains a repo-local Promptfoo configuration for AI/application red-team testing.

Install:

```sh
npm install
```

Run:

```sh
PROMPTFOO_TARGET_URL=http://127.0.0.1:8000/v1/chat/completions \
PROMPTFOO_TARGET_TOKEN=redacted-or-empty \
npm run redteam:generate

PROMPTFOO_TARGET_URL=http://127.0.0.1:8000/v1/chat/completions \
PROMPTFOO_TARGET_TOKEN=redacted-or-empty \
npm run redteam:run
```

The configured plugin set covers the requested Promptfoo families plus local threat vectors: RBAC, cross-session leaks, MCP exploitation, prompt extraction, RAG exfiltration/poisoning/source attribution, PII leakage, shell/SQL injection, SSRF, denial-of-service reasoning patterns, context-compliance attacks, special-token injection, tool discovery, and system-prompt override.

Generation requires a configured test-case generator. Set `OPENAI_API_KEY`, set a provider `apiKey`, or enable a self-hosted Promptfoo generator with `PROMPTFOO_REMOTE_GENERATION_URL`. The restricted visual datasets `unsafebench` and `vlguard` also require authenticated Hugging Face dataset access.

NVIDIA NeMo Guardrails is intentionally not installed here per current scope.

Do not commit generated Promptfoo result artifacts. They are ignored by `.gitignore`.
