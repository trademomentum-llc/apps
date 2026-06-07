#!/usr/bin/env python3
"""llm_wandb_trainer.py

Wrapper for bigger MorphlexLLM training + fine-tune runs with optional W&B (weights & biases) tracking.
Part of "Bigger training + fine-tune + wandb" task for MorphlexLLM (integer vector, multi-task, sovereign).

Usage (from repo root, mac worktree):
  python scripts/llm_wandb_trainer.py \
    --data datasets/neurodivergence/compiled/shards/neuro_all.db \
    --size small --epochs 5 --batch-size 4 --lr 0.0001 \
    --output llm_checkpoints/neuro_wandb_enriched

  # Fine-tune stage (uses cargo ... llm fine-tune, lower lr profile, resume support)
  python scripts/llm_wandb_trainer.py \
    --data datasets/neurodivergence/compiled/shards/neuro_all.db \
    --finetune --epochs 3 --lr 0.00005 \
    --output llm_checkpoints/neuro_finetune_wandb_enriched

Falls back gracefully to local "wandb style" logs (wandb_metrics.jsonl already emitted by trainer + this summary json)
if wandb package missing or --no-wandb. Will attempt 'python -m pip install --user wandb' on mac.

Emits to W&B (if used):
- config (size, lr, epochs, data, manifest info, sample_seq_len=32)
- metrics per epoch/step (lm_loss, train_total, ppl, eval_*, tokens, best)
- system/platform info
- artifacts: the .db used + entire output checkpoint dir (with best_* + jsonl)
- tags: morphlex-neuro-finetune, primitives-enriched, integer-vectors, no-core-floats, mac-worktree
- notes about deterministic pipeline stub opt for tracking demo

See datasets/neurodivergence/README.md for full usage + updated training section.
See src/llm/training.rs (wandb_metrics.jsonl emit, save_best, from_database proper parse + seq) and src/main.rs (LlmAction FineTune + dispatch).

Straight ASCII only; no && chains in any shell examples.
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

def main():
    parser = argparse.ArgumentParser(
        description="MorphlexLLM bigger training + fine-tune + optional wandb MLOps wrapper (mac)"
    )
    parser.add_argument("--data", required=True, type=Path, help="Path to morphlex .db shard e.g. neuro_all.db (enriched)")
    parser.add_argument("--size", default="small", choices=["small", "medium", "large"], help="Model size")
    parser.add_argument("--epochs", type=int, default=5, help="Num epochs (bigger: 5-10)")
    parser.add_argument("--batch-size", type=int, default=4, dest="batch_size", help="Batch size (4 for mac demo)")
    parser.add_argument("--lr", type=float, default=0.0001, help="LR (use 5e-5 for finetune)")
    parser.add_argument("--output", default="llm_checkpoints/neuro_wandb_enriched", help="Checkpoint output dir")
    parser.add_argument("--finetune", action="store_true", help="Run as fine-tune (llm fine-tune subcmd, separate dir, lower lr intent)")
    parser.add_argument("--no-wandb", action="store_true", dest="no_wandb", help="Skip wandb even if installed; pure local logs")
    parser.add_argument("--project", default="morphlex-llm", help="W&B project name")
    parser.add_argument("--run-name", default=None, help="Optional explicit W&B run name")
    args = parser.parse_args()

    data = args.data.resolve()
    if not data.exists():
        print(f"ERROR: data not found: {data}")
        sys.exit(1)

    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)

    subcmd = "fine-tune" if args.finetune else "train"
    action_tag = "finetune" if args.finetune else "train"

    print("=== MorphlexLLM WandB Trainer Wrapper ===")
    print(f"Data: {data}")
    print(f"Action: llm {subcmd} (release build)")
    print(f"Config: size={args.size} epochs={args.epochs} batch_size={args.batch_size} lr={args.lr}")
    print(f"Output: {output}")
    print("Tags will include: morphlex-neuro-finetune, primitives-enriched, integer-vectors, no-core-floats")
    print()

    # WandB import + optional install (mac --user)
    use_wandb = not args.no_wandb
    wandb = None
    if use_wandb:
        try:
            import wandb as _wandb  # type: ignore
            wandb = _wandb
        except ImportError:
            print("wandb not found in env; trying pip install --user wandb (mac compatible)...")
            pip_cmd = [sys.executable, "-m", "pip", "install", "--user", "wandb"]
            try:
                res = subprocess.run(pip_cmd, capture_output=True, text=True, timeout=180)
                if res.returncode == 0:
                    print("pip install --user wandb succeeded.")
                    import wandb as _wandb  # type: ignore
                    wandb = _wandb
                else:
                    print(f"pip failed: {res.stderr[:300]}")
            except Exception as e:
                print(f"Install attempt error (no net? perms?): {e}")
                wandb = None

    # Enrich config from manifest (parallel data agent added foundation_wandb etc)
    manifest_info = {}
    manifest_candidates = [
        data.parent.parent / "manifest" / "neuro_manifest.json",
        Path("datasets/neurodivergence/compiled/manifest/neuro_manifest.json"),
    ]
    for mp in manifest_candidates:
        if mp.exists():
            try:
                manifest_info = json.loads(mp.read_text())
                print(f"Loaded manifest: total_vectors={manifest_info.get('total_vectors')}, categories={len(manifest_info.get('categories', []))}")
                break
            except Exception:
                pass

    config = {
        "model_size": args.size,
        "epochs": args.epochs,
        "batch_size": args.batch_size,
        "lr": args.lr,
        "data": str(data),
        "output": str(output),
        "action": action_tag,
        "sample_seq_len": 32,
        "data_total_vectors": manifest_info.get("total_vectors"),
        "categories": manifest_info.get("categories", []),
        "platform": sys.platform,
        "python": sys.version.split()[0],
        "timestamp_start": datetime.now(timezone.utc).isoformat(),
    }

    run = None
    if wandb is not None:
        run_name = args.run_name or f"morphlex-{action_tag}-{args.size}-ep{args.epochs}-{int(time.time())}"
        run = wandb.init(
            project=args.project,
            name=run_name,
            config=config,
            tags=[
                "morphlex-neuro-finetune",
                "primitives-enriched",
                "integer-vectors",
                "no-core-floats",
                "mac-worktree",
                "sovereign-llm",
            ],
            notes=(
                "Deterministic natural language tokenizer/vector compiler based MorphlexLLM. "
                "Multi-task (LM + lemma + POS + role) on 12-byte integer TokenVectors (no core floats in vecs). "
                "Bigger run + fine-tune on enriched neuro_all.db (foundation + neuro + wandb practices). "
                "Stub AdamW (no real backprop) for full pipeline + eval + MLOps tracking demo. "
                "Wandb metrics from stdout + trainer-emitted wandb_metrics.jsonl ."
            ),
            job_type=action_tag,
        )
        print("W&B run started.")

    # Cargo command: use --release (per build guidance in CLAUDE.md and history)
    # NEVER chain with && ; this is single cmd list.
    cargo_cmd = [
        "cargo", "run", "--release", "--",
        "llm", subcmd,
        "--data", str(data),
        "--size", args.size,
        "--epochs", str(args.epochs),
        "--batch-size", str(args.batch_size),
        "--lr", str(args.lr),
        "--output", str(output),
    ]
    print(f"Spawning: {' '.join(cargo_cmd)}")
    print("Streaming output (parsed for metrics in real time + post from jsonl)...")

    start_ts = time.time()
    proc = subprocess.Popen(
        cargo_cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
        cwd=Path.cwd(),
    )

    stdout_lines = []
    step_re = re.compile(r"Step\s+(\d+):\s+loss=([\d.]+),\s+lm_loss=([\d.]+),\s+ppl=([\d.]+)")
    epoch_re = re.compile(r"Epoch\s+(\d+)/(\d+):\s+train_loss=([\d.]+),\s+lm_loss=([\d.]+),\s+ppl=([\d.]+)")
    eval_re = re.compile(r"Eval after epoch\s+(\d+):\s+loss=([\d.]+),\s+lm=([\d.]+),\s+ppl=([\d.]+)")
    loaded_re = re.compile(r"Loaded\s+(\d+)\s+training samples")
    tokens_re = re.compile(r"Tokens processed:\s+(\d+)")
    best_re = re.compile(r"Best loss:\s+([\d.]+)")

    for line in proc.stdout:
        # Stream live (user sees cargo progress)
        sys.stdout.write(line)
        sys.stdout.flush()
        stdout_lines.append(line)

        if wandb is not None:
            m = step_re.search(line)
            if m:
                wandb.log({
                    "step": int(m.group(1)),
                    "loss": float(m.group(2)),
                    "lm_loss": float(m.group(3)),
                    "ppl": float(m.group(4)),
                })
            m = epoch_re.search(line)
            if m:
                wandb.log({
                    "epoch": int(m.group(1)),
                    "train_total": float(m.group(3)),
                    "lm_loss": float(m.group(4)),
                    "ppl": float(m.group(5)),
                })
            m = eval_re.search(line)
            if m:
                wandb.log({
                    "eval_epoch": int(m.group(1)),
                    "eval_total": float(m.group(2)),
                    "eval_lm": float(m.group(3)),
                    "eval_ppl": float(m.group(4)),
                })
            m = loaded_re.search(line)
            if m:
                wandb.config.update({"loaded_samples": int(m.group(1))}, allow_val_change=True)

    proc.wait()
    duration = time.time() - start_ts
    print(f"\n=== Cargo subprocess finished (code={proc.returncode}) in {duration:.1f}s ===")

    full_out = "".join(stdout_lines)

    # Post-run: parse final stats
    final = {"exit_code": proc.returncode, "duration_sec": round(duration, 1)}
    m = tokens_re.search(full_out)
    if m:
        final["tokens_processed"] = int(m.group(1))
    m = best_re.search(full_out)
    if m:
        final["best_loss"] = float(m.group(1))

    # Prefer the reliable trainer-emitted jsonl over fragile stdout regex
    metrics_jsonl = output / "wandb_metrics.jsonl"
    parsed_epochs = []
    if metrics_jsonl.exists():
        for ln in metrics_jsonl.read_text().strip().split("\n"):
            ln = ln.strip()
            if ln:
                try:
                    parsed_epochs.append(json.loads(ln))
                except Exception:
                    pass
        print(f"Parsed {len(parsed_epochs)} structured epoch records from {metrics_jsonl} (preferred source)")

    # W&B final logs + artifacts
    if wandb is not None and run is not None:
        for m in parsed_epochs:
            # log only numeric
            loggable = {k: v for k, v in m.items() if isinstance(v, (int, float))}
            if loggable:
                wandb.log(loggable)
        wandb.log({"final": final})

        # System info (light)
        try:
            import platform as _plat
            sysinfo = {
                "python": sys.version.split()[0],
                "platform": _plat.platform(),
                "machine": _plat.machine(),
            }
            wandb.config.update({"system": sysinfo}, allow_val_change=True)
        except Exception:
            pass

        # Artifacts: the db + checkpoints dir
        try:
            art_name = f"morphlex-{action_tag}-{args.size}-{datetime.now(timezone.utc).strftime('%Y%m%d%H%M')}"
            artifact = wandb.Artifact(
                name=art_name,
                type="model-checkpoint",
                metadata={
                    "best_loss": final.get("best_loss"),
                    "data_path": str(data),
                    "total_vectors": manifest_info.get("total_vectors"),
                    "epochs": args.epochs,
                },
            )
            if data.exists():
                artifact.add_reference(f"file://{data}", name="neuro_all_enriched.db")
            if output.exists():
                artifact.add_dir(str(output), name="checkpoints")
            wandb.log_artifact(artifact)
            print(f"Logged W&B artifact: {art_name} (db ref + checkpoints dir)")
        except Exception as e:
            print(f"Artifact logging skipped: {e}")

        wandb.finish()
        print("W&B run finished.")

    # ALWAYS write local wandb-style summary (even on fallback)
    summary = {
        "config": config,
        "duration_sec": round(duration, 1),
        "exit_code": proc.returncode,
        "parsed_epochs": parsed_epochs,
        "final": final,
        "stdout_tail": full_out[-3000:],
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "wandb_used": wandb is not None,
        "data_manifest_note": manifest_info.get("wandb_note", ""),
    }
    summary_path = output / "wandb_style_summary.json"
    summary_path.write_text(json.dumps(summary, indent=2))
    print(f"Wrote local wandb-style summary + metrics: {summary_path}")

    # Also copy metrics jsonl to top level if wanted, but already in output

    if wandb is None:
        print("W&B not active (not installed or --no-wandb). Local logs are complete equivalent for this run.")
        print("To enable cloud: pip install --user wandb && wandb login ; re-run script.")

    print("=== Wrapper complete. See README updates and checkpoints for results. ===")
    if proc.returncode != 0:
        sys.exit(proc.returncode)

if __name__ == "__main__":
    main()
