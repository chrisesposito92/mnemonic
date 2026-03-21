---
phase: 19-compact-subcommand
plan: 01
subsystem: cli
tags: [rust, clap, compaction, cli, llm, embedding]

# Dependency graph
requires:
  - phase: 17-remember-subcommand
    provides: init_db_and_embedding() medium-init helper pattern
  - phase: 18-search-subcommand
    provides: run_search() handler pattern, SearchArgs struct pattern
  - phase: 6-compaction
    provides: CompactionService, CompactRequest, CompactResponse
provides:
  - CompactArgs struct with agent_id, threshold, max_candidates, dry_run fields
  - Commands::Compact(CompactArgs) variant in Commands enum
  - init_compaction() full-init helper (DB + embedding + optional LLM -> CompactionService)
  - run_compact() handler with output formatting per D-12/D-13/D-14/D-15
  - Compact dispatch arm in main.rs match
affects: [19-compact-subcommand plan 02 (integration tests)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Full-init pattern: DB + embedding + optional LLM -> CompactionService (heavier than medium-init)
    - Optional LLM summarization init with eprintln status messages
    - dry_run captured before consuming args.agent_id to avoid partial-move

key-files:
  created: []
  modified:
    - src/cli.rs
    - src/main.rs

key-decisions:
  - "init_compaction() cannot reuse init_db_and_embedding() because that returns MemoryService; compact needs individual components for CompactionService"
  - "dry_run and max_candidates_val captured before consuming agent_id to avoid Rust partial-move issues"
  - "Compact dispatch arm added to main.rs immediately (Rule 3: blocking) since cargo check fails on non-exhaustive match without it"

patterns-established:
  - "Full-init pattern: register_sqlite_vec -> load_config -> validate_config -> open -> Arc -> embedding -> optional LLM -> CompactionService"
  - "Status eprintln messages during init: 'Loading embedding model...' / 'LLM summarization: enabled/disabled'"

requirements-completed: [CMP-01, CMP-02, CMP-03]

# Metrics
duration: 8min
completed: 2026-03-21
---

# Phase 19 Plan 01: compact-subcommand Summary

**mnemonic compact CLI subcommand wired end-to-end: CompactArgs, init_compaction() full-init helper (DB + embedding + optional LLM -> CompactionService), run_compact() handler with dry-run/truncation output, and main.rs dispatch arm**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-21T14:05:30Z
- **Completed:** 2026-03-21T14:08:22Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- CompactArgs struct with 4 fields (agent_id, threshold, max_candidates, dry_run) added to cli.rs
- Commands::Compact(CompactArgs) variant added to Commands enum
- init_compaction() full-init helper constructs CompactionService with DB + embedding + optional LLM engine
- run_compact() handler formats output per D-12/D-13/D-14/D-15 with correct dry-run count handling
- main.rs Compact dispatch arm added before Serve|None fallthrough

## Task Commits

Each task was committed atomically:

1. **Task 1: Add CompactArgs struct and init_compaction() full-init helper** - `fd20aef` (feat)
2. **Task 2: Add run_compact() handler and wire Compact dispatch arm in main.rs** - `a1c213d` (feat)

## Files Created/Modified
- `src/cli.rs` - Added CompactArgs struct, Compact variant, init_compaction() helper, run_compact() handler (153 lines added)
- `src/main.rs` - Added Compact dispatch arm: calls init_compaction then run_compact (5 lines added)

## Decisions Made
- init_compaction() cannot reuse init_db_and_embedding() because that returns MemoryService; CompactionService needs individual components (Arc<Connection>, Arc<dyn EmbeddingEngine>, Option<Arc<dyn SummarizationEngine>>) — duplication is acceptable
- dry_run and max_candidates captured before consuming agent_id via unwrap_or_default() to avoid Rust partial-move compile error

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added run_compact() and main.rs dispatch arm in Task 1 commit to satisfy cargo check**
- **Found during:** Task 1 (CompactArgs + init_compaction)
- **Issue:** cargo check fails with "non-exhaustive patterns: `Some(Commands::Compact(_))` not covered" when Compact variant exists in Commands enum but main.rs match has no arm for it
- **Fix:** Added run_compact() to cli.rs and the Compact dispatch arm to main.rs as part of Task 1 execution; only main.rs change was committed in Task 2's commit
- **Files modified:** src/cli.rs, src/main.rs
- **Verification:** cargo check passes after fix
- **Committed in:** fd20aef (Task 1) and a1c213d (Task 2)

---

**Total deviations:** 1 auto-fixed (blocking)
**Impact on plan:** Required to make cargo check pass for Task 1 verification. Both changes match the plan's exact specification; just moved slightly earlier to unblock compilation.

## Issues Encountered
- Non-exhaustive match in main.rs prevented Task 1's cargo check from passing. Tasks 1 and 2 are tightly coupled — adding Commands::Compact requires the dispatch arm immediately.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- compact subcommand is fully functional end-to-end
- All 54 existing tests pass
- `mnemonic compact --help` shows all 4 flags: --agent-id, --threshold, --max-candidates, --dry-run
- `mnemonic --help` shows `compact` in subcommands list
- Phase 19 Plan 02 (integration tests) can proceed

---
*Phase: 19-compact-subcommand*
*Completed: 2026-03-21*

## Self-Check: PASSED

- FOUND: .planning/phases/19-compact-subcommand/19-01-SUMMARY.md
- FOUND: src/cli.rs
- FOUND: src/main.rs
- FOUND: commit fd20aef (Task 1)
- FOUND: commit a1c213d (Task 2)
