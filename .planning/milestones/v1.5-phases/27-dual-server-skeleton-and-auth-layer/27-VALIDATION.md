---
phase: 27
slug: dual-server-skeleton-and-auth-layer
status: audited
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-22
---

# Phase 27 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in Rust test framework) |
| **Config file** | Cargo.toml (test config via `[profile.test]`) |
| **Quick run command** | `cargo test --features interface-grpc --lib` |
| **Full suite command** | `cargo test --features interface-grpc` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --features interface-grpc --lib`
- **After every plan wave:** Run `cargo test --features interface-grpc`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 27-01-01 | 01 | 1 | SERVER-02 | unit | `cargo test config::tests --lib` | src/config.rs (23 tests) | ✅ green |
| 27-01-02 | 01 | 1 | SERVER-01 | integration | `cargo test --features interface-grpc --test grpc_integration test_grpc_health` | tests/grpc_integration.rs | ✅ green |
| 27-01-03 | 01 | 1 | SERVER-01, SERVER-03 | integration | `cargo test --features interface-grpc --test grpc_integration` | tests/grpc_integration.rs (14 tests) | ✅ green |
| 27-02-01 | 02 | 1 | AUTH-01 | unit | `cargo test --features interface-grpc grpc::auth::tests --lib` | src/grpc/auth.rs (6 tests) | ✅ green |
| 27-02-02 | 02 | 1 | AUTH-03 | unit | `cargo test --features interface-grpc grpc::auth::tests::test_grpc_auth_open_mode --lib` | src/grpc/auth.rs | ✅ green |
| 27-02-03 | 02 | 1 | AUTH-02 | integration | `cargo test --features interface-grpc --test grpc_integration test_grpc_.*scope` | tests/grpc_integration.rs (4 scope tests) | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `src/grpc/mod.rs` — gRPC module skeleton with include_proto! and MnemonicService struct
- [x] `src/grpc/auth.rs` — Tower Layer auth for gRPC with 6 unit tests
- [x] Existing test infrastructure covers config tests — no new framework install needed
- [x] `tests/grpc_integration.rs` — 14 integration tests covering handlers, scope enforcement, health, reflection

*Existing cargo test infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| grpcurl health check | SERVER-01 | Requires running server and grpcurl CLI | Start `mnemonic serve`, run `grpcurl -plaintext localhost:50051 grpc.health.v1.Health/Check` |
| Port bind failure message | SERVER-01 | Requires occupied port | Start server, then start second instance on same port — verify clear error |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 15s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** passed

---

## Validation Audit 2026-03-22

| Metric | Count |
|--------|-------|
| Gaps found | 3 |
| Resolved | 3 |
| Escalated | 0 |

**Audit notes:** All 3 "gaps" were documentation mismatches, not missing coverage. The VALIDATION.md was created pre-execution with predicted test commands/paths. Post-execution, actual test locations differ:
- `grpc::tests` module doesn't exist → tests live in `tests/grpc_integration.rs` (health, reflection)
- `tests/grpc_dual_port.rs` doesn't exist → dual-port covered by `tests/grpc_integration.rs` (14 tests exercise shared Arc services)
- `tests/grpc_auth.rs` doesn't exist → scope enforcement covered by `tests/grpc_integration.rs` (4 scope tests)

Total automated coverage: **23 config unit tests + 6 auth unit tests + 14 integration tests = 43 tests**
