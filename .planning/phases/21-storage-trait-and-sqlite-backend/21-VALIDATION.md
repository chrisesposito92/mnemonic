---
phase: 21
slug: storage-trait-and-sqlite-backend
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 21 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo test --lib 2>&1 \| tail -5` |
| **Full suite command** | `cargo test 2>&1 \| tail -10` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib 2>&1 | tail -5`
- **After every plan wave:** Run `cargo test 2>&1 | tail -10`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 21-01-01 | 01 | 1 | STOR-01 | compilation | `cargo check 2>&1 \| tail -5` | N/A | ⬜ pending |
| 21-01-02 | 01 | 1 | STOR-02 | unit + integration | `cargo test 2>&1 \| tail -10` | ✅ | ⬜ pending |
| 21-02-01 | 02 | 2 | STOR-03, STOR-04 | unit + integration | `cargo test 2>&1 \| tail -10` | ✅ | ⬜ pending |
| 21-02-02 | 02 | 2 | STOR-05 | full suite | `cargo test 2>&1 \| tail -10` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. The project already has 239 passing tests — this phase is a refactor that must preserve them all.

---

## Manual-Only Verifications

All phase behaviors have automated verification. The primary validation is that `cargo test` reports 239 tests passing with zero regressions after the refactor.

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
