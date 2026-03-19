---
phase: 1
slug: foundation
status: approved
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-19
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness + `#[tokio::test]` |
| **Config file** | Cargo.toml (dev-dependencies section) |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo test -- --include-ignored` |
| **Estimated runtime** | ~9 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test`
- **After every plan wave:** Run `cargo test -- --include-ignored`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Test File(s) | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | STOR-01 | integration | `cargo test test_schema_created test_vec_memories_exists` | tests/integration.rs | ✅ green |
| 01-01-02 | 01 | 1 | STOR-02 | integration | `cargo test test_wal_mode` | tests/integration.rs | ✅ green |
| 01-01-03 | 01 | 1 | STOR-03 | integration | `cargo test test_db_open_async` | tests/integration.rs | ✅ green |
| 01-01-04 | 01 | 1 | STOR-04 | integration | `cargo test test_embedding_model_column` | tests/integration.rs | ✅ green |
| 01-02-01 | 02 | 1 | CONF-01 | unit | `cargo test config::tests::test_config_defaults` | src/config.rs | ✅ green |
| 01-02-02 | 02 | 1 | CONF-02 | unit | `cargo test config::tests::test_config_env` | src/config.rs | ✅ green |
| 01-02-03 | 02 | 1 | CONF-03 | unit | `cargo test config::tests::test_config_toml` | src/config.rs | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements.

- [x] `tests/integration.rs` — 5 async integration tests for STOR-01 through STOR-04
- [x] `src/config.rs` — 5 inline unit tests for CONF-01 through CONF-03
- [x] `tokio` dev-dependency with `macros` and `rt-multi-thread` features for `#[tokio::test]`

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Startup message prints port, storage path, embedding provider | CONF-01 | Requires visual confirmation of formatted output | Run `cargo run` and verify stdout contains port, path, and provider |

*All other phase behaviors have automated verification.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 10s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-03-19

---

## Validation Audit 2026-03-19

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |
| Total tests covering Phase 1 | 10 (5 unit + 5 integration) |
| All tests passing | 21/21 (full suite, 8.22s) |
