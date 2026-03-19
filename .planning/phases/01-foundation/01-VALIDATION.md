---
phase: 1
slug: foundation
status: draft
nyquist_compliant: false
wave_0_complete: false
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
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test`
- **After every plan wave:** Run `cargo test -- --include-ignored`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | STOR-01 | unit | `cargo test db::tests` | ❌ W0 | ⬜ pending |
| 01-01-02 | 01 | 1 | STOR-02 | unit | `cargo test db::tests` | ❌ W0 | ⬜ pending |
| 01-01-03 | 01 | 1 | STOR-03 | unit | `cargo test db::tests` | ❌ W0 | ⬜ pending |
| 01-01-04 | 01 | 1 | STOR-04 | unit | `cargo test db::tests` | ❌ W0 | ⬜ pending |
| 01-02-01 | 02 | 1 | CONF-01 | unit | `cargo test config::tests` | ❌ W0 | ⬜ pending |
| 01-02-02 | 02 | 1 | CONF-02 | unit | `cargo test config::tests` | ❌ W0 | ⬜ pending |
| 01-02-03 | 02 | 1 | CONF-03 | unit | `cargo test config::tests` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/` directory — integration test stubs for STOR and CONF requirements
- [ ] `tokio` dev-dependency with `macros` and `rt-multi-thread` features for `#[tokio::test]`
- [ ] `tempfile` dev-dependency for isolated SQLite database test fixtures

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Startup message prints port, storage path, embedding provider | CONF-01 | Requires visual confirmation of formatted output | Run `./mnemonic` and verify stdout contains port, path, and provider |

*All other phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
