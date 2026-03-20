---
phase: 05
slug: config-cleanup
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
---

# Phase 05 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test + tokio-test (async) |
| **Config file** | none (Cargo.toml `[[test]]` is implicit) |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo build 2>&1 | grep warning` (must be zero) + `cargo test --lib`
- **After every plan wave:** Run `cargo test` (full suite including integration)
- **Before `/gsd:verify-work`:** Full suite must be green + zero compiler warnings
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 05-01-01 | 01 | 1 | CONF-02 | unit | `cargo test --lib config::tests::test_validate_config_openai_no_key` | ❌ W0 | ⬜ pending |
| 05-01-02 | 01 | 1 | CONF-02 | unit | `cargo test --lib config::tests::test_validate_config_unknown_provider` | ❌ W0 | ⬜ pending |
| 05-01-03 | 01 | 1 | CONF-02 | unit | `cargo test --lib config::tests::test_config_defaults` | ✅ | ⬜ pending |
| 05-01-04 | 01 | 1 | CONF-03 | build check | `grep openai_api_key mnemonic.toml.example` | ✅ after fix | ⬜ pending |
| 05-01-05 | 01 | 1 | EMBD-04 | integration | `cargo test -- --ignored test_openai_embedding` | ✅ existing | ⬜ pending |
| 05-01-06 | 01 | 1 | (all) | build gate | `cargo build 2>&1 \| grep warning` | N/A | ⬜ pending |
| 05-01-07 | 01 | 1 | (all) | regression | `cargo test` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/config.rs` — add `validate_config()` function + unit tests for: (a) `embedding_provider=openai` + no key → error, (b) `embedding_provider=unknown` → error, (c) `embedding_provider=local` + no key → ok. Uses `figment::Jail` pattern already established.

*Existing infrastructure covers all other phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| OpenAI engine selection with real API key | EMBD-04 | Requires valid OpenAI API key | Set `MNEMONIC_EMBEDDING_PROVIDER=openai` + `MNEMONIC_OPENAI_API_KEY=sk-...` and verify startup log says `provider = "openai"` |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
