---
phase: 7
slug: summarization-engine
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-20
audited: 2026-03-20
---

# Phase 7 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo test --lib summarization` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib summarization`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 07-01-01 | 01 | 1 | LLM-02 | unit | `cargo test -p mnemonic summarization` | src/summarization.rs | ✅ green |
| 07-01-02 | 01 | 1 | LLM-03 | unit | `cargo test -p mnemonic summarization` | src/summarization.rs | ✅ green |
| 07-01-03 | 01 | 1 | LLM-04 | build | `cargo build` | src/main.rs | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Test Coverage Detail

| Requirement | Tests | Covers |
|-------------|-------|--------|
| LLM-02 (SummarizationEngine + OpenAiSummarizer) | `test_trait_object_compiles`, `test_openai_summarizer_send_sync`, `test_prompt_structure` | Trait object safety, Send+Sync bounds, XML prompt structure |
| LLM-03 (MockSummarizer) | `test_mock_summarizer_send_sync`, `test_mock_summarizer_output`, `test_mock_summarizer_single_input`, `test_mock_summarizer_empty_returns_err` | Send+Sync, deterministic output, single input, empty input guard |
| LLM-04 (main.rs wiring) | `cargo build` (compile-time verification) | Engine init compiles, type-checks against SummarizationEngine trait |

**Total: 7 unit tests + 1 build verification = 8 automated checks**

---

## Wave 0 Requirements

- [x] `src/summarization.rs` — new module with SummarizationEngine trait + implementations
- [x] Tests within `src/summarization.rs` — unit tests for trait, mock, object safety

*Existing test infrastructure (cargo test) covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Real OpenAI API call | LLM-02 | Requires API key + network | Set MNEMONIC_LLM_PROVIDER=openai, MNEMONIC_LLM_API_KEY=sk-..., run integration test |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 15s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** passed

---

## Validation Audit 2026-03-20

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |
