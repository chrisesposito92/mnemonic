---
phase: 12
slug: auth-middleware
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 12 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[cfg(test)]` + `#[tokio::test]` |
| **Config file** | Cargo.toml (already configured) |
| **Quick run command** | `cargo test --lib auth` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib auth`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 12-01-01 | 01 | 1 | AUTH-01 | integration | `cargo test --test integration auth` | ✅ | ⬜ pending |
| 12-01-02 | 01 | 1 | AUTH-02 | integration | `cargo test --test integration auth` | ✅ | ⬜ pending |
| 12-01-03 | 01 | 1 | AUTH-03 | integration | `cargo test --test integration auth` | ✅ | ⬜ pending |
| 12-01-04 | 01 | 1 | AUTH-05 | integration | `cargo test --test integration auth` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements. The `tests/integration.rs` test harness with `build_test_state()`, `build_router()`, `json_request()`, and `response_json()` helpers already exists.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
