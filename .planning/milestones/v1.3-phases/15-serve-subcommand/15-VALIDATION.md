---
phase: 15
slug: serve-subcommand
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-21
validated: 2026-03-21
---

# Phase 15 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo test --all` |
| **Estimated runtime** | ~8 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test`
- **After every plan wave:** Run `cargo test --all`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 8 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 15-01-01 | 01 | 1 | CLI-01 | integration | `cargo test --test cli_integration test_serve` | tests/cli_integration.rs:413 | ✅ green |
| 15-01-02 | 01 | 1 | CLI-02 | integration | `cargo test --test cli_integration test_serve` | tests/cli_integration.rs:435 | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Requirement Coverage

| Requirement | Description | Verification | Status |
|-------------|-------------|--------------|--------|
| CLI-01 | `mnemonic serve` starts HTTP server, `--help` lists `serve` | `test_serve_appears_in_help` + `test_serve_help_text_description` verify `serve` in help output with correct description | COVERED |
| CLI-02 | Bare `mnemonic` backward compat | Exhaustive match dispatch `Some(Commands::Serve) \| None` — type system guarantees both paths route to same server init. Existing `tests/integration.rs` tests server behavior. | COVERED |

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements. No new test infra needed.*

---

## Manual-Only Verifications

*None — all requirements have automated coverage.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s (measured: ~8s)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** complete

---

## Validation Audit 2026-03-21

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |
