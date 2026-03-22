---
phase: 24-postgres-backend
plan: 01
subsystem: database
tags: [postgres, sqlx, pgvector, rust, storage-backend, connection-pooling]

# Dependency graph
requires:
  - phase: 23-qdrant-backend
    provides: QdrantBackend structural template (new(), ensure_collection(), helpers, trait impl, unit tests)
  - phase: 22-config-extension-backend-factory-and-config-cli
    provides: backend-postgres feature flag in Cargo.toml, postgres_url in Config, create_backend() factory
  - phase: 21-storage-trait-and-sqlite-backend
    provides: StorageBackend trait (7 methods), StoreRequest/CandidateRecord/MergedMemoryRequest types
provides:
  - PostgresBackend struct with PgPool, schema auto-creation, store/get_by_id/delete implemented
  - src/storage/postgres.rs gated behind #[cfg(feature = "backend-postgres")]
  - Factory wiring: create_backend() postgres arm routes to PostgresBackend::new()
  - sqlx 0.8 + pgvector 0.4 as optional deps under backend-postgres feature
  - row_to_memory() helper using TO_CHAR for TIMESTAMPTZ-to-String decoding
  - Unit tests for now_iso8601() and map_db_err() (no DB required)
affects: [24-02, verifier]

# Tech tracking
tech-stack:
  added:
    - sqlx 0.8.6 (default-features=false, runtime-tokio + postgres features) — async Postgres SQL with PgPool
    - pgvector 0.4.1 (sqlx feature) — Vector type with sqlx Encode/Decode for vector(384) column
  patterns:
    - TO_CHAR(col AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') in all SELECT queries to decode TIMESTAMPTZ as String
    - default-features=false on sqlx to prevent libsqlite3-sys version conflict with rusqlite
    - fetch-then-delete pattern for delete() (mirrors QdrantBackend)
    - PgPool stored directly in struct (no Arc wrapping — PgPool is already Clone+Send+Sync)

key-files:
  created:
    - src/storage/postgres.rs
  modified:
    - Cargo.toml
    - src/storage/mod.rs

key-decisions:
  - "sqlx requires default-features=false to avoid libsqlite3-sys version conflict with rusqlite 0.37 (sqlx's default includes sqlx-sqlite which pulls in a different libsqlite3-sys)"
  - "store() uses INSERT then SELECT-back pattern rather than RETURNING clause for consistent TO_CHAR timestamp formatting"
  - "now_iso8601() included in postgres.rs for Plan 02 use (write_compaction_result will need it for created_at in INSERT)"

patterns-established:
  - "TO_CHAR(timestamptz AT TIME ZONE 'UTC', ...) pattern for all SELECT queries returning created_at/updated_at as String"
  - "sqlx::query() runtime function (not macro) throughout PostgresBackend — avoids DATABASE_URL compile-time requirement"
  - "map_db_err() helper mapping sqlx::Error to ApiError::Internal(DbError::Query) — consistent error conversion"

requirements-completed: [PGVR-01, PGVR-04]

# Metrics
duration: 6min
completed: 2026-03-21
---

# Phase 24 Plan 01: PostgresBackend CRUD Summary

**PostgresBackend struct with PgPool, idempotent schema auto-creation (pgvector extension + HNSW index), and store/get_by_id/delete implemented via sqlx 0.8 + pgvector 0.4 — feature-gated behind backend-postgres**

## Performance

- **Duration:** ~6 min
- **Started:** 2026-03-21T20:40:00Z
- **Completed:** 2026-03-21T20:46:04Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- sqlx 0.8 and pgvector 0.4 added as optional dependencies under backend-postgres feature, with `default-features = false` to resolve libsqlite3-sys version conflict with rusqlite 0.37
- PostgresBackend struct created in `src/storage/postgres.rs` with PgPool, schema auto-creation (CREATE EXTENSION vector + CREATE TABLE memories + 4 indexes including HNSW), and 3 trait methods implemented (store, get_by_id, delete)
- Factory wired: `create_backend()` postgres arm now calls `PostgresBackend::new(config).await?` replacing the `todo!()` stub
- All 80 existing unit tests pass unchanged; default build unaffected

## Task Commits

Each task was committed atomically:

1. **Task 1: Add sqlx and pgvector dependencies to Cargo.toml** - `2653b0d` (chore)
2. **Task 2: Create PostgresBackend with construction, CRUD, helpers, unit tests, and module wiring** - `8c3a1de` (feat)

## Files Created/Modified

- `Cargo.toml` — backend-postgres feature gains dep:sqlx + dep:pgvector; sqlx added with default-features=false
- `src/storage/postgres.rs` (new) — PostgresBackend struct, ensure_schema(), store/get_by_id/delete, helpers (map_db_err, now_iso8601, row_to_memory), unit tests
- `src/storage/mod.rs` — cfg-gated pub mod postgres + re-export + factory wiring for postgres arm

## Decisions Made

- **sqlx default-features=false**: sqlx's default feature set includes `sqlx-sqlite` which brings in a different version of `libsqlite3-sys` than `rusqlite v0.37.0`. Setting `default-features = false` and explicitly specifying `["runtime-tokio", "postgres"]` avoids the "Only one package may specify links = sqlite3" conflict.
- **store() INSERT then SELECT-back**: Rather than using a RETURNING clause, store() does INSERT then SELECT back using the same TO_CHAR timestamp query pattern. This ensures the returned Memory.created_at always has consistent ISO 8601 format from TO_CHAR processing.
- **now_iso8601() kept in postgres.rs**: Even though Plan 01 methods don't call it directly (schema uses DEFAULT NOW()), it's kept for Plan 02's write_compaction_result() which needs to bind created_at as a string.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Added `default-features = false` to sqlx dependency**
- **Found during:** Task 1 (cargo check after adding sqlx)
- **Issue:** sqlx's default feature set includes `sqlx-sqlite` → pulls in `libsqlite3-sys` at a version incompatible with `rusqlite v0.37.0`'s `libsqlite3-sys v0.35.0`. Cargo resolver error: "Only one package in the dependency graph may specify the same links value."
- **Fix:** Changed `sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"], optional = true }` to add `default-features = false`
- **Files modified:** Cargo.toml
- **Verification:** `cargo check` succeeded after fix
- **Committed in:** `2653b0d` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - dependency conflict)
**Impact on plan:** Required fix for the build to succeed. Plan specified the features to add but didn't anticipate the default-features conflict. The added flag reduces dependency surface (no sqlite driver for postgres-only feature).

## Issues Encountered

None beyond the libsqlite3-sys conflict documented above.

## User Setup Required

None - PostgresBackend is not yet runtime-testable without a running Postgres+pgvector instance (expected). Plan 02 completes the remaining 4 trait methods. Integration testing requires `docker run -e POSTGRES_PASSWORD=test -p 5432:5432 pgvector/pgvector:pg17`.

## Known Stubs

The following methods are intentionally stubbed for Plan 02:

| File | Method | Stub |
|------|---------|------|
| `src/storage/postgres.rs` | `list()` | `todo!("Implemented in Plan 02")` |
| `src/storage/postgres.rs` | `search()` | `todo!("Implemented in Plan 02")` |
| `src/storage/postgres.rs` | `fetch_candidates()` | `todo!("Implemented in Plan 02")` |
| `src/storage/postgres.rs` | `write_compaction_result()` | `todo!("Implemented in Plan 02")` |

These stubs are intentional and documented in the plan. Plan 02 will implement all four remaining methods.

## Next Phase Readiness

- PostgresBackend foundation complete: struct, connection pooling, schema auto-creation, and 3/7 trait methods
- Plan 02 can build directly on top of this foundation for the remaining 4 methods (list, search, fetch_candidates, write_compaction_result)
- The `row_to_memory()` helper and `now_iso8601()` are available for Plan 02 reuse
- Factory wiring is complete — no changes needed to mod.rs or Cargo.toml in Plan 02

---
*Phase: 24-postgres-backend*
*Completed: 2026-03-21*
