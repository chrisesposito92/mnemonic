---
phase: 06-foundation
plan: "02"
subsystem: database
tags: [sqlite, rusqlite, schema-migration, tokio-rusqlite, compaction]

# Dependency graph
requires: []
provides:
  - memories table has source_ids column (TEXT NOT NULL DEFAULT '[]') for tracking merged memory ancestry
  - compact_runs table with 10 columns (id, agent_id, started_at, completed_at, clusters_found, memories_merged, memories_created, dry_run, threshold, status)
  - idx_compact_runs_agent_id index for efficient agent-scoped compaction queries
  - db::open() is idempotent on existing databases (duplicate column name silently ignored)
affects: [07-llm-provider, 08-compaction-core, 09-compaction-api]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - SQLite ALTER TABLE idempotency via error-swallowing (ignore SQLITE_ERROR code 1 = duplicate column name)
    - Config struct literals use ..Default::default() spread for forward compatibility with new fields

key-files:
  created: []
  modified:
    - src/db.rs
    - tests/integration.rs

key-decisions:
  - "SQLite does not support ALTER TABLE ADD COLUMN IF NOT EXISTS (not a real SQLite feature despite docs suggesting 3.37+ support) — use error-swallowing pattern: attempt ALTER TABLE, ignore SQLITE_ERROR (extended_code=1) for duplicate column name"
  - "Compact_runs table uses BOOLEAN for dry_run with DEFAULT 0 and REAL for threshold — both correct SQLite types"
  - "Config struct literals in integration tests now use ..Default::default() spread for forward compatibility with Plan 01 llm_* fields"

patterns-established:
  - "SQLite idempotent migrations: use CREATE TABLE IF NOT EXISTS for new tables, error-swallow for ADD COLUMN since IF NOT EXISTS is not supported"

requirements-completed:
  - LLM-01

# Metrics
duration: 9min
completed: "2026-03-20"
---

# Phase 06 Plan 02: Schema Migration (source_ids + compact_runs) Summary

**source_ids column and compact_runs audit table added to SQLite schema with idempotent migration, verified by 3 new integration tests (23 total passing)**

## Performance

- **Duration:** ~9 min
- **Started:** 2026-03-20T13:41:47Z
- **Completed:** 2026-03-20T13:50:47Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added `source_ids TEXT NOT NULL DEFAULT '[]'` column to `memories` table via idempotent ALTER TABLE migration
- Created `compact_runs` table with 10 columns and `idx_compact_runs_agent_id` index, ready for Phase 8-9 compaction
- Updated all Config struct literals to use `..Default::default()` spread for forward compatibility
- Added `test_compact_runs_exists` and `test_db_open_idempotent` integration tests
- Updated `test_schema_created` to assert 9 columns including `source_ids`
- All 23 integration tests pass with 0 regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: Add source_ids column and compact_runs table to db::open()** - `3257933` (feat)
2. **Task 2: Update integration tests for new schema** - `ce595a4` (feat)

**Plan metadata:** (final commit below)

## Files Created/Modified

- `/Users/chrisesposito/Documents/github/mnemonic/src/db.rs` - Added compact_runs CREATE TABLE, idx_compact_runs_agent_id, and idempotent ALTER TABLE for source_ids
- `/Users/chrisesposito/Documents/github/mnemonic/tests/integration.rs` - Updated schema test, added 2 new tests, spread syntax for Config literals

## Decisions Made

- **SQLite ADD COLUMN IF NOT EXISTS is NOT supported**: Despite the plan claiming SQLite 3.37+ supports `ALTER TABLE ADD COLUMN IF NOT EXISTS`, testing on SQLite 3.50.2 (the bundled version) confirmed this syntax throws a parse error. The correct idempotency pattern for SQLite is to attempt the ADD COLUMN and silently ignore extended_code=1 (duplicate column name). This was auto-fixed per Rule 1 (bug in plan specification).
- **Extended_code 1 = duplicate column name**: Used `err.extended_code == 1` to specifically match SQLITE_ERROR for duplicate column — this is the only non-fatal error that can occur on a second db::open() call.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] SQLite ALTER TABLE ADD COLUMN IF NOT EXISTS syntax not supported**
- **Found during:** Task 2 (run integration tests)
- **Issue:** Plan specified `ALTER TABLE memories ADD COLUMN IF NOT EXISTS source_ids ...` but SQLite (even 3.50.2) throws "near `EXISTS`: syntax error" — `ADD COLUMN IF NOT EXISTS` is not a real SQLite syntax feature
- **Fix:** Replaced the single `execute_batch` with a separate `execute_batch` call for the ALTER TABLE, wrapping it in a match that ignores `rusqlite::Error::SqliteFailure` with `extended_code == 1` (duplicate column name). The main batch was also reorganized to separate CREATE TABLE compact_runs from the ALTER TABLE.
- **Files modified:** `src/db.rs`
- **Verification:** test_schema_created, test_compact_runs_exists, and test_db_open_idempotent all pass; full suite 23/23 pass
- **Committed in:** `ce595a4` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - Bug)
**Impact on plan:** Required for correctness — the plan's DDL would have caused db::open() to fail on every call. The fix achieves the same idempotency guarantee through a different SQLite-compatible mechanism.

## Issues Encountered

- The plan's claim that "bundled rusqlite 0.37 ships SQLite 3.47+" was imprecise — the actual bundled SQLite is 3.50.2 (libsqlite3-sys 0.35.0), but `ADD COLUMN IF NOT EXISTS` is not a standard SQLite syntax at any version. It exists in some other databases (PostgreSQL) but not SQLite. This was caught during testing.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `source_ids` column is present in `memories` table — Phase 8 can begin populating it during compaction
- `compact_runs` table is present with all 10 columns — Phase 8 CompactionService can INSERT audit records
- `idx_compact_runs_agent_id` index ready for efficient agent-scoped compaction history queries
- db::open() is verified idempotent — v1.0 databases will upgrade cleanly on first v1.1 server start
- No blockers for Phase 07 (LLM Provider) or Phase 08 (Compaction Core)

---
*Phase: 06-foundation*
*Completed: 2026-03-20*
