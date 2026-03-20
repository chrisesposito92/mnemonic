---
phase: 02-embedding
plan: "02"
subsystem: embedding
tags: [openai, reqwest, bert, candle, async-trait, integration-testing, semantic-similarity]

# Dependency graph
requires:
  - phase: 02-embedding-01
    provides: "EmbeddingEngine trait, LocalEngine, EmbeddingError variants, Config.openai_api_key"
provides:
  - "OpenAiEngine struct implementing EmbeddingEngine via reqwest POST to text-embedding-3-small"
  - "AppState.embedding field: Arc<dyn EmbeddingEngine> shared across all request handlers"
  - "main.rs engine selection: LocalEngine (default) vs OpenAiEngine (when MNEMONIC_OPENAI_API_KEY set)"
  - "Integration tests: 384-dimension validation, L2 normalization, semantic similarity, empty input error, engine reuse"
affects: [03-storage, phase-3-memory-api, request-handlers]

# Tech tracking
tech-stack:
  added: []  # reqwest was already added in 02-01
  patterns:
    - "OnceLock-based shared engine for parallel test safety (prevents HF Hub file lock contention)"
    - "Arc<dyn EmbeddingEngine> dynamic dispatch in AppState for runtime provider selection"
    - "spawn_blocking wraps LocalEngine::new() at startup to prevent tokio runtime blocking"
    - "dimensions=384 Matryoshka parameter aligns OpenAI embeddings with local model output shape"

key-files:
  created:
    - .planning/phases/02-embedding/02-02-SUMMARY.md
  modified:
    - src/embedding.rs
    - src/server.rs
    - src/main.rs
    - tests/integration.rs

key-decisions:
  - "OnceLock shared engine in integration tests prevents HF Hub file lock contention during parallel test runs"
  - "OpenAiEngine validates response embedding.len() == 384 before returning (mirrors LocalEngine guard)"
  - "LocalEngine::new() wrapped in spawn_blocking in main.rs startup (not just embed() calls)"

patterns-established:
  - "AppState holds embedding provider as Arc<dyn EmbeddingEngine> for handler-agnostic access"
  - "Engine selection at startup: config.openai_api_key presence determines provider"
  - "All embedding providers validate 384-dimension output before returning Ok(Vec<f32>)"

requirements-completed: [EMBD-04, EMBD-05]

# Metrics
duration: 3min
completed: 2026-03-19
---

# Phase 2 Plan 02: OpenAiEngine, AppState Wiring, and Embedding Integration Tests Summary

**OpenAiEngine with reqwest calling text-embedding-3-small at 384 dims, Arc<dyn EmbeddingEngine> in AppState, startup engine selection in main.rs, and 5 integration tests proving semantic quality (dog/puppy cosine similarity > dog/database)**

## Performance

- **Duration:** ~3 min
- **Started:** 2026-03-19T20:53:46Z
- **Completed:** 2026-03-19T20:56:53Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- OpenAiEngine added to `src/embedding.rs`: POSTs to `https://api.openai.com/v1/embeddings` with bearer auth, requests 384 dims from text-embedding-3-small, validates response length, returns `EmbeddingError::EmptyInput` for empty input
- AppState gains `embedding: Arc<dyn EmbeddingEngine>` field enabling all Phase 3 request handlers to call `state.embedding.embed(text).await`
- main.rs selects LocalEngine (default, wrapped in spawn_blocking) vs OpenAiEngine (when `config.openai_api_key` is Some) with structured tracing logs
- 5 integration tests pass: 384-dimension count, L2 norm ~1.0, semantic similarity (dog/puppy > dog/database), empty-input error, and engine reuse without reinit

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement OpenAiEngine and update AppState** - `91f5883` (feat)
2. **Task 2: Wire engine selection in main.rs and add integration tests** - `18161bd` (feat)

**Plan metadata:** (docs commit — see final_commit step below)

## Files Created/Modified

- `src/embedding.rs` - Added OpenAiEngine struct, private serde structs, EmbeddingEngine impl with bearer auth + 384-dim validation, compile-time Send+Sync tests
- `src/server.rs` - AppState gains `pub embedding: Arc<dyn EmbeddingEngine>` field
- `src/main.rs` - Added `mod embedding`, engine selection block (steps 5/6), spawn_blocking for LocalEngine::new(), structured tracing logs
- `tests/integration.rs` - Added OnceLock-based shared engine helper, 5 embedding tests, cosine_similarity() helper

## Decisions Made

- **OnceLock shared engine in integration tests:** When multiple tokio::test functions call `LocalEngine::new()` concurrently, the HuggingFace Hub file lock (for cache management) causes failures. Sharing a single engine via `OnceLock<Arc<LocalEngine>>` serializes model initialization and eliminates lock contention. The engine is loaded once; all tests reuse the same Arc.
- **OpenAiEngine validates response dimension:** Mirrors the LocalEngine guard (`embedding.len() != 384`). Catches API changes or configuration errors before returning incorrect data to sqlite-vec inserts in Phase 3.
- **spawn_blocking in main.rs for LocalEngine::new():** The model download + file I/O + weight loading in `LocalEngine::new()` is blocking. Wrapping it in `spawn_blocking` at startup (not just at embed time) prevents blocking the tokio runtime during server initialization.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed HuggingFace Hub lock contention in parallel integration tests**
- **Found during:** Task 2 (running `cargo test`)
- **Issue:** Multiple `#[tokio::test]` functions called `LocalEngine::new()` concurrently. The HF Hub cache uses file locks to prevent concurrent downloads. All tests racing to acquire the lock caused `ModelLoad("Lock acquisition failed: ...")` panics in 3 of 5 embedding tests.
- **Fix:** Added `OnceLock<Arc<LocalEngine>>` static at the top of `tests/integration.rs`. The `local_engine()` helper calls `OnceLock::get_or_init()` which is thread-safe — only one initialization runs, all others block until complete, then all share the same Arc. Tests call `tokio::task::spawn_blocking(local_engine)` to get the engine.
- **Files modified:** `tests/integration.rs`
- **Verification:** `cargo test` — all 10 integration tests green (was 7 pass / 3 fail before fix)
- **Committed in:** `18161bd` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Required for correct test execution. No scope creep — the fix is additive to the test helper only and doesn't change production code.

## Issues Encountered

None beyond the HF Hub lock contention documented above.

## User Setup Required

To use the OpenAI embedding engine, set `MNEMONIC_OPENAI_API_KEY` in your environment or `mnemonic.toml`:

```toml
openai_api_key = "sk-..."
```

Without this, the server defaults to the local all-MiniLM-L6-v2 model (no configuration needed).

## Next Phase Readiness

- Phase 3 (Storage/Service) can use `state.embedding.embed(text).await` in memory store/search handlers
- AppState provides `db`, `config`, and `embedding` — all three needed for Phase 3 handlers
- The 384-dimension contract is enforced by both engines, safe to INSERT into `vec_memories` (float[384])
- No blockers

---
*Phase: 02-embedding*
*Completed: 2026-03-19*
