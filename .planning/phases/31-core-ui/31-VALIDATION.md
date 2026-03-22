---
phase: 31
slug: core-ui
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-22
---

# Phase 31 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test runner (`cargo test`) |
| **Config file** | Cargo.toml feature flags |
| **Quick run command** | `cargo test --features dashboard` |
| **Full suite command** | `cargo test --features dashboard,backend-qdrant,backend-postgres` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --features dashboard`
- **After every plan wave:** Run `cargo test --features dashboard`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 31-01-01 | 01 | 1 | BROWSE-05 | integration | `cargo test --features dashboard -- stats_endpoint_returns_agent_breakdown` | ❌ W0 | ⬜ pending |
| 31-01-02 | 01 | 1 | AUTH-02 | integration | `cargo test --features dashboard -- dashboard_ui_includes_csp_header` | ❌ W0 | ⬜ pending |
| 31-XX-XX | XX | X | BROWSE-01 | integration | `cargo test --features dashboard -- integration::` | ✅ | ⬜ pending |
| 31-XX-XX | XX | X | BROWSE-02 | integration | `cargo test --features dashboard -- integration::` | ✅ | ⬜ pending |
| 31-XX-XX | XX | X | BROWSE-03 | integration | `cargo test --features dashboard -- integration::` | ✅ | ⬜ pending |
| 31-XX-XX | XX | X | BROWSE-04 | unit (type) | Compile-time verification via Memory struct | ✅ | ⬜ pending |
| 31-XX-XX | XX | X | OPS-01 | integration | `cargo test --features dashboard -- health_endpoint` | ✅ | ⬜ pending |
| 31-XX-XX | XX | X | AUTH-01 | integration | `cargo test --features dashboard -- auth_` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/dashboard_integration.rs` — add `stats_endpoint_returns_agent_breakdown` test stub for BROWSE-05
- [ ] `tests/dashboard_integration.rs` — add `dashboard_ui_includes_csp_header` test stub for AUTH-02

*Existing test infrastructure covers all other requirements. Two new test functions in the existing file, no new file needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Distance bar fill direction correct | BROWSE-03 | Visual rendering in browser | Open search tab, search for a known term, verify top results show near-full bars (not empty) |
| CSP does not block inline scripts | AUTH-02 | Browser-level CSP enforcement | Load dashboard in browser, check console for CSP violations |
| Login screen renders on 401 | AUTH-01 | Full browser auth flow | Start server with auth enabled, load dashboard, verify login screen appears |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
