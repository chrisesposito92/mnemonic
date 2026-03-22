---
phase: 29-storagebackend-routing-fix
plan: "01"
subsystem: cli
tags: [storage-backend, recall, tech-debt, routing]
dependency_graph:
  requires: []
  provides: [recall-backend-routing]
  affects: [src/cli.rs, src/main.rs]
tech_stack:
  added: []
  patterns: [StorageBackend trait delegation, fast-path init helper]
key_files:
  created: []
  modified:
    - src/cli.rs
    - src/main.rs
decisions:
  - "init_recall() follows init_db() fast-path pattern (no validate_config) — embedding not needed for list/get"
  - "run_recall() receives Arc<dyn StorageBackend> instead of Arc<Connection> to honor configured backend"
  - "cmd_list_memories and cmd_get_memory call backend.list()/backend.get_by_id() — zero raw SQL remains"
metrics:
  duration: "~10 minutes"
  completed: "2026-03-22T17:50:00Z"
  tasks_completed: 2
  files_modified: 2
---

# Phase 29 Plan 01: StorageBackend Routing Fix Summary

Route `mnemonic recall` through the StorageBackend trait so list and get-by-id operations use the configured backend (SQLite, Qdrant, or Postgres) instead of always reading from SQLite directly.

## What Was Built

Fixed DEBT-01: recall CLI was bypassing the StorageBackend trait and querying SQLite directly via raw `conn.call(...)` blocks, silently ignoring the user's configured `storage_provider`.

### Changes

**src/cli.rs:**
- Added `init_recall()` — fast-path init that calls `init_db()` then `create_backend()`, returns `Arc<dyn StorageBackend>`. Does not call `validate_config()` (no embedding needed).
- Changed `run_recall()` signature from `Arc<Connection>` to `Arc<dyn StorageBackend>`.
- Rewrote `cmd_list_memories()` — replaced the entire `conn.call(...)` raw SQL block with `backend.list(ListParams).await`. Removed dead variables `limit_i64`, `agent_id_c`, `session_id_c`.
- Rewrote `cmd_get_memory()` — replaced the `conn.call(...)` raw SQL block with `backend.get_by_id(&id).await`. Removed `id_clone` and `use rusqlite::OptionalExtension` import.
- Added `test_backend()` helper in test module — returns `Arc<dyn StorageBackend>` with in-memory SQLite.
- Added `test_recall_list_delegates_to_backend` — seeds via `backend.store()`, verifies `backend.list()`, calls `cmd_list_memories(backend, ...)`.
- Added `test_recall_get_by_id_delegates_to_backend` — seeds via `backend.store()`, verifies `backend.get_by_id()`, calls `cmd_get_memory(backend, ...)`.

**src/main.rs:**
- Updated recall branch from `cli::init_db(db_override)` + `cli::run_recall(recall_args, conn_arc, json)` to `cli::init_recall(db_override)` + `cli::run_recall(recall_args, backend, json)`.

## Decisions Made

| Decision | Rationale |
|----------|-----------|
| init_recall() does NOT call validate_config() | Fast-path: recall only needs DB + backend, no embedding provider validation required |
| init_recall() does NOT use MemoryService | MemoryService requires embedding engine; recall needs only list/get_by_id trait methods |
| Output formatting kept identical | Zero behavior change for end users — only data source changed |

## Test Results

- Lib tests: 86 passed, 0 failed (up from 84 pre-plan — 2 new delegation tests added)
- Integration tests: 54 passed, 0 failed, 1 ignored
- Build: 0 new warnings (2 pre-existing unrelated warnings)

## Commits

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Route recall through StorageBackend trait | 6bed634 | src/cli.rs, src/main.rs |
| 2 | Add test_backend helper and delegation tests | 83e90d4 | src/cli.rs |

## Deviations from Plan

**1. [Rule 2 - Correctness] StoreRequest includes required `id` field**
- **Found during:** Task 2 (delegation tests)
- **Issue:** Plan's test code showed `StoreRequest` without `id` field, but `src/storage/mod.rs` defines `StoreRequest` with `pub id: String`. Plan notes said to verify by reading the file.
- **Fix:** Added `id: uuid::Uuid::now_v7().to_string()` to both StoreRequest instances in the tests.
- **Files modified:** src/cli.rs (tests only)
- **Commit:** 83e90d4

## Known Stubs

None — all recall operations fully routed through StorageBackend trait.

## Self-Check: PASSED
