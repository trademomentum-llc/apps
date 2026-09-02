# Confidential — NNOS Release Tree Review

Date: 2026-08-22
Scope: `/Users/nnos/Projects/Sovereign/System/apps`, with cross-checks against
`../engine` and `../NeuroDiOS`.

## 1. Commit

The following changes are release-relevant and should be committed rather than
discarded:

- fail-closed multicast sync, NINL authenticity/replay/rate controls, and RR/JSH
  safety gates;
- JStar canonical self-host baseline, bootstrap checks, and forensics;
- Promptfoo red-team configuration and package lock;
- release provenance gate, deployment manifest validators, and provenance
  generator/verifier scripts;
- pinned Docker build inputs and generated release metadata;
- removal of placeholder CircleCI workflow and stale example keys;
- `.dockerignore` and `.gitignore` updates required to keep secrets out while
  allowing public release provenance files in.

## 2. Quarantine

The following material was moved out of production paths and retained with hash
evidence under `/private/tmp/nnos-quarantine-2026-08-22`:

- noncanonical JStar C helpers and Mac-only stale binaries;
- `debug_logs/` and `jstar_bootstrap_out/` generated artifacts;
- generated Python caches;
- the pre-initialization `NeuroDiOS/external/engine` directory that occupied a
  submodule path before the pinned submodule was initialized.

## 3. Restore / preserve

Deleted engine recipe sources were not treated as disposable cleanup. They are
project work product and were restored. The VectorDB provenance recipe was then
hardened to remove shell-form `curl` execution.

## 4. Discard from release tree

The release tree should not contain:

- inline private keys or raw sync/NINL secrets;
- generated crash logs or bootstrap output directories;
- zero digest image placeholders;
- generic pod publication of UDP `20046`;
- unpinned `:latest` daemon images;
- noncanonical JStar bootstrap helper binaries/sources.

## 5. Agent/session validation

Validation used:

- current Git state and file hashes;
- the three red-team subagent reports from this Codex session;
- Kimi session index/history for apps, engine/nnos, NeuroDiOS, and Sovereign
  work directories, which showed prior JStar, microkernel, AArch64, QEMU, and
  self-host/compiler activity;
- Grok CLI discovery. Grok session listing was unavailable because its auth token
  is expired, so only local session files were inspected read-only.

Agent transcripts were treated as untrusted evidence. They were used only to
identify historical claims to verify against code, Git history, and generated
hashes.
