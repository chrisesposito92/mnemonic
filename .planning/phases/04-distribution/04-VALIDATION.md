---
phase: 4
slug: distribution
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (existing) |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 04-01-01 | 01 | 1 | DOCS-01 | manual | quickstart walkthrough | N/A | ⬜ pending |
| 04-01-02 | 01 | 1 | DOCS-02 | manual | endpoint count check | N/A | ⬜ pending |
| 04-01-03 | 01 | 1 | DOCS-03 | manual | example code review | N/A | ⬜ pending |
| 04-02-01 | 02 | 1 | N/A | automated | `cargo test` | ✅ | ⬜ pending |
| 04-02-02 | 02 | 1 | N/A | automated | `gh workflow view` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements. Phase 4 is primarily documentation; the 21 existing integration tests already prove the server behavior that the README will describe.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Quickstart 3-command flow | DOCS-01 | UX walkthrough, not automatable | Follow README quickstart from scratch: download/install, start server, store memory via curl |
| API reference completeness | DOCS-02 | Content review, not automatable | Verify every endpoint has request params, response schema, and curl example |
| Python/agent examples work | DOCS-03 | Cross-language validation | Run Python examples against running server; verify output matches documented responses |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
