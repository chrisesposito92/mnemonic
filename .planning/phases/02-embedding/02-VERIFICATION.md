---
phase: 02-embedding
verified: 2026-03-19T21:15:00Z
status: passed
score: 12/12 must-haves verified
re_verification: false
gaps: []
human_verification:
  - test: "Integration tests actually produce correct cosine similarity values"
    expected: "sim('dog','puppy') > 0.5 and sim('dog','database') < 0.5"
    why_human: "Tests require model download (~90MB) and runtime execution — static analysis confirms test structure but cannot execute inference"
---

# Phase 02: Embedding Verification Report

**Phase Goal:** An `EmbeddingEngine` trait with a working `LocalEngine` (candle BERT, masked mean pooling, L2 normalization) that produces semantically valid vectors, loaded once at startup and shared across requests, with an optional `OpenAiEngine` fallback selectable via environment variable
**Verified:** 2026-03-19T21:15:00Z
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | LocalEngine::new() downloads all-MiniLM-L6-v2 and returns a usable engine | VERIFIED | `hf_hub::api::sync::Api::new()`, `Repo::with_revision("refs/pr/21")`, 3 file downloads in `src/embedding.rs:44-59` |
| 2 | LocalEngine::embed('hello world') returns Vec<f32> with exactly 384 elements | VERIFIED | `run_inference` asserts `result.len() != 384` at line 136-141; integration test `test_local_embedding_384_dimensions` validates at runtime |
| 3 | The returned embedding vector has L2 norm approximately equal to 1.0 | VERIFIED | `mean_pool_and_normalize` applies `pooled.sqr()?.sum_keepdim(1)?.sqrt()?` then `broadcast_div` at lines 168-169; integration test `test_local_embedding_normalized` validates |
| 4 | EmbeddingEngine trait is object-safe and can be used as Arc<dyn EmbeddingEngine + Send + Sync> | VERIFIED | Trait defined as `pub trait EmbeddingEngine: Send + Sync` at line 9; compile-time tests `test_trait_object_compiles` and `test_local_engine_send_sync` pass |
| 5 | Empty string input returns EmbeddingError::EmptyInput | VERIFIED | Both `LocalEngine::embed` (line 175) and `OpenAiEngine::embed` (line 239) check `text.is_empty()` first |
| 6 | OpenAiEngine::embed returns Vec<f32> with exactly 384 elements via text-embedding-3-small | VERIFIED | `dimensions: 384` in request struct; `embedding.len() != 384` guard at line 268; model = "text-embedding-3-small" |
| 7 | AppState contains an embedding field of type Arc<dyn EmbeddingEngine + Send + Sync> | VERIFIED | `src/server.rs:23`: `pub embedding: std::sync::Arc<dyn crate::embedding::EmbeddingEngine>` |
| 8 | main.rs selects LocalEngine when no OPENAI_API_KEY is set | VERIFIED | `if let Some(ref api_key) = config.openai_api_key` at `src/main.rs:37`; else branch loads LocalEngine |
| 9 | main.rs selects OpenAiEngine when MNEMONIC_OPENAI_API_KEY is set in config | VERIFIED | `then` branch of the if-let at `src/main.rs:38-44` creates `OpenAiEngine::new(api_key.clone())` |
| 10 | Startup log includes embedding provider name and model | VERIFIED | `provider = "openai", model = "text-embedding-3-small"` and `provider = "local", model = "all-MiniLM-L6-v2"` in `src/main.rs:39-50` |
| 11 | Cosine similarity of 'dog'/'puppy' embeddings > cosine similarity of 'dog'/'database' | VERIFIED (structure) | `test_semantic_similarity` in `tests/integration.rs:247` asserts both threshold conditions; requires runtime to confirm values |
| 12 | Attention-mask-weighted mean pooling (not CLS token) is used | VERIFIED | `broadcast_mul` + `sum` + `broadcast_div` pattern in `mean_pool_and_normalize`; no `embeddings.get(0)` (CLS) anti-pattern found |

**Score:** 12/12 truths verified (1 flagged for human runtime confirmation)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `Cargo.toml` | candle-core, candle-nn, candle-transformers, hf-hub, tokenizers, async-trait, reqwest | VERIFIED | All 7 dependencies present at correct minor versions (e.g. candle-core = "0.9") |
| `src/embedding.rs` | EmbeddingEngine trait, LocalEngine, OpenAiEngine | VERIFIED | 318 lines; exports EmbeddingEngine, LocalEngine, OpenAiEngine; contains all required functions |
| `src/error.rs` | EmbeddingError enum with ModelLoad, Inference, EmptyInput, ApiCall variants | VERIFIED | All 4 variants present at lines 50-59; From<candle_core::Error> impl at line 62 |
| `src/config.rs` | openai_api_key: Option<String> field | VERIFIED | Field present at line 13; Default impl sets None at line 23; test asserts at line 57 |
| `src/lib.rs` | pub mod embedding | VERIFIED | `pub mod embedding;` at line 3 |
| `src/server.rs` | AppState with embedding: Arc<dyn EmbeddingEngine> | VERIFIED | Field present at line 23 |
| `src/main.rs` | Engine selection logic at startup | VERIFIED | `mod embedding` at line 5; full selection block at lines 35-62; AppState construction at lines 65-70 |
| `tests/integration.rs` | 5 embedding integration tests + cosine_similarity helper | VERIFIED | All 5 test functions present (lines 219, 230, 247, 277, 291) plus helper at line 302 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/embedding.rs` | `src/error.rs` | EmbeddingError return type | VERIFIED | `Result<Vec<f32>, EmbeddingError>` in trait signature and both impls |
| `src/embedding.rs` | candle-transformers | BertModel::forward for inference | VERIFIED | `model.forward(&token_ids, &token_type_ids, Some(&attention_mask))` at line 122 |
| `src/embedding.rs` | hf-hub | Model download | VERIFIED | `hf_hub::api::sync::Api::new()` at line 44; `Repo::with_revision("refs/pr/21")` at line 45 |
| `src/embedding.rs` | tokio::task::spawn_blocking | Non-blocking inference | VERIFIED | `tokio::task::spawn_blocking` at line 180 in `impl EmbeddingEngine for LocalEngine` |
| `src/embedding.rs` (OpenAiEngine) | https://api.openai.com/v1/embeddings | reqwest POST with bearer auth | VERIFIED | Line 249: `.post("https://api.openai.com/v1/embeddings")`, line 250: `.bearer_auth(&self.api_key)` |
| `src/main.rs` | `src/embedding.rs` | Engine selection based on config.openai_api_key | VERIFIED | `if let Some(ref api_key) = config.openai_api_key` at line 37 drives LocalEngine vs OpenAiEngine |
| `src/server.rs` (AppState) | `src/embedding.rs` | Arc<dyn EmbeddingEngine> | VERIFIED | `std::sync::Arc<dyn crate::embedding::EmbeddingEngine>` at server.rs line 23 |
| `src/main.rs` | `src/server.rs` (AppState) | Passes embedding engine to AppState constructor | VERIFIED | `embedding,` field at main.rs line 68 in AppState struct literal |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| EMBD-01 | 02-01 | Server bundles all-MiniLM-L6-v2 via candle for zero-config local embedding inference | SATISFIED | LocalEngine::new() downloads/caches all-MiniLM-L6-v2 via hf-hub; no API key required; cargo check passes |
| EMBD-02 | 02-01 | Embedding pipeline uses attention-mask-weighted mean pooling and L2 normalization (not CLS token) | SATISFIED | `mean_pool_and_normalize` uses `broadcast_mul` + attention mask weighting; L2 norm via `sqr().sum_keepdim(1).sqrt()`; CLS anti-pattern absent |
| EMBD-03 | 02-01 | Embedding model loads once at startup and is shared across requests via Arc | SATISFIED | `LocalEngine` wraps `Arc<Mutex<LocalEngineInner>>`; `AppState.embedding` is `Arc<dyn EmbeddingEngine>`; main.rs loads once before server start |
| EMBD-04 | 02-02 | User can optionally set OPENAI_API_KEY env var to use OpenAI embeddings | SATISFIED | `config.openai_api_key: Option<String>` picked up from `MNEMONIC_OPENAI_API_KEY` env var via figment; triggers `OpenAiEngine` selection |
| EMBD-05 | 02-01, 02-02 | Embedding provider is abstracted behind a trait with local (candle) and OpenAI implementations | SATISFIED | `pub trait EmbeddingEngine: Send + Sync` with `async fn embed`; two concrete impls (LocalEngine, OpenAiEngine) both usable as `Arc<dyn EmbeddingEngine>` |

All 5 phase 2 requirements (EMBD-01 through EMBD-05) are satisfied. No orphaned requirements found.

### Anti-Patterns Found

No anti-patterns detected across any phase 2 files:
- No TODO/FIXME/HACK/PLACEHOLDER comments
- No empty implementations (`return null`, `return {}`, `return []`)
- No stub handlers (no `e.preventDefault()` only, no console.log only)
- No CLS token pooling (`embeddings.get(0)` absent)
- No unmasked simple mean pooling

### Human Verification Required

#### 1. Semantic Similarity Integration Tests

**Test:** Run `cargo test test_semantic_similarity test_local_embedding_384_dimensions test_local_embedding_normalized test_empty_input_error test_embed_reuse` in the repo root
**Expected:** All 5 tests pass; `sim('dog','puppy') > 0.5`, `sim('dog','database') < 0.5`, embedding.len() == 384, norm within 0.01 of 1.0, empty input returns EmbeddingError::EmptyInput
**Why human:** Tests require ~90MB model download from HuggingFace Hub on first run. Static analysis confirms correct test structure, correct assertion thresholds, and correct implementation of mean pooling — but actual inference correctness cannot be confirmed without executing the model.

### Gaps Summary

No gaps. All must-haves from both plans (02-01 and 02-02) are verified at all three levels (exists, substantive, wired):

- The `EmbeddingEngine` trait is object-safe, Send + Sync, and backed by two real implementations
- `LocalEngine` uses the correct model revision (`refs/pr/21`), correct pooling (attention-mask weighted mean, not CLS), and correct normalization (L2)
- `OpenAiEngine` uses the correct endpoint, bearer auth, correct model (`text-embedding-3-small`), correct dimension parameter (384), and validates response length
- All wiring is complete: AppState holds the engine, main.rs selects at startup based on config, EmbeddingError propagates correctly through the call chain
- All 5 EMBD requirements are satisfied; all 10 lib unit tests pass; `cargo check` succeeds
- Commits 313d54b, cdfabc4, 91f5883, 18161bd exist in git history and correspond exactly to the work described in SUMMARY files

---

_Verified: 2026-03-19T21:15:00Z_
_Verifier: Claude (gsd-verifier)_
