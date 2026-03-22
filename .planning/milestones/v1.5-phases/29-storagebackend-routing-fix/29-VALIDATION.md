---
phase: 29
slug: storagebackend-routing-fix
status: audited
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-22
audited: 2026-03-22
---

# Phase 29 — Validation Strategy

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
| 29-01-01 | 01 | 1 | DEBT-01 | integration | `cargo test --test cli_integration recall` | :white_check_mark: | :white_check_mark: green |
| 29-01-02 | 01 | 1 | DEBT-01 | unit | `cargo test --lib recall` | :white_check_mark: | :white_check_mark: green |

*Status: :white_large_square: pending · :white_check_mark: green · :x: red · :warning: flaky*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Recall with Qdrant backend | DEBT-01 | Requires running Qdrant instance | Set `MNEMONIC_STORAGE_PROVIDER=qdrant`, run `mnemonic recall`, verify results from Qdrant |
| Recall with Postgres backend | DEBT-01 | Requires running Postgres+pgvector | Set `MNEMONIC_STORAGE_PROVIDER=postgres`, run `mnemonic recall`, verify results from Postgres |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved

---

## Validation Audit 2026-03-22

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |

**Fixes applied:**
- Corrected integration test filter from `rcl` to `recall` (typo — original filter matched 0 tests)
- Updated task statuses from pending to green (14 integration + 2 unit tests all pass)
- Set `nyquist_compliant: true`

**Test Evidence:**
- `cargo test --test cli_integration recall`: 14 passed, 0 failed
- `cargo test --lib recall`: 2 passed, 0 failed (delegation tests)
- `cargo test --lib`: 87 passed, 0 failed (full lib suite, zero regression)
