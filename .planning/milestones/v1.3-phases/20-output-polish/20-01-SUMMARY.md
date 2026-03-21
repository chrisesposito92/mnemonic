---
phase: 20-output-polish
plan: 01
subsystem: cli
tags: [clap, serde_json, serde, json-output, machine-readable]

# Dependency graph
requires:
  - phase: 19-compact-subcommand
    provides: run_compact handler and CompactionService wiring in main.rs
  - phase: 18-search-subcommand
    provides: run_search handler in cli.rs
  - phase: 17-remember-subcommand
    provides: run_remember handler in cli.rs
  - phase: 16-recall-subcommand
    provides: run_recall, cmd_list_memories, cmd_get_memory handlers
  - phase: 14-keys-cli
    provides: run_keys, cmd_create, cmd_list, cmd_revoke handlers and ApiKey struct
provides:
  - Global --json flag on Cli struct (global = true, propagates to all subcommands)
  - ApiKey derives serde::Serialize enabling JSON serialization
  - JSON output branches in all 8 data-producing handlers: cmd_list_memories, cmd_get_memory, run_remember, run_search, run_compact, cmd_create, cmd_list, cmd_revoke
  - json: bool extracted in main.rs before match, passed to all 5 dispatch arms
affects: [20-output-polish-plan02, integration-tests, documentation]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "if json { serde_json::to_string_pretty } else { existing human output } branching pattern in CLI handlers"
    - "serde_json::json!() for ad-hoc JSON shapes (remember id, keys create token+meta, revoke confirmation)"
    - "Empty-state early returns (is_empty checks) moved inside else branch so JSON mode always returns valid empty arrays/objects"
    - "stderr audit output (run_id, truncation warning) always emitted regardless of json mode in run_compact"

key-files:
  created: []
  modified:
    - src/cli.rs
    - src/main.rs
    - src/auth.rs

key-decisions:
  - "json bool extracted before match in main.rs to avoid Rust partial-move compile error (same pattern as db_override)"
  - "ApiKey serde::Serialize added to auth.rs to enable keys list --json without wrapper type"
  - "cmd_revoke outputs {revoked: true} in JSON mode for both display_id and full-UUID success paths"
  - "run_compact: eprintln audit trail (run_id, truncation warning) emitted regardless of json mode -- stderr is always for humans"
  - "Empty-result early returns for is_empty checks placed inside else (human) branch only -- JSON mode returns {memories:[]} not early exit"
  - "run_remember JSON mode omits eprintln Stored memory stderr line -- JSON consumers do not need human context messages"

patterns-established:
  - "json: bool parameter threaded through all handler call chains via positional arg (not global state)"

requirements-completed: [OUT-01, OUT-02, OUT-03, OUT-04]

# Metrics
duration: 5min
completed: 2026-03-21
---

# Phase 20 Plan 01: Output Polish - JSON Flag Summary

**Global --json flag added to all CLI subcommands using serde_json branches, with ApiKey Serialize derive enabling machine-readable output across all 8 data-producing handlers**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-21T17:34:12Z
- **Completed:** 2026-03-21T17:39:12Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Added `--json` global flag to `Cli` struct (clap `global = true` propagates to all subcommands)
- Added `serde::Serialize` derive to `ApiKey` in `auth.rs` (no other changes to auth.rs)
- Extracted `let json = cli_args.json` in `main.rs` before match statement to prevent partial-move
- Wired `json: bool` through all 5 public and 5 private handler signatures in `cli.rs`
- Added `if json { serde_json } else { existing }` output branches to all 8 data-producing handlers
- All 54 existing tests pass unchanged (exercise the `else`/human branches)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add --json global flag, Serialize on ApiKey, and wire through main.rs** - `55f65c7` (feat)
2. **Task 2: Add JSON output branches to all CLI handlers** - `fe0f5c3` (feat)

## Files Created/Modified
- `src/cli.rs` - Added `pub json: bool` field; updated all 10 handler signatures; added 9 `if json` output branches across all handlers
- `src/main.rs` - Added `let json = cli_args.json` extraction; passed `json` to all 5 dispatch arms
- `src/auth.rs` - Added `serde::Serialize` to `ApiKey` derive

## Decisions Made
- `json` bool extracted before match in `main.rs` (same pattern as `db_override`) to prevent Rust partial-move compile error
- `ApiKey` gets `serde::Serialize` directly on the struct rather than a wrapper type — simplest approach for `keys list --json`
- `cmd_revoke` outputs `{"revoked": true}` in JSON mode for consistency — both display_id and full-UUID success paths
- `run_compact` stderr audit trail (`eprintln!` for run_id and truncation warning) always emitted regardless of `json` mode — stderr is for operators/humans
- Empty-result early returns (`is_empty` checks) moved inside `else` (human) branch — JSON mode must return `{"memories":[]}` not silent exit
- `run_remember` JSON mode omits the `eprintln!("Stored memory ...")` stderr message — script consumers don't need human context messages

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- `--json` flag fully implemented for all subcommands; ready for Phase 20 Plan 02 (integration tests / verification)
- Zero new Cargo.toml dependencies added (serde_json and serde already present in the project)

---
*Phase: 20-output-polish*
*Completed: 2026-03-21*
