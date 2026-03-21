---
phase: 12
slug: auth-middleware
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-20
validated: 2026-03-21
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
| 12-01-01 | 01 | 1 | AUTH-01 | integration | `cargo test test_auth_valid_token_allows` | ✅ | ✅ green |
| 12-01-02 | 01 | 1 | AUTH-02 | integration | `cargo test test_auth_invalid_token_rejects test_auth_revoked_token_rejects` | ✅ | ✅ green |
| 12-01-03 | 01 | 1 | AUTH-03 | integration | `cargo test test_auth_open_mode_allows` | ✅ | ✅ green |
| 12-01-04 | 01 | 1 | AUTH-05 | integration | `cargo test test_auth_health_no_token` | ✅ | ✅ green |
| 12-01-05 | 01 | 1 | AUTH-02 (missing header) | integration | `cargo test test_auth_missing_header_rejects` | ✅ | ✅ green |
| 12-01-06 | 01 | 1 | Malformed header → 400 | integration | `cargo test test_auth_malformed_header_400` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements. The `tests/integration.rs` test harness with `build_test_state()`, `build_router()`, `json_request()`, and `response_json()` helpers already exists.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** complete

---

## Validation Audit 2026-03-21

| Metric | Count |
|--------|-------|
| Gaps found | 1 |
| Resolved | 1 |
| Escalated | 0 |
