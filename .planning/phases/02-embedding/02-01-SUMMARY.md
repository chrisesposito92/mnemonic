---
phase: 02-embedding
plan: "01"
subsystem: embedding
tags: [candle, bert, embedding, async-trait, hf-hub]
dependency_graph:
  requires: [01-foundation]
  provides: [EmbeddingEngine-trait, LocalEngine, EmbeddingError]
  affects: [src/embedding.rs, src/error.rs, src/config.rs, src/lib.rs, Cargo.toml]
tech_stack:
  added: [candle-core, candle-nn, candle-transformers, hf-hub, tokenizers, async-trait, reqwest]
  patterns: [Arc<Mutex<LocalEngineInner>> for Send+Sync model sharing, spawn_blocking for sync inference, async-trait for dyn dispatch]
key_files:
  created: [src/embedding.rs]
  modified: [Cargo.toml, src/error.rs, src/config.rs, src/lib.rs, tests/integration.rs]
decisions:
  - "Use Arc<Mutex<LocalEngineInner>> to wrap BertModel+Tokenizer for Send+Sync compatibility"
  - "Use refs/pr/21 revision for all-MiniLM-L6-v2 to guarantee model.safetensors availability"
  - "Attention-mask-weighted mean pooling (not CLS token) per official candle bert/main.rs"
metrics:
  duration: "2 min"
  completed: "2026-03-19"
  tasks_completed: 2
  files_modified: 5
  files_created: 1
---

# Phase 02 Plan 01: EmbeddingEngine Trait and LocalEngine Summary

**One-liner:** BERT embedding via candle-transformers with hf-hub model download, attention-mask mean pooling, L2 normalization, and spawn_blocking async wrapper.

## What Was Built

Added the core embedding capability to mnemonic: an `EmbeddingEngine` async trait backed by `LocalEngine`, which downloads `all-MiniLM-L6-v2` from HuggingFace Hub on first use, runs BERT inference with candle-transformers, applies attention-mask-weighted mean pooling and L2 normalization to produce 384-dim unit vectors, and wraps all sync operations in `tokio::task::spawn_blocking`.

## Tasks Completed

| Task | Name | Commit | Key Files |
|------|------|--------|-----------|
| 1 | Add dependencies, EmbeddingError, Config.openai_api_key | 313d54b | Cargo.toml, src/error.rs, src/config.rs |
| 2 | Implement EmbeddingEngine trait and LocalEngine | cdfabc4 | src/embedding.rs, src/lib.rs, tests/integration.rs |

## Verification

- `cargo test --lib config` — 5/5 passed (all original config tests still green)
- `cargo test --lib embedding` — 3/3 passed (trait object safety, Send+Sync bounds, EmptyInput)
- `cargo test` — 18/18 passed (8 lib + 5 config bin + 5 integration)
- `cargo check` — clean (no errors)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed missing openai_api_key field in integration test Config structs**
- **Found during:** Task 2 — running `cargo test` after adding openai_api_key to Config struct
- **Issue:** `tests/integration.rs` has two places that construct `Config` by struct literal; both were missing the new `openai_api_key` field, causing compile errors
- **Fix:** Added `openai_api_key: None` to both struct literal initializations in the integration test file
- **Files modified:** tests/integration.rs
- **Commit:** cdfabc4

## Decisions Made

1. **`Arc<Mutex<LocalEngineInner>>`** — both `BertModel` and `Tokenizer` are wrapped together in a single mutex so that both are `Send` (Mutex<T>: Send when T: Send), avoiding a dedicated background thread. This serializes inference but is correct for single-text-per-call Phase 2 requirements.

2. **`refs/pr/21` revision** — used for `sentence-transformers/all-MiniLM-L6-v2` to guarantee `model.safetensors` availability. The main branch may serve `pytorch_model.bin` instead; this revision is the one validated in the official candle bert example.

3. **Attention-mask-weighted mean pooling** — implemented via `broadcast_mul` + `sum` + `broadcast_div` exactly as in candle's official `bert/main.rs`. CLS token pooling was explicitly avoided — all-MiniLM-L6-v2 was fine-tuned for mean pooling and CLS pooling produces lower-quality embeddings.

## Self-Check: PASSED

- src/embedding.rs: FOUND
- src/error.rs: FOUND
- src/config.rs: FOUND
- 02-01-SUMMARY.md: FOUND
- Commit 313d54b: FOUND
- Commit cdfabc4: FOUND
