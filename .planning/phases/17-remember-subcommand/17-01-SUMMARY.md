---
phase: 17-remember-subcommand
plan: "01"
subsystem: cli
tags: [clap, embedding, stdin, rust, memory-storage]

# Dependency graph
requires:
  - phase: 16-recall-subcommand
    provides: init_db() helper, RecallArgs pattern, run_recall handler established in cli.rs
  - phase: 15-serve-subcommand
    provides: dual-mode dispatch pattern in main.rs, db_override extraction before match
provides:
  - RememberArgs struct with optional content positional and --agent-id, --session-id, --tags flags
  - Commands::Remember(RememberArgs) variant in Commands enum
  - init_db_and_embedding() medium-init helper (DB + embedding engine) for embedding commands
  - run_remember() handler calling MemoryService::create_memory, printing UUID to stdout
  - stdin pipe detection via std::io::IsTerminal for piped content
  - Early empty-content validation before model load
affects: [18-search-subcommand, 19-compact-subcommand]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - medium-init helper (init_db_and_embedding) for CLI commands that need embeddings
    - stdin pipe detection with std::io::IsTerminal + read_to_string pattern
    - early content validation before expensive model load (avoids 2-3s wait on bad input)
    - args.content.take() to move content out of args before passing args to handler

key-files:
  created: []
  modified:
    - src/cli.rs
    - src/main.rs

key-decisions:
  - "init_db_and_embedding() extracted as shared medium-init helper — reusable by search (Phase 18) and compact (Phase 19)"
  - "mut args + args.content.take() pattern chosen over args.content.clone() — moves content cleanly, avoids cloning"
  - "empty content validated BEFORE init_db_and_embedding() call — prevents 2-3s model load on invalid input"
  - "run_remember takes resolved content String (not Option) — content resolution lives in main.rs dispatch arm"

patterns-established:
  - "Medium-init pattern: init_db_and_embedding() = register_sqlite_vec + load_config + validate_config + open DB + spawn_blocking LocalEngine::new"
  - "Stdin detection: !std::io::stdin().is_terminal() branch with std::io::Read::read_to_string fallback"
  - "CLI handler signature: run_remember(content: String, args: RememberArgs, service: MemoryService) — content already resolved"

requirements-completed: [REM-01, REM-02, REM-03, REM-04]

# Metrics
duration: 5min
completed: 2026-03-21
---

# Phase 17 Plan 01: Remember Subcommand Summary

**`mnemonic remember` CLI subcommand with stdin pipe support, medium-init embedding helper, and MemoryService::create_memory integration**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-03-21T08:01:28Z
- **Completed:** 2026-03-21T08:03:41Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- RememberArgs struct with optional content positional arg and --agent-id, --session-id, --tags flags wired into clap
- init_db_and_embedding() medium-init helper that loads DB + embedding engine via spawn_blocking (reusable by search/compact phases)
- run_remember() handler that parses comma-separated tags, constructs CreateMemoryRequest, calls create_memory, prints UUID to stdout and short confirmation to stderr
- main.rs dispatch arm with IsTerminal-based stdin detection, args.content.take() pattern, early empty-content validation before model load

## Task Commits

Each task was committed atomically:

1. **Task 1: Add RememberArgs, init_db_and_embedding helper, and run_remember in cli.rs** - `a23696c` (feat)
2. **Task 2: Add Remember dispatch arm with stdin/content resolution in main.rs** - `0154345` (feat)

## Files Created/Modified

- `src/cli.rs` - Added RememberArgs struct, Commands::Remember variant, init_db_and_embedding() helper, run_remember() handler
- `src/main.rs` - Added Commands::Remember match arm with stdin detection, early validation, and init_db_and_embedding call

## Decisions Made

- `init_db_and_embedding()` extracted as a medium-init helper in cli.rs (not inline in main.rs) — consistent with init_db() pattern, reusable by Phase 18 (search) and Phase 19 (compact)
- `mut args` + `args.content.take()` pattern used instead of `args.content.clone()` — moves String out of args cleanly without copying
- Content validation (empty check) placed BEFORE `init_db_and_embedding()` call — avoids 2-3s embedding model load on invalid input, per plan requirement
- `run_remember` takes `content: String` (resolved, not `Option`) — keeps content resolution logic in main.rs, handler stays focused on storage

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None. The build failed after Task 1 with a non-exhaustive match error (expected — Commands::Remember variant added but main.rs not yet updated). Task 2 resolved this immediately.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `mnemonic remember` fully wired: positional arg, stdin pipe, --agent-id, --session-id, --tags, early validation, model load, storage, UUID output
- `init_db_and_embedding()` is ready for reuse in Phase 18 (search) and Phase 19 (compact)
- All 63 lib tests pass, binary compiles with zero errors

## Self-Check: PASSED

- src/cli.rs: FOUND
- src/main.rs: FOUND
- 17-01-SUMMARY.md: FOUND
- Commit a23696c: FOUND (feat: cli.rs changes)
- Commit 0154345: FOUND (feat: main.rs dispatch arm)

---
*Phase: 17-remember-subcommand*
*Completed: 2026-03-21*
