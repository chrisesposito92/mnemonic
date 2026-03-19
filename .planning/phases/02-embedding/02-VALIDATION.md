---
phase: 2
slug: embedding
status: validated
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-19
validated: 2026-03-19
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~8 seconds (model cached) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Test File | Status |
|---------|------|------|-------------|-----------|-------------------|-----------|--------|
| 02-01-01 | 01 | 1 | EMBD-05 | unit | `cargo test --lib embedding` | src/embedding.rs (4 tests) | ✅ green |
| 02-01-02 | 01 | 1 | EMBD-01, EMBD-02 | integration | `cargo test test_local_embedding` | tests/integration.rs | ✅ green |
| 02-01-03 | 01 | 1 | EMBD-03 | integration | `cargo test test_embed_reuse` | tests/integration.rs | ✅ green |
| 02-02-01 | 02 | 2 | EMBD-04 | unit | `cargo test --lib embedding` | src/embedding.rs (Send+Sync, trait object) | ✅ green (compile-time) |
| 02-02-02 | 02 | 2 | EMBD-02 | integration | `cargo test test_semantic_similarity` | tests/integration.rs | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Test Coverage Detail

### Unit Tests (src/embedding.rs — 5 tests)

| Test | Validates | Requirement |
|------|-----------|-------------|
| `test_trait_object_compiles` | EmbeddingEngine is object-safe for `Arc<dyn EmbeddingEngine>` | EMBD-05 |
| `test_local_engine_send_sync` | LocalEngine is Send + Sync | EMBD-05 |
| `test_openai_engine_send_sync` | OpenAiEngine is Send + Sync | EMBD-04, EMBD-05 |
| `test_both_engines_as_trait_object` | Both engines can be used as trait objects | EMBD-05 |
| `test_empty_input_returns_error` | EmbeddingError::EmptyInput variant exists and matches | EMBD-05 |

### Integration Tests (tests/integration.rs — 5 embedding tests)

| Test | Validates | Requirement |
|------|-----------|-------------|
| `test_local_embedding_384_dimensions` | LocalEngine returns 384-dim vector | EMBD-01, EMBD-02 |
| `test_local_embedding_normalized` | Output vector has L2 norm ≈ 1.0 | EMBD-02 |
| `test_semantic_similarity` | dog/puppy cosine sim > dog/database cosine sim | EMBD-02 |
| `test_empty_input_error` | Empty string returns EmbeddingError::EmptyInput | EMBD-01 |
| `test_embed_reuse` | Engine reusable without reinit, different inputs ≠ different embeddings | EMBD-03 |

---

## Wave 0 Requirements

*Existing cargo test infrastructure covers all phase requirements. No additional setup needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Model downloads from HuggingFace on first run | EMBD-01 | Network dependency, cache state | Delete `~/.cache/huggingface/`, run server, verify model downloads |
| OpenAI API call returns valid embeddings | EMBD-04 | Requires valid API key | Set `MNEMONIC_OPENAI_API_KEY`, run `cargo run`, verify logs show "openai" provider |

---

## Requirement Coverage Summary

| Requirement | Description | Automated Tests | Manual Verification | Coverage |
|-------------|-------------|-----------------|---------------------|----------|
| EMBD-01 | Zero-config local embedding | `test_local_embedding_384_dimensions`, `test_empty_input_error` | Model download (first run) | Full |
| EMBD-02 | Mean pooling + L2 normalization | `test_local_embedding_normalized`, `test_semantic_similarity`, `test_local_embedding_384_dimensions` | — | Full |
| EMBD-03 | Model shared via Arc | `test_embed_reuse` | — | Full |
| EMBD-04 | OpenAI fallback provider | `test_openai_engine_send_sync`, `test_both_engines_as_trait_object` | API call with key | Partial (compile-time + manual) |
| EMBD-05 | Trait abstraction | `test_trait_object_compiles`, `test_both_engines_as_trait_object`, `test_local_engine_send_sync`, `test_openai_engine_send_sync` | — | Full |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 10s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** validated 2026-03-19

---

## Validation Audit 2026-03-19

| Metric | Count |
|--------|-------|
| Total requirements | 5 |
| Fully covered (automated) | 4 (EMBD-01, EMBD-02, EMBD-03, EMBD-05) |
| Partially covered (compile-time + manual) | 1 (EMBD-04) |
| Missing | 0 |
| Total automated tests | 10 (5 unit + 5 integration) |
| All tests passing | 21/21 (includes Phase 1 + Phase 3 tests) |
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |
