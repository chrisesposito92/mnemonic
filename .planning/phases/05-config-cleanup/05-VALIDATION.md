---
phase: 05
slug: config-cleanup
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-19
audited: 2026-03-19
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
| 05-01-01 | 01 | 1 | CONF-02 | unit | `cargo test --lib config::tests::test_validate_config_openai_no_key` | ✅ | ✅ green |
| 05-01-02 | 01 | 1 | CONF-02 | unit | `cargo test --lib config::tests::test_validate_config_unknown_provider` | ✅ | ✅ green |
| 05-01-03 | 01 | 1 | CONF-02 | unit | `cargo test --lib config::tests::test_config_defaults` | ✅ | ✅ green |
| 05-01-04 | 01 | 1 | CONF-03 | build check | `grep openai_api_key mnemonic.toml.example` | ✅ | ✅ green |
| 05-01-05 | 01 | 1 | EMBD-04 | integration | `cargo test -- --ignored test_openai_embedding` | ✅ | ✅ green |
| 05-01-06 | 01 | 1 | (all) | build gate | `cargo build 2>&1 \| grep warning` | ✅ | ✅ green |
| 05-01-07 | 01 | 1 | (all) | regression | `cargo test` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `src/config.rs` — `validate_config()` function exists with 4 unit tests: openai_no_key, unknown_provider, local_ok, openai_with_key. All pass.

*All Wave 0 requirements fulfilled during phase execution.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| OpenAI engine selection with real API key | EMBD-04 | Requires valid OpenAI API key | Set `MNEMONIC_EMBEDDING_PROVIDER=openai` + `MNEMONIC_OPENAI_API_KEY=sk-...` and verify startup log says `provider = "openai"` |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 15s (full suite: ~8s)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** passed

---

## Validation Audit 2026-03-19

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |

All 7 verification items confirmed green. 9 config unit tests pass, 21 integration tests pass, zero compiler warnings. Phase is Nyquist-compliant.
