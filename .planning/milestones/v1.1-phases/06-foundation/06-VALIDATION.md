---
phase: 6
slug: foundation
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-20
audited: 2026-03-20
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~30 seconds |

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
| 06-01-01 | 01 | 1 | LLM-01 | unit | `cargo test --lib config::tests` | ✅ | ✅ green |
| 06-01-02 | 01 | 1 | LLM-01 | unit | `cargo test --lib config::tests::test_validate_config` | ✅ | ✅ green |
| 06-02-01 | 02 | 1 | SC-4 | integration | `cargo test --test integration test_schema` | ✅ | ✅ green |
| 06-02-02 | 02 | 1 | SC-4 | integration | `cargo test --test integration test_compact_runs` | ✅ | ✅ green |
| 06-02-03 | 02 | 1 | SC-4 | integration | `cargo test --test integration test_compact_runs_agent_id_index` | ✅ | ✅ green |
| 06-01-03 | 01 | 1 | LLM-01 | integration | `cargo test --test error_types` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Gap Audit (Nyquist Auditor — 2026-03-20)

Gaps resolved by `gsd-nyquist-auditor`:

| Gap | Requirement | Test Added | File | Command | Status |
|-----|-------------|-----------|------|---------|--------|
| GAP 1 | LlmError variant display strings | `test_llm_error_api_call_display`, `test_llm_error_timeout_display`, `test_llm_error_parse_display` | `tests/error_types.rs` | `cargo test --test error_types` | ✅ green |
| GAP 1 | LlmError #[from] into MnemonicError | `test_llm_error_into_mnemonic` | `tests/error_types.rs` | `cargo test --test error_types` | ✅ green |
| GAP 2 | compact_runs agent_id index | `test_compact_runs_agent_id_index` | `tests/integration.rs` | `cargo test --test integration test_compact_runs_agent_id_index` | ✅ green |
| GAP 3 | LlmError display strings | covered by GAP 1 tests | `tests/error_types.rs` | `cargo test --test error_types` | ✅ green |

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. cargo test already runs unit and integration tests.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Server starts on v1.0 DB | SC-1 | Requires existing DB file | Run `cargo run` against a v1.0 mnemonic.db — verify no errors at startup |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** audited green — 2026-03-20
