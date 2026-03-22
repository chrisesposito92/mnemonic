---
phase: 28
slug: core-rpc-handlers-health-discoverability
status: audited
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-22
---

# Phase 28 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test framework (`cargo test`) |
| **Config file** | None — standard `#[cfg(test)]` modules + `tests/` directory |
| **Quick run command** | `cargo test --features interface-grpc grpc` |
| **Full suite command** | `cargo test --features interface-grpc` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --features interface-grpc grpc`
- **After every plan wave:** Run `cargo test --features interface-grpc`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 28-01-01 | 01 | 1 | GRPC-05 | unit | `cargo test --features interface-grpc test_grpc_store_memory_empty_content` | Yes — `tests/grpc_integration.rs` | green |
| 28-01-02 | 01 | 1 | GRPC-01 | integration | `cargo test --features interface-grpc test_grpc_store_memory` | Yes — `tests/grpc_integration.rs` | green |
| 28-01-03 | 01 | 1 | GRPC-02 | integration | `cargo test --features interface-grpc test_grpc_search_memories` | Yes — `tests/grpc_integration.rs` | green |
| 28-01-04 | 01 | 1 | GRPC-03 | integration | `cargo test --features interface-grpc test_grpc_list_memories` | Yes — `tests/grpc_integration.rs` | green |
| 28-01-05 | 01 | 1 | GRPC-04 | integration | `cargo test --features interface-grpc test_grpc_delete_memory_not_found` | Yes — `tests/grpc_integration.rs` | green |
| 28-02-01 | 02 | 1 | GRPC-05 | integration | `cargo test --features interface-grpc test_grpc_store_memory_scope_enforcement` | Yes — `tests/grpc_integration.rs` | green |
| 28-02-02 | 02 | 1 | GRPC-05 | integration | `cargo test --features interface-grpc test_grpc_search_memories_scope_enforcement` | Yes — `tests/grpc_integration.rs` | green |
| 28-02-03 | 02 | 1 | GRPC-05 | integration | `cargo test --features interface-grpc test_grpc_list_memories_scope_enforcement` | Yes — `tests/grpc_integration.rs` | green |
| 28-02-04 | 02 | 1 | GRPC-05 | integration | `cargo test --features interface-grpc test_grpc_delete_memory_scope_enforcement` | Yes — `tests/grpc_integration.rs` | green |
| 28-03-01 | 03 | 2 | HEALTH-02 | smoke | `cargo test --features interface-grpc test_grpc_reflection_builds` | Yes — `tests/grpc_integration.rs` | green |
| 28-03-02 | 03 | 2 | HEALTH-01 | smoke | `cargo test --features interface-grpc test_grpc_health_serving` | Yes — `tests/grpc_integration.rs` | green |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

- [x] `tests/grpc_integration.rs` — integration test harness for GRPC-01 through GRPC-05 (14 tests, all green)
- [x] Per-handler scope enforcement tests (4 tests) — covers STATE.md critical research flag
- [x] `build.rs` update — `file_descriptor_set_path` generates `mnemonic_descriptor.bin` for reflection

*Existing infrastructure covers health (tonic-health already wired in Phase 27).*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `grpcurl list` enumerates services | HEALTH-02 | Requires running server + grpcurl binary | Start server with `cargo run --features interface-grpc`, run `grpcurl -plaintext localhost:50051 list` |
| `grpcurl Health/Check` returns SERVING | HEALTH-01 | Requires running server + grpcurl binary | Start server, run `grpcurl -plaintext localhost:50051 grpc.health.v1.Health/Check` |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s (14 tests run in 0.02s)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved

---

## Validation Audit 2026-03-22

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |

**Coverage:** 11/11 tasks have automated tests, all green. 14 integration tests in `tests/grpc_integration.rs` cover all 7 requirements (GRPC-01 through GRPC-05, HEALTH-01, HEALTH-02). Full suite: 223 tests, 0 failures.
