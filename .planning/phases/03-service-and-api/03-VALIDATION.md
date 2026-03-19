---
phase: 3
slug: service-and-api
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml (already configured) |
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
| 03-01-01 | 01 | 1 | API-01 | integration | `cargo test store_memory` | ❌ W0 | ⬜ pending |
| 03-01-02 | 01 | 1 | API-06 | integration | `cargo test error_response` | ❌ W0 | ⬜ pending |
| 03-02-01 | 02 | 1 | API-02, AGNT-03 | integration | `cargo test search_memory` | ❌ W0 | ⬜ pending |
| 03-02-02 | 02 | 1 | API-03 | integration | `cargo test list_memories` | ❌ W0 | ⬜ pending |
| 03-03-01 | 03 | 2 | API-04 | integration | `cargo test delete_memory` | ❌ W0 | ⬜ pending |
| 03-03-02 | 03 | 2 | AGNT-01, AGNT-02 | integration | `cargo test agent_isolation` | ❌ W0 | ⬜ pending |
| 03-03-03 | 03 | 2 | API-05 | integration | `cargo test health_endpoint` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/api_tests.rs` — integration test stubs for all API endpoints
- [ ] `src/service.rs` — MemoryService struct with method signatures (compile-check stubs)
- [ ] MockEmbeddingEngine in test helpers for fast API tests without model loading

*Existing test infrastructure (cargo test, integration test pattern) covers framework needs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Memory persists across server restarts | API-01 | Requires process restart | Store memory, stop server, restart, verify GET returns it |

*All other behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
