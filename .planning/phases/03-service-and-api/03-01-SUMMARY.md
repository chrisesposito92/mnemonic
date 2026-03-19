---
phase: 03-service-and-api
plan: 01
subsystem: api
tags: [rust, axum, rusqlite, sqlite-vec, tokio-rusqlite, zerocopy, uuid, serde]

# Dependency graph
requires:
  - phase: 02-embedding
    provides: Arc<dyn EmbeddingEngine> trait and LocalEngine/OpenAiEngine implementations
  - phase: 01-foundation
    provides: tokio_rusqlite::Connection, schema (memories + vec_memories tables), error types
provides:
  - MemoryService struct with create_memory, search_memories, list_memories, delete_memory
  - ApiError enum with axum::response::IntoResponse (400/404/500 + JSON error body)
  - Request/response types: CreateMemoryRequest, SearchParams, ListParams, Memory, SearchResponse, ListResponse, SearchResultItem
affects: [03-02-handlers, 03-03-integration]

# Tech tracking
tech-stack:
  added: [zerocopy 0.8 (f32 Vec to raw bytes for sqlite-vec embedding storage), tower 0.5 dev, http-body-util 0.1 dev]
  patterns:
    - "CTE over-fetch KNN: WITH knn_candidates AS (SELECT ... WHERE embedding MATCH ? AND k = ?) JOIN memories — 10x multiplier when agent_id/session_id filter present"
    - "Atomic dual-table insert via rusqlite transaction: memories + vec_memories in single tx"
    - "tokio-rusqlite closure borrow scope: drop Statement before calling c.transaction() to avoid immutable/mutable borrow conflict"
    - "IS NULL OR pattern for optional WHERE filters: (?1 IS NULL OR col = ?1)"

key-files:
  created:
    - src/service.rs
  modified:
    - src/error.rs
    - Cargo.toml
    - src/lib.rs

key-decisions:
  - "zerocopy::IntoBytes used to convert Vec<f32> to raw bytes for sqlite-vec MATCH parameter — plan-specified pattern"
  - "delete_memory scopes stmt in inner block before c.transaction() to avoid E0502 borrow conflict — Rust borrow checker requires Statement drop before mutable borrow"
  - "search over-fetch multiplier is 10x capped at 1000; list default limit 20, search default limit 10 — both max 100"

patterns-established:
  - "Service layer pattern: MemoryService holds Arc<Connection> + Arc<dyn EmbeddingEngine> + String — handlers will be thin wrappers"
  - "ApiError as axum handler return type: impl IntoResponse converts to JSON {error: ...} with correct status codes"

requirements-completed: [API-01, API-02, API-03, API-04, API-06, AGNT-01, AGNT-02, AGNT-03]

# Metrics
duration: 8min
completed: 2026-03-19
---

# Phase 3 Plan 1: MemoryService Orchestrator and ApiError Summary

**MemoryService with atomic dual-table CRUD, CTE KNN search with 10x over-fetch, and axum ApiError type using zerocopy f32-to-bytes for sqlite-vec**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-19T21:28:00Z
- **Completed:** 2026-03-19T21:36:00Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Created MemoryService with all four public async methods (create, search, list, delete)
- Implemented ApiError with IntoResponse for clean handler error propagation via `?`
- Established CTE KNN over-fetch pattern for agent-filtered vector search
- Atomic dual-table insert (memories + vec_memories) and delete via rusqlite transactions
- All 10 existing tests continue to pass — zero regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: ApiError, zerocopy dep, service module declaration** - `03ad451` (feat)
2. **Task 2: MemoryService with all four CRUD+search operations** - `e1661a0` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `src/service.rs` — MemoryService struct and all four async methods with request/response types
- `src/error.rs` — ApiError enum with IntoResponse, From<EmbeddingError>, From<tokio_rusqlite::Error>
- `Cargo.toml` — zerocopy 0.8 added to dependencies; tower 0.5 and http-body-util 0.1 added to dev-dependencies
- `src/lib.rs` — pub mod service added

## Decisions Made
- `zerocopy::IntoBytes` to convert `Vec<f32>` to `&[u8]` for sqlite-vec MATCH parameter (plan-specified)
- Scoped `stmt` in inner block before `c.transaction()` in `delete_memory` to satisfy Rust's E0502 borrow conflict — Statement borrows `c` immutably and must be dropped before taking a mutable borrow for the transaction

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed borrow conflict in delete_memory**
- **Found during:** Task 2 (MemoryService implementation)
- **Issue:** `stmt` held an immutable borrow on `c` while `c.transaction()` needed a mutable borrow — E0502 compile error
- **Fix:** Wrapped `stmt.query_row(...)` call in an inner `{}` scope so `stmt` is dropped before the transaction is opened
- **Files modified:** src/service.rs
- **Verification:** `cargo check` succeeds; borrow checker satisfied
- **Committed in:** e1661a0 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 Rule 1 bug)
**Impact on plan:** Fix required for correct Rust borrow semantics. No behavior change, no scope creep.

## Issues Encountered
None beyond the borrow conflict documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- MemoryService is fully implemented and compiles
- ApiError provides clean handler return type via `impl IntoResponse`
- Plan 02 (HTTP handlers) can immediately import `MemoryService` from `crate::service`
- Request/response types are all `serde::Deserialize`/`serde::Serialize` ready for axum extractors and JSON responses

---
*Phase: 03-service-and-api*
*Completed: 2026-03-19*
