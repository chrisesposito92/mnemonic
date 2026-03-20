---
phase: 9
slug: http-integration
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 9 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml — test config already present |
| **Quick run command** | `cargo test --test integration compact` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --test integration compact`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 09-01-01 | 01 | 1 | API-01 | integration | `cargo test --test integration compact_success` | ❌ W0 | ⬜ pending |
| 09-01-02 | 01 | 1 | API-02 | integration | `cargo test --test integration compact_dry_run` | ❌ W0 | ⬜ pending |
| 09-01-03 | 01 | 1 | API-03 | integration | `cargo test --test integration compact_id_mapping` | ❌ W0 | ⬜ pending |
| 09-01-04 | 01 | 1 | API-04 | integration | `cargo test --test integration compact_agent_isolation` | ❌ W0 | ⬜ pending |
| 09-01-05 | 01 | 1 | API-01 | integration | `cargo test --test integration compact_validation` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. `tests/integration.rs` already has `build_test_compaction()`, `json_request()`, `response_json()`, and `MockEmbeddingEngine`. Extend with `build_test_compact_state()` for HTTP-layer tests.

---

## Manual-Only Verifications

All phase behaviors have automated verification.

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
