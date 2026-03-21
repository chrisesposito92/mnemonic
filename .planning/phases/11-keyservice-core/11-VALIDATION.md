---
phase: 11
slug: keyservice-core
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
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
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib auth`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 11-01-01 | 01 | 1 | KEY-01 | unit | `cargo test --lib auth::tests::test_create_key` | ❌ W0 | ⬜ pending |
| 11-01-02 | 01 | 1 | INFRA-02 | unit | `cargo test --lib auth::tests::test_blake3_hash` | ❌ W0 | ⬜ pending |
| 11-01-03 | 01 | 1 | KEY-02 | unit | `cargo test --lib auth::tests::test_list_keys` | ❌ W0 | ⬜ pending |
| 11-01-04 | 01 | 1 | KEY-03 | unit | `cargo test --lib auth::tests::test_revoke_key` | ❌ W0 | ⬜ pending |
| 11-01-05 | 01 | 1 | KEY-04 | unit | `cargo test --lib auth::tests::test_validate_key` | ❌ W0 | ⬜ pending |
| 11-01-06 | 01 | 1 | KEY-03 | unit | `cargo test --lib auth::tests::test_validate_revoked` | ❌ W0 | ⬜ pending |
| 11-01-07 | 01 | 1 | KEY-04 | unit | `cargo test --lib auth::tests::test_validate_scope` | ❌ W0 | ⬜ pending |
| 11-01-08 | 01 | 1 | INFRA-02 | unit | `cargo test --lib auth::tests::test_constant_time` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/auth.rs` `#[cfg(test)] mod tests` — stubs for all KEY-* and INFRA-02 requirements
- [ ] `blake3`, `constant_time_eq`, `rand` — added to Cargo.toml dependencies

*Existing test infrastructure (Cargo.toml dev-dependencies, tower, http-body-util) covers framework needs.*

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
