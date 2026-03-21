---
phase: 16
slug: recall-subcommand
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-21
audited: 2026-03-21
---

# Phase 16 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml (workspace root) |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 16-01-01 | 01 | 1 | RCL-01 | integration | `cargo test --test cli_integration test_recall_empty_state test_recall_lists_with_table_headers test_recall_shows_truncated_id_and_content test_recall_shows_footer test_recall_shows_none_for_empty_agent test_recall_json_list test_recall_json_empty` | ✅ | ✅ green |
| 16-01-02 | 01 | 1 | RCL-02 | integration | `cargo test --test cli_integration test_recall_by_id_shows_detail test_recall_by_id_not_found_exits_one test_recall_json_by_id` | ✅ | ✅ green |
| 16-01-03 | 01 | 1 | RCL-03 | integration | `cargo test --test cli_integration test_recall_filter_agent_id test_recall_filter_session_id test_recall_limit` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `tests/cli_integration.rs` — recall test functions with DB seeding (direct rusqlite inserts)
- [x] `seed_memory()` helper for creating temp DB with seeded memory rows

*All Wave 0 requirements fulfilled in Plan 02 execution.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Sub-100ms response time | Success Criteria 4 | Timing varies by hardware | Run `time mnemonic recall` on a DB with 100+ rows, verify < 100ms |

---

## Test Coverage Summary

| Requirement | Tests | Coverage |
|-------------|-------|----------|
| RCL-01 (list recent) | 7 tests (empty state, headers, truncated ID, content, footer, none-agent, json-list, json-empty) | COVERED |
| RCL-02 (--id retrieval) | 3 tests (detail format, not-found exit-1, json-by-id) | COVERED |
| RCL-03 (filter flags) | 3 tests (agent-id, session-id, limit) | COVERED |
| Help output | 1 test (recall in --help) | COVERED |
| **Total** | **14 integration tests** | **All green** |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 15s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** complete

---

## Validation Audit 2026-03-21

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |
| Total tests | 14 |
| All green | yes |
