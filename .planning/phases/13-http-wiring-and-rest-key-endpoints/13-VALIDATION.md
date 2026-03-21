---
phase: 13
slug: http-wiring-and-rest-key-endpoints
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 13 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in + tokio-test |
| **Config file** | Cargo.toml (test harness built-in) |
| **Quick run command** | `cargo test --test integration 2>&1 \| tail -20` |
| **Full suite command** | `cargo test 2>&1 \| tail -30` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --test integration 2>&1 | tail -20`
- **After every plan wave:** Run `cargo test 2>&1 | tail -30`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 13-01-01 | 01 | 1 | AUTH-04 | unit | `cargo test test_forbidden_variant 2>&1` | ❌ W0 | ⬜ pending |
| 13-01-02 | 01 | 1 | AUTH-04 | unit | `cargo test test_enforce_scope 2>&1` | ❌ W0 | ⬜ pending |
| 13-02-01 | 02 | 1 | AUTH-04 | integration | `cargo test test_scope_mismatch_returns_403 2>&1` | ❌ W0 | ⬜ pending |
| 13-02-02 | 02 | 1 | AUTH-04 | integration | `cargo test test_scope_forces_agent_id 2>&1` | ❌ W0 | ⬜ pending |
| 13-02-03 | 02 | 1 | AUTH-04 | integration | `cargo test test_wildcard_key_passes_through 2>&1` | ❌ W0 | ⬜ pending |
| 13-02-04 | 02 | 1 | AUTH-04 | integration | `cargo test test_scoped_delete_wrong_owner_403 2>&1` | ❌ W0 | ⬜ pending |
| 13-02-05 | 02 | 1 | AUTH-04 | integration | `cargo test test_scoped_delete_own_memory_ok 2>&1` | ❌ W0 | ⬜ pending |
| 13-03-01 | 03 | 2 | INFRA-03 | integration | `cargo test test_post_keys_creates_key 2>&1` | ❌ W0 | ⬜ pending |
| 13-03-02 | 03 | 2 | INFRA-03 | integration | `cargo test test_get_keys_no_raw_token 2>&1` | ❌ W0 | ⬜ pending |
| 13-03-03 | 03 | 2 | INFRA-03 | integration | `cargo test test_delete_key_revokes_access 2>&1` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/error.rs` — add `ApiError::Forbidden(String)` variant and `IntoResponse` arm
- [ ] `src/service.rs` — add `get_memory_agent_id()` method for delete scope check
- [ ] `tests/integration.rs` — add 8+ new integration tests covering AUTH-04 and key endpoint scenarios

*Existing infrastructure covers test framework — no new dependencies needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Startup log shows open vs authenticated mode | INFRA-03 | Already implemented in Phase 10/12; regression surfaces through AUTH-04 and key endpoint tests | Start server with 0 keys, verify "open mode" log; create key, restart, verify "authenticated mode" log |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
