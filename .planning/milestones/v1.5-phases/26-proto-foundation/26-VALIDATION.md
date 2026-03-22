---
phase: 26
slug: proto-foundation
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-22
validated: 2026-03-22
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
| **Full suite command** | `cargo test --features interface-grpc && cargo build --features interface-grpc` |
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo build --features interface-grpc`
- **After every plan wave:** Run `cargo test --features interface-grpc && cargo build --features interface-grpc`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 26-01-01 | 01 | 1 | PROTO-04 | build | `cargo build` (default features, no gRPC deps) | N/A | green |
| 26-01-02 | 01 | 1 | PROTO-01 | build | `cargo build --features interface-grpc` | tests/grpc_integration.rs | green |
| 26-01-03 | 01 | 1 | PROTO-02 | build | `cargo build --features interface-grpc` twice, second < 2s (measured: 0.14s) | N/A | green |
| 26-01-04 | 01 | 1 | PROTO-04 | inspection | `cargo tree --features interface-grpc,backend-qdrant -d 2>&1 \| grep -E '^prost v' \| sort -u \| wc -l` equals 1 | N/A | green |
| 26-02-01 | 02 | 1 | PROTO-03 | CI | `grep -A2 "Install protoc" .github/workflows/release.yml` shows arduino/setup-protoc@v3 before Build binary | .github/workflows/release.yml | green |

*Status: pending / green / red / flaky*

**Note on 26-01-04:** Original criterion was "zero tonic/prost duplicate lines in `cargo tree -d`". Per accepted deviation in 26-01-SUMMARY.md, two tonic versions (0.12.3 from qdrant-client, 0.13.1 ours) are expected and acceptable. The real invariant is zero prost version conflicts — only prost 0.13.5 appears in the tree. Verification command updated to check this invariant.

---

## Wave 0 Requirements

- Existing infrastructure covers all phase requirements. No new test framework needed.
- Phase 26 validation is primarily build verification (cargo build succeeds) rather than test-based.
- 14 gRPC integration tests exist in tests/grpc_integration.rs (from later phases) that exercise the proto-generated types, providing additional coverage for PROTO-01.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| CI protoc installation works | PROTO-03 | Requires GitHub Actions runner | Push tag, verify release workflow passes |

---

## Validation Sign-Off

- [x] All tasks have automated verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** validated

---

## Validation Audit 2026-03-22

| Metric | Count |
|--------|-------|
| Gaps found | 1 |
| Resolved | 1 |
| Escalated | 0 |

**Details:**
- 26-01-04: Updated verification command from "zero tonic/prost duplicate lines" to "single prost version" to match accepted deviation documented in 26-01-SUMMARY.md. Real invariant (zero prost version conflicts) confirmed passing.
