---
phase: 8
slug: compaction-core
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 8 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 08-01-01 | 01 | 1 | DEDUP-01 | unit | `cargo test --lib compaction` | ❌ W0 | ⬜ pending |
| 08-01-02 | 01 | 1 | DEDUP-02 | unit | `cargo test --lib compaction` | ❌ W0 | ⬜ pending |
| 08-01-03 | 01 | 1 | DEDUP-03 | unit | `cargo test --lib compaction` | ❌ W0 | ⬜ pending |
| 08-01-04 | 01 | 1 | DEDUP-04 | unit | `cargo test --lib compaction` | ❌ W0 | ⬜ pending |
| 08-02-01 | 02 | 2 | DEDUP-01,02,03,04 | integration | `cargo test --test integration` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/compaction.rs` — CompactionService struct, types, clustering algorithm, unit tests
- [ ] `tests/integration.rs` — integration test stubs for compaction pipeline

*Existing test infrastructure covers framework needs. No new test dependencies required.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
