---
phase: 11
slug: keyservice-core
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-20
audited: 2026-03-21
---

# Phase 11 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust #[cfg(test)] inline + integration tests |
| **Config file** | Cargo.toml (dev-dependencies already configured) |
| **Quick run command** | `cargo test --lib auth` |
| **Full suite command** | `cargo test` |
| **Actual runtime** | < 1 second (12 auth tests) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib auth`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** < 1 second

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Tests | Status |
|---------|------|------|-------------|-----------|-------------------|-------|--------|
| 11-01-01 | 01 | 1 | KEY-01 | unit | `cargo test --lib auth::tests::test_create_returns_raw_token` | test_create_returns_raw_token, test_create_stores_hash_not_raw, test_display_id_is_hash_derived, test_generate_raw_token | ✅ green |
| 11-01-02 | 01 | 1 | KEY-01 | unit | `cargo test --lib auth::tests::test_create_with_name_and_scope` | test_create_with_name_and_scope | ✅ green |
| 11-01-03 | 01 | 1 | KEY-02 | unit | `cargo test --lib auth::tests::test_list_returns_all_keys` | test_list_returns_all_keys | ✅ green |
| 11-01-04 | 01 | 1 | KEY-03 | unit | `cargo test --lib auth::tests::test_revoke_prevents_validate` | test_revoke_prevents_validate, test_revoke_idempotent, test_validate_rejects_revoked_key | ✅ green |
| 11-01-05 | 01 | 1 | KEY-04 | unit | `cargo test --lib auth::tests::test_validate_returns_auth_context` | test_validate_returns_auth_context, test_validate_rejects_wrong_token | ✅ green |
| 11-01-06 | 01 | 1 | INFRA-02 | structural | `grep constant_time_eq_32 src/auth.rs` | constant_time_eq_32 at line 219; no == on hash values | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `src/auth.rs` `#[cfg(test)] mod tests` — 12 tests covering all KEY-* and INFRA-02 requirements
- [x] `blake3`, `constant_time_eq`, `rand` — added to Cargo.toml dependencies

*Existing test infrastructure (Cargo.toml dev-dependencies, tower, http-body-util) covers framework needs.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 1s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** passed

---

## Validation Audit 2026-03-21

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |
| Total tests | 12 |
| Requirements covered | 5/5 (KEY-01, KEY-02, KEY-03, KEY-04, INFRA-02) |
