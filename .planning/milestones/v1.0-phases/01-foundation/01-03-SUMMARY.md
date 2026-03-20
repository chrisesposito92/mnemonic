---
phase: 01-foundation
plan: "03"
subsystem: testing
tags: [rusqlite, tokio-rusqlite, sqlite-vec, tokio, integration-tests, toml]

# Dependency graph
requires:
  - phase: 01-01
    provides: "config module with Config struct and load_config()"
  - phase: 01-02
    provides: "db module with register_sqlite_vec() and open(), server module with AppState"
provides:
  - "tests/integration.rs with 5 async integration tests covering STOR-01 through STOR-04"
  - "src/lib.rs re-exporting all public modules for external test access"
  - "mnemonic.toml.example documenting all config fields with env var names and defaults"
affects: [02-embedding, 03-storage]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Integration tests in tests/ directory using tokio::test and conn.call() for all SQL"
    - "Once guard for SQLite extension registration shared across test functions"
    - "Temp file DB for WAL mode tests (in-memory DBs do not support WAL in SQLite)"

key-files:
  created:
    - src/lib.rs
    - tests/integration.rs
    - mnemonic.toml.example
  modified: []

key-decisions:
  - "WAL mode test uses a temp file DB, not :memory:, because SQLite in-memory databases always use memory journal mode regardless of PRAGMA journal_mode=WAL"
  - "src/lib.rs created as minimal re-export shim so tests/ crate can import mnemonic::db, mnemonic::config, etc."

patterns-established:
  - "Integration tests: call setup() (Once guard) at top of each test, then test_config() for :memory: Config"
  - "All SQL in tests goes through conn.call(|c| -> Result<_, rusqlite::Error> { ... }).await — never raw rusqlite in async context"

requirements-completed: [STOR-01, STOR-02, STOR-03, STOR-04, CONF-01, CONF-02, CONF-03]

# Metrics
duration: 5min
completed: 2026-03-19
---

# Phase 1 Plan 03: Integration Tests and Config Example Summary

**5 async integration tests prove memories schema (8 cols), WAL mode, vec_memories virtual table, and tokio-rusqlite round-trip; mnemonic.toml.example documents all MNEMONIC_* env vars**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-03-19T20:15:00Z
- **Completed:** 2026-03-19T20:20:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Created `src/lib.rs` re-exporting all public modules so `tests/` can import the crate
- Added 5 `#[tokio::test]` integration tests covering every STOR-* requirement: table schema (8 columns), WAL mode (file-based DB), vec_memories virtual table, embedding_model TEXT column, async insert+query round-trip
- Created `mnemonic.toml.example` with all 3 config fields documented, including `MNEMONIC_PORT`, `MNEMONIC_DB_PATH`, and `MNEMONIC_EMBEDDING_PROVIDER`
- All 15 tests pass: 5 unit (config), 5 unit (main binary), 5 integration

## Task Commits

Each task was committed atomically:

1. **Task 1: Write integration tests for database requirements** - `cddcca0` (feat)
2. **Task 2: Create example TOML config file** - `bea3a3c` (feat)

**Plan metadata:** `25a3b8a` (docs: complete plan)

## Files Created/Modified

- `src/lib.rs` - Minimal lib crate re-exporting config, db, error, server for integration test access
- `tests/integration.rs` - 5 async integration tests with shared Once setup and test_config() helper
- `mnemonic.toml.example` - Documented config file with all fields, defaults, and MNEMONIC_ env var names

## Decisions Made

- WAL mode test uses a temp file DB path (not `:memory:`) — SQLite in-memory databases always use `memory` journal mode regardless of `PRAGMA journal_mode=WAL`; this is a SQLite constraint, not a code bug
- `src/lib.rs` is a minimal shim only; all real logic stays in the existing modules declared in `main.rs`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] WAL mode test used :memory: DB which does not support WAL**
- **Found during:** Task 1 (integration test execution)
- **Issue:** `test_wal_mode` asserted `journal_mode == "wal"` but SQLite returns "memory" for in-memory connections
- **Fix:** Changed `test_wal_mode` to open a temp file-based DB, verify WAL mode, then delete the temp files
- **Files modified:** tests/integration.rs
- **Verification:** `cargo test -- --test-threads=1` passes, `test_wal_mode` returns "wal"
- **Committed in:** cddcca0 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - behavior bug in test)
**Impact on plan:** Required fix for correctness — WAL mode is verified correctly on file-based DB, which is the only relevant scenario for production use. No scope creep.

## Issues Encountered

None beyond the WAL mode deviation documented above.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 1 is fully complete: compilable binary, SQLite+sqlite-vec, WAL, layered config, health endpoint, integration tests
- Phase 2 (embedding) can import from mnemonic crate via the lib.rs shim
- Blockers from STATE.md remain (candle tensor shapes, sqlite-vec KNN query syntax) — these are research items for Phase 2/3, not blockers for this plan

---
*Phase: 01-foundation*
*Completed: 2026-03-19*
