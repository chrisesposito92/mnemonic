---
phase: 20
slug: output-polish
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-21
validated: 2026-03-21
---

# Phase 20 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo test --test cli_integration` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo check`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 20-01-01 | 01 | 1 | OUT-02 | integration | `cargo test --test cli_integration json` | tests/cli_integration.rs | ✅ green |
| 20-01-02 | 01 | 1 | OUT-02 | integration | `cargo test --test cli_integration json` | tests/cli_integration.rs | ✅ green |
| 20-02-01 | 02 | 2 | OUT-01,OUT-02,OUT-03,OUT-04 | integration | `cargo test --test cli_integration json` | tests/cli_integration.rs | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Requirement Coverage

| Requirement | Tests | Status |
|-------------|-------|--------|
| OUT-01 (Human-readable default) | 44 existing non-JSON tests + `test_json_flag_no_human_output` | ✅ COVERED |
| OUT-02 (All subcommands support --json) | `test_recall_json_list`, `test_recall_json_empty`, `test_recall_json_by_id`, `test_remember_json`, `test_search_json`, `test_compact_json`, `test_keys_create_json`, `test_keys_list_json`, `test_keys_list_json_empty`, `test_json_flag_appears_in_help` | ✅ COVERED |
| OUT-03 (Exit codes 0/1) | All 11 JSON tests assert `status.success()` | ✅ COVERED |
| OUT-04 (stderr/stdout split) | JSON tests parse stdout as JSON (no stderr leak); `test_remember_json` checks stderr | ✅ COVERED |

---

## Wave 0 Requirements

- Existing test infrastructure covers all phase requirements (integration tests in tests/cli_integration.rs)
- 11 new `--json` tests added in Plan 02

*All requirements have automated verification.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Human-readable default output unchanged | OUT-01 | Visual inspection of table formatting | Run `mnemonic recall` and `mnemonic search "test"` without --json, verify table output |

*All other behaviors have automated verification via integration tests.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 10s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved

---

## Validation Audit 2026-03-21

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |

**Full test suite:** 54 passed, 0 failed (11 JSON-specific, 44 existing)
