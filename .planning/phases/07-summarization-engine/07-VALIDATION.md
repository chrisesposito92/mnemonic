---
phase: 7
slug: summarization-engine
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
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
| 07-01-01 | 01 | 1 | LLM-02 | unit | `cargo test --lib summarization::tests` | ❌ W0 | ⬜ pending |
| 07-01-02 | 01 | 1 | LLM-03 | unit | `cargo test --lib summarization::tests` | ❌ W0 | ⬜ pending |
| 07-01-03 | 01 | 1 | LLM-04 | unit | `cargo test --lib summarization::tests` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/summarization.rs` — new module with SummarizationEngine trait + implementations
- [ ] Tests within `src/summarization.rs` — unit tests for trait, mock, object safety

*Existing test infrastructure (cargo test) covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Real OpenAI API call | LLM-02 | Requires API key + network | Set MNEMONIC_LLM_PROVIDER=openai, MNEMONIC_LLM_API_KEY=sk-..., run integration test |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
