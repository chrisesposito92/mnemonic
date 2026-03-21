---
phase: 17
slug: remember-subcommand
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-21
validated: 2026-03-21
---

# Phase 17 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | `Cargo.toml` `[dev-dependencies]` section |
| **Quick run command** | `cargo test --test cli_integration test_remember` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~4 seconds (includes model load) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --test cli_integration test_remember`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Test Function | Status |
|---------|------|------|-------------|-----------|-------------------|---------------|--------|
| 17-01-01 | 01 | 1 | REM-01 | integration | `cargo test --test cli_integration test_remember_stores_memory_and_prints_uuid` | `test_remember_stores_memory_and_prints_uuid` | ✅ green |
| 17-01-02 | 01 | 1 | REM-02 | integration | `cargo test --test cli_integration test_remember_stdin_pipe_stores_memory` | `test_remember_stdin_pipe_stores_memory` | ✅ green |
| 17-01-03 | 01 | 1 | REM-03 | integration | `cargo test --test cli_integration test_remember_with_agent_and_session_id` | `test_remember_with_agent_and_session_id` | ✅ green |
| 17-01-04 | 01 | 1 | REM-04 | integration | `cargo test --test cli_integration test_remember_with_tags` | `test_remember_with_tags` | ✅ green |
| 17-02-01 | 02 | 1 | D-16/D-17 | integration | `cargo test --test cli_integration test_remember_empty_content_exits_one` | `test_remember_empty_content_exits_one` | ✅ green |
| 17-02-02 | 02 | 1 | D-16/D-17 | integration | `cargo test --test cli_integration test_remember_whitespace_only_content_exits_one` | `test_remember_whitespace_only_content_exits_one` | ✅ green |
| 17-02-03 | 02 | 1 | Help text | integration | `cargo test --test cli_integration test_remember_appears_in_help` | `test_remember_appears_in_help` | ✅ green |
| 17-02-04 | 02 | 1 | OUT-02/--json | integration | `cargo test --test cli_integration test_remember_json` | `test_remember_json` | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Model loading stderr progress | D-09 | stderr timing output is informational | Run `mnemonic remember "test"` and verify "Loading embedding model..." appears on stderr |
| No-arg terminal stdin error | D-03 | Requires PTY simulation; `Command::new()` provides `/dev/null` stdin | Run `mnemonic remember` in a terminal (no pipe) and verify error message and exit 1 |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** complete

---

## Validation Audit 2026-03-21

| Metric | Count |
|--------|-------|
| Gaps found | 1 |
| Resolved | 0 |
| Escalated | 1 (D-03 → manual-only, PTY required) |
| Tests passing | 8/8 |
| Requirements covered | REM-01, REM-02, REM-03, REM-04 |
