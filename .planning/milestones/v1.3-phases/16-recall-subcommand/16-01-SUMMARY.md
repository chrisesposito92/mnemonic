---
phase: 16-recall-subcommand
plan: "01"
subsystem: cli
tags: [cli, recall, db-fast-path, subcommand]
dependency_graph:
  requires: []
  provides: [recall-subcommand, init_db-helper]
  affects: [src/cli.rs, src/main.rs]
tech_stack:
  added: []
  patterns: [db-only-fast-path, clap-args-struct, tokio-rusqlite-conn-call]
key_files:
  created: []
  modified:
    - src/cli.rs
    - src/main.rs
decisions:
  - "init_db() extracts shared DB-only init from main.rs Keys arm — deduplicates register_sqlite_vec + load_config + open across recall and keys"
  - "RecallArgs uses default_value_t = 20 for --limit so mutual-exclusivity check against --id uses != 20 sentinel"
  - "cmd_list_memories and cmd_get_memory are private functions (no pub) matching Keys handler pattern"
metrics:
  duration_minutes: 3
  completed_date: "2026-03-21"
  tasks_completed: 2
  files_modified: 2
---

# Phase 16 Plan 01: Recall Subcommand Summary

**One-liner:** `mnemonic recall` DB-only fast path with table listing, per-ID detail view, and shared `init_db()` helper extracted from Keys arm.

## What Was Built

The `recall` subcommand gives users a fast (<100ms, no model loading) way to retrieve memories from the terminal. Implementation spans two files:

- **src/cli.rs**: `RecallArgs` struct (4 flags), `Recall` variant added to `Commands` enum, `init_db()` shared helper, `run_recall()` dispatcher, `cmd_list_memories()` table output, `cmd_get_memory()` detail view
- **src/main.rs**: Keys arm refactored to use `cli::init_db()`, new Recall arm added before Serve/None

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Extract init_db helper and add RecallArgs struct with Recall variant | 3c51caa | src/cli.rs, src/main.rs |
| 2 | Implement run_recall, cmd_list_memories, and cmd_get_memory | 3c51caa | src/cli.rs |

Note: Tasks 1 and 2 were committed atomically because Task 1's `cargo build` verification requires `run_recall` to exist (referenced in main.rs Recall arm).

## Verification Results

- `cargo build` — success, zero new warnings
- `cargo test -p mnemonic --lib -- cli` — 10/10 tests pass
- `cargo test -p mnemonic` — 54/54 lib tests pass
- `mnemonic --help` — `recall` appears in subcommands list
- `mnemonic recall --help` — all four flags shown (--id, --agent-id, --session-id, --limit)

## Key Decisions

1. **init_db() extracted as shared helper** — The 10-line DB init block in main.rs Keys arm was replaced with a single `cli::init_db(db_override)` call. Both Keys and Recall arms now use this helper. Server path retains its inline init (with tracing + validate_config). This completes the deduplication planned in D-04/D-05.

2. **RecallArgs --limit default_value_t = 20** — Using clap's `default_value_t` means the field is always populated. The mutual-exclusivity check in `run_recall` uses `args.limit != 20` as the sentinel to detect user-specified --limit when --id is also given.

3. **Private cmd_* functions** — `cmd_list_memories` and `cmd_get_memory` are private (no `pub`), matching the pattern established by `cmd_create`, `cmd_list`, `cmd_revoke` in the Keys implementation.

## Deviations from Plan

None - plan executed exactly as written. Both tasks matched the specified code exactly. The single commit for both tasks (vs. two) was the only variation, necessitated by the compile dependency between Task 1's main.rs Recall arm and Task 2's `run_recall` function.

## Known Stubs

None. The recall subcommand is fully functional end-to-end with real DB queries.

## Self-Check: PASSED

Files exist:
- FOUND: src/cli.rs
- FOUND: src/main.rs

Commits exist:
- FOUND: 3c51caa (feat(16-01): implement recall subcommand with DB-only fast path)
