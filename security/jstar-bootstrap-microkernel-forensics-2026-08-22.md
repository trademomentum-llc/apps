# Confidential Forensics Report: JStar Bootstrap and Jasterish Microkernel

Date: 2026-08-22  
Scope: `/Users/nnos/Projects/Sovereign/System/apps`, `/Users/nnos/Projects/Sovereign/System/engine`, `/Users/nnos/Projects/Sovereign/System/NeuroDiOS`, `/Users/nnos/Projects/Sovereign/System/tables`  
Mode: read-only evidence collection plus report creation  
Classification: internal security and provenance review

## 1. Executive finding

The empty `jstar3` finding is real, but it applies to the stale `jstar_bootstrap_out` lineage, not to the later top-level JStar self-host lineage.

The current evidence supports this split:

| Lineage | Path | Status | Security meaning |
|---|---|---|---|
| Candidate self-host baseline | `/Users/nnos/Projects/Sovereign/System/apps/jstar/` | `jstar4` and `jstar5` are byte-identical | Supports the claim that a self-host/fixpoint state existed |
| Invalid bootstrap-output lineage | `/Users/nnos/Projects/Sovereign/System/apps/jstar_bootstrap_out/quarantine/2026-08-22-invalid-bootstrap/` | `jstar3` is zero bytes | Must not be release input |
| Expanded JMK implementation | `/Users/nnos/Projects/Sovereign/System/engine/nnos/neurodios/jasterish-microkernel/` | Contains arch split, AArch64 work, JMK binaries, and boot-to-shell logs | Explains continued microkernel progress |
| Kimi corroboration | `/Users/nnos/Projects/Sovereign/System/NeuroDiOS/external/engine/nnos/lsa/synthesized/kimi_execution/` | Contains KDB stating `JStar 4 == JStar 5` | Corroborating evidence, but not canonical release provenance by itself |

The microkernel did not have to progress from the broken `jstar_bootstrap_out/jstar3`. It plausibly progressed from the later `apps/jstar` fixpoint chain plus engine-side JMK expansion work.

## 2. Reconstructed timeline

| Date | Repo/root | Commit or artifact | Event | Finding |
|---|---|---|---|---|
| 2026-03-09 | `apps` | `6b79e6c` | Initial morphlex/JStar compiler and shell | Start of Rust-side compiler lineage |
| 2026-03-23 | `apps` | `12d00e9` | `jstar: restore self-host bootstrap fixpoint` | Adds `jstar/compiler.jstr` and bootstrap work |
| 2026-03-25 | `apps` | `78f28f4` | Intent-to-execution smoke ladder added | Establishes bootstrap trace path |
| 2026-04-01 | `apps` | debug logs | T-diagram crash data | Records `jstar2` SIGSEGV and zero-byte `jstar3` in the older path |
| 2026-04-02 | `apps` | `91765d5`, `7984c0e` | `jstar_bootstrap_out` updates | `jstar3` remains empty in this lineage |
| 2026-05-11 | `apps` | `4659add` | Fix JStar self-hosting | Commit message claims `JStar 4 == JStar 5`; source fixes tokenizer, BSS, and if/else codegen |
| 2026-05-11 | `apps` | `e383623` | Adds `jstar2`, `jstar3`, `jstar4`, `jstar5` | Current top-level binaries are present and tracked |
| 2026-05-30 | Kimi/engine | KDB `025`, `026` | JMK Makefile wrapper fix and Lenox compiler tracking | Kimi records build-chain blockers and NUC-side compiler fixes |
| 2026-05-31 | Kimi/NeuroDiOS external | `2026-05-31-06-JStar4-TDiagram-Fixpoint-Byte-Identical.kdb` | Kimi records fixpoint | Explicitly states `jstar4 == jstar5`, SHA `d510be40...`, size `70,925` |
| 2026-06-07 | `apps` branch `macos` | `ad15e00`, `516fa98` | Expanded JMK branch | Adds larger microkernel with bus, disk, ELF, IDT, VFS, and expansion plan |
| 2026-06-14 | `apps` current lineage | `2c69bcc` | Adds Jasterish Microkernel | Current `apps` mirror is smaller than the earlier macOS-expanded branch |
| 2026-07-03 | cleanup lineage | `27f412d` | Removes binary artifacts | Explains why some external-app snapshots no longer contain `jstar2..5` |
| 2026-07-15 | `engine` | `a2e275b` and follow-on commits | JMK arch restructure | Splits source into `arch/x86_64`, `arch/aarch64`, and `common` |
| 2026-07-16 | `engine` | `1f0b339` | Adds prebuilt JMK binaries | Adds `jmk.elf` and `jmk.bin`; local `file` identifies `jmk.elf` as AArch64 |
| 2026-07-29 | `apps` and `engine` | regression commits | Adds compiler/JMK regression harness | x86_64 kernel case skipped on macOS; AArch64 boot log exists |

## 3. Artifact evidence

### 3.1 Candidate JStar self-host baseline

Path: `/Users/nnos/Projects/Sovereign/System/apps/jstar/`

| File | Size | SHA-256 | Finding |
|---|---:|---|---|
| `compiler.jstr` | 212,771 | `a43fb131f86b57a2c126112b371692a407171beea6d6dbd2c3eff5e77e91a32a` | Current compiler source |
| `jstar2` | 123,259 | `d4619f2370bd5dc0ab554caba99ba2f6c05de9e6b4f5f077806c9675b9a5a0f4` | ELF x86-64 static |
| `jstar3` | 70,925 | `76dabe3d96b8375dd78c5a7d64f51b12a89bb0cbdbbebc009b8cc724f66b568e` | Non-empty; differs from `jstar4/5` |
| `jstar4` | 70,925 | `d510be40bea44ece8442e66289e39a4f5a89822307316ed80ca84ad969187dc1` | Fixpoint candidate |
| `jstar5` | 70,925 | `d510be40bea44ece8442e66289e39a4f5a89822307316ed80ca84ad969187dc1` | Byte-identical to `jstar4` |

Assessment: This is the strongest local evidence that the self-hosted compiler completed a fixpoint. It is not yet release-clean because there is no signed manifest, SBOM, build attestation, or reproducible rebuild record attached to the artifact set.

### 3.2 Invalid/quarantined bootstrap-output lineage

Path: `/Users/nnos/Projects/Sovereign/System/apps/jstar_bootstrap_out/quarantine/2026-08-22-invalid-bootstrap/`

| File | Size | SHA-256 | Finding |
|---|---:|---|---|
| `jstar1` | 3,931,061 | `f15a5940f53b565ae6317e0e7873c292d6d17f13f0bc2dbf4c191cb7d2c693a6` | ELF x86-64 static |
| `jstar2` | 68,707 | `ff9caa6413a4b737c670199675a0bc462d2a34edba9e2fe009995632de95fd68` | ELF x86-64 static |
| `jstar3` | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Empty file |
| `intent_stage2.elf` | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | Empty file |
| `sha256.txt` | 219 | `59698c4111a7543a3ce77fbcae35ee3b7420e64b36ee1d515eed3cf709bf7d42` | Local manifest content differs from HEAD LFS pointer hash |

Assessment: This lineage is invalid release provenance. It should remain quarantined until regenerated from a clean tree with signed provenance.

### 3.3 Jasterish Microkernel evidence

Current richer implementation path: `/Users/nnos/Projects/Sovereign/System/engine/nnos/neurodios/jasterish-microkernel/`

| File | Size | SHA-256 | Finding |
|---|---:|---|---|
| `jmk.elf` | 107,432 | `abde0c85a821487ece69af956080a3ec1c912a694c150776513ba496cd8d1c69` | ELF 64-bit AArch64, statically linked, no section header |
| `jmk.bin` | 107,312 | `d5aa7ac2d6bb12ad541de62e6ef1b1a0dc198794b00b25bc1cc06ffc2f552b32` | Raw binary |
| `common/process.jstr` | 101,881 | `a661b57b8808cc1d3ad23fa6f9c9dfd0188498ecd76913c904c4e8f58a8a4908` | Largest common subsystem |
| `common/kernel.jstr` | 31,031 | `63fca7a6ff8257e3c35f0ca06b66bb08659f71048be9a862494f860a1f061815` | Common kernel entry logic |
| `common/syscall.jstr` | 28,870 | `688481ee51f5ba0f8716ba3ffb2eebac403bdbbbaee6eab5e918aafabb33d2c0` | Syscall path |

Boot evidence:

- `/Users/nnos/Projects/Sovereign/System/engine/nnos/neurodios/jasterish-microkernel/tests/regression/boot-to-shell/actual.aarch64.log` contains boot output through `JMK>`.
- The log is ignored/untracked and should be treated as operational evidence only, not release provenance.
- The x86_64 actual log is empty and the x86_64 kernel case is skipped on macOS.

Assessment: The engine root has credible JMK progression evidence after the compiler fixpoint. Release provenance is still incomplete because there is no signed build manifest tying `jmk.elf` and `jmk.bin` to a compiler-stage hash and exact toolchain.

## 4. Kimi/session data incorporated

Kimi evidence was found primarily in:

- `/Users/nnos/Projects/Sovereign/System/engine/nnos/lsa/synthesized/kimi_execution/`
- `/Users/nnos/Projects/Sovereign/System/NeuroDiOS/external/engine/nnos/lsa/synthesized/kimi_execution/`

Key files:

| File | Evidence |
|---|---|
| `deltas/2026-05-31-06-JStar4-TDiagram-Fixpoint-Byte-Identical.kdb` | States the self-hosting T-diagram fixpoint was achieved; records `jstar4 == jstar5`, SHA `d510be40...`, size `70,925`, and commit `4659add` |
| `deltas/025_2026-05-30_jmk_makefile_fix.md` | Records JMK Makefile compiler-variable fix and `jstar1` wrapper; notes QEMU-user-mode `jstar1` was too slow for practical builds |
| `deltas/026_2026-05-30_lenox_compiler_evolution_tracking.md` | Records 43 NUC-side uncommitted compiler changes and warns that removing the divergence panic permits progress but requires external validation |
| `deltas/2026-05-31-05-Course-Correction-Step2-Must-Be-Jasterish.kdb` | Records correction that the sovereign foundation and event bus should be Jasterish-native |
| `deltas/2026-05-31-07-Architectural-Scope-Clarification-Jasterish-Foundation.md` | Clarifies Jasterish is the sovereign foundation language, not necessarily the entire OS language |
| `Kimi_Execution_Log.md` | The richer NeuroDiOS external log records the fixpoint delta and later Jasterish/JMK entries |

Integrity notes:

- The NeuroDiOS external formatted checksum manifest `Kimi_Execution_Log.sha256.txt` verifies cleanly with `shasum -a 256 -c`.
- The raw `.sha256` files are raw digest files, not `shasum -c` compatible manifests.
- The current engine Kimi master log is older/fragmented and reports parse failures for KDB entries because the logger expects stricter JSON than the KDB dialect uses.
- Some Kimi paths refer to legacy roots such as `/Users/nnos/Projects/engine/...` and `/Users/nnos/Projects/apps/...`; current roots are under `/Users/nnos/Projects/Sovereign/System/...`.

Assessment: Kimi logs support the self-host/fixpoint claim and explain how JMK work proceeded. They should be treated as corroborating reconstruction evidence until the logger/parser/root issues are fixed and the log is regenerated into a signed manifest.

## 5. Expected divergence versus tamper-looking indicators

### 5.1 Expected divergence

The microkernel started as a Linux/x86_64-oriented architecture and then diverged into macOS and AArch64 work. That branch divergence is expected and should not be classified as tampering by itself.

Known legitimate divergence:

- `apps` branch `macos` contains an earlier larger JMK source set with `bus.jstr`, `disk.jstr`, `elf.jstr`, `idt.jstr`, `vfs.jstr`, and `EXPANSION_PLAN.md`.
- Current `apps/docs/jasterish-updates` has a smaller JMK mirror from `2c69bcc`.
- `engine` contains the richer restructured JMK with `arch/x86_64`, `arch/aarch64`, and `common`.

### 5.2 Tamper-looking or integrity-impacting indicators

| Severity | Indicator | Evidence | Interpretation | Required remediation |
|---|---|---|---|---|
| High | Invalid bootstrap-output chain | `jstar_bootstrap_out/.../jstar3` and `intent_stage2.elf` are zero bytes | Broken or stale lineage; not proof of adversary by itself | Keep quarantined; regenerate from clean tree; never release from this path |
| High | Missing signed provenance | No SBOM/signature/attestation files found beside JStar/JMK artifacts | Chain is reconstructable but not release-trustworthy | Generate signed manifest, SBOM, build attestation, and reproducible rebuild record |
| High | Git ref contamination | `.git/refs/.DS_Store` exists in `apps` and `NeuroDiOS`; `git fsck` fails with `badRefName` | Likely macOS Finder metadata contamination, but it breaks integrity checks | Remove explicit files after approval; prevent Finder metadata in repo internals |
| High | Unmanaged gitlink content | Richer Kimi evidence under `NeuroDiOS/external/engine` is in a gitlink path but not a normal initialized nested Git checkout | Corroborating evidence only; not tamper-resistant provenance | Initialize/pin submodule or move evidence into a tracked provenance root |
| Medium | Kimi log parser drift | Engine log reports KDB parse errors; logger has legacy hardcoded roots | Master log is stale/fragmented | Fix root discovery and parser, then regenerate and sign |
| Medium | Fixpoint test drift | `src/jstar/mod.rs` still has ignored T-diagram test text around `jstar2 == jstar3` while accepted evidence is `jstar4 == jstar5` | Test/spec drift weakens automated verification | Replace with active test asserting the accepted invariant |
| Medium | Build label ambiguity | Commit subject says `feat(jmk/linux)` but local `jmk.elf` is AArch64 | Could mean Linux host, not Linux target; ambiguous release label | Manifest must record host OS, target arch, compiler hash, and command |
| Medium | Ignored runtime logs | AArch64 boot-to-shell log exists but is ignored/untracked | Useful diagnostic evidence, not provenance | Capture runtime evidence as signed CI artifact |

## 6. Current security posture conclusion

The correct state is:

1. A real JStar self-host/fixpoint candidate exists in `/Users/nnos/Projects/Sovereign/System/apps/jstar/`.
2. The zero-byte `jstar3` problem is confined to the stale `jstar_bootstrap_out` path and should not be used to characterize the later JStar lineage.
3. The Jasterish Microkernel progression is supported by engine-side git history, AArch64 source expansion, prebuilt JMK artifacts, and boot-to-shell logs.
4. The system is not yet release-trustworthy because provenance is fragmented across roots, Kimi logs are not fully regenerated/attested, and Git integrity checks fail due to `.DS_Store` files under `.git/refs`.

## 7. Required remediation sequence

### 7.1 Immediate cleanup requiring explicit approval

Do not proceed to release or provenance signing until these are addressed:

1. Remove these exact files from Git ref storage:
   - `/Users/nnos/Projects/Sovereign/System/apps/.git/refs/.DS_Store`
   - `/Users/nnos/Projects/Sovereign/System/NeuroDiOS/.git/refs/.DS_Store`
2. Re-run:
   - `git fsck --no-reflogs --connectivity-only` in `apps`
   - `git fsck --no-reflogs --connectivity-only` in `NeuroDiOS`
3. Keep the existing `jstar_bootstrap_out` quarantine out of release artifacts.

### 7.2 Establish a clean JStar baseline

Use `/Users/nnos/Projects/Sovereign/System/apps/jstar/` as the candidate baseline, but promote it only after:

1. Re-run the full T-diagram on Linux or a pinned x86_64 container.
2. Emit fresh `jstar2`, `jstar3`, `jstar4`, `jstar5`.
3. Require `jstar4 == jstar5`.
4. Record whether `jstar3` differs and why that is acceptable.
5. Produce a signed manifest covering:
   - Git commit and branch.
   - Source hash for `compiler.jstr`.
   - SHA-256 and BLAKE3 for every JStar stage.
   - Compiler stage used for each output.
   - Host OS, target arch, container digest/toolchain digest.
   - Build command.
   - Clean-tree state.

### 7.3 Tie JMK to the compiler baseline

For `/Users/nnos/Projects/Sovereign/System/engine/nnos/neurodios/jasterish-microkernel/`:

1. Rebuild `jmk.elf` and `jmk.bin` using the signed JStar baseline.
2. Capture target architecture explicitly: `x86_64` or `aarch64`.
3. Capture runtime proof as signed CI artifacts, not ignored local logs.
4. Make x86_64 skip behavior explicit: macOS skip is acceptable, release skip is not.

### 7.4 Repair Kimi provenance

1. Fix `kimi_execution_logger.py` root discovery so it uses the current Sovereign paths.
2. Either normalize KDB files to strict JSON fields or teach the parser the current KDB dialect.
3. Regenerate the master log from all KDBs.
4. Emit both:
   - raw digest file for quick comparison, and
   - formatted `.sha256.txt` for `shasum -c`.
5. Sign the regenerated log and include it in the release provenance bundle.

### 7.5 Align roots

1. Decide which root is canonical for the JStar compiler baseline.
2. Pin `apps`, `NeuroDiOS/external/apps`, and `engine` to that same compiler lineage.
3. Do not leave unmanaged files inside gitlink/submodule paths.
4. Make branch divergence explicit:
   - Linux/x86_64 original path.
   - macOS support branch.
   - AArch64/JMK engine branch.

## 8. Offer for implementation

Recommended next implementation pass:

1. Remove the two `.git/refs/.DS_Store` files after explicit approval.
2. Patch the Kimi logger root discovery/parser.
3. Add a provenance generator for JStar/JMK artifacts.
4. Replace the ignored stale T-diagram test with an active `jstar4 == jstar5` gate.
5. Add a release gate that rejects any artifact bundle missing signed provenance, SBOM, build attestation, clean-tree proof, and submodule pin proof.

## Appendix A. Late security-surface subagent findings

A parallel read-only security-surface scan returned after the main timeline was drafted. Its findings do not change the bootstrap conclusion, but they affect the next security layer.

| Severity | Finding | Required response |
|---|---|---|
| Critical | A stale external copy of `lsa_ethernet_sync` under `NeuroDiOS/external/engine` still appears to contain the old multicast group, permissive bind/default-key/plaintext-style behaviors, and remote merge behavior that conflict with the hardened canonical engine copy. | Quarantine or replace the external copy; add CI drift gates blocking `239.73.78.69`, default/zero keys, plaintext fallback, and `INADDR_ANY` multicast bind. |
| High | External quadlet copy still permits host-network sync and carries example key material comments. | Remove example material and require explicit firewall/VLAN manifest plus host-network approval variable before any deployment. |
| High | Shell execution surfaces remain in C utilities through `system()`/`popen()` patterns. | Replace with `execve`/`posix_spawn` argv arrays or native APIs; avoid shell-form command construction. |
| High | RBAC/capability enforcement is still partly declarative in lower layers. | Centralize signed capability tokens, field-level authorization, mission/order signatures, monotonic replay windows, and fail-closed policy. |
| High | NINL authenticity currently relies on shared HMAC material plus issuer IDs rather than true per-node asymmetric identity. | Move to per-node keypairs or at minimum per-peer HMAC keys bound to issuer identity. |
| Medium | Kimi/KDB ingestion is a context-compliance and indirect-prompt-injection surface if unsigned `.kdb` or `.md` files become future agent context. | Require signed KDBs, schema validation, 0700 writable directories, no `.md` auto-ingest, and explicit untrusted-data labels in agent context. |
| Medium | JSH native execution/file I/O remain env-gated but not OS-confined when enabled. | Keep disabled in production; add Seatbelt/seccomp/container jail, path allowlist, limits, and secret stripping before enabling. |
| Medium | C bootstrap compiler artifacts have memory-safety risks such as unchecked token/code-buffer/variable counts. | Add bounds checks, fail closed on overflow, fuzz tokenizer/codegen/linker, and keep C bootstrap artifacts out of the trusted chain unless independently verified. |

This addendum reinforces the same release rule: the authoritative release chain must use the hardened canonical copies only, and duplicated external snapshots must either be pinned as evidence or quarantined as stale drift.
