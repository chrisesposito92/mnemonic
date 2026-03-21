---
phase: 15-serve-subcommand
plan: 01
subsystem: cli
tags: [rust, clap, cli, subcommand, dispatch]

# Dependency graph
requires:
  - phase: 14-keys-cli
    provides: Commands enum with Keys variant, cli::Cli struct, if-let dispatch pattern in main.rs
provides:
  - Commands enum with Serve variant (src/cli.rs)
  - match-based dispatch in main.rs routing Serve|None to server path
  - --db override applied in both CLI and server paths
  - Integration tests verifying serve appears in --help
affects: [16-recall, 17-remember, 18-search, 19-compact]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - match-based CLI dispatch (replaces if-let, extensible for future subcommands in phases 16-19)
    - Global flag extraction before match to avoid partial move

key-files:
  created: []
  modified:
    - src/cli.rs
    - src/main.rs
    - tests/cli_integration.rs

key-decisions:
  - "Serve variant has no args — port/host/config already handled by config::load_config() via env+TOML"
  - "Some(Commands::Serve) | None arm falls through to server init inline (no helper extracted per D-07)"
  - "db_override extracted before match to avoid partial move into Keys arm"
  - "--db override applied in server path after config load, before validate_config"

patterns-established:
  - "match dispatch pattern: Keys arm returns early, Serve|None arm falls through to server init"

requirements-completed: [CLI-01, CLI-02]

# Metrics
duration: 2min
completed: 2026-03-21
---

# Phase 15 Plan 01: serve-subcommand Summary

**`mnemonic serve` subcommand added via Commands enum Serve variant + match dispatch in main.rs, with backward-compatible `mnemonic` bare invocation and two new CLI integration tests**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-21T05:52:59Z
- **Completed:** 2026-03-21T05:55:03Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Added `Serve` unit variant to `Commands` enum in src/cli.rs with "Start the HTTP server" doc-comment
- Converted `if let Some(Commands::Keys)` dispatch in main.rs to exhaustive `match` with `Some(Commands::Serve) | None` arm falling through to server init inline
- Applied `--db` override in the server path (previously only applied in Keys path) by extracting `db_override` before the match
- Added two integration tests: `test_serve_appears_in_help` and `test_serve_help_text_description` verifying CLI-01 requirements

## Task Commits

Each task was committed atomically:

1. **Task 1: Add Serve variant to Commands enum and convert main.rs dispatch to match** - `b28cb57` (feat)
2. **Task 2: Add CLI integration tests for serve subcommand in --help output** - `d764a15` (test)

**Plan metadata:** (docs commit follows)

## Files Created/Modified

- `src/cli.rs` - Added `Serve,` variant above `Keys(KeysArgs)` in Commands enum with doc-comment
- `src/main.rs` - Converted if-let to match dispatch; extracted db_override; applied --db in server path; config becomes `let mut`
- `tests/cli_integration.rs` - Added `test_serve_appears_in_help` and `test_serve_help_text_description`

## Decisions Made

- `Serve` variant has no args — port/host/config already handled by `config::load_config()` via env vars and TOML (per D-02)
- Server init code stays inline in main.rs, not extracted to a helper function (per D-07 — Phases 16-19 will extract helpers as needed)
- `db_override` extracted before match to avoid partial move of `cli_args` into the Keys arm
- `--db` override applied in server path (correctness fix from RESEARCH.md open question)

## Deviations from Plan

None - plan executed exactly as written. The `--db` override for the server path was explicitly called out in Task 1 Step 3 of the plan as a correctness fix to implement.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 15 complete — match dispatch pattern established, ready for Phase 16 (recall) and subsequent subcommand phases
- Each new subcommand in Phases 16-19 adds a new arm to the match in main.rs
- Blocker note from STATE.md: confirm `MemoryService::get_memory(id)` exists before planning Phase 16

## Self-Check: PASSED

- SUMMARY.md: FOUND
- src/cli.rs: FOUND
- src/main.rs: FOUND
- tests/cli_integration.rs: FOUND
- Commit b28cb57: FOUND
- Commit d764a15: FOUND

---
*Phase: 15-serve-subcommand*
*Completed: 2026-03-21*
