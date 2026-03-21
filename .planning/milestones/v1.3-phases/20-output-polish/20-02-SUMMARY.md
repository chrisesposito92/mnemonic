---
phase: 20-output-polish
plan: 02
subsystem: tests
tags: [serde_json, json-output, integration-tests, cli, machine-readable]

# Dependency graph
requires:
  - phase: 20-output-polish
    plan: 01
    provides: --json global flag wired through all 8 handlers in cli.rs

provides:
  - 11 integration tests verifying --json output for every data-producing subcommand
  - Test coverage for OUT-01 (human defaults not broken), OUT-02 (all subcommands support --json), OUT-03 (exit codes), OUT-04 (stderr/stdout split)

affects: [tests/cli_integration.rs]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "serde_json::from_str::<serde_json::Value>(&stdout) for asserting JSON structure in integration tests"
    - "seed via mnemonic remember (not seed_memory) for search/compact tests needing embeddings"
    - "seed_memory() reused for recall/by-id tests (fast, no model load)"

key-files:
  created: []
  modified:
    - tests/cli_integration.rs

key-decisions:
  - "seed_memory() used for recall JSON tests (fast path, no model load); mnemonic remember used for search/compact (need embeddings)"
  - "test_json_flag_no_human_output asserts absence of table headers (ID, CONTENT, Showing) to verify human output suppressed"
  - "test_keys_create_json verifies scope field matches --agent-id value (not agent_id key)"

requirements-completed: [OUT-01, OUT-02, OUT-03, OUT-04]

# Metrics
duration: 2min
completed: 2026-03-21
---

# Phase 20 Plan 02: Output Polish - JSON Integration Tests Summary

**11 integration tests added to tests/cli_integration.rs proving --json output for every data-producing subcommand via serde_json::from_str structure assertions**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-21T14:42:04Z
- **Completed:** 2026-03-21T14:44:08Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Added 11 new test functions in a `// ---- Phase 20: --json output tests` section at the end of `tests/cli_integration.rs`
- Each JSON test: invokes binary with `--json`, parses `stdout` via `serde_json::from_str::<serde_json::Value>`, asserts on structure fields
- Covered all data-producing subcommands: recall (list, empty, by-id), remember, search, compact, keys (create, list, list-empty), help flag, and human-output suppression
- All 11 new tests pass; 0 regressions in existing 44 integration tests
- Full test suite passes: 63 + 55 + 4 + 54 unit/integration tests, 0 failures

## Task Commits

1. **Task 1: Add --json integration tests for all subcommands** - `c6f2fbc` (test)

## Files Created/Modified

- `tests/cli_integration.rs` - Added 301 lines: 11 new test functions in the Phase 20 --json output tests section

## Decisions Made

- `seed_memory()` used for recall JSON tests (fast, no model load); `mnemonic remember` used for search/compact since CompactionService requires embeddings in `vec_memories`
- `test_json_flag_no_human_output` asserts `!stdout.contains("ID")`, `!stdout.contains("CONTENT")`, `!stdout.contains("Showing")` to verify human output is fully suppressed in JSON mode
- `test_keys_create_json` uses `--agent-id agent-j` and asserts `parsed["scope"] == "agent-j"` — the JSON field name is `scope` (matching `cmd_create` output)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None.

## Next Phase Readiness

- All Phase 20 requirements (OUT-01 through OUT-04) covered by automated tests
- Phase 20 output-polish milestone complete — v1.3 CLI feature set fully implemented and tested

---
*Phase: 20-output-polish*
*Completed: 2026-03-21*
