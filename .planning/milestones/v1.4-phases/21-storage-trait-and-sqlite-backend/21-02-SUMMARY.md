---
phase: 21-storage-trait-and-sqlite-backend
plan: "02"
subsystem: storage
tags: [storage, trait, sqlite, refactor, decoupling, arc-dyn]
dependency_graph:
  requires:
    - phase: 21-01
      provides: "StorageBackend trait, SqliteBackend, StoreRequest, CandidateRecord, MergedMemoryRequest"
  provides:
    - "MemoryService refactored to hold Arc<dyn StorageBackend> instead of Arc<Connection>"
    - "CompactionService refactored with dual-connection design (backend + audit_db)"
    - "All constructor call sites updated in main.rs, cli.rs, tests/integration.rs"
    - "Full test suite passing (247 tests, 0 failures, 1 ignored)"
  affects: [src/service.rs, src/compaction.rs, src/main.rs, src/cli.rs, tests/integration.rs]
tech-stack:
  added: []
  patterns: [Arc<dyn Trait> service injection, factory wiring in main.rs, dual-connection design for audit vs memory ops]
key-files:
  created: []
  modified:
    - src/service.rs
    - src/compaction.rs
    - src/main.rs
    - src/cli.rs
    - tests/integration.rs
key-decisions:
  - "CompactionService uses dual-connection design: backend (Arc<dyn StorageBackend>) for memory ops, audit_db (Arc<Connection>) for compact_runs audit table — compact_runs is SQLite-specific and not part of StorageBackend interface"
  - "Per-cluster write_compaction_result() calls replace single all-clusters transaction — per-cluster atomicity is correct for backend abstraction; SQLite-specific all-clusters-in-one-transaction optimization is dropped"
  - "build_test_compaction() helper in integration tests also needed updating — discovered as third call site not listed in plan"
patterns-established:
  - "Factory pattern: create Arc<dyn StorageBackend> in main.rs/cli.rs before services — backends are constructible anywhere but consumed as trait objects everywhere downstream"
  - "Service injection: all MemoryService and CompactionService construction goes through the trait object, not concrete types"
requirements-completed: [STOR-03, STOR-04, STOR-05]
duration: ~15min
completed: "2026-03-21"
---

# Phase 21 Plan 02: Service Wiring Summary

**MemoryService and CompactionService fully decoupled from SQLite via Arc<dyn StorageBackend>, with all 247 tests passing and KeyService unchanged.**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-21T17:10:00Z
- **Completed:** 2026-03-21T17:25:00Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Replaced `pub db: Arc<Connection>` with `pub backend: Arc<dyn StorageBackend>` in MemoryService — all 5 storage methods delegate to the trait object
- Refactored CompactionService to dual-connection design: `backend: Arc<dyn StorageBackend>` for memory operations, `audit_db: Arc<Connection>` for compact_runs audit records
- Removed `fetch_candidates` from CompactionService (delegated to `self.backend.fetch_candidates`), removed `CandidateMemory` struct (replaced by `CandidateRecord` from `crate::storage`)
- Updated all 3 call sites (main.rs, cli.rs, integration tests) with SqliteBackend factory pattern
- Full test suite passes: 247 tests (67 lib + 67 bin + 55 CLI integration + 4 error types + 54 integration + 1 ignored)

## Task Commits

Each task was committed atomically:

1. **Task 1: Refactor MemoryService and CompactionService** - `57d50ff` (refactor)
2. **Task 2: Update all constructor call sites and verify tests** - `86581ac` (feat)

## Files Created/Modified

- `src/service.rs` - Removed all direct SQLite code; now delegates all storage ops to `self.backend`; `Arc<Connection>` fully removed
- `src/compaction.rs` - Replaced `CandidateMemory` with `CandidateRecord`; dual-connection struct; `fetch_candidates` removed; compact_runs writes use `self.audit_db`; per-cluster `write_compaction_result()` calls
- `src/main.rs` - Added SqliteBackend factory after `db_arc = Arc::new(conn)`; passes `backend.clone()` to both services; KeyService unchanged
- `src/cli.rs` - `init_db_and_embedding` and `init_compaction` both wrap `conn_arc` in `SqliteBackend` before service construction
- `tests/integration.rs` - Added `mnemonic::storage::{StorageBackend, SqliteBackend}` import; updated 3 test helper functions with backend factory pattern

## Decisions Made

- **Dual-connection design for CompactionService:** The `compact_runs` audit table is SQLite-specific infrastructure (not part of the memory storage domain), so it stays on a direct `Arc<Connection>`. The `backend: Arc<dyn StorageBackend>` handles all memory operations. This is the correct split — future Qdrant or Postgres backends would need a separate SQLite audit connection anyway.

- **Per-cluster atomicity instead of all-clusters-in-one-transaction:** The original SQLite implementation used a single transaction for all clusters in one compaction run. The new approach calls `write_compaction_result()` once per cluster (each call is atomic: insert merged + delete sources). This drops the SQLite-specific all-clusters optimization but is necessary for backend abstraction — Qdrant/Postgres won't support cross-entity transactions in the same way.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing] Third call site not listed in plan**
- **Found during:** Task 2 (running cargo test after updating listed call sites)
- **Issue:** `build_test_compaction()` helper in tests/integration.rs (around line 1016) was not listed in the plan's call sites to update, but it also constructed `MemoryService` and `CompactionService` with old signatures — causing 3 test compilation errors
- **Fix:** Applied the same backend factory pattern to this function (added `let backend: Arc<dyn StorageBackend> = Arc::new(SqliteBackend::new(db.clone()));` and updated both service constructor calls)
- **Files modified:** tests/integration.rs
- **Verification:** cargo test passes with 247 tests, 0 failures
- **Committed in:** 86581ac (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (missing call site)
**Impact on plan:** Necessary fix for test compilation. No scope creep — same pattern applied consistently.

## Issues Encountered

None beyond the missing call site documented above.

## Known Stubs

None — all operations flow through `SqliteBackend` which is fully implemented. No hardcoded empty values, no placeholder text, no unwired data sources.

## Next Phase Readiness

- Phase 21 is now complete: StorageBackend trait defined (Plan 01) and wired (Plan 02)
- New backends (Qdrant, Postgres) can be added in `src/storage/` and wired in `main.rs` without modifying MemoryService or CompactionService
- Phase 23 (Qdrant backend) can proceed — only needs to implement the 7-method `StorageBackend` trait and add factory wiring

## Self-Check: PASSED

- FOUND: src/service.rs
- FOUND: src/compaction.rs
- FOUND: src/main.rs
- FOUND: src/cli.rs
- FOUND: tests/integration.rs
- FOUND: commit 57d50ff (refactor: wire MemoryService and CompactionService to Arc<dyn StorageBackend>)
- FOUND: commit 86581ac (feat: update all constructor call sites to use SqliteBackend and new service signatures)
- FOUND: .planning/phases/21-storage-trait-and-sqlite-backend/21-02-SUMMARY.md

---
*Phase: 21-storage-trait-and-sqlite-backend*
*Completed: 2026-03-21*
