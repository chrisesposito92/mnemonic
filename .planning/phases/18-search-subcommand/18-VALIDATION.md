---
phase: 18
slug: search-subcommand
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-21
validated: 2026-03-21
---

# Phase 18 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml (existing) |
| **Quick run command** | `cargo test --test cli_integration test_search` |
| **Full suite command** | `cargo test` |
| **Actual runtime** | ~18 seconds (9 tests) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --test cli_integration search`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 18 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Test(s) | Status |
|---------|------|------|-------------|-----------|-------------------|---------|--------|
| 18-01-01 | 01 | 1 | SRC-01 | integration | `cargo test --test cli_integration test_search_returns_ranked_results` | `test_search_returns_ranked_results` | ✅ green |
| 18-01-02 | 01 | 1 | SRC-01 | integration | `cargo test --test cli_integration test_search_empty` | `test_search_empty_query_exits_one`, `test_search_whitespace_query_exits_one`, `test_search_no_results_message` | ✅ green |
| 18-01-03 | 01 | 1 | SRC-02 | integration | `cargo test --test cli_integration test_search_limit test_search_agent test_search_threshold` | `test_search_limit_flag`, `test_search_agent_id_filter`, `test_search_threshold_flag` | ✅ green |
| 18-02-01 | 02 | 1 | SRC-01 | integration | `cargo test --test cli_integration test_search` | All 9 search tests | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

**Additional tests (not in original map):**
- `test_search_appears_in_help` — SRC-01 discoverability (help text)
- `test_search_json` — JSON output format coverage

---

## Wave 0 Requirements

- Existing test infrastructure in `tests/cli_integration.rs` covers phase needs
- Helper functions (`TempDb`, `binary()`) already available from Phase 16-17

*Existing infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 18s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** complete

---

## Validation Audit 2026-03-21

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |
| Total tests | 9 |
| Requirements covered | SRC-01, SRC-02 |
