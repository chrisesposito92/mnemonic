---
phase: 26
slug: proto-foundation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-22
---

# Phase 26 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo build --features interface-grpc` |
| **Full suite command** | `cargo test && cargo build --features interface-grpc` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo build --features interface-grpc`
- **After every plan wave:** Run `cargo test && cargo build --features interface-grpc`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 26-01-01 | 01 | 1 | PROTO-04 | build | `cargo build` (default features, no gRPC deps) | N/A | pending |
| 26-01-02 | 01 | 1 | PROTO-01 | build | `cargo build --features interface-grpc` | N/A | pending |
| 26-01-03 | 01 | 1 | PROTO-02 | build | `cargo build --features interface-grpc` twice, second < 2s | N/A | pending |
| 26-01-04 | 01 | 1 | PROTO-04 | inspection | `cargo tree -d --features interface-grpc \| grep -E "tonic\|prost"` shows zero | N/A | pending |
| 26-02-01 | 02 | 1 | PROTO-03 | CI | release.yml has protoc step before build | N/A | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

- Existing infrastructure covers all phase requirements. No new test framework needed.
- Phase 26 validation is primarily build verification (cargo build succeeds) rather than test-based.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| CI protoc installation works | PROTO-03 | Requires GitHub Actions runner | Push tag, verify release workflow passes |

---

## Validation Sign-Off

- [ ] All tasks have automated verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
