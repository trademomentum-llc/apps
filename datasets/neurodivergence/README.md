# Neurodivergence LLM Training Dataset

**Compiled using Morphlex deterministic pipeline + Jasterish tooling on Mac build.**

This dataset is designed to train specialized MorphlexLLM (and future NeuroDiOS Neural Engine components) on topics supporting neurodivergent individuals, practitioners, and optimization of human performance.

## Categories (expanding)
- `behavioral_health_patterns`: Executive dysfunction, sensory overload, hyperfocus, emotional dysregulation, rejection sensitive dysphoria, etc.
- `healthy_lifestyle_habits`: Routines, sleep hygiene adapted for ADHD/autism, nutrition for focus, movement for regulation, environmental design.
- `optimizing_human_capabilities`: Strengths-based approaches, flow states, compensatory strategies, leveraging special interests, performance under neurodivergent conditions.
- `performance_output`: Task initiation, time blindness compensation, sustainable productivity, burnout prevention, energy accounting.
- `pattern_recognition`: Spotting masking, identifying triggers, meta-cognition of own neurotype, environmental pattern mapping.
- `health_practitioner_responses`: Affirming language, avoiding pathologizing, collaborative goal setting, trauma-informed for neurodivergent clients, differential (not mis-)diagnosis patterns.
- `synthetic_augmentation`: Generated variations for balance, edge cases, multi-turn dialogues.

## Formats
- `raw/`: Source texts (MD, TXT). Curated + synthetic.
- `compiled/`: 
  - `category_*.db` : Morphlex vector databases (12-byte TokenVector + lemma tables) produced by running sources through the full lexer→morphology→AST→semantics→vectorizer pipeline.
  - `neuro_manifest.json` : Index of shards, sample counts, category tags, morphlex version.
- `instructions/`: Instruction-tuning JSONL (Alpaca/ShareGPT style) for SFT on practitioner responses, self-coaching, pattern explanations. Each example's "text" fields can be further vectorized.
- `manifest/`: Metadata, licenses (public domain / CC for generated; attribute curated sources), ethics notes (supportive, non-diagnostic, strengths-focused, never medical advice).

## Compilation Pipeline (Mac build)
The dataset is "compiled" (not scraped blindly) using the project's own deterministic tools:

1. Raw knowledge (project docs, KDB deltas on Neural Engine/Jasterish, public patterns) + synthetic templates.
2. `morphlex compile` / `DataLoader::from_text_file` (or direct `compile()`) → TokenVector sequences with explicit POS/role/morph.
3. Multi-task labels auto-derived (next-token, lemma, POS, semantic role) for the hybrid MorphlexLLM objective.
4. Optional: JStar-side processing in kernel or compiler for higher-order "neuro pattern" recognition once kernel expands.

See `cargo run -- help` for `llm` subcommands (Train uses these .db or text directly via the same pipeline). New `compile-neuro` (to be added) automates category-aware compilation + manifest.

## Usage for Training (on Mac)
```bash
# Compile a focused shard
cargo run -- llm compile-neuro --sources datasets/neurodivergence/raw/behavioral --category behavioral_health --output datasets/neurodivergence/compiled/

# Train (example)
cargo run -- llm train --data datasets/neurodivergence/compiled/behavioral.db --size medium --epochs 5 --output llm_checkpoints/neuro

# Export for deployment / NeuroDiOS
cargo run -- llm export --model llm_checkpoints/neuro/... --output models/neurodivergence.gguf --quantize
```

## Ethics & Scope
- Supportive tools for self-understanding, habit design, communication with practitioners.
- Explicitly **not** a diagnostic system, medical device, or replacement for professional care.
- Data emphasizes lived experience patterns, strengths, accommodations, and evidence-informed (where possible) strategies.
- All synthetic data reviewed for non-harmful, destigmatizing language.
- Mac build isolation: this work lives primarily in the `macos` branch / worktree; shared compiler/kernel expansions merged to master only when portable.

## Next (expand compiler + kernel)
- New JStar instructions / kernel syscalls for "load_neuro_pattern", vector similarity on-device (using morphlex id== for fast lookup).
- Kernel VFS / process support for mapping compiled .db into user-space Neural Engine.
- Extend morph flags for neuro-specific (e.g., EXEC_DYSFUNCTION, SENSORY_SEEK, etc.).
- Self-host dataset compiler in JStar once bootstrap is further along.
- Integrate with rr/ swarm agents for "Neuro Support Swarm" orchestration.
- EEG / NINL (see docs/) data fusion once hardware interface matures.

**Status:** Active. Foundation ML datasets (Transformers, PEFT, Alpaca-style instruction tuning, etc.) have been explicitly translated via morphlex:

- Run `cargo run -- llm compile-neuro --sources datasets/neurodivergence/raw/foundation --category foundation` to produce `shards/neuro_foundation.db` (pure translated foundation vectors).
- When running with `--category all` (or default neuro compile), the foundation texts under `raw/foundation/` are auto-processed with `morphlex::compile` (full pipeline: lexer, morphology, AST, semantics, vectorizer) and mixed into the main `neuro_all.db` as `foundation_*` categories.
- This translates the natural language "base knowledge" (model APIs, LoRA configs, instruction formats, fine-tuning recipes) into the project's 12-byte integer TokenVectors (with lemma_id, pos, role, morph) for direct use in MorphlexLLM multi-task training.

Current compiled shards include vectors from these foundation sources alongside neurodivergence-specific content.

## Primitive Discovery via CLI
A "poor primitive" subcommand now automates the manual pull process:

```bash
cargo run -- primitives pull --db datasets/neurodivergence/compiled/shards/neuro_all.db --sources datasets/neurodivergence/raw --output primitives_report.md --apply
```

It loads vectors+lemmas (via read_database or on-the-fly morphlex::compile), filters for clean Verb+Action, applies the 8 Architecture criteria (deterministic, verb syntax, Action role, no conflicts with known_operation_verbs + keyword table, pure/syscall, extendable, neuro+ML utility, synergy), reports freq+reasons in markdown, and with --apply writes proposal patches for review.

Latest run (post 9+3) surfaced "train", "mask", "optimize" (after lemma normalization); 3 promoted. The CLI is the mechanism for disciplined continued expansion of the primitives that govern the JStar/NeuroDiOS Architecture. See src/primitives.rs and the generated report.

See `cargo run -- llm compile-neuro --help` and the source in src/main.rs for the compile logic (it calls morphlex compile on lines from the txt/md files).

Use for training:
```bash
# Train on the integrated dataset (foundation base + neuro specialization)
cargo run -- llm train --data datasets/neurodivergence/compiled/shards/neuro_all.db --size small --epochs 3 --output llm_checkpoints/neuro_foundation

# Fine-tune (new CLI action) + full W&B tracking (weights, biases, metrics, artifacts for train/eval .db + checkpoints)
# (script handles logging to wandb.com if installed+logged-in, else local JSON + artifacts)
python scripts/llm_wandb_trainer.py --data datasets/neurodivergence/compiled/shards/neuro_all.db --size small --epochs 5 --finetune --output llm_checkpoints/neuro_finetune_wandb
# Or direct (after parallel agents complete enrichment):
cargo run -- llm finetune --data ... --lr 0.00005 ...

# Or specifically on the translated foundation for base knowledge
cargo run -- llm train --data datasets/neurodivergence/compiled/shards/neuro_foundation.db ...
```

## Foundation Datasets Translated
- transformers/core_concepts.txt → vectors for HF Transformers usage, Trainer, quantization, etc.
- peft/lora_adapters.txt → vectors for PEFT/LoRA patterns, efficient fine-tuning.
- general/alpaca_instruction_tuning.txt → vectors for instruction tuning formats and best practices.
- wandb/wandb_training_best_practices.txt → vectors for W&B (Weights & Biases) best practices: experiment tracking, config + metric logging (incl. loss/lm_loss/ppl), run.watch for weights/grads, artifacts for dataset/model/checkpoint versioning + lineage, sweeps for HPO, reproducibility (git/seeds/artifacts). Enriches foundation before neuro specialization. (970 vectors from the txt)

These provide tried-and-tested ML engineering base knowledge, translated deterministically with morphlex for integration into the custom LLM (no floats, integer vectors, explicit morphology/semantics).

To add more foundation datasets (e.g. Dolly, FLAN, other HF libs), drop .txt files with key excerpts into `raw/foundation/<category>/` and re-run the compile command. The morphlex pipeline will translate them automatically.

**wandb foundation layer added (2026-06 Mac worktree):** `raw/foundation/wandb/wandb_training_best_practices.txt` ingested via `cargo run -- llm compile-neuro ... --category foundation_wandb` (and `all` to mix). Now present in `neuro_manifest.json` categories as "foundation_wandb" and in neuro_all.db (12587 total vectors post-recompile). Provides "tried and tested" MLOps patterns for the sovereign LLM training/eval pipeline.

## Training Run + Eval on the Combined Set
A training run + per-epoch evaluation was executed on the combined set (neuro_all.db = foundation base knowledge translated via morphlex + neurodivergence patterns):

Command (Mac release build):
```
target/release/morphlex llm train \
  --data datasets/neurodivergence/compiled/shards/neuro_all.db \
  --size small --epochs 2 --batch-size 4 --lr 0.0001 \
  --output llm_checkpoints/neuro_combined
```

**Run output summary (latest run with log-softmax CE fix):**
- Data: combined (9 samples from foundation + neuro, 3 batches/epoch)
- Model: small (512d, 6 layers, ~95M params)
- Epoch 1: train_loss=41.4235, lm_loss=10.8197, ppl=49996.34
  Eval after epoch 1: loss=43.1683, lm=10.8197, ppl=49995.62
- Epoch 2: train_loss=43.1683, lm_loss=10.8197, ppl=49995.62
  Eval after epoch 2: loss=43.1683, lm=10.8197, ppl=49995.62
- Best: 41.4235 | Total steps: 6 | Tokens: 8730
- Checkpoints: llm_checkpoints/neuro_combined/ (model_step_*.json + checkpoint json; ~1.6G each due to JSON weight serialization)

Note: Per-epoch eval (multi-task loss on the combined set) is logged thanks to Trainer::evaluate. The absolute loss numbers are consistent with random initialization for the large vocab (lm ~10.8 nats). The "training" loop runs but param updates are limited in the current demo optimizer (no full backprop yet). This successfully exercises training + eval end-to-end on the morphlex-translated combined foundation+neuro set on Mac.

Checkpoints can be inspected/exported with the llm subcommands.

See also the trainer code for the eval addition.

## Bigger Training + Fine-Tune + Weights & Biases (WandB) Tracking

The training pipeline now supports "bigger" runs (release build, 5-10 epochs, batch=4, sample_seq_len=32 for ~300+ samples from enriched 12k+ vec neuro_all.db which includes foundation_wandb from parallel agent) + dedicated fine-tune action (lower LR default 5e-5, separate output dir, resume-from-best support).

All runs emit `wandb_metrics.jsonl` (structured per-epoch train/eval + multi-task losses, ppl, steps, tokens) + `best_checkpoint.json`/`best_model.json` for MLOps.

### Direct CLI (no wandb)
```bash
# Bigger base training on enriched data (foundation + neuro + wandb practices)
cargo run --release -- llm train \
  --data datasets/neurodivergence/compiled/shards/neuro_all.db \
  --size small --epochs 5 --batch-size 4 --lr 0.0001 \
  --output llm_checkpoints/neuro_wandb_enriched

# Fine-tune (continued / adaptation on same enriched set, lower lr, separate dir)
cargo run --release -- llm fine-tune \
  --data datasets/neurodivergence/compiled/shards/neuro_all.db \
  --size small --epochs 3 --batch-size 4 --lr 0.00005 \
  --output llm_checkpoints/neuro_finetune_wandb_enriched
```
(The fine-tune subcommand is an alias for train but with fine-tune profile + resume logic + naming for experiment tracking.)

### WandB Script (recommended for full tracking)
See `scripts/llm_wandb_trainer.py` (created for this task). It:
- (pip installs wandb --user if missing on mac)
- Inits wandb run with hyperparams, data manifest info, tags=["morphlex-neuro-finetune", "primitives-enriched", "integer-vectors-no-floats"]
- Runs the cargo --release llm (train or --finetune) subprocess
- Parses stdout + consumes the wandb_metrics.jsonl for reliable logging of lm_loss, total, ppl, eval_*, tokens etc.
- Logs system info, artifacts (the .db + final checkpoint dir)
- Falls back to local wandb_style logs + summary json if no wandb.

Example:
```bash
python scripts/llm_wandb_trainer.py \
  --data datasets/neurodivergence/compiled/shards/neuro_all.db \
  --size small --epochs 5 --batch-size 4 --lr 0.0001 \
  --output llm_checkpoints/neuro_wandb_enriched

# for fine-tune stage (uses llm fine-tune subcmd)
python scripts/llm_wandb_trainer.py \
  --data datasets/neurodivergence/compiled/shards/neuro_all.db \
  --finetune --epochs 3 --lr 0.00005 \
  --output llm_checkpoints/neuro_finetune_wandb_enriched
```
Future runs: `python scripts/llm_wandb_trainer.py --help`

Manifest now notes the wandb practices category + eval tracking notes.

See also updated training code (Trainer::train emits jsonl, DataLoader proper db parse + configurable seq, save_best, eval freq).

See also: docs/*NeuroDiOS*, CUSTOM_LLM_PLAN.md, context/kimi_execution/deltas/ for the sovereign Neural Engine vision this supports.
