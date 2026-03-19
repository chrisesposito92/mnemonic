---
phase: 2
slug: embedding
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
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
| **Estimated runtime** | ~30 seconds (includes model download on first run) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | EMBD-05 | unit | `cargo test embedding` | ❌ W0 | ⬜ pending |
| 02-01-02 | 01 | 1 | EMBD-01, EMBD-02 | integration | `cargo test test_local_embedding` | ❌ W0 | ⬜ pending |
| 02-01-03 | 01 | 1 | EMBD-03 | unit | `cargo test test_model_shared` | ❌ W0 | ⬜ pending |
| 02-02-01 | 02 | 2 | EMBD-04 | integration | `cargo test test_openai` | ❌ W0 | ⬜ pending |
| 02-02-02 | 02 | 2 | EMBD-02 | integration | `cargo test test_semantic_similarity` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/embedding_tests.rs` — integration test stubs for embedding trait, local engine, and semantic similarity
- [ ] Test utilities for cosine similarity computation

*Existing cargo test infrastructure covers framework requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Model downloads from HuggingFace on first run | EMBD-01 | Network dependency, cache state | Delete `~/.cache/huggingface/`, run server, verify model downloads |
| OpenAI API key switches provider | EMBD-04 | Requires valid API key | Set `MNEMONIC_OPENAI_API_KEY`, verify logs show "openai" provider |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
