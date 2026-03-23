---
phase: 32
slug: operational-actions
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-22
---

# Phase 32 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | None (frontend); cargo test (backend) |
| **Config file** | None (frontend); Cargo.toml (backend) |
| **Quick run command** | `cd dashboard && npm run build` |
| **Full suite command** | `cargo test && cd dashboard && npm run build` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cd dashboard && npm run build`
- **After every plan wave:** Run `cargo test && cd dashboard && npm run build`
- **Before `/gsd:verify-work`:** Full suite must be green + manual browser test of dry-run → confirm flow
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 32-01-01 | 01 | 1 | OPS-02 | integration | `cargo test` | ❌ W0 | ⬜ pending |
| 32-01-02 | 01 | 1 | OPS-02 | build | `cd dashboard && npm run build` | ✅ | ⬜ pending |
| 32-02-01 | 02 | 1 | OPS-02 | build | `cd dashboard && npm run build` | ✅ | ⬜ pending |
| 32-02-02 | 02 | 1 | OPS-02 | manual | — | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Verify `src/service.rs` has `get_memory(id)` method or plan to add it
- [ ] Backend integration test for GET /memories/{id} — new route needs test coverage

*No frontend test infrastructure to create — existing pattern is build-only validation for the dashboard layer.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Dry-run compaction shows cluster preview | OPS-02 | No frontend test infra | 1. Navigate to Compact tab 2. Select agent 3. Click Run Dry Run 4. Verify cluster table renders |
| Confirm compaction executes and refreshes | OPS-02 | No frontend test infra | 1. After dry-run, click Confirm 2. Verify success message 3. Navigate to Memories tab 4. Verify count reflects compaction |
| Empty states render correctly | OPS-02 | Visual verification | 1. Start with empty DB 2. Visit each tab 3. Verify empty state messages |
| Loading skeletons appear during fetch | OPS-02 | Timing-dependent | 1. Throttle network 2. Navigate tabs 3. Verify skeleton rows appear |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
